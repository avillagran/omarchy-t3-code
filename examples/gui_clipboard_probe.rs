//! Synthetic ownership driver for the isolated native GUI plugin acceptance.
use std::io::{BufRead, Write};
fn main() {
    let session = omarchy_t3_client::gui_clipboard::start().expect("actual GUI admission");
    assert!(session.alive());
    println!("admitted");
    std::io::stdout().flush().unwrap();
    for line in std::io::stdin().lock().lines() {
        match line.unwrap().as_str() {
            "alive" => {
                println!("{}", session.alive());
                std::io::stdout().flush().unwrap();
            }
            "display" => {
                use std::os::fd::AsRawFd;
                let display = omarchy_t3_client::browser_native_display::Display::launch(
                    std::path::Path::new("/usr/bin/Xvfb"),
                )
                .unwrap();
                let pid = display.server_pid();
                let descriptor = display.authorization().as_raw_fd();
                assert_eq!(
                    unsafe { libc::fcntl(descriptor, libc::F_GET_SEALS) },
                    libc::F_SEAL_WRITE
                        | libc::F_SEAL_SHRINK
                        | libc::F_SEAL_GROW
                        | libc::F_SEAL_SEAL
                );
                assert_ne!(
                    unsafe { libc::fcntl(descriptor, libc::F_GETFD) } & libc::FD_CLOEXEC,
                    0
                );
                let run = |authorization: &str| {
                    std::process::Command::new("/usr/bin/xprop")
                        .args(["-display", &display.name(), "-root"])
                        .env("XAUTHORITY", authorization)
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status()
                        .unwrap()
                        .success()
                };
                assert!(!run("/tmp/no-owned-x-authorization"));
                assert!(run(&display.authorization_path()));
                drop(display);
                assert_ne!(unsafe { libc::kill(pid as i32, 0) }, 0);
                println!("owned-display");
                std::io::stdout().flush().unwrap();
            }
            "quit" => break,
            _ => panic!("unknown private fixture command"),
        }
    }
}
