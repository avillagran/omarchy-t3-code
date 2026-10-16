//! Desktop-only adapter. Native Clerk session/configuration are owned by T3.
//! The portable t3-protocol library does not read files or invoke processes.
use crate::*;
use base64::Engine;
use std::{io::Read, process::Command, sync::Arc};
use zeroize::Zeroizing;
struct NativeConfig {
    clerk: String,
    template: String,
    relay: String,
}
fn literal(source: &str, marker: &str) -> Option<String> {
    for (offset, _) in source.match_indices(marker) {
        let value = source[offset + marker.len()..].trim_start();
        let Some(open) = value.find('(').filter(|i| *i < 64) else {
            continue;
        };
        if !value[..open]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_$".contains(&b))
        {
            continue;
        }
        let value = value[open + 1..].trim_start();
        let Some(quote) = value.chars().next() else {
            continue;
        };
        if !matches!(quote, '\'' | '"' | '`') {
            continue;
        }
        let Some(end) = value[1..].find(quote).map(|e| e + 1) else {
            continue;
        };
        let v = &value[1..end];
        if v.is_empty() || v.len() > 1024 || v.contains(['\\', '$', '\n', '\r']) {
            continue;
        }
        return Some(v.into());
    }
    None
}
fn native_config() -> Result<NativeConfig> {
    // Read public build configuration from the actual installed T3 server assets.
    // These are data; no JavaScript engine or original GUI is started.
    let runtime = read_json(&t3_home().join("userdata/server-runtime.json")).unwrap_or(Value::Null);
    let mut clients = vec![];
    if let Some(pid) = runtime["pid"].as_i64().filter(|p| *p > 0) {
        if let Ok(exe) = fs::read_link(format!("/proc/{pid}/exe")) {
            if let Some(parent) = exe.parent() {
                clients.push(parent.join("client/assets"));
            }
        }
    }
    if let Ok(entries) = fs::read_dir(t3_home().join("runtime/versions")) {
        let mut versions = entries
            .flatten()
            .filter_map(|e| e.file_type().ok().filter(|t| t.is_dir()).map(|_| e.path()))
            .collect::<Vec<_>>();
        versions.sort();
        for version in versions.into_iter().rev() {
            let assets = version.join("client/assets");
            if !clients.contains(&assets) {
                clients.push(assets);
            }
        }
    }
    for dir in clients {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for f in entries.flatten().filter(|f| {
            f.file_name().to_string_lossy().starts_with("runtime-")
                && f.path().extension().is_some_and(|s| s == "js")
        }) {
            let file = fs::File::open(f.path()).map_err(|_| "t3_connect_config_unavailable")?;
            let mut source = String::new();
            file.take(MAX_BYTES + 1)
                .read_to_string(&mut source)
                .map_err(|_| "t3_connect_config_unavailable")?;
            if source.len() as u64 > MAX_BYTES {
                continue;
            }
            let Some(key) = literal(&source, "clerkPublishableKey:") else {
                continue;
            };
            let Some(template) = literal(&source, "clerkJwtTemplate:") else {
                continue;
            };
            let Some(relay) = literal(&source, "relayUrl:") else {
                continue;
            };
            let key = key
                .strip_prefix("pk_live_")
                .or_else(|| key.strip_prefix("pk_test_"))
                .ok_or("t3_connect_config_unavailable")?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(key)
                .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(key))
                .map_err(|_| "t3_connect_config_unavailable")?;
            let host = std::str::from_utf8(&bytes)
                .map_err(|_| "t3_connect_config_unavailable")?
                .trim_end_matches('$');
            if host.is_empty()
                || !host
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b".-".contains(&b))
                || !template
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            {
                return Err("t3_connect_config_unavailable".into());
            }
            let clerk = format!("https://{host}");
            validate_base(&clerk)?;
            validate_base(&relay)?;
            if !relay.starts_with("https://") {
                return Err("invalid_endpoint".into());
            }
            return Ok(NativeConfig {
                clerk,
                template,
                relay,
            });
        }
    }
    Err("t3_connect_config_unavailable".into())
}
fn native_client_token() -> Result<Zeroizing<String>> {
    let path = t3_home().join("userdata/clerk-tokens.json");
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| "t3_connect_sign_in_required")?;
    let m = file.metadata().map_err(|_| "t3_connect_sign_in_required")?;
    // electron-store uses its own file modes (including 0666). The token must
    // be encrypted by T3's keyring; do not change the original app's file.
    if !m.is_file() || m.uid() != unsafe { libc::geteuid() } || m.len() > 65536 {
        return Err("unauthorized".into());
    }
    let mut bytes = Zeroizing::new(Vec::new());
    file.take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| "t3_connect_sign_in_required")?;
    if bytes.len() > 65536 {
        return Err("too_large".into());
    }
    let doc: Value = serde_json::from_slice(&bytes).map_err(|_| "t3_connect_sign_in_required")?;
    let stored = text(&doc, "__clerk_client_jwt");
    let Some(encoded) = stored.strip_prefix("enc:") else {
        return Err("unsupported_clerk_storage".into());
    };
    let encrypted = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| "unsupported_clerk_storage")?;
    let password = if encrypted.starts_with(b"v10") {
        Zeroizing::new(b"peanuts".to_vec())
    } else if encrypted.starts_with(b"v11") {
        let output = Command::new("timeout")
            .args(["4s", "secret-tool", "lookup", "application", "t3code"])
            .output()
            .map_err(|_| "catalog_locked")?;
        if !output.status.success() {
            return Err("catalog_locked".into());
        }
        let mut p = Zeroizing::new(output.stdout);
        while matches!(p.last(), Some(b'\n' | b'\r')) {
            p.pop();
        }
        p
    } else {
        return Err("unsupported_clerk_storage".into());
    };
    let plain = crate::connections::decrypt_storage(&encrypted, &password)?;
    let value = std::str::from_utf8(&plain).map_err(|_| "unsupported_clerk_storage")?;
    if value.len() < 16 || value.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("t3_connect_sign_in_required".into());
    }
    Ok(Zeroizing::new(value.into()))
}
fn clerk_response(
    response: reqwest::blocking::Response,
    client: &mut Zeroizing<String>,
) -> Result<Value> {
    if !response.status().is_success() {
        return Err(if matches!(response.status().as_u16(), 401 | 403) {
            "t3_connect_sign_in_required"
        } else {
            "t3_connect_session_unavailable"
        }
        .into());
    }
    // Clerk rotates this header exactly as @clerk/electron's native transport.
    // Keep rotation in memory; never overwrite the official app's store.
    if let Some(next) = response
        .headers()
        .get("authorization")
        .and_then(|s| s.to_str().ok())
    {
        let next = next.strip_prefix("Bearer ").unwrap_or(next);
        if next.len() >= 16 && !next.chars().any(|c| c.is_whitespace() || c.is_control()) {
            *client = Zeroizing::new(next.into());
        }
    }
    let mut bytes = Zeroizing::new(Vec::new());
    response
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "t3_connect_session_unavailable")?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("too_large".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "t3_connect_session_unavailable".into())
}
fn cloud_jwt(config: &NativeConfig) -> Result<Zeroizing<String>> {
    cloud_jwt_with_client(config, native_client_token()?)
}
fn cloud_jwt_with_client(
    config: &NativeConfig,
    mut client: Zeroizing<String>,
) -> Result<Zeroizing<String>> {
    let http = Http::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|_| "local_error")?;
    let response = http
        .get(format!("{}/v1/client?_is_native=1", config.clerk))
        .bearer_auth(client.as_str())
        .header("Accept", "application/json")
        .send()
        .map_err(|_| "t3_connect_session_unavailable")?;
    let body = clerk_response(response, &mut client)?;
    let state = &body["response"];
    let active = text(state, "last_active_session_id");
    let sessions = state["sessions"]
        .as_array()
        .ok_or("t3_connect_sign_in_required")?;
    let session = sessions
        .iter()
        .find(|s| s["status"] == "active" && s["id"] == active)
        .or_else(|| {
            let active = sessions
                .iter()
                .filter(|s| s["status"] == "active")
                .collect::<Vec<_>>();
            if active.len() == 1 {
                Some(active[0])
            } else {
                None
            }
        })
        .ok_or("t3_connect_sign_in_required")?;
    let id = text(session, "id");
    if !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') || id.is_empty() {
        return Err("t3_connect_sign_in_required".into());
    }
    let response = http
        .post(format!(
            "{}/v1/client/sessions/{id}/tokens/{}?_is_native=1",
            config.clerk, config.template
        ))
        .bearer_auth(client.as_str())
        .header("Accept", "application/json")
        .form(&[] as &[(&str, &str)])
        .send()
        .map_err(|_| "t3_connect_session_unavailable")?;
    let body = clerk_response(response, &mut client)?;
    let jwt = body["jwt"]
        .as_str()
        .or_else(|| body["response"]["jwt"].as_str())
        .filter(|s| s.split('.').count() == 3)
        .ok_or("t3_connect_session_unavailable")?;
    Ok(Zeroizing::new(jwt.into()))
}
pub(super) fn connect(env: &mut Environment) -> Result<()> {
    let config = native_config()?;
    let jwt = cloud_jwt(&config)?;
    let key = Arc::new(protocol::DpopKey::generate());
    let connection = protocol::connect_relay(&config.relay, &jwt, "t3-web", &env.id, key)?;
    env.base_url = connection.base_url().into();
    env.connection = Some(Arc::new(connection));
    Ok(())
}
/// Checks the official session without exposing tokens or connecting an environment.
pub fn check_native_session() -> Result<Value> {
    let config = native_config()?;
    let jwt = cloud_jwt(&config)?;
    let _relay = protocol::RelayClient::new(
        &config.relay,
        &jwt,
        "t3-web",
        Arc::new(protocol::DpopKey::generate()),
    )?;
    Ok(json!({"session":"ready","relayAuthentication":"ready","configuration":"inherited"}))
}
pub fn check_native_environment(id: &str) -> Result<Value> {
    let config = native_config()?;
    let jwt = cloud_jwt(&config)?;
    let connection = protocol::connect_relay(
        &config.relay,
        &jwt,
        "t3-web",
        id,
        Arc::new(protocol::DpopKey::generate()),
    )?;
    let shell = connection.request_json("GET", "/api/orchestration/shell", None, true)?;
    let config = connection.rpc("server.getConfig", json!({}))?;
    Ok(
        json!({"environmentId":id,"connection":"ready","http":"ready","rpc":"ready","providerCount":config["providers"].as_array().map(|p|p.len()),"shellReceived":shell.is_object()}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_config_parser_rejects_executable_literals() {
        assert_eq!(
            literal("clerkJwtTemplate:Fv(`t3-relay`)", "clerkJwtTemplate:"),
            Some("t3-relay".into())
        );
        assert!(literal("clerkJwtTemplate:Fv(`${execute()}`)", "clerkJwtTemplate:").is_none());
        assert!(literal("clerkJwtTemplate:Fv(`bad\\nvalue`)", "clerkJwtTemplate:").is_none());
    }
}

#[cfg(test)]
mod session_tests {
    use super::*;
    use std::{io::Write, net::TcpListener, thread};
    #[test]
    fn inherited_session_uses_active_account_and_rotation_without_writing_store() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            for i in 0..2 {
                let (mut s, _) = listener.accept().unwrap();
                s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
                let mut bytes = vec![];
                while !bytes.ends_with(b"\r\n\r\n") {
                    let mut b = [0];
                    s.read_exact(&mut b).unwrap();
                    bytes.push(b[0]);
                }
                let request = String::from_utf8(bytes).unwrap();
                let body=if i==0 {
                    assert!(request.starts_with("GET /v1/client?_is_native=1"));assert!(request.contains("Bearer synthetic-client-token"));
                    json!({"response":{"last_active_session_id":"sess_selected","sessions":[{"id":"sess_other","status":"active"},{"id":"sess_selected","status":"active"}]}})
                }else{
                    assert!(request.starts_with("POST /v1/client/sessions/sess_selected/tokens/native-template?_is_native=1"));assert!(request.contains("Bearer rotated-native-client-token"));
                    json!({"jwt":"synthetic.session.jwt"})
                }.to_string();
                write!(s,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAuthorization: Bearer rotated-native-client-token\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            }
        });
        let config = NativeConfig {
            clerk: base,
            template: "native-template".into(),
            relay: "https://unused.example".into(),
        };
        assert_eq!(
            cloud_jwt_with_client(&config, Zeroizing::new("synthetic-client-token".into()))
                .unwrap()
                .as_str(),
            "synthetic.session.jwt"
        );
        server.join().unwrap();
    }
}
