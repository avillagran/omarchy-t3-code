//! A multiplexed forward succeeds with no long-lived `ssh` client process.
use aes::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};
use base64::Engine;
use omarchy_t3_client::*;
use serde_json::json;
use std::{fs, net::TcpStream, os::unix::fs::PermissionsExt};

#[test]
fn multiplex_forward_survives_client_exit_and_reconnect_cancels_only_its_listener() {
    let root = std::env::temp_dir().join(format!("ot3-mux-{}", &uuid::Uuid::new_v4().to_string()[..8]));
    let data = root.join("native/userdata");
    fs::create_dir_all(&data).unwrap();
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join("runtime")).unwrap();
    std::env::set_var("HOME", &root);
    std::env::set_var("T3CODE_HOME", root.join("native"));
    std::env::set_var("XDG_RUNTIME_DIR", root.join("runtime"));
    std::env::set_var("XDG_CONFIG_HOME", root.join("config"));
    std::env::set_var("PATH", format!("{}:/usr/bin:/bin", root.join("bin").display()));
    std::env::set_var("OT3_MUX_TEST_ROOT", &root);
    let id = "8b55eaad-3149-443c-953f-afd6dd002666";
    let catalog = json!({"schemaVersion":1,"targets":[{"_tag":"SshConnectionTarget","environmentId":id,"connectionId":"ssh-test","label":"Fixture"}],"profiles":[{"connectionId":"ssh-test","target":{"hostname":"fixture","username":"test"}}],"credentials":[]});
    let mut key = [0u8; 16];
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(b"peanuts", b"saltysalt", 1, &mut key);
    let mut bytes = b"v10".to_vec();
    bytes.extend(cbc::Encryptor::<aes::Aes128>::new_from_slices(&key, &[b' ';16]).unwrap().encrypt_padded_vec_mut::<Pkcs7>(catalog.to_string().as_bytes()));
    fs::write(data.join("connection-catalog.json"), json!({"version":1,"encryptedCatalog":base64::engine::general_purpose::STANDARD.encode(bytes)}).to_string()).unwrap();
    fs::write(data.join("desktop-settings.json"), r#"{"localEnvironmentEnabled":false}"#).unwrap();
    let ssh = root.join("bin/ssh");
    fs::write(&ssh, r#"#!/usr/bin/python3
import base64, json, os, pathlib, signal, socket, subprocess, sys, time
root = pathlib.Path(os.environ['OT3_MUX_TEST_ROOT'])
args = sys.argv[1:]
with (root/'calls').open('a') as out: out.write(json.dumps(args)+'\n')
if '-O' in args:
    action = args[args.index('-O')+1]
    forward = args[args.index('-L')+1]
    port = int(forward.split(':')[1])
    pidfile = root/str(port)
    if action == 'forward':
        if (root/'fail').exists(): sys.exit(1)
        # Listener owned by the master, not this short-lived command.
        code = '''import os,pathlib,socket,sys
s=socket.socket();s.bind(('127.0.0.1',int(sys.argv[1])));s.listen()
pathlib.Path(sys.argv[2]).write_text(str(os.getpid()))
while True:
 c,_=s.accept();c.close()
'''
        subprocess.Popen(['/usr/bin/python3','-c',code,str(port),str(pidfile)], stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,start_new_session=True)
        for _ in range(100):
            if pidfile.exists(): sys.exit(0)
            time.sleep(.01)
        sys.exit(1)
    if action == 'cancel':
        os.kill(int(pidfile.read_text()), signal.SIGTERM)
        for _ in range(100):
            try:
                with socket.create_connection(('127.0.0.1',port),.05): pass
            except OSError: break
            time.sleep(.01)
        pidfile.unlink()
        sys.exit(0)
    sys.exit(1)
if '-N' in args: sys.exit(0) # Reproduce the old successful client exit.
sys.stdin.read()
print(json.dumps({'port':8643}))
print('T3_AUTH')
payload = base64.urlsafe_b64encode(json.dumps({'exp':int(time.time()*1000)+86400000}).encode()).decode().rstrip('=')
print(payload+'.synthetic-signature')
"#).unwrap();
    fs::set_permissions(&ssh, fs::Permissions::from_mode(0o700)).unwrap();
    let first = environments().unwrap().remove(0);
    assert_eq!(first.connection_error, None);
    let address = first.base_url.trim_start_matches("http://");
    assert!(TcpStream::connect(address).is_ok());
    let calls = fs::read_to_string(root.join("calls")).unwrap();
    refresh_connections();
    assert_eq!(environments().unwrap()[0].base_url, first.base_url);
    assert_eq!(fs::read_to_string(root.join("calls")).unwrap(), calls, "cached listener is reused");
    write::execute(&json!({"op":"connections-manage","action":"reconnect","environmentId":id})).unwrap();
    let second = environments().unwrap().remove(0);
    assert_eq!(second.connection_error, None);
    assert!(TcpStream::connect(address).is_err(), "old listener was cancelled");
    assert!(TcpStream::connect(second.base_url.trim_start_matches("http://")).is_ok());
    // A failed replacement must not leave the prior forward behind.
    fs::write(root.join("fail"), "").unwrap();
    write::execute(&json!({"op":"connections-manage","action":"reconnect","environmentId":id})).unwrap();
    assert_eq!(environments().unwrap()[0].connection_error.as_deref(), Some("ssh_unavailable"));
    assert!(TcpStream::connect(second.base_url.trim_start_matches("http://")).is_err());
    let calls = fs::read_to_string(root.join("calls")).unwrap();
    assert!(!calls.contains("\"exit\""), "never stop the shared master");
    assert_eq!(calls.lines().filter(|line|line.contains("\"cancel\"")).count(), 2);
    fs::remove_file(root.join("fail")).unwrap();
    // An abrupt owner exit closes the guardian's pipe: no static destructor or
    // signal handler in the owner is required to remove the master's listener.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let status = std::process::Command::new("/usr/bin/python3").args(["-c", r#"
import os,pathlib,subprocess,sys,time
pidfile=pathlib.Path(sys.argv[2])/sys.argv[3]
child=subprocess.Popen([sys.argv[1],'__ssh-forward','127.0.0.1:'+sys.argv[3]+':127.0.0.1:8643','--','fixture'],stdin=subprocess.PIPE,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
for _ in range(300):
 if pidfile.exists(): os._exit(0)
 if child.poll() is not None: os._exit(1)
 time.sleep(.01)
os._exit(2)
"#, env!("CARGO_BIN_EXE_omarchy-t3-client"), root.to_str().unwrap(), &port.to_string()]).status().unwrap();
    assert!(status.success());
    for _ in 0..300 {
        if !root.join(port.to_string()).exists() { break }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(!root.join(port.to_string()).exists(), "guardian cleaned up after abrupt owner exit");
    assert!(TcpStream::connect((std::net::Ipv4Addr::LOCALHOST,port)).is_err());
    fs::remove_dir_all(root).unwrap();
}
