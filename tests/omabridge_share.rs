//! Automatic T3 sharing end to end with fake OmaBridge, OpenSSH, systemd and T3:
//! a peer that shares `t3` and `ssh` is paired natively and lands in T3's
//! catalog behind a fixed loopback forward; withdrawal and user removal undo it.
use omarchy_t3_client::{omabridge_share as share,*};
use serde_json::json;
use std::{fs,os::unix::fs::PermissionsExt,path::Path};
const PEER:&str="f3fc84495ad12fcc7bcc5490eae2a4e98da1499b2668b7a05648efe899c17910";
const GONE:&str="0f6da8065c780e468ec60ecb8c9640717766692666135a81e934c246c7f42329";
const DEVICE:&str="75ebf82c1c0f3c6df7c6ccab7ce79e0c5c957e2027d9ea8e144a6966b43fe764";
const ENV:&str="3f0e4a52-6f07-4c8e-9a43-0f4bd5f1c7d2";
const SERVER:&str=r#"
import http.server, json, sys, urllib.parse
port, env, log = int(sys.argv[1]), sys.argv[2], sys.argv[3]
class H(http.server.BaseHTTPRequestHandler):
    def reply(self, body, code=200):
        data = json.dumps(body).encode()
        self.send_response(code); self.send_header("Content-Type", "application/json"); self.send_header("Content-Length", str(len(data))); self.end_headers(); self.wfile.write(data)
    def do_GET(self):
        if self.path == "/.well-known/t3/environment": return self.reply({"environmentId": env, "label": "Devbox"})
        if self.path == "/api/auth/session" and self.headers.get("Authorization") == "Bearer shared-session": return self.reply({"authenticated": True})
        self.reply({}, 401)
    def do_POST(self):
        form = urllib.parse.parse_qs(self.rfile.read(int(self.headers["Content-Length"])).decode())
        open(log, "a").write(json.dumps(form) + "\n")
        if form.get("subject_token") != ["one-use-pairing"]: return self.reply({}, 401)
        self.reply({"access_token": "shared-session", "token_type": "Bearer", "scope": "orchestration:read"})
    def log_message(self, *a): pass
