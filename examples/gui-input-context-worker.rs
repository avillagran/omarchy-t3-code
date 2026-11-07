//! Owned-process registration fixture. It is never a production engine fallback.
use omarchy_t3_client::{browser_engine_identity, gui_clipboard, gui_input_context};
use std::{
    io::{BufRead, Write},
    os::{fd::AsRawFd, unix::process::CommandExt},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
struct Renderer(Child);
impl Drop for Renderer {
    fn drop(&mut self) {
        if let Some(input) = self.0.stdin.as_mut() {
            let _ = input.write_all(b"q");
        }
        for _ in 0..100 {
            if self.0.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn main() {
    if run().is_err() {
        eprintln!("INPUT_CONTEXT_WORKER_FAILED");
        std::process::exit(1);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = std::io::stdin().lock().lines();
    if input.next().transpose()?.as_deref() != Some("go") {
        return Err("fixture control".into());
    }
    let session = Arc::new(Mutex::new(gui_clipboard::start().ok_or("registration")?));
    let pending = browser_engine_identity::Pending::new()?;
    let descriptor = pending.source().as_raw_fd();
    let script = r#"
import array,ctypes,os,signal,socket,sys
descriptor=int(sys.argv[1])
engine=os.fork()
if engine==0:
 parent=os.getppid()
 if ctypes.CDLL(None).prctl(1,signal.SIGKILL)!=0 or os.getppid()!=parent:os._exit(2)
 peer=socket.socket(fileno=descriptor)
 pidfd=os.pidfd_open(os.getpid())
 peer.sendmsg([b'o3-native-engine-identity-v1'],[(socket.SOL_SOCKET,socket.SCM_RIGHTS,array.array('i',[pidfd]))])
 os.close(pidfd)
 while True:signal.pause()
sys.stdin.buffer.read(1)
os.kill(engine,signal.SIGTERM)
os.waitpid(engine,0)
"#;
    let mut command = Command::new("/usr/bin/python3");
    command
        .arg("-c")
        .arg(script)
        .arg(descriptor.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    unsafe {
        command.pre_exec(move || {
            if libc::fcntl(descriptor, libc::F_SETFD, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let renderer = Renderer(command.spawn()?);
    let engine = pending.admit(renderer.0.id())?;
    let id = uuid::Uuid::new_v4().to_string();
    let mut lease = gui_input_context::Lease::register(session, &id, &engine)?;
    println!(
        "{}",
        serde_json::json!({"kind":"input-context-registered","cohort":id})
    );
    std::io::stdout().flush()?;
    if input.next().transpose()?.as_deref() != Some("publish") {
        return Err("fixture control".into());
    }
    let binding = gui_input_context::Binding {
        scope: "page-replacement".into(),
        connection: format!("{id}:owned-wayland-socket"),
        surface: "owned-surface".into(),
        tab: "owned-tab".into(),
        frame: "owned-frame".into(),
    };
    let mut state = gui_input_context::State {
        serial: 1,
        caret: Some([8, 8, 0, 2]),
        surrounding: Some(gui_input_context::Surrounding {
            text: "A😀日本ZQ",
            cursor_byte: 13,
            anchor_byte: 13,
        }),
        hints: 4,
        purpose: 0,
    };
    lease.publish(&binding, &state)?;
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(request) = lease.request()? {
            if request != binding {
                return Err("fixture request binding".into());
            }
            break;
        }
        if Instant::now() >= deadline {
            return Err("fixture request timeout".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    println!(
        "{}",
        serde_json::json!({"kind":"input-state-published","requestVerified":true})
    );
    std::io::stdout().flush()?;
    if input.next().transpose()?.as_deref() != Some("invalidate") {
        return Err("fixture control".into());
    }
    lease.invalidate(&binding)?;
    println!("{}", serde_json::json!({"kind":"input-state-invalidated"}));
    std::io::stdout().flush()?;
    if input.next().transpose()?.as_deref() != Some("retire") {
        return Err("fixture control".into());
    }
    state.serial = 2;
    lease.publish(&binding, &state)?;
    println!("{}", serde_json::json!({"kind":"input-state-republished"}));
    std::io::stdout().flush()?;
    if input.next().transpose()?.as_deref() != Some("invalid-state") {
        return Err("fixture control".into());
    }
    let invalid = gui_input_context::State {
        serial: 3,
        caret: state.caret,
        hints: 4,
        purpose: 0,
        surrounding: Some(gui_input_context::Surrounding {
            text: "A😀日本ZQ",
            cursor_byte: 2,
            anchor_byte: 13,
        }),
    };
    if lease.publish(&binding, &invalid).is_ok() {
        return Err("invalid state accepted".into());
    }
    println!(
        "{}",
        serde_json::json!({"kind":"input-invalid-state-refused"})
    );
    std::io::stdout().flush()?;
    if input.next().transpose()?.as_deref() != Some("restore") {
        return Err("fixture control".into());
    }
    state.serial = 3;
    lease.publish(&binding, &state)?;
    println!("{}", serde_json::json!({"kind":"input-state-restored"}));
    std::io::stdout().flush()?;
    if input.next().transpose()?.as_deref() != Some("finish") {
        return Err("fixture control".into());
    }
    drop(lease);
    println!("{}", serde_json::json!({"kind":"input-channel-retired"}));
    std::io::stdout().flush()?;
    if input.next().transpose()?.as_deref() != Some("exit") {
        return Err("fixture control".into());
    }
    drop(renderer);
    Ok(())
}
