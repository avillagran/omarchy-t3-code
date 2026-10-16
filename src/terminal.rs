//! External terminal frontend. Native T3 owns the shell, including on remote environments.
use crate::*;
use std::{io::{self,Read,Write},os::fd::AsRawFd,process::{Command,Stdio},sync::{Arc,atomic::{AtomicBool,Ordering}},time::Duration};
static INTERRUPTED:AtomicBool=AtomicBool::new(false);
static RESIZED:AtomicBool=AtomicBool::new(false);
extern "C" fn signal(sig:libc::c_int){if sig==libc::SIGWINCH{RESIZED.store(true,Ordering::Relaxed)}else{INTERRUPTED.store(true,Ordering::Relaxed)}}
struct Tty{original:libc::termios,signals:Vec<(libc::c_int,libc::sigaction)>}
impl Tty{
 fn enter()->Result<Self>{unsafe{
  let mut original=std::mem::zeroed();if libc::tcgetattr(0,&mut original)!=0{return Err("terminal_tty_required".into())}
  let mut raw=original;libc::cfmakeraw(&mut raw);if libc::tcsetattr(0,libc::TCSANOW,&raw)!=0{return Err("terminal_tty_required".into())}
  let mut tty=Self{original,signals:vec![]};INTERRUPTED.store(false,Ordering::Relaxed);
  for sig in [libc::SIGWINCH,libc::SIGHUP,libc::SIGTERM,libc::SIGINT]{let mut action:libc::sigaction=std::mem::zeroed();let mut old=std::mem::zeroed();action.sa_sigaction=signal as *const () as usize;libc::sigemptyset(&mut action.sa_mask);if libc::sigaction(sig,&action,&mut old)!=0{return Err("terminal_tty_required".into())}tty.signals.push((sig,old));}Ok(tty)
 }}
}
impl Drop for Tty{fn drop(&mut self){unsafe{libc::tcsetattr(0,libc::TCSANOW,&self.original);for (sig,old) in &self.signals{libc::sigaction(*sig,old,std::ptr::null_mut());}}let _=io::stdout().write_all(b"\x1b[?2004l\x1b[?25h\x1b[0m\r\n");}}
fn size()->(u16,u16){let mut w:libc::winsize=unsafe{std::mem::zeroed()};if unsafe{libc::ioctl(io::stdout().as_raw_fd(),libc::TIOCGWINSZ,&mut w)}==0&&w.ws_col>0&&w.ws_row>0{(w.ws_col.clamp(1,1000),w.ws_row.clamp(1,500))}else{(80,24)}}
fn connection(e:&Environment)->Result<protocol::Connection>{if let Some(error)=&e.connection_error{return Err(error.clone())}if let Some(c)=&e.connection{return Ok(c.as_ref().clone())}protocol::Connection::new(&e.base_url,None,protocol::Credential::bearer(&environment_token(e,true)?)?)}
fn writable(e:&Environment)->Result<()>{if e.write_token_file.is_none()&&e.bearer.is_none()&&e.connection.is_none(){Err("write_access_required".into())}else{Ok(())}}
/// No command or path is interpolated into a shell; the bridge resolves thread context itself.
pub fn launch(e:&Environment,thread:&str)->Result<serde_json::Value>{
 protocol::terminal::session(thread,"term-1")?;writable(e)?;if let Some(error)=&e.connection_error{return Err(error.clone())}
 let executable=std::env::current_exe().map_err(|_|"terminal_launch_failed")?;
 let mut child=Command::new("omarchy-launch-terminal").arg(executable).args(["terminal",&e.id,thread]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|_|"terminal_launch_failed")?;
 // Reap the short launcher without retaining a worker for the terminal's lifetime.
 std::thread::spawn(move||{let _=child.wait();});Ok(serde_json::json!({"opened":true}))
}
/// Run only in the native terminal process. Closing the frontend detaches; it never kills T3's shell.
pub fn run(e:&Environment,thread:&str)->Result<()>{
 writable(e)?;let terminal="term-1";protocol::terminal::session(thread,terminal)?;
 let c=connection(e)?;let shell=Client::new()?.get(e,"/api/orchestration/shell",true)?;
 let context=protocol::terminal::context(&shell,thread,"").or_else(|error|{if error!="terminal_directory_unavailable"{return Err(error)}let config=c.rpc("server.getConfig",serde_json::json!({}))?;protocol::terminal::context(&shell,thread,text(&config,"scratchWorkspaceRoot"))})?;
 let (cols,rows)=size();let payload=protocol::terminal::attach(thread,terminal,&context,cols,rows)?;
 eprintln!("T3 · {} · {}\r",e.name,text(&context,"cwd"));
 let tty=Tty::enter()?;let done=Arc::new(AtomicBool::new(false));let ready=Arc::new(AtomicBool::new(false));let stream_done=done.clone();let stream_ready=ready.clone();let stream=c.clone();
 let output=std::thread::spawn(move||{
  let mut output=protocol::terminal::Output::default();
  let result=stream.subscribe("terminal.attach",payload,||stream_done.load(Ordering::Relaxed)||INTERRUPTED.load(Ordering::Relaxed),|event|{
   if let Some(event)=event{
    if let Some(bytes)=output.apply(&event)?{let mut stdout=io::stdout().lock();stdout.write_all(bytes.as_bytes()).and_then(|_|stdout.flush()).map_err(|_|"terminal_output_closed")?;}
    if event["type"]=="snapshot"||event["type"]=="started"||event["type"]=="restarted"{stream_ready.store(!output.finished,Ordering::Relaxed);}
    if output.finished{stream_done.store(true,Ordering::Relaxed);}
   }Ok(())
  });stream_done.store(true,Ordering::Relaxed);result
 });
 let mut buffer=[0u8;16384];let mut pending=Vec::new();let mut last_size=(cols,rows);
 let result=(||->Result<()>{while !done.load(Ordering::Relaxed)&&!INTERRUPTED.load(Ordering::Relaxed){
  if !ready.load(Ordering::Relaxed){std::thread::sleep(Duration::from_millis(20));continue}
  if RESIZED.swap(false,Ordering::Relaxed){let next=size();if next!=last_size{c.terminal_resize(thread,terminal,next.0,next.1)?;last_size=next;}}
  let mut fd=libc::pollfd{fd:0,events:libc::POLLIN,revents:0};let count=unsafe{libc::poll(&mut fd,1,100)};
  if count<0{if io::Error::last_os_error().kind()==io::ErrorKind::Interrupted{continue}return Err("terminal_input_closed".into())}
  if count==0{continue}if fd.revents&(libc::POLLHUP|libc::POLLERR|libc::POLLNVAL)!=0{break}
  let read=io::stdin().read(&mut buffer).map_err(|_|"terminal_input_closed")?;if read==0{break}
  pending.extend_from_slice(&buffer[..read]);
  // A UTF-8 character can cross read boundaries. Preserve it without replacement bytes.
  let valid=match std::str::from_utf8(&pending){Ok(s)=>s.len(),Err(e) if e.error_len().is_none()=>e.valid_up_to(),Err(_)=>return Err("invalid_terminal_input".into())};
  if valid>0{let input=std::str::from_utf8(&pending[..valid]).map_err(|_|"invalid_terminal_input")?;c.terminal_write(thread,terminal,input)?;pending.drain(..valid);}
 }Ok(())})();
 done.store(true,Ordering::Relaxed);let streamed=output.join().unwrap_or_else(|_|Err("terminal_disconnected".into()));drop(tty);result?;if INTERRUPTED.load(Ordering::Relaxed){Ok(())}else{streamed}
}
