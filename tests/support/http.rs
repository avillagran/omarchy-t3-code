use std::{io::Read, net::TcpStream, time::Duration};

pub fn read_headers(stream: &mut TcpStream) {
    stream
        .set_read_timeout(Some(Duration::from_secs(4)))
        .unwrap();
    let mut headers = Vec::new();
    while !headers.ends_with(b"\r\n\r\n") {
        assert!(
            headers.len() < 16 * 1024,
            "Fixture request header limit exceeded"
        );
        let mut byte = [0];
        stream.read_exact(&mut byte).unwrap();
        headers.push(byte[0]);
    }
}
