//! "Install on another device with your AI": the prompt for a new chat that
//! sets up another host and pairs it with this device through OmaBridge. The
//! prompt only names the invitation file this device created; its contents (a
//! one-use secret) are copied host to host over SSH, never into the chat.
use crate::*;
/// A host as SSH accepts it: an alias, a name or `user@host`.
fn valid_host(host:&str)->bool{
 !host.is_empty()&&host.len()<=253&&!host.starts_with('-')&&host.bytes().all(|b|b.is_ascii_alphanumeric()||b"._-@".contains(&b))&&host.matches('@').count()<=1
}
/// Only an invitation OmaBridge just wrote for this user, by its file name.
fn valid_invitation(path:&str)->bool{
 let p=Path::new(path);
 p.is_absolute()&&p.file_name().and_then(|n|n.to_str()).is_some_and(|n|n.starts_with("omabridge-invite-")&&n.ends_with(".json")&&n.bytes().all(|b|b.is_ascii_alphanumeric()||b"-_.".contains(&b)))
  &&path.bytes().all(|b|b.is_ascii_graphic()&&b!=b'\''&&b!=b'"'&&b!=b'\\'&&b!=b'$'&&b!=b'`')
}
fn pin(field:&str)->String{
 include_str!("../integrations/omabridge/release").lines().find_map(|l|l.strip_prefix(&format!("{field}="))).unwrap_or("").to_string()
}
/// Puts this device's invitation where the chat's agent runs. On this device it
/// stays where OmaBridge wrote it; on an inherited SSH environment it is piped
/// over SSH into `~/.cache/omabridge/` there (0600) and the local copy removed,
/// so the secret travels host to host and never through the chat.
pub fn handoff(input:&Value)->Result<Value>{
 let environment=text(input,"environment");let invitation=text(input,"invitation");
 if !valid_invitation(invitation){return Err("invalid_command".into())}
 if environment=="local"{return Ok(json!({"path":invitation}))}
 let name=Path::new(invitation).file_name().and_then(|n|n.to_str()).ok_or("invalid_command")?.to_string();
 let body=fs::read(invitation).map_err(|_|"omabridge_invite_unreadable")?;
 if body.len()>64*1024{return Err("omabridge_invite_invalid".into())}
 let args=crate::connections::ssh_args_for(environment)?;
 use std::{io::Write,process::{Command,Stdio}};
 let script=format!("umask 077; d=\"$HOME/.cache/omabridge\"; mkdir -p \"$d\" && cat > \"$d/{name}\" && printf '%s' \"$d/{name}\"");
 let mut child=Command::new("timeout").arg("30s").arg("ssh").args(&args).arg(script).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|_|"ssh_unavailable")?;
 if let Some(mut stdin)=child.stdin.take(){stdin.write_all(&body).map_err(|_|"ssh_unavailable")?;}
 let out=child.wait_with_output().map_err(|_|"ssh_unavailable")?;
 let path=String::from_utf8_lossy(&out.stdout).trim().to_string();
 if !out.status.success()||!valid_invitation(&path)||!path.ends_with(&format!("/.cache/omabridge/{name}")){return Err(if out.status.code()==Some(124){"ssh_timeout"}else{"ssh_unavailable"}.into())}
 let _=fs::remove_file(invitation);
 Ok(json!({"path":path}))
}
/// `{host, profile: headless|ot3|t3code, manage, invitation, device, runsOn}` to the prompt text.
pub fn prompt(input:&Value)->Result<String>{
 let host=text(input,"host").trim();if !valid_host(host){return Err("invalid_host".into())}
 let profile=text(input,"profile");if !matches!(profile,"headless"|"ot3"|"t3code"|"bridge"){return Err("invalid_command".into())}
 let invitation=text(input,"invitation");if !valid_invitation(invitation){return Err("invalid_command".into())}
 let manage=input["manage"]==true;
 let device=text(input,"device").chars().filter(|c|!c.is_control()).take(80).collect::<String>();
 let runs_on=text(input,"runsOn").chars().filter(|c|!c.is_control()).take(80).collect::<String>();
 let inviter=if device.is_empty(){"the inviting device".to_string()}else{format!("`{device}`")};
 let version=pin("version");
 let (aarch64,x86_64)=(pin("sha256_aarch64"),pin("sha256_x86_64"));
 let mut p=String::new();
 let what=match profile{
  "headless"=>"a headless server: T3 Code running as a background service, reachable only through OmaBridge",
  "bridge"=>"a device reached only through OmaBridge: OmaBridge alone, publishing its SSH server. Do not install T3 Code or any AI tool there; agents keep running elsewhere and work on it over SSH carried by OmaBridge",
  "ot3"=>"an Omarchy desktop with Omarchy T3 Code (ot3), the full client, for coding on that device too",
  _=>"a desktop with T3 Code and OmaBridge, for coding locally on that device",
 };
 p.push_str(&format!("Set up the device `{host}` as {what}, and pair it through OmaBridge with {inviter}.\n\n"));
 if !runs_on.is_empty(){p.push_str(&format!("You run on `{runs_on}`; reach `{host}` from there.\n"));}
 p.push_str("Work over SSH (`ssh "); p.push_str(host); p.push_str("`) and as that device's normal user. Ask me before any command that needs sudo, changes the firewall, or opens a port. Never print, paste or log tokens, keys, invitation contents or pairing codes; refer to files by path only. Stop and tell me if a step fails instead of working around it.\n\n");
 p.push_str("1. Check the device: `uname -m`, the glibc version (`ldd --version`; OmaBridge needs 2.39 or newer), the OS, and whether T3 Code (`t3`) and OmaBridge (`omabridge`) are already installed. Report what you find before changing anything.\n");
 match profile{
  "ot3"=>p.push_str("2. Install Omarchy T3 Code with its official installer, run as the desktop user: `curl -fsSL https://raw.githubusercontent.com/avillagran/omarchy-t3-code/main/install.sh | bash`. It needs Omarchy and T3 Code opened at least once, and it installs the pinned OmaBridge as `~/.config/omarchy/plugins/omarchy-t3-code/bin/omabridge`; link it as `~/.local/bin/omabridge`.\n"),
  "bridge"=>{
   p.push_str("2. Do not install T3 Code. Check that an SSH server listens on 127.0.0.1:22 (`ss -ltn`); if none does, ask me before installing or enabling one.\n");
   p.push_str(&format!("3. Install OmaBridge {version} as `~/.local/bin/omabridge`: download `omabridge-{version}-<arch>-linux.tar.gz` and `SHA256SUMS` from https://github.com/avillagran/omabridge/releases/download/{version}/ and accept the archive only if its SHA256 matches both `SHA256SUMS` and this pinned hash: aarch64 `{aarch64}`, x86_64 `{x86_64}`. Extract only `omabridge-{version}-<arch>-linux/omabridge`, check `omabridge --version`, and install it with mode 755.\n"));
  }
  _=>{
   p.push_str("2. Install T3 Code from its official distribution (https://github.com/pingdotgg/t3code; follow its Linux instructions, do not guess)");
   if profile=="headless"{p.push_str(", then run it as a background service with `t3 service install` and check `t3 service status`. Keep it bound to loopback; OmaBridge carries it");}
   p.push_str(".\n");
   p.push_str(&format!("3. Install OmaBridge {version} as `~/.local/bin/omabridge`: download `omabridge-{version}-<arch>-linux.tar.gz` and `SHA256SUMS` from https://github.com/avillagran/omabridge/releases/download/{version}/ and accept the archive only if its SHA256 matches both `SHA256SUMS` and this pinned hash: aarch64 `{aarch64}`, x86_64 `{x86_64}`. Extract only `omabridge-{version}-<arch>-linux/omabridge`, check `omabridge --version`, and install it with mode 755.\n"));
  }
 }
 let n=if profile=="ot3"{3}else{4};
 let bridge=profile=="bridge";
 let granted=if bridge{"[\"ssh\"]"}else{"[\"t3\"<, \"ssh\" if published>]"};
 let publish=if bridge{"\"t3\":false,\"ssh\":true"}else{"\"t3\":true,\"ssh\":<true if an SSH server listens on 127.0.0.1:22>"};
 p.push_str(&format!("{n}. Check that OmaBridge can be managed: `echo '{{\"action\":\"read\"}}' | omabridge manage` must print a JSON snapshot. If it says the command is unknown, this OmaBridge is too old for the next steps: stop and tell me.\n"));
 p.push_str(&format!("{}. On that device, create its identity and publish it, using `omabridge manage` with one JSON request on stdin each time:\n   - `{{\"action\":\"identity-create\",\"confirmed\":true}}`\n   - `{{\"action\":\"publish\",{publish},\"confirmed\":true}}`, then `read` and check `published.active`.\n",n+1));
 p.push_str(&format!("{}. Copy the one-use invitation from {inviter} (already here, at `{invitation}`) to it without printing it: `scp {invitation} {host}:~/omabridge-invite.json` and `ssh {host} chmod 600 ~/omabridge-invite.json`. Then delete `{invitation}` here. It expires within an hour.\n",n+2));
 p.push_str(&format!("{}. Pair from it on that device: `{{\"action\":\"invite-inspect\",\"path\":\"<absolute path of the copy>\"}}` first, to see the inviting device's `id`, then `{{\"action\":\"pair\",\"path\":\"<absolute path of the copy>\",\"expect\":\"<that id>\"{}{},\"confirmed\":true}}`. Delete the copy afterwards.\n",n+3,if manage{",\"allowManage\":true"}else{""},""));
 if manage{p.push_str(&format!("   I chose to let {inviter} manage that one: `allowManage` is my explicit consent, and the invitation asks for it.\n"));}
 p.push_str(&format!("{}. Tell me to approve the request in OmaBridge on {inviter} (Preferences → Connections → OmaBridge, or the bar indicator). Wait until I say it is approved, then run `{{\"action\":\"pair-retry\",\"id\":\"<its id under awaiting>\",\"confirmed\":true}}` there if it still shows as waiting.\n",n+4));
 p.push_str(&format!("{}. On that device, let {inviter} reach the services it published in step {}: `{{\"action\":\"grant-set\",\"peer\":\"<the inviting device's id from invite-inspect>\",\"services\":{granted},\"public\":[],\"confirmed\":true}}`.\n",n+5,n+1));
 p.push_str(&format!("{}. Verify: `read` on that device lists {inviter} as a peer; then I check on {inviter} that it appears under Paired devices{}.\n",n+6,if manage{" with \"You manage it\""}else{""}));
 if bridge{p.push_str(&format!("\nFrom then on, {inviter} reaches that device's SSH through OmaBridge, with no port open and no SSH configuration on the way. Tell me which user name logs in; never copy private keys.\n"));}
 p.push_str("\nAt the end, summarize what was installed and where, the device's OmaBridge name and ID, and anything I still need to do.");
 Ok(p)
}
#[cfg(test)]
mod tests{
 use super::*;
 fn base()->Value{json!({"host":"vps","profile":"headless","manage":true,"invitation":"/home/u/Downloads/omabridge-invite-20261020-120000.json","device":"laptop"})}
 #[test]
 fn rejects_unsafe_input(){
  for host in ["","-oProxyCommand=x","a b","a;b","a$(x)","u@h@x"]{let mut v=base();v["host"]=json!(host);assert!(prompt(&v).is_err(),"{host}")}
  for path in ["relative.json","/tmp/other.json","/tmp/omabridge-invite-x.json;rm","/tmp/omabridge-invite-$(x).json"]{let mut v=base();v["invitation"]=json!(path);assert!(prompt(&v).is_err(),"{path}")}
  let mut v=base();v["profile"]=json!("other");assert!(prompt(&v).is_err());
 }
 #[test]
 fn hands_off_locally_without_copying(){
  assert_eq!(handoff(&json!({"environment":"local","invitation":"/home/u/Downloads/omabridge-invite-1.json"})).unwrap()["path"],"/home/u/Downloads/omabridge-invite-1.json");
  assert!(handoff(&json!({"environment":"local","invitation":"/etc/passwd"})).is_err());
 }
 #[test]
 fn bridge_only_installs_no_t3_and_shares_ssh(){
  let mut v=base();v["profile"]=json!("bridge");let p=prompt(&v).unwrap();
  assert!(p.contains("Do not install T3 Code")&&p.contains("\"t3\":false,\"ssh\":true")&&p.contains("\"services\":[\"ssh\"]"));
  assert!(!p.contains("t3 service install")&&!p.contains("install.sh"));
 }
 #[test]
 fn names_where_it_runs_and_who_invites(){
  let mut v=base();v["runsOn"]=json!("node");let p=prompt(&v).unwrap();
  assert!(p.contains("You run on `node`")&&p.contains("with `laptop`")&&p.contains("approve the request in OmaBridge on `laptop`"));
 }
 #[test]
 fn names_the_invitation_by_path_only_and_asks_consent_for_management(){
  let p=prompt(&base()).unwrap();
  assert!(p.contains("scp /home/u/Downloads/omabridge-invite-20261020-120000.json vps:"));
  assert!(p.contains("\"allowManage\":true")&&p.contains("explicit consent"));
  assert!(p.contains("t3 service install")&&p.contains("published in step 5")&&p.contains("\"pair-retry\""));
  let mut v=base();v["manage"]=json!(false);v["profile"]=json!("ot3");let p=prompt(&v).unwrap();
  assert!(!p.contains("allowManage")&&p.contains("install.sh"));
 }
}
