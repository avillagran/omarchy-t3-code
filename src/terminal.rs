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
fn writable(e:&Environment)->Result<()>{if e.write_token_file.is_none()&&e.bearer.is_none()&&e.connection.is_none(){Err("write_access_required".into())}else{Ok(())}}
/// No command or path is interpolated into a shell; the bridge resolves thread context itself.
pub fn launch(e:&Environment,thread:&str)->Result<serde_json::Value>{launch_command(e,thread,None)}
pub fn launch_command(e:&Environment,thread:&str,command:Option<&str>)->Result<serde_json::Value>{
 protocol::terminal::session(thread,"term-1")?;writable(e)?;if let Some(error)=&e.connection_error{return Err(error.clone())}
 let executable=std::env::current_exe().map_err(|_|"terminal_launch_failed")?;
 let mut launcher=Command::new("omarchy-launch-terminal");launcher.arg(executable).args(["terminal",&e.id,thread]);
 let mut run_file=None;
 if let Some(command)=command{
  if !protocol::code_blocks::valid_command(command){return Err("invalid_terminal_input".into())}
  let directory=xdg("XDG_STATE_HOME",".local/state").join("terminal-commands");std::fs::create_dir_all(&directory).map_err(|_|"local_error")?;
  use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(&directory,std::fs::Permissions::from_mode(0o700)).map_err(|_|"local_error")?;
  let id=uuid::Uuid::new_v4().to_string();let file=directory.join(format!("{id}.json"));save_json(&file,&serde_json::json!({"command":command,"environment":e.id,"thread":thread,"terminalId":format!("term-ot3-{id}")}))?;
  launcher.arg("--run-file").arg(&file);run_file=Some(file);
 }
 let mut child=match launcher.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn(){Ok(child)=>child,Err(_)=>{if let Some(file)=run_file{let _=std::fs::remove_file(file);}return Err("terminal_launch_failed".into())}};
 // Reap the short launcher without retaining a worker for the terminal's lifetime.
 std::thread::spawn(move||{let _=child.wait();});Ok(serde_json::json!({"opened":true}))
}
/// Attach the frontend to the exact native session opened by the script plan.
pub fn launcher_available()->bool{std::env::var_os("PATH").is_some_and(|paths|std::env::split_paths(&paths).any(|p|p.join("omarchy-launch-terminal").is_file()))}
fn script_session(thread:&str,terminal:&str)->Result<()>{protocol::terminal::session(thread,terminal)?;uuid::Uuid::parse_str(terminal.strip_prefix("term-ot3-script-").ok_or("invalid_terminal")?).map_err(|_|"invalid_terminal")?;Ok(())}
pub fn launch_session(e:&Environment,thread:&str,terminal:&str)->Result<serde_json::Value>{
 script_session(thread,terminal)?;writable(e)?;let executable=std::env::current_exe().map_err(|_|"terminal_launch_failed")?;
 let mut child=Command::new("omarchy-launch-terminal").arg(executable).args(["terminal-session",&e.id,thread,terminal]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|_|"terminal_launch_failed")?;
 std::thread::spawn(move||{let _=child.wait();});Ok(serde_json::json!({"opened":true,"terminalId":terminal}))
}
pub fn run_session(e:&Environment,thread:&str,terminal:&str)->Result<()>{script_session(thread,terminal)?;run_frontend(e,thread,None,Some(terminal))}
/// Run only in the native terminal process. Closing the frontend detaches; it never kills T3's shell.
pub fn run(e:&Environment,thread:&str)->Result<()>{run_command(e,thread,None)}
fn consume(e:&Environment,thread:&str,path:&std::path::Path)->Result<serde_json::Value>{
 use std::os::unix::fs::{OpenOptionsExt,MetadataExt,PermissionsExt};
 let directory=xdg("XDG_STATE_HOME",".local/state").join("terminal-commands");
 if path.parent()!=Some(directory.as_path())||path.extension().and_then(|s|s.to_str())!=Some("json"){return Err("invalid_terminal_input".into())}
 let id=path.file_stem().and_then(|s|s.to_str()).ok_or("invalid_terminal_input")?;uuid::Uuid::parse_str(id).map_err(|_|"invalid_terminal_input")?;
 let claimed=directory.join(format!("{id}.claimed"));std::fs::rename(path,&claimed).map_err(|_|"terminal_command_consumed")?;
 let result=(||->Result<serde_json::Value>{let mut file=std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW).open(&claimed).map_err(|_|"invalid_terminal_input")?;let meta=file.metadata().map_err(|_|"local_error")?;if !meta.is_file()||meta.uid()!=unsafe{libc::geteuid()}||meta.permissions().mode()&0o077!=0||meta.len()>65536{return Err("invalid_terminal_input".into())}let mut bytes=Vec::new();file.read_to_end(&mut bytes).map_err(|_|"local_error")?;let value:serde_json::Value=serde_json::from_slice(&bytes).map_err(|_|"invalid_terminal_input")?;if value["environment"]!=e.id||value["thread"]!=thread||value["terminalId"]!=format!("term-ot3-{id}")||!protocol::code_blocks::valid_command(text(&value,"command")){return Err("invalid_terminal_input".into())}Ok(value)})();let _=std::fs::remove_file(&claimed);result
}
pub fn run_command(e:&Environment,thread:&str,path:Option<&std::path::Path>)->Result<()>{run_frontend(e,thread,path,None)}
fn run_frontend(e:&Environment,thread:&str,path:Option<&std::path::Path>,session:Option<&str>)->Result<()>{
 writable(e)?;let launch=path.map(|p|consume(e,thread,p)).transpose()?;let terminal=session.or_else(||launch.as_ref().map(|v|text(v,"terminalId"))).unwrap_or("term-1");protocol::terminal::session(thread,terminal)?;
 let c=e.connection(true)?;let shell=Client::shared()?.get(e,"/api/orchestration/shell",true)?;
 let context=protocol::terminal::context(&shell,thread,"").or_else(|error|{if error!="terminal_directory_unavailable"{return Err(error)}let config=c.rpc("server.getConfig",serde_json::json!({}))?;protocol::terminal::context(&shell,thread,text(&config,"scratchWorkspaceRoot"))})?;
 let (cols,rows)=size();let mut payload=protocol::terminal::attach(thread,terminal,&context,cols,rows)?;if session.is_some(){payload["restartIfNotRunning"]=serde_json::json!(false);}
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
 let mut buffer=[0u8;16384];let mut pending=Vec::new();let mut last_size=(cols,rows);let mut initial=launch.as_ref().map(|v|format!("{}\r",text(v,"command")));
 let result=(||->Result<()>{while !done.load(Ordering::Relaxed)&&!INTERRUPTED.load(Ordering::Relaxed){
  if !ready.load(Ordering::Relaxed){std::thread::sleep(Duration::from_millis(20));continue}
  if let Some(command)=initial.take(){c.terminal_write(thread,terminal,&command)?;}
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
