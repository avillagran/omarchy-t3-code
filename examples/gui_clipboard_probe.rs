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
            "identity" => {
                use std::{os::fd::AsRawFd, os::unix::process::CommandExt};
                for mode in [
                    "valid",
                    "bad-body",
                    "regular-fd",
                    "foreign-pidfd",
                    "extra-rights",
                    "many-rights",
                    "wrong-parent",
                ] {
                    let pending =
                        omarchy_t3_client::browser_engine_identity::Pending::new().unwrap();
                    let source = pending.source().as_raw_fd();
                    let mut command = std::process::Command::new("/fixture/identity-peer");
                    command
                        .arg(source.to_string())
                        .arg(mode)
                        .stdin(std::process::Stdio::piped())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null());
                    unsafe {
                        command.pre_exec(move || {
                            let flags = libc::fcntl(source, libc::F_GETFD);
                            if flags < 0
                                || libc::fcntl(source, libc::F_SETFD, flags & !libc::FD_CLOEXEC)
                                    != 0
                            {
                                Err(std::io::Error::last_os_error())
                            } else {
                                Ok(())
                            }
                        });
                    }
                    let mut child = command.spawn().unwrap();
                    let expected = if mode == "wrong-parent" {
                        u32::MAX
                    } else {
                        std::process::id()
                    };
                    let admitted = pending.admit(expected);
                    if mode == "valid" {
                        let engine = admitted.unwrap();
                        assert_eq!(engine.pid() as u32, child.id());
                        assert_ne!(
                            unsafe { libc::fcntl(engine.pidfd().as_raw_fd(), libc::F_GETFD) }
                                & libc::FD_CLOEXEC,
                            0
                        );
                    } else {
                        assert!(admitted.is_err(), "{mode}");
                    }
                    child.stdin.take().unwrap().write_all(b"quit\n").unwrap();
                    assert!(child.wait().unwrap().success());
                }
                println!("engine-identity");
                std::io::stdout().flush().unwrap();
            }
            "quit" => break,
            _ => panic!("unknown private fixture command"),
        }
    }
}