http.server.HTTPServer(("127.0.0.1", port), H).serve_forever()
"#;
fn script(path:&Path,body:&str){fs::write(path,format!("#!/bin/sh\n{body}")).unwrap();fs::set_permissions(path,fs::Permissions::from_mode(0o700)).unwrap();}
fn run()->serde_json::Value{share::execute(&json!({"action":"run"})).unwrap()}
fn targets()->Vec<serde_json::Value>{read_catalog().map(|c|c["targets"].as_array().cloned().unwrap_or_default()).unwrap_or_default()}
#[test]
fn shared_peers_join_the_catalog_and_leave_it_when_withdrawn(){
 let root=std::env::temp_dir().join(format!("ot3-share-{}",&uuid::Uuid::new_v4().to_string()[..8]));
 let _cleanup=Cleanup(root.clone());
 let (bin,host)=(root.join("bin"),root.join("host"));
 for d in [&bin,&host.join(".t3/userdata"),&host.join(".t3/bin"),&root.join("native/userdata"),&root.join("native/bin"),&root.join(".ssh"),&root.join("run")]{fs::create_dir_all(d).unwrap();}
 let state=root.join("data/omabridge");fs::create_dir_all(&state).unwrap();fs::set_permissions(&state,fs::Permissions::from_mode(0o700)).unwrap();fs::write(state.join("state.json"),"{}").unwrap();
 std::env::set_var("HOME",&root);std::env::set_var("T3CODE_HOME",root.join("native"));std::env::set_var("XDG_DATA_HOME",root.join("data"));std::env::set_var("XDG_CONFIG_HOME",root.join("config"));std::env::set_var("XDG_RUNTIME_DIR",root.join("run"));
 std::env::set_var("PATH",format!("{}:{}",bin.display(),std::env::var("PATH").unwrap_or_default()));
 let log=|name:&str|root.join(format!("{name}.log"));
 let read_log=|name:&str|fs::read_to_string(log(name)).unwrap_or_default();
 fs::write(root.join(".ssh/config"),"Host devbox\n  HostName 192.0.2.7\nHost *\n  ServerAliveInterval 30\n").unwrap();
 fs::write(root.join("server.py"),SERVER).unwrap();
 fs::write(root.join("grants"),"[\"t3\",\"ssh\"]").unwrap();
 // OmaBridge: one peer sharing what `grants` says, and this device's identity.
 let omabridge=root.join("omabridge");
 script(&omabridge,&format!("case \"$*\" in\n *' peers') printf '{{\"peers\":[{{\"id\":\"{PEER}\",\"name\":\"devbox\",\"services\":[\"ssh\",\"t3\"],\"remote_services\":%s,\"address\":{{\"addrs\":[{{\"Ip\":\"192.0.2.7:4000\"}}]}}}}]}}\\n' \"$(cat '{grants}')\" ;;\n *manage*) cat >/dev/null; printf '{{\"device\":{{\"id\":\"{DEVICE}\"}}}}\\n' ;;\n *) exit 2 ;;\nesac\n",grants=root.join("grants").display()));
 std::env::set_var("OT3_OMABRIDGE_BIN",&omabridge);
 // OpenSSH: `-G` resolves every alias to the peer's address; a session runs the
 // remote script in the host's home. Only that address is in known_hosts.
 script(&bin.join("ssh"),&format!("for a in \"$@\"; do last=$a; done\ncase \" $* \" in *' -G '*) printf 'hostname 192.0.2.7\\nport 22\\nuser u\\n'; exit 0 ;; esac\nprintf '%s\\n' \"$*\" >> '{}'\nHOME='{}' exec sh -c \"$last\"\n",log("ssh").display(),host.display()));
 script(&bin.join("ssh-keygen"),"[ \"$2\" = 192.0.2.7 ]\n");
 script(&bin.join("secret-tool"),"exit 1\n");
 // systemd: `enable --now` serves the fake T3 on the unit's port; `disable` stops it.
 script(&bin.join("systemctl"),&format!("printf '%s\\n' \"$*\" >> '{log}'\nunit=$(for a in \"$@\"; do last=$a; done; echo $last)\ncase \"$2\" in\n enable) port=$(sed -n 's/.*--listen 127.0.0.1:\\([0-9]*\\).*/\\1/p' \"$XDG_CONFIG_HOME/systemd/user/$unit\"); python3 '{py}' \"$port\" {ENV} '{oauth}' >/dev/null 2>&1 & echo $! > '{root}/'\"$unit\".pid ;;\n disable) [ -f '{root}/'\"$unit\".pid ] && kill $(cat '{root}/'\"$unit\".pid) && rm '{root}/'\"$unit\".pid && sleep 0.3 ;;\nesac\nexit 0\n",log=log("systemctl").display(),py=root.join("server.py").display(),oauth=log("oauth").display(),root=root.display()));
 // T3 on the host mints the pairing; T3 here lists one session per peer label.
 fs::write(host.join(".t3/userdata/environment-id"),ENV).unwrap();
 script(&host.join(".t3/bin/t3"),&format!("printf '%s\\n' \"$*\" >> '{}'\nprintf '{{\"id\":\"p\",\"credential\":\"one-use-pairing\"}}\\n'\n",log("host-t3").display()));
 script(&root.join("native/bin/t3"),&format!("case \"$*\" in\n *list*) printf '[{{\"sessionId\":\"keep-1\",\"client\":{{\"label\":\"ot3-share:{PEER}\"}}}},{{\"sessionId\":\"drop-1\",\"client\":{{\"label\":\"ot3-share:{GONE}\"}}}}]\\n' ;;\n *revoke*) printf '%s\\n' \"$*\" >> '{}' ;;\nesac\n",log("revoke").display()));

 let first=run();
 let row=&first["peers"][0];
 assert_eq!(row["state"],"shared","{first}");
 let port=row["port"].as_u64().unwrap();
 let unit=share::unit_name(PEER);
 let id=share::connection_id(ENV,PEER);
 // T3's catalog has the peer behind the fixed forward, with T3's own session.
 let catalog=read_catalog().unwrap();
 assert_eq!(catalog["targets"][0]["connectionId"],id.as_str());assert_eq!(catalog["targets"][0]["label"],"devbox");
 assert_eq!(catalog["profiles"][0]["httpBaseUrl"],format!("http://127.0.0.1:{port}"));
 assert_eq!(catalog["credentials"][0]["credential"]["token"],"shared-session");
 // The pairing came from the host's CLI over the bridged SSH, labelled for revocation.
 assert!(read_log("host-t3").contains(&format!("auth pairing create --base-dir {}/.t3 --ttl 2m --label ot3-share:{DEVICE} --json",host.display())));
 let ssh=read_log("ssh");
 assert!(ssh.contains("HostKeyAlias=192.0.2.7")&&ssh.contains(&format!("connect --peer {PEER} --service ssh"))&&ssh.contains("-- devbox"),"{ssh}");
 assert!(read_log("oauth").contains(&format!("\"client_label\": [\"ot3-share:{DEVICE}\"]")));
 let unit_body=fs::read_to_string(root.join("config/systemd/user").join(&unit)).unwrap();
 assert!(unit_body.contains(&format!("forward --peer {PEER} --service t3 --listen 127.0.0.1:{port}")));
 // Nothing secret outside T3's encrypted catalog.
 let saved=fs::read_to_string(share::config_path()).unwrap();
 assert!(!saved.contains("shared-session")&&!saved.contains("one-use-pairing"));
 // Host side: only the session of a device no longer shared with is revoked.
 assert!(read_log("revoke").contains("drop-1")&&!read_log("revoke").contains("keep-1"));

 // An unchanged second pass keeps everything and does not pair again.
 let again=run();assert_eq!(again["peers"][0]["state"],"shared");
 assert_eq!(read_log("host-t3").lines().count(),1);
 assert!(read_log("systemctl").contains(&format!("is-active --quiet {unit}")));

 // The peer stops sharing T3: route, forward and share are withdrawn.
 fs::write(root.join("grants"),"[\"ssh\"]").unwrap();
 let withdrawn=run();
 assert!(targets().is_empty());
 assert!(!root.join("config/systemd/user").join(&unit).exists());
 assert!(read_log("systemctl").contains(&format!("disable --now {unit}")));
 assert_eq!(withdrawn["peers"].as_array().unwrap().len(),0);

 // Shared again: paired again.
 fs::write(root.join("grants"),"[\"t3\",\"ssh\"]").unwrap();
 assert_eq!(run()["peers"][0]["state"],"shared");
 assert_eq!(targets().len(),1);

 // The user removes the route: it is not added back until asked to.
 let view=connections_view();
 connections_manage(json!({"action":"remove","revision":view["revision"],"environmentId":ENV,"confirmed":true}));
 assert_eq!(run()["peers"][0]["state"],"skipped");
 assert_eq!(run()["peers"][0]["state"],"skipped");
 assert!(targets().is_empty());
 assert_eq!(share::execute(&json!({"action":"retry","peer":PEER})).unwrap_err(),"confirmation_required");
 share::execute(&json!({"action":"retry","peer":PEER,"confirmed":true})).unwrap();
 let retried=run();assert_eq!(retried["peers"][0]["state"],"shared","{retried}");
 assert_eq!(targets().len(),1);

 // Without SSH there is no native way to pair: reported, nothing installed.
 fs::write(root.join("grants"),"[\"ssh\"]").unwrap();run();
 fs::write(root.join("grants"),"[\"t3\"]").unwrap();
 let blocked=run();
 assert_eq!(blocked["peers"][0]["state"],"error");assert_eq!(blocked["peers"][0]["error"],"share_needs_ssh");
 assert!(!root.join("config/systemd/user").join(&unit).exists());

}
/// Stops the fake T3 servers and removes the fixture, also when an assertion fails.
struct Cleanup(std::path::PathBuf);
impl Drop for Cleanup{fn drop(&mut self){
 for pid in fs::read_dir(&self.0).into_iter().flatten().flatten().filter(|e|e.path().extension().is_some_and(|x|x=="pid")){let _=std::process::Command::new("kill").arg(fs::read_to_string(pid.path()).unwrap_or_default().trim()).status();}
 let _=fs::remove_dir_all(&self.0);
}}
fn connections_view()->serde_json::Value{write::execute(&json!({"op":"connections-manage","action":"read"})).unwrap()}
fn connections_manage(mut v:serde_json::Value){v["op"]=json!("connections-manage");write::execute(&v).unwrap();}
