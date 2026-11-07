use omarchy_t3_client::desktop_browser_ipc::*;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
#[test]
fn actual_native_schema_projection_cases() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/private-browser-codecs-4288.json")).unwrap();
    let cases = fixture["schemaCases"].as_array().unwrap();
    assert_eq!(cases.len(), 346);
    for (index, case) in cases.iter().enumerate() {
        let actual = if case["kind"] == "command" {
            decode_command(&case["input"])
        } else {
            decode_event(&case["input"])
        };
        assert_eq!(
            actual.as_ref().unwrap_or(&Value::Null),
            &case["expected"],
            "case {index}"
        );
        if let (Some(actual), Some(expected)) = (
            actual.as_ref().and_then(Value::as_object),
            case["expected"].as_object(),
        ) {
            assert_eq!(
                actual.keys().collect::<Vec<_>>(),
                expected.keys().collect::<Vec<_>>(),
                "property order {index}"
            );
        }
    }
}
#[test]
fn actual_native_parent_control_stream_cases() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/private-browser-codecs-4288.json")).unwrap();
    let cases = fixture["controlTransportCases"].as_array().unwrap();
    assert_eq!(cases.len(), 242);
    for (index, case) in cases.iter().enumerate() {
        let mut decoder = ControlDecoder::default();
        let mut lines = Vec::new();
        for chunk in case["chunks"].as_array().unwrap() {
            let bytes = chunk
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect::<Vec<_>>();
            let batch = decoder.feed(&bytes);
            assert!(!batch.cancelled, "case {index}");
            lines.extend(batch.values);
        }
        let last = decoder.finish();
        assert!(!last.cancelled);
        lines.extend(last.values);
        assert_eq!(json!(lines), case["expected"]["lines"], "lines {index}");
        let commands = lines
            .iter()
            .filter_map(|line| decode_command_line(line))
            .collect::<Vec<_>>();
        // The native fixture is JSON.stringify output: JS represents all
        // numbers as doubles and serializes either signed zero as integer 0.
        // Keep production pointer values unchanged; normalize only this snapshot.
        let mut snapshot = json!(commands);
        for command in snapshot.as_array_mut().unwrap() {
            for name in ["x", "y"] {
                if command[name].as_f64() == Some(0.0) {
                    command[name] = json!(0);
                }
            }
        }
        assert_eq!(snapshot, case["expected"]["commands"], "commands {index}");
    }
}
#[test]
fn framing_limit_retires_buffer_without_replaying_tail() {
    let mut decoder = ControlDecoder::new(4);
    assert!(!decoder.feed(b"abcd").cancelled);
    let failed = decoder.feed(b"x");
    assert!(failed.cancelled);
    assert!(failed.values.is_empty());
    assert!(decoder.finish().values.is_empty());
    assert!(decoder.feed(b"{}\n").cancelled);
}
#[test]
fn dedicated_descriptors_have_exact_direction_and_cancellable_reads() {
    let (mut parent, mut child) = endpoints().unwrap();
    let descriptors = [
        parent.events.as_raw_fd(),
        parent.control.as_raw_fd(),
        child.events.as_raw_fd(),
        child.control.as_raw_fd(),
    ];
    for fd in descriptors {
        assert_ne!(
            unsafe { libc::fcntl(fd, libc::F_GETFD) } & libc::FD_CLOEXEC,
            0
        );
    }
    assert_ne!(child.events.as_raw_fd(), child.control.as_raw_fd());
    let bytes=encode_event(&json!({"type":"cdp","threadId":" thread ","tabId":"tab","message":"opaque\n\"secret-fixture\""})).unwrap();
    parent.events.write_all(&bytes).unwrap();
    let mut received = vec![0; bytes.len()];
    child.events.read_exact(&mut received).unwrap();
    assert_eq!(received, bytes);
    child.control.write_all(b"control\n").unwrap();
    let mut command = [0; 8];
    parent.control.read_exact(&mut command).unwrap();
    assert_eq!(&command, b"control\n");
    assert!(parent.control.write_all(b"wrong-direction").is_err());
    assert!(child.events.write_all(b"wrong-direction").is_err());
    let mut reader = parent.control.try_clone().unwrap();
    reader
        .set_read_timeout(Some(std::time::Duration::from_secs(2)))
        .unwrap();
    let waiting = std::thread::spawn(move || {
        let mut byte = [0; 1];
        reader.read(&mut byte)
    });
    parent.close();
    assert_eq!(waiting.join().unwrap().unwrap(), 0);
}

#[test]
fn admitted_child_copies_survive_exec_without_changing_parent_flags() {
    use std::process::{Command, Stdio};
    let (mut parent, child) = endpoints().unwrap();
    let mut command = Command::new("/usr/bin/python3");
    let (events, control) = child.configure(&mut command).unwrap();
    assert!(events >= 64 && control >= 64 && events != control);
    for fd in [events, control] {
        assert_ne!(
            unsafe { libc::fcntl(fd, libc::F_GETFD) } & libc::FD_CLOEXEC,
            0
        );
    }
    command.args(["-c","import os,sys; assert os.read(int(sys.argv[1]),7)==b'private'; os.write(int(sys.argv[2]),b'control')",&events.to_string(),&control.to_string()]).env_clear().stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    let mut process = command.spawn().unwrap();
    for fd in [events, control] {
        assert_ne!(
            unsafe { libc::fcntl(fd, libc::F_GETFD) } & libc::FD_CLOEXEC,
            0
        );
    }
    parent.events.write_all(b"private").unwrap();
    let mut reply = [0; 7];
    parent.control.read_exact(&mut reply).unwrap();
    assert_eq!(&reply, b"control");
    assert!(process.wait().unwrap().success());
}
