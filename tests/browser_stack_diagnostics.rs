#[path = "../src/browser_stack_diagnostics.rs"]
mod browser_stack_diagnostics;

#[test]
fn native_stack_diagnostics_keep_addresses_without_arguments_or_payloads() {
    let input = b"Authorization: fixture-canary\n#0 0x1234 Function(secret-canary)\n  #12 0xABCDEF other-private-data\n#1 0x12z invalid\n#1 0x private\n#1000 0x12 invalid-index\n#3 0x123456789abcdef01 invalid-width\n#2 0x7654\n";
    let mut output = Vec::new();
    browser_stack_diagnostics::drain(&input[..], &mut output);
    assert_eq!(
        output,
        b"native-stack-pc 0x1234\nnative-stack-pc 0xABCDEF\nnative-stack-pc 0x7654\n"
    );
}

#[test]
fn native_stack_diagnostics_drain_after_output_limit_and_drop_long_lines() {
    use std::io::{Cursor, Read};
    let mut input = Vec::new();
    input.extend_from_slice(b"#0 0x1234 ");
    input.extend(std::iter::repeat_n(b'x', 8192));
    input.push(b'\n');
    for _ in 0..10_000 {
        input.extend_from_slice(b"#1 0x1234 argument-canary\n");
    }
    let mut source = Cursor::new(input);
    let mut output = Vec::new();
    browser_stack_diagnostics::drain(&mut source, &mut output);
    assert!(output.len() <= 65_536);
    assert!(output.starts_with(b"native-stack-pc 0x1234\n"));
    assert_eq!(source.read(&mut [0; 1]).unwrap(), 0);
    assert!(output.iter().all(u8::is_ascii));
    assert!(!String::from_utf8(output).unwrap().contains("canary"));
}
