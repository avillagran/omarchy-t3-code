use aes::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};
use base64::Engine;
use omarchy_t3_client::*;
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    thread,
};
fn fixture_catalog(path: &std::path::Path, c: &Value) {
    let mut key = [0u8; 16];
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(b"peanuts", b"saltysalt", 1, &mut key);
    let mut bytes = b"v10".to_vec();
    bytes.extend(
        cbc::Encryptor::<aes::Aes128>::new_from_slices(&key, &[b' '; 16])
            .unwrap()
            .encrypt_padded_vec_mut::<Pkcs7>(c.to_string().as_bytes()),
    );
    fs::write(path,json!({"version":1,"encryptedCatalog":base64::engine::general_purpose::STANDARD.encode(bytes)}).to_string()).unwrap();
}
#[test]
fn canonical_encrypted_crud_and_native_pairing_preserve_other_routes() {
    let root = std::env::temp_dir().join(format!("ot3-catalog-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(root.join("userdata")).unwrap();
    std::env::set_var("T3CODE_HOME", &root);
    let mut c = protocol::connection_catalog::empty();
    c=protocol::connection_catalog::register(&c,json!({"_tag":"SshConnectionTarget","environmentId":"existing","connectionId":"ssh-existing","label":"Existing"}),json!({"_tag":"SshConnectionProfile","environmentId":"existing","connectionId":"ssh-existing","label":"Existing","target":{"alias":"existing"}}),None).unwrap();
    c["futureField"] = json!({"keep":true});
    let path = root.join("userdata/connection-catalog.json");
    fixture_catalog(&path, &c);
    let mut state = write::execute(&json!({"op":"connections-manage","action":"read"})).unwrap();
    let stale = state["revision"].clone();
    state=write::execute(&json!({"op":"connections-manage","action":"rename","environmentId":"existing","label":"Renamed","revision":state["revision"]})).unwrap();
    assert_eq!(state["environments"][0]["name"], "Renamed");
    assert_eq!(write::execute(&json!({"op":"connections-manage","action":"enabled","environmentId":"existing","enabled":false,"revision":stale})).unwrap_err(),"catalog_changed_reload");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let env = uuid::Uuid::new_v4().to_string();
    let expected = env.clone();
    let server = thread::spawn(move || {
        for n in 0..3 {
            let (mut s, _) = listener.accept().unwrap();
            let mut data = vec![];
            let mut part = [0; 4096];
            loop {
                let read = s.read(&mut part).unwrap();
                data.extend(&part[..read]);
                let text = String::from_utf8_lossy(&data);
                if let Some(end) = text.find("\r\n\r\n") {
                    let len = text[..end]
                        .lines()
                        .find_map(|l| {
                            l.to_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|s| s.parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if data.len() >= end + 4 + len {
                        break;
                    }
                }
            }
            let req = String::from_utf8_lossy(&data);
            let value = match n {
                0 => {
                    assert!(req.starts_with("GET /.well-known/t3/environment"));
                    assert!(!req.contains("one-time-code"));
                    json!({"environmentId":expected,"label":"Paired host"})
                }
                1 => {
                    assert!(req.starts_with("POST /oauth/token"));
                    assert!(req.contains("subject_token=one-time-code"));
                    json!({"token_type":"Bearer","access_token":"synthetic-private-session-token"})
                }
                _ => {
                    assert!(req.starts_with("GET /api/auth/session"));
                    assert!(req.contains("Bearer synthetic-private-session-token"));
                    json!({"authenticated":true})
                }
            };
            let body = value.to_string();
            write!(s,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        }
    });
    state=write::execute(&json!({"op":"connections-manage","action":"save-route","kind":"bearer","url":format!("{base}/#token=one-time-code"),"revision":state["revision"]})).unwrap();
    server.join().unwrap();
    assert_eq!(state["environments"].as_array().unwrap().len(), 2);
    assert!(!state.to_string().contains("synthetic-private"));
    assert!(!fs::read_to_string(&path)
        .unwrap()
        .contains("synthetic-private"));
    let stored = read_catalog().unwrap();
    assert_eq!(stored["futureField"]["keep"], true);
    assert_eq!(
        stored["credentials"][0]["credential"]["token"],
        "synthetic-private-session-token"
    );
    assert!(root
        .join("userdata/connection-catalog.ot3-backup.json")
        .exists());
    state=write::execute(&json!({"op":"connections-manage","action":"enabled","environmentId":env,"enabled":false,"revision":state["revision"]})).unwrap();
    assert_eq!(state["environments"][1]["enabled"], false);
    state=write::execute(&json!({"op":"connections-manage","action":"remove","environmentId":env,"confirmed":true,"revision":state["revision"]})).unwrap();
    assert_eq!(state["environments"].as_array().unwrap().len(), 1);
    assert_eq!(read_catalog().unwrap()["credentials"], json!([]));
    fs::write(
        root.join("userdata/desktop-settings.json"),
        "{\"unrelated\":42}",
    )
    .unwrap();
    state=write::execute(&json!({"op":"connections-manage","action":"local-enabled","enabled":false,"revision":state["revision"]})).unwrap();
    assert_eq!(state["localEnabled"], false);
    assert_eq!(
        read_json(&root.join("userdata/desktop-settings.json")).unwrap()["unrelated"],
        42
    );
    // Exercise the production dispatcher and encrypted persistence together.
    fs::write(root.join("userdata/environment-id"), "primary-native-id\n").unwrap();
    fs::write(root.join("userdata/server-runtime.json"), json!({"origin":"http://127.0.0.1:3773"}).to_string()).unwrap();
    fs::write(root.join("userdata/client-settings.json"), json!({"futureSetting":{"keep":true}}).to_string()).unwrap();
    let mut prefs = write::execute(&json!({"op":"connections-manage","action":"preferences-read"})).unwrap();
    assert_eq!(prefs["primaryEnvironmentId"], "primary-native-id");
    let old_client_revision = prefs["client"]["revision"].clone();
    prefs = write::execute(&json!({"op":"connections-manage","action":"client-update","clientRevision":old_client_revision,"patch":{"environmentId":"existing","weight":100}})).unwrap();
    assert_eq!(prefs["client"]["loadBalancingWeights"]["existing"], 100);
    assert_eq!(read_json(&root.join("userdata/client-settings.json")).unwrap()["futureSetting"]["keep"], true);
    assert_eq!(write::execute(&json!({"op":"connections-manage","action":"client-update","clientRevision":old_client_revision,"patch":{"loadBalancingEnabled":true}})).unwrap_err(), "client_settings_changed_reload");
    assert_eq!(write::execute(&json!({"op":"connections-manage","action":"client-update","clientRevision":prefs["client"]["revision"],"patch":{"environmentId":"unknown","weight":100}})).unwrap_err(), "unknown_environment");
    let old_catalog_revision = prefs["catalogRevision"].clone();
    prefs = write::execute(&json!({"op":"connections-manage","action":"github-update","revision":old_catalog_revision,"environmentId":"existing","permission":"read"})).unwrap();
    assert!(prefs["github"]["environments"].as_array().unwrap().iter().any(|r|r["environmentId"]=="existing" && r["permission"]=="read"));
    assert_ne!(prefs["catalogRevision"], old_catalog_revision);
    assert_eq!(read_catalog().unwrap()["futureField"]["keep"], true);
    assert!(read_catalog().unwrap()["githubRoutingPermissions"].as_array().is_some_and(|rows|!rows.is_empty()));
    assert!(!fs::read_to_string(&path).unwrap().contains("githubRoutingPermissions"));
    assert_eq!(write::execute(&json!({"op":"connections-manage","action":"github-update","revision":old_catalog_revision,"environmentId":"existing","permission":"off"})).unwrap_err(), "catalog_changed_reload");
    fs::remove_dir_all(root).unwrap();
}
