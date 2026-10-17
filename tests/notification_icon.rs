use omarchy_t3_client::*;
use serde_json::{json, Value};
use std::{
    fs,
    io::{Cursor, Read, Write},
    net::TcpListener,
    os::unix::fs::{MetadataExt, PermissionsExt},
    thread,
};
#[test]
fn native_favicon_is_signed_cached_and_badged_with_safe_fallback() {
    let root = std::env::temp_dir().join(format!("t3-notification-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    std::env::set_var("XDG_CACHE_HOME", &root);
    let token = root.join("token");
    fs::write(&token, "synthetic-secret-token").unwrap();
    fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let config = root.join("env.json");
    fs::write(&config,json!({"environments":[{"id":"fixture","name":"Fixture","base_url":format!("http://{}",listener.local_addr().unwrap()),"token_file":token}]}).to_string()).unwrap();
    std::env::set_var("OMARCHY_T3_FIXTURE_ENVIRONMENTS", config);
    let server = thread::spawn(move || {
        let (s, _) = listener.accept().unwrap();
        let mut ws = tungstenite::accept(s).unwrap();
        let req: Value = serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(req["tag"], "assets.createUrl");
        assert_eq!(req["payload"]["resource"]["_tag"], "project-favicon");
        assert_eq!(req["payload"]["resource"]["cwd"], "/fixture/project");
        ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":{"relativeUrl":"/api/assets/project?signature=synthetic","expiresAt":9999999999999u64}}}).to_string().into())).unwrap();
        let (mut s, _) = listener.accept().unwrap();
        s.set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        let mut request = [0; 8192];
        let n = s.read(&mut request).unwrap();
        let header = String::from_utf8_lossy(&request[..n]).to_lowercase();
        assert!(header.starts_with("get /api/assets/project?signature=synthetic "));
        assert!(!header.contains("authorization:"));
        let image = image::RgbaImage::from_pixel(32, 32, image::Rgba([220, 20, 60, 255]));
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        write!(s,"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",bytes.get_ref().len()).unwrap();
        s.write_all(bytes.get_ref()).unwrap();
    });
    let snapshot = json!({"projects":[{"id":"project","environmentId":"fixture","workspaceRoot":"/fixture/project"}]});
    let thread = json!({"projectId":"project","environmentId":"fixture","reason":"request"});
    let path = notification_icon::path(&snapshot, &thread).unwrap();
    server.join().unwrap();
    assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
    let icon = image::open(&path).unwrap().to_rgba8();
    assert_eq!(icon.get_pixel(30, 30).0, [220, 20, 60, 255]);
    assert_eq!(icon.get_pixel(76, 64).0, [55, 115, 225, 255]);
    assert_eq!(notification_icon::path(&snapshot, &thread).unwrap(), path);
    assert!(!path.to_string_lossy().contains("synthetic"));
    let fallback = notification_icon::path(&json!({}), &json!({"reason":"failed"})).unwrap();
    let failure = image::open(fallback).unwrap().to_rgba8();
    assert_eq!(failure.get_pixel(76, 64).0, [215, 55, 65, 255]);
    assert_ne!(failure.get_pixel(40, 40).0, [0, 0, 0, 0]);
    let plain = notification_icon::path(&json!({}), &json!({})).unwrap();
    assert_ne!(fs::read(plain).unwrap(), fs::read(path).unwrap());

    let override_snapshot = json!({"projects":[{"id":"custom","environmentId":"offline","projectIcon":{"kind":"lucide","name":"flask-conical","color":"green"}}]});
    let custom = notification_icon::path(
        &override_snapshot,
        &json!({"projectId":"custom","environmentId":"offline","reason":"review"}),
    )
    .unwrap();
    let custom = image::open(custom).unwrap().to_rgba8();
    assert!(custom
        .pixels()
        .any(|p| p[1] as u16 > p[0] as u16 * 2 && p[3] > 0));
    assert_eq!(custom.get_pixel(76, 64).0, [34, 160, 88, 255]);
    fs::remove_dir_all(root).unwrap();
}
