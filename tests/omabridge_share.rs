//! Automatic T3 sharing end to end with fake OmaBridge, systemd and T3: a peer
//! that shares `t3` hands out a native one-use pairing over OmaBridge itself (no
//! SSH), which lands in T3's catalog behind a fixed loopback forward; withdrawal
//! and user removal undo it.
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
 for d in [&bin,&host.join(".t3/userdata"),&host.join(".t3/bin"),&root.join("native/userdata"),&root.join("native/bin"),&root.join("run")]{fs::create_dir_all(d).unwrap();}
 let state=root.join("data/omabridge");fs::create_dir_all(&state).unwrap();fs::set_permissions(&state,fs::Permissions::from_mode(0o700)).unwrap();fs::write(state.join("state.json"),"{}").unwrap();
 std::env::set_var("HOME",&root);std::env::set_var("T3CODE_HOME",root.join("native"));std::env::set_var("XDG_DATA_HOME",root.join("data"));std::env::set_var("XDG_CONFIG_HOME",root.join("config"));std::env::set_var("XDG_RUNTIME_DIR",root.join("run"));
 std::env::set_var("PATH",format!("{}:{}",bin.display(),std::env::var("PATH").unwrap_or_default()));
 let log=|name:&str|root.join(format!("{name}.log"));
 let read_log=|name:&str|fs::read_to_string(log(name)).unwrap_or_default();
 fs::write(root.join("server.py"),SERVER).unwrap();
 fs::write(root.join("grants"),"[\"t3\"]").unwrap();
 // OmaBridge: one peer sharing what `grants` says. Its server issues a pairing
 // from the host's T3 while `t3` is granted and the host has not refused.
 let omabridge=root.join("omabridge");
 script(&omabridge,&format!("case \"$*\" in\n *' peers') printf '{{\"peers\":[{{\"id\":\"{PEER}\",\"name\":\"devbox\",\"services\":[\"ssh\",\"t3\"],\"remote_services\":%s}}]}}\\n' \"$(cat '{grants}')\" ;;\n *'t3-pair --peer {PEER}') printf '%s\\n' \"$*\" >> '{pairs}'\n  if [ -e '{refuse}' ] || ! grep -q t3 '{grants}'; then echo 'Error: omabridge_t3_pair_refused' >&2; exit 1; fi\n  HOME='{host}' '{host}/.t3/bin/t3' auth pairing create --base-dir '{host}/.t3' --ttl 2m --label omabridge:{DEVICE} --json >/dev/null\n  printf '{{\"environmentId\":\"{ENV}\",\"credential\":\"one-use-pairing\",\"label\":\"omabridge:{DEVICE}\"}}\\n' ;;\n *) exit 2 ;;\nesac\n",grants=root.join("grants").display(),pairs=log("omabridge-pair").display(),refuse=root.join("refuse").display(),host=host.display()));
 std::env::set_var("OT3_OMABRIDGE_BIN",&omabridge);
 script(&bin.join("secret-tool"),"exit 1\n");
 // systemd: `enable --now` serves the fake T3 on the unit's port; `disable` stops it.
 script(&bin.join("systemctl"),&format!("printf '%s\\n' \"$*\" >> '{log}'\nunit=$(for a in \"$@\"; do last=$a; done; echo $last)\ncase \"$2\" in\n enable) port=$(sed -n 's/.*--listen 127.0.0.1:\\([0-9]*\\).*/\\1/p' \"$XDG_CONFIG_HOME/systemd/user/$unit\"); python3 '{py}' \"$port\" {ENV} '{oauth}' >/dev/null 2>&1 & echo $! > '{root}/'\"$unit\".pid ;;\n disable) [ -f '{root}/'\"$unit\".pid ] && kill $(cat '{root}/'\"$unit\".pid) && rm '{root}/'\"$unit\".pid && sleep 0.3 ;;\nesac\nexit 0\n",log=log("systemctl").display(),py=root.join("server.py").display(),oauth=log("oauth").display(),root=root.display()));
 // T3 on the host mints the pairing; T3 here lists one session per peer label.
 fs::write(host.join(".t3/userdata/environment-id"),ENV).unwrap();
 script(&host.join(".t3/bin/t3"),&format!("printf '%s\\n' \"$*\" >> '{}'\nprintf '{{\"id\":\"p\",\"credential\":\"one-use-pairing\"}}\\n'\n",log("host-t3").display()));
 script(&root.join("native/bin/t3"),&format!("case \"$*\" in\n *list*) printf '[{{\"sessionId\":\"keep-1\",\"client\":{{\"label\":\"omabridge:{PEER}\"}}}},{{\"sessionId\":\"drop-1\",\"client\":{{\"label\":\"omabridge:{GONE}\"}}}}]\\n' ;;\n *revoke*) printf '%s\\n' \"$*\" >> '{}' ;;\nesac\n",log("revoke").display()));

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
 // The pairing came over OmaBridge itself, from the host's T3, labelled for revocation.
 assert!(read_log("omabridge-pair").contains(&format!("--name ot3 t3-pair --peer {PEER}")));
 assert!(read_log("host-t3").contains(&format!("auth pairing create --base-dir {}/.t3 --ttl 2m --label omabridge:{DEVICE} --json",host.display())));
 assert!(read_log("oauth").contains(&format!("\"client_label\": [\"omabridge:{DEVICE}\"]")));
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

 // The host refuses the pairing (for example, an outdated OmaBridge there):
 // reported with its code, nothing installed.
 fs::write(root.join("grants"),"[\"ssh\"]").unwrap();run();
 fs::write(root.join("refuse"),"").unwrap();
 fs::write(root.join("grants"),"[\"t3\"]").unwrap();
 let blocked=run();
 assert_eq!(blocked["peers"][0]["state"],"error");assert_eq!(blocked["peers"][0]["error"],"omabridge_t3_pair_refused");
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
