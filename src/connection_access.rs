//! The same HTTP access-management contract used by T3's ConnectionsSettings.
//! Credentials are returned only by explicit creation, never by refresh/list.
use crate::*;
use base64::Engine;
use std::os::unix::fs::{OpenOptionsExt,MetadataExt};
use std::io::Write;
use zeroize::Zeroizing;

const SCOPES: &[&str] = &["orchestration:read","orchestration:operate","settings:write","providers:manage","environment:maintain","preview:operate","diagnostics:read","terminal:read","terminal:operate","source-control:write","filesystem:read","filesystem:write","access:read","access:write","relay:read","relay:write"];
fn local_credential(path:&std::path::Path)->Result<Zeroizing<String>>{
    if let Ok(meta)=fs::symlink_metadata(&path){
        if meta.is_file()&&meta.uid()==unsafe{libc::geteuid()}&&meta.mode()&0o077==0 {
            if let Ok(token)=fs::read_to_string(&path){
                let token=Zeroizing::new(token);let parts=token.trim().split('.').collect::<Vec<_>>();
                let payload=match parts.len(){2=>parts[0],3=>parts[1],_=>""};
                if let Ok(bytes)=base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload){
                    if let Ok(claims)=serde_json::from_slice::<Value>(&bytes){
                        let exp=claims["exp"].as_i64().unwrap_or(0);let expiry=if exp<10_000_000_000{exp*1000}else{exp};
                        if expiry>Utc::now().timestamp_millis()+3_600_000&&SCOPES.iter().all(|s|claims["scopes"].as_array().is_some_and(|a|a.contains(&json!(s)))){return Ok(token)}
                    }
                }
            }
        }
    }
    let cli=home().join(".t3/bin/t3");let mut cmd=std::process::Command::new("timeout");
    cmd.arg("12s").arg(if cli.exists(){cli.into_os_string()}else{"t3".into()}).args(["auth","session","issue","--base-dir"]).arg(t3_home());
    for scope in SCOPES{cmd.args(["--scope",scope]);}
    let output=cmd.args(["--ttl","30d","--label","ot3 connections","--token-only"]).stdin(std::process::Stdio::null()).output().map_err(|_|"t3_auth_issue_failed")?;
    if !output.status.success(){return Err(format!("t3_auth_issue_failed:{}",output.status.code().unwrap_or(-1)))}
    let bytes=Zeroizing::new(output.stdout);let token=std::str::from_utf8(&bytes).map_err(|_|"t3_auth_invalid_response")?.trim();
    if token.len()<16||token.contains(char::is_whitespace){return Err("t3_auth_invalid_response".into())}
    fs::create_dir_all(path.parent().unwrap()).map_err(|_|"local_error")?;
    let tmp=path.with_extension(format!("{}.tmp",uuid::Uuid::new_v4()));
    let result:Result<()>=(||{let mut file=fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp).map_err(|_|"local_error")?;file.write_all(token.as_bytes()).and_then(|_|file.sync_all()).map_err(|_|"local_error")?;fs::rename(&tmp,&path).map_err(|_|"local_error")?;Ok(())})();
    if result.is_err(){let _=fs::remove_file(&tmp);}result?;
    Ok(Zeroizing::new(token.to_string()))
}
fn connection(e: &Environment) -> Result<protocol::Connection> {
    if let Some(error)=&e.connection_error {return Err(error.clone())}
    // The local server is administered with a session the T3 CLI issues on the spot, not with
    // the stored environment token, so it is the one connection with its own credential.
    if e.id=="local"{return t3_connection(&e.base_url,protocol::Credential::bearer(local_credential(&e.token_file)?.trim())?)}
    e.connection(true)
}
fn request(c:&protocol::Connection,method:&str,path:&str,body:Option<&Value>)->Result<Value>{c.request_json(method,path,body,false)}
fn scopes(session:&Value)->Vec<String>{session.get("permissions").or_else(||session.get("scopes")).and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect()}
fn need(granted:&[String],scope:&str)->Result<()>{if granted.iter().any(|s|s==scope){Ok(())}else{Err(format!("scope_required:{scope}"))}}
/// Pure payload validation: remote credentials can delegate only their own grant.
pub fn pairing_payload(input:&Value,granted:&[String])->Result<Value>{
    need(granted,"access:write")?;
    let label=text(input,"label").trim();if label.chars().count()>256||label.chars().any(char::is_control){return Err("invalid_connection_label".into())}
    let selected=input["scopes"].as_array().ok_or("invalid_scope")?;
    if selected.is_empty()||selected.len()>SCOPES.len(){return Err("invalid_scope".into())}
    let mut seen=std::collections::HashSet::new();
    for s in selected{let s=s.as_str().ok_or("invalid_scope")?;if !SCOPES.contains(&s)||!seen.insert(s){return Err("invalid_scope".into())}need(granted,s)?;}
    if seen.contains("terminal:operate")&&!seen.contains("terminal:read"){return Err("invalid_scope".into())}
    let mut body=json!({"scopes":selected});if !label.is_empty(){body["label"]=json!(label)}Ok(body)
}
fn snapshot(e:&Environment,c:&protocol::Connection,session:Value)->Value{
    let granted=scopes(&session);let mut result=json!({"session":session,"clients":null,"pairingLinks":null,"accessError":"","descriptor":null,"descriptorError":"","connect":null,"connectError":"","endpoint":e.base_url,"environmentIcon":null,"iconError":""});
    match request(c,"GET","/.well-known/t3/environment",None){Ok(v)=>result["descriptor"]=v,Err(err)=>result["descriptorError"]=json!(err)}
    if result["descriptor"]["capabilities"]["environmentIcon"]==true {
        match crate::write::rpc(e,"server.getConfig",json!({})){Ok(config)=>result["environmentIcon"]=config["settings"]["environmentIcon"].clone(),Err(err)=>result["iconError"]=json!(err)}
    }
    if granted.iter().any(|s|s=="access:read"){
        match request(c,"GET","/api/auth/clients",None){Ok(v)=>result["clients"]=v,Err(err)=>result["accessError"]=json!(err)}
        match request(c,"GET","/api/auth/pairing-links",None){Ok(v)=>result["pairingLinks"]=v,Err(err)=>result["accessError"]=json!(err)}
    }
    if granted.iter().any(|s|s=="relay:read"){
        match request(c,"GET","/api/connect/link-state",None){Ok(v)=>result["connect"]=v,Err(err)=>result["connectError"]=json!(err)}
    }
    result
}
fn pairing_artifact(mut result:Value,endpoint:&str)->Value{
    let Ok(mut url)=crate::Url::parse(endpoint)else{return result};
    if !matches!(url.scheme(),"http"|"https")||url.host_str().is_none()||!url.username().is_empty()||url.password().is_some(){return result}
    let token=text(&result,"credential").to_string();if token.is_empty(){return result}
    url.set_path("/pair");url.set_query(None);let fragment=crate::Url::parse_with_params("https://placeholder.invalid/",&[("token",token)]).ok().and_then(|u|u.query().map(str::to_owned));url.set_fragment(fragment.as_deref());
    result["pairingUrl"]=json!(url.as_str());
    let loopback=url.host_str().is_some_and(|h|h=="localhost"||h.trim_matches(['[',']']).parse::<std::net::IpAddr>().is_ok_and(|ip|ip.is_loopback()||ip.is_unspecified()));
    result["qrShareable"]=json!(!loopback);
    if !loopback {
        use std::process::{Command,Stdio};
        if let Ok(mut child)=Command::new("timeout").args(["3s","qrencode","-t","SVG","-o","-","-m","2"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn(){
            if let Some(mut stdin)=child.stdin.take(){let _=stdin.write_all(url.as_str().as_bytes());}
            if let Ok(output)=child.wait_with_output(){if output.status.success()&&output.stdout.len()<512*1024{result["qrImage"]=json!(format!("data:image/svg+xml;base64,{}",base64::engine::general_purpose::STANDARD.encode(output.stdout)))}}
        }
    }
    result
}
pub fn execute(e:&Environment,input:&Value)->Result<Value>{
    let c=connection(e)?;let session=request(&c,"GET","/api/auth/session",None)?;
    if session["authenticated"]!=true{return Err("unauthorized".into())}
    let granted=scopes(&session);let action=text(input,"action");
    let result=match action{
        "read"=>return Ok(snapshot(e,&c,session)),
        "create-link"=>{
            let endpoint=input["endpoint"].as_str().unwrap_or(&e.base_url);
            let url=crate::Url::parse(endpoint).map_err(|_|"invalid_endpoint")?;
            if !matches!(url.scheme(),"http"|"https")||url.host_str().is_none()||!url.username().is_empty()||url.password().is_some(){return Err("invalid_endpoint".into())}
            pairing_artifact(request(&c,"POST","/api/auth/pairing-token",Some(&pairing_payload(input,&granted)?))?,endpoint)
        },
        "revoke-link"|"revoke-client"|"revoke-others"=>{
            need(&granted,"access:write")?;if input["confirmed"]!=true{return Err("confirmation_required".into())}
            let (path,field)=match action{"revoke-link"=>("/api/auth/pairing-links/revoke","id"),"revoke-client"=>("/api/auth/clients/revoke","sessionId"),_=>("/api/auth/clients/revoke-others","")};
            let mut body=json!({});if !field.is_empty(){let id=text(input,field);if id.is_empty()||id.len()>512||id.chars().any(char::is_control){return Err("invalid_id".into())}body[field]=json!(id)}
            request(&c,"POST",path,Some(&body))?
        },
        "environment-icon"=>{
            need(&granted,"settings:write")?;
            let kind=&input["icon"];if !kind.is_null()&&!matches!(kind.as_str(),Some("server"|"cloud"|"linux"|"desktop"|"laptop"|"mac-mini"|"mac-studio")){return Err("invalid_command".into())}
            let descriptor=request(&c,"GET","/.well-known/t3/environment",None)?;
            if descriptor["capabilities"]["environmentIcon"]!=true{return Err("unsupported_action".into())}
            let response=crate::write::rpc(e,"server.updateSettings",json!({"patch":{"environmentIcon":kind}}))?;
            json!({"environmentIcon":response["environmentIcon"]})
        },
        "connect-preferences"=>{
            need(&granted,"relay:write")?;let publish=input["publishAgentActivity"].as_bool().ok_or("invalid_command")?;let hold=input["holdWebhooksWhileOffline"].as_bool().ok_or("invalid_command")?;
            request(&c,"POST","/api/connect/preferences",Some(&json!({"publishAgentActivity":publish,"holdWebhooksWhileOffline":hold})))?
        },
        _=>return Err("invalid_action".into())
    };
    // Return the mutation receipt independently of later refresh errors. In particular
    // a newly minted credential must not be lost or reissued due to a failed list.
    Ok(json!({"mutation":result}))
}
