use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use p256::ecdsa::{signature::Verifier, Signature, VerifyingKey};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread,
};
use t3_protocol::*;
const ID: &str = "00000000-0000-4000-8000-000000000001";
struct Request {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}
fn read(s: &mut TcpStream) -> Request {
    s.set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    let mut bytes = vec![];
    while !bytes.ends_with(b"\r\n\r\n") {
        let mut b = [0];
        s.read_exact(&mut b).unwrap();
        bytes.push(b[0]);
        assert!(bytes.len() < 65536);
    }
    let text = String::from_utf8(bytes).unwrap();
    let mut lines = text.split("\r\n");
    let first = lines.next().unwrap().split_whitespace().collect::<Vec<_>>();
    let headers = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_string()))
        .collect::<HashMap<_, _>>();
    let len = headers
        .get("content-length")
        .map(|s| s.parse().unwrap())
        .unwrap_or(0);
    let mut body = vec![0; len];
    s.read_exact(&mut body).unwrap();
    Request {
        method: first[0].into(),
        path: first[1].into(),
        headers,
        body,
    }
}
fn reply(s: &mut TcpStream, v: Value) {
    let body = v.to_string();
    write!(s,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
}
fn proof(r: &Request, base: &str, access: Option<&str>, seen: &mut HashSet<String>) -> String {
    let parts = r.headers["dpop"].split('.').collect::<Vec<_>>();
    assert_eq!(parts.len(), 3);
    let h: Value = serde_json::from_slice(&B64.decode(parts[0]).unwrap()).unwrap();
    let p: Value = serde_json::from_slice(&B64.decode(parts[1]).unwrap()).unwrap();
    assert_eq!(h["typ"], "dpop+jwt");
    assert_eq!(h["alg"], "ES256");
    assert!(h["jwk"].get("d").is_none());
    assert_eq!(p["htm"], r.method);
    assert_eq!(
        p["htu"],
        format!("{base}{}", r.path.split('?').next().unwrap())
    );
    assert!(seen.insert(p["jti"].as_str().unwrap().into()));
    assert!(p["iat"].as_i64().unwrap() > 0);
    if let Some(t) = access {
        assert_eq!(p["ath"], B64.encode(Sha256::digest(t)));
    } else {
        assert!(p.get("ath").is_none());
    }
    let mut key = vec![4];
    key.extend(B64.decode(h["jwk"]["x"].as_str().unwrap()).unwrap());
    key.extend(B64.decode(h["jwk"]["y"].as_str().unwrap()).unwrap());
    VerifyingKey::from_sec1_bytes(&key)
        .unwrap()
        .verify(
            format!("{}.{}", parts[0], parts[1]).as_bytes(),
            &Signature::from_slice(&B64.decode(parts[2]).unwrap()).unwrap(),
        )
        .unwrap();
    B64.encode(Sha256::digest(h["jwk"].to_string()))
}
fn fixture(mismatch: bool) -> (String, thread::JoinHandle<()>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let server_base = base.clone();
    let t = thread::spawn(move || {
        let mut seen = HashSet::new();
        let base = server_base;
        let mut expected_thumb = String::new();
        for i in 0..(if mismatch { 3 } else { 6 }) {
            let (mut s, _) = l.accept().unwrap();
            let r = read(&mut s);
            match i {
                0 => {
                    assert_eq!(r.path, "/v1/client/dpop-token");
                    expected_thumb = proof(&r, &base, None, &mut seen);
                    let form = reqwest::Url::parse(&format!(
                        "https://fixture.invalid/?{}",
                        String::from_utf8(r.body).unwrap()
                    ))
                    .unwrap();
                    let form = form.query_pairs().into_owned().collect::<HashMap<_, _>>();
                    assert_eq!(form["resource"], base);
                    assert_eq!(form["subject_token"], "existing-cloud-jwt");
                    assert_eq!(form["scope"], "environment:connect");
                    reply(
                        &mut s,
                        json!({"token_type":"DPoP","access_token":"relay-access-token","scope":"environment:connect","expires_in":1200}),
                    );
                }
                1 => {
                    assert_eq!(r.path, format!("/v1/environments/{ID}/connect"));
                    assert_eq!(r.headers["authorization"], "DPoP relay-access-token");
                    assert_eq!(
                        proof(&r, &base, Some("relay-access-token"), &mut seen),
                        expected_thumb
                    );
                    let body: Value = serde_json::from_slice(&r.body).unwrap();
                    assert_eq!(body["clientProofKeyThumbprint"], expected_thumb);
                    reply(
                        &mut s,
                        json!({"environmentId":ID,"endpoint":{"httpBaseUrl":base,"wsBaseUrl":base.replacen("http:","ws:",1)},"credential":"one-use-bootstrap-secret","expiresAt":"2099-01-01T00:00:00Z"}),
                    );
                }
                2 => {
                    assert_eq!(r.path, "/.well-known/t3/environment");
                    assert!(!r.headers.contains_key("authorization"));
                    reply(
                        &mut s,
                        json!({"environmentId":if mismatch{"other-environment"}else{ID}}),
                    );
                }
                3 => {
                    assert_eq!(r.path, "/oauth/token");
                    assert_eq!(proof(&r, &base, None, &mut seen), expected_thumb);
                    let body = String::from_utf8(r.body).unwrap();
                    assert!(body.contains("one-use-bootstrap-secret"));
                    assert!(body.contains("orchestration%3Aoperate"));assert!(body.contains("filesystem%3Aread"));assert!(body.contains("settings%3Awrite"));
                    reply(
                        &mut s,
                        json!({"token_type":"DPoP","access_token":"environment-access-token","expires_in":600,"scope":"orchestration:read orchestration:operate"}),
                    );
                }
                4 => {
                    assert_eq!(r.path, "/api/orchestration/shell?limit=2");
                    assert_eq!(r.headers["authorization"], "DPoP environment-access-token");
                    assert_eq!(r.headers["x-t3-orchestration-protocol"], "2");
                    assert_eq!(
                        proof(&r, &base, Some("environment-access-token"), &mut seen),
                        expected_thumb
                    );
                    reply(&mut s, json!({"threads":[]}));
                }
                5 => {
                    assert_eq!(r.method, "POST");
                    assert_eq!(r.path, "/api/auth/websocket-ticket");
                    assert_eq!(
                        proof(&r, &base, Some("environment-access-token"), &mut seen),
                        expected_thumb
                    );
                    reply(&mut s, json!({"ticket":"one-use-ticket"}));
                }
                _ => unreachable!(),
            }
        }
        if !mismatch {
            let (s, _) = l.accept().unwrap();
            s.set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut ws = tungstenite::accept_hdr(
                s,
                |req: &tungstenite::handshake::server::Request,
                 resp: tungstenite::handshake::server::Response| {
                    assert!(req
                        .uri()
                        .query()
                        .unwrap()
                        .contains("wsTicket=one-use-ticket"));
                    assert!(!req.headers().contains_key("authorization"));
                    Ok(resp)
                },
            )
            .unwrap();
            let r: Value = serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
            assert_eq!(r["headers"], json!([]));
            assert_eq!(r["tag"], "orchestration.dispatchCommand");
            ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":{"accepted":true}}}).to_string().into())).unwrap();
        }
    });
    (base, t)
}
#[test]
fn relay_dpop_http_and_rpc_contract() {
    let (base, server) = fixture(false);
    let key = Arc::new(DpopKey::generate());
    let c = connect_relay(&base, "existing-cloud-jwt", "t3-mobile", ID, key).unwrap();
    assert_eq!(
        c.request_json("GET", "/api/orchestration/shell?limit=2", None, true)
            .unwrap(),
        json!({"threads":[]})
    );
    assert_eq!(
        c.rpc(
            "orchestration.dispatchCommand",
            json!({"type":"synthetic-fixture-only"})
        )
        .unwrap(),
        json!({"accepted":true})
    );
    server.join().unwrap();
}
#[test]
fn wrong_environment_never_receives_bootstrap_secret() {
    let (base, server) = fixture(true);
    let err = connect_relay(
        &base,
        "existing-cloud-jwt",
        "t3-web",
        ID,
        Arc::new(DpopKey::generate()),
    )
    .err()
    .unwrap();
    assert_eq!(err, "t3_environment_mismatch");
    server.join().unwrap();
}
#[test]
fn proof_key_roundtrip_and_fresh_request_binding() {
    let key = DpopKey::from_bytes(&[1; 32]).unwrap();
    let other = DpopKey::from_bytes(&key.secret_bytes()).unwrap();
    assert_eq!(key.thumbprint(), other.thumbprint());
    let a = key
        .proof(
            "get",
            "https://t3.example/api/x?a=1#frag",
            Some("access-token"),
        )
        .unwrap();
    let b = key
        .proof("GET", "https://t3.example/api/x?a=2", Some("access-token"))
        .unwrap();
    assert_ne!(a, b);
    let payload: Value =
        serde_json::from_slice(&B64.decode(a.split('.').nth(1).unwrap()).unwrap()).unwrap();
    assert_eq!(payload["htu"], "https://t3.example/api/x");
}
#[test]
fn credentials_cannot_cross_origin_or_use_expired_sessions() {
    assert!(Credential::dpop("expired-token", Arc::new(DpopKey::generate()), 1).is_err());
    for url in [
        "https://user:secret@t3.example",
        "https://t3.example/?token=secret",
        "http://localhost:0",
        "https://t3.example/subpath",
    ] {
        assert!(Connection::new(url, None, Credential::bearer("secret-token").unwrap()).is_err());
    }
    assert!(Connection::new(
        "https://t3.example",
        Some("wss://other.example"),
        Credential::bearer("secret-token").unwrap()
    )
    .is_err());
    let c = Connection::new(
        "https://t3.example",
        None,
        Credential::bearer("secret-token").unwrap(),
    )
    .unwrap();
    assert_eq!(
        c.request_json("GET", "//other.example/path", None, true)
            .unwrap_err(),
        "invalid_endpoint"
    );
}
