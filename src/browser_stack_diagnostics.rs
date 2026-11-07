//! Address-only diagnostics for the isolated native-engine fixture.
use std::io::{Read, Write};

const LIMIT: usize = 65_536;

pub(crate) fn drain(mut source: impl Read, mut target: impl Write) {
    let mut chunk = [0; 4096];
    let mut line = Vec::with_capacity(256);
    let mut oversized = false;
    let mut retained = 0;
    loop {
        let count = match source.read(&mut chunk) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        for byte in &chunk[..count] {
            if *byte == b'\n' {
                if !oversized {
                    if let Some(pc) = address(&line) {
                        let length = b"native-stack-pc ".len() + pc.len() + 1;
                        if length <= LIMIT.saturating_sub(retained) {
                            retained += length;
                            let _ = target.write_all(b"native-stack-pc ");
                            let _ = target.write_all(pc);
                            let _ = target.write_all(b"\n");
                        }
                    }
                }
                line.clear();
                oversized = false;
            } else if line.len() < 256 {
                line.push(*byte);
            } else {
                oversized = true;
            }
        }
    }
}

fn address(line: &[u8]) -> Option<&[u8]> {
    let start = line.iter().position(|byte| !byte.is_ascii_whitespace())?;
    let line = line.get(start..)?.strip_prefix(b"#")?;
    let digits = line.iter().take_while(|byte| byte.is_ascii_digit()).count();
    if !(1..=3).contains(&digits) || line.get(digits) != Some(&b' ') {
        return None;
    }
    let pc = line.get(digits + 1..)?.strip_prefix(b"0x")?;
    let end = pc
        .iter()
        .take_while(|byte| byte.is_ascii_hexdigit())
        .count();
    if !(1..=16).contains(&end) || pc.get(end).is_some_and(|byte| !byte.is_ascii_whitespace()) {
        return None;
    }
    line.get(digits + 1..digits + 3 + end)
}
