//! Desktop adapter for T3's canonical encrypted catalog. Never returns credentials.
use super::*;
use aes::cipher::BlockEncryptMut;
use std::os::unix::fs::OpenOptionsExt;
fn catalog_path() -> PathBuf {
    t3_home().join("userdata/connection-catalog.json")
}
fn revision() -> Result<String> {
    let bytes = match fs::read(catalog_path()) {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
        Err(_) => return Err("catalog_read_failed".into()),
    };
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn password(prefix: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    if prefix == b"v10" {
        return Ok(Zeroizing::new(b"peanuts".to_vec()));
    }
    let output = Command::new("timeout")
        .args(["4s", "secret-tool", "lookup", "application", "t3code"])
        .output()
        .map_err(|_| "catalog_locked")?;
    if !output.status.success() {
        return Err("catalog_locked".into());
    }
    let mut value = Zeroizing::new(output.stdout);
    while value.last() == Some(&b'\n') || value.last() == Some(&b'\r') {
        value.pop();
    }
    if value.is_empty() {
        return Err("catalog_locked".into());
    }
    Ok(value)
}
fn encrypt(catalog: &Value, prefix: &[u8], password: &[u8]) -> Result<Value> {
    let plain = Zeroizing::new(serde_json::to_vec(catalog).map_err(|_| "invalid_t3_catalog")?);
    let mut key = Zeroizing::new([0u8; 16]);
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(password, b"saltysalt", 1, key.as_mut());
    let mut bytes = prefix.to_vec();
    bytes.extend(
        cbc::Encryptor::<aes::Aes128>::new_from_slices(key.as_ref(), &[b' '; 16])
            .map_err(|_| "catalog_locked")?
            .encrypt_padded_vec_mut::<Pkcs7>(&plain),
    );
    Ok(
        json!({"version":1,"encryptedCatalog":base64::engine::general_purpose::STANDARD.encode(bytes)}),
    )
}
fn load() -> Result<Value> {
    if catalog_path().exists() {
        read_catalog()
    } else {
        Ok(protocol::connection_catalog::empty())
    }
}
fn snapshot(c: &Value) -> Result<Value> {
    let enabled = read_json(&t3_home().join("userdata/desktop-settings.json"))
        .ok()
        .is_none_or(|d| d["localEnvironmentEnabled"] != false);
    Ok(
        json!({"revision":revision()?,"localEnabled":enabled,"environments":protocol::connection_catalog::view(c)}),
    )
}
pub(crate) fn native_primary()->Option<Value>{
    let env=fs::read_to_string(t3_home().join("userdata/environment-id")).ok()?;
    let env=env.trim();if env.is_empty(){return None}
    let runtime=read_json(&t3_home().join("userdata/server-runtime.json")).ok()?;
    let base=text(&runtime,"origin");validate_base(base).ok()?;
    let mut ws=reqwest::Url::parse(base).ok()?;let scheme=if ws.scheme()=="https"{"wss"}else{"ws"};ws.set_scheme(scheme).ok()?;
    Some(json!({"_tag":"PrimaryConnectionTarget","environmentId":env,"httpBaseUrl":base,"wsBaseUrl":ws.as_str().trim_end_matches('/')}))
}
fn preferences_snapshot(c:&Value)->Result<Value>{
    let view=snapshot(c)?;let primary=native_primary();let mut ids=view["environments"].as_array().into_iter().flatten().filter(|v|v["enabled"]==true).filter_map(|v|v["id"].as_str().map(str::to_owned)).collect::<Vec<_>>();
    if view["localEnabled"]==true{if let Some(p)=&primary{ids.push(text(p,"environmentId").into())}}
    Ok(json!({"client":crate::connection_preferences::read_client_preferences()?,"catalogRevision":view["revision"],"primaryEnvironmentId":primary.as_ref().map(|p|p["environmentId"].clone()),"github":crate::connection_preferences::github_preferences(c,primary.as_ref(),&ids)}))
}
fn persist(c: &Value, expected: &str) -> Result<()> {
    if revision()? != expected {
        return Err("catalog_changed_reload".into());
    }
    let prefix = if catalog_path().exists() {
        let d = read_json(&catalog_path())?;
        let b = base64::engine::general_purpose::STANDARD
            .decode(text(&d, "encryptedCatalog"))
            .map_err(|_| "invalid_t3_catalog")?;
        b.get(..3).ok_or("invalid_t3_catalog")?.to_vec()
    } else {
        if password(b"v11").is_ok() {
            b"v11".to_vec()
        } else {
            b"v10".to_vec()
        }
    };
    if !matches!(prefix.as_slice(), b"v10" | b"v11") {
        return Err("unsupported_catalog_encryption".into());
    }
    let encrypted = encrypt(c, &prefix, &password(&prefix)?)?;
    if revision()? != expected {
        return Err("catalog_changed_reload".into());
    }
    // The backup contains the original ciphertext, never decrypted credentials.
    if catalog_path().exists() {
        let old = read_json(&catalog_path())?;
        save_json(&catalog_path().with_extension("ot3-backup.json"), &old)?;
    }
    save_json(&catalog_path(), &encrypted)
}
fn discover_ssh(target: &Value) -> Result<String> {
    let args = ssh_args(target)?;
    let output = Command::new("timeout")
        .args(["12s", "ssh"])
        .args(args)
        .arg("cat \"$HOME/.t3/userdata/environment-id\"")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|_| "ssh_unavailable")?;
    if !output.status.success() {
        return Err("ssh_unavailable".into());
    }
    let id = String::from_utf8(output.stdout)
        .map_err(|_| "invalid_t3_environment_id")?
        .trim()
        .to_string();
    uuid::Uuid::parse_str(&id).map_err(|_| "invalid_t3_environment_id")?;
    Ok(id)
}
fn prepare(c: &Value, i: &Value) -> Result<Value> {
    let kind = text(i, "kind");
    let expected = text(i, "environmentId");
    let name = text(i, "label").trim();
    if name.len() > 256 || name.chars().any(char::is_control) {
        return Err("invalid_connection_label".into());
    }
    if kind == "ssh" {
        let target = &i["ssh"];
        let env = discover_ssh(target)?;
        if !expected.is_empty() && expected != env {
            return Err("t3_environment_mismatch".into());
        }
        let id = if text(i, "connectionId").is_empty() {
            format!(
                "ssh:{env}:{}",
                json!([
                    target["alias"],
                    target["hostname"],
                    target["username"],
                    target["port"]
                ])
            )
        } else {
            text(i, "connectionId").to_string()
        };
        let label = if name.is_empty() {
            text(target, "alias")
        } else {
            name
        };
        let profile = json!({"_tag":"SshConnectionProfile","connectionId":id,"environmentId":env,"label":label,"target":target});
        let mut check = remote_base(&json!({"environmentId":env,"label":label}));
        let _tunnel = connect_ssh(&profile, &mut check)?;
        protocol::connection_catalog::register(
            c,
            json!({"_tag":"SshConnectionTarget","environmentId":env,"connectionId":id,"label":label}),
            profile,
            None,
        )
    } else if kind == "bearer" {
        let (base, code) =
            protocol::connection_catalog::pairing_target(text(i, "url"), text(i, "pairingCode"))?;
        validate_base(&base)?;
        let http = Client::new()?.http;
        let descriptor: Value = http
            .get(format!(
                "{}/.well-known/t3/environment",
                base.trim_end_matches('/')
            ))
            .send()
            .map_err(|_| "unreachable")?
            .error_for_status()
            .map_err(|_| "unreachable")?
            .json()
            .map_err(|_| "malformed")?;
        let env = text(&descriptor, "environmentId");
        uuid::Uuid::parse_str(env).map_err(|_| "invalid_t3_environment_id")?;
        if !expected.is_empty() && expected != env {
            return Err("t3_environment_mismatch".into());
        }
        let id = if text(i, "connectionId").is_empty() {
            format!(
                "bearer:{env}:{}",
                reqwest::Url::parse(&base)
                    .unwrap()
                    .origin()
                    .ascii_serialization()
            )
        } else {
            text(i, "connectionId").to_string()
        };
        let token = if code.is_empty() {
            c["credentials"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|v| v["connectionId"] == id)
                .and_then(|v| v["credential"]["token"].as_str())
                .ok_or("pairing_code_required")?
                .to_string()
        } else {
            let access: Value = http
                .post(format!("{}/oauth/token", base.trim_end_matches('/')))
                .form(&[
                    (
                        "grant_type",
                        "urn:ietf:params:oauth:grant-type:token-exchange",
                    ),
                    ("subject_token", code.as_str()),
                    (
                        "subject_token_type",
                        "urn:t3:params:oauth:token-type:environment-bootstrap",
                    ),
                    (
                        "requested_token_type",
                        "urn:ietf:params:oauth:token-type:access_token",
                    ),
                    ("client_label", "ot3"),
                    ("client_device_type", "desktop"),
                    ("client_os", "linux"),
                ])
                .send()
                .map_err(|_| "unreachable")?
                .error_for_status()
                .map_err(|_| "pairing_rejected")?
                .json()
                .map_err(|_| "malformed")?;
            if access["token_type"] != "Bearer" {
                return Err("unsupported_pairing_token".into());
            }
            text(&access, "access_token").to_string()
        };
        // Not an Environment yet: the pairing is checked with the token just exchanged.
        let conn = crate::t3_connection(&base, protocol::Credential::bearer(&token)?)?;
        conn.request_json("GET", "/api/auth/session", None, false)?;
        let label = if name.is_empty() {
            text(&descriptor, "label")
        } else {
            name
        };
        let mut ws = reqwest::Url::parse(&base).unwrap();
        let scheme = if ws.scheme() == "https" { "wss" } else { "ws" };
        ws.set_scheme(scheme).map_err(|_| "invalid_endpoint")?;
        protocol::connection_catalog::register(
            c,
            json!({"_tag":"BearerConnectionTarget","environmentId":env,"connectionId":id,"label":label}),
            json!({"_tag":"BearerConnectionProfile","environmentId":env,"connectionId":id,"label":label,"httpBaseUrl":base.trim_end_matches('/'),"wsBaseUrl":ws.as_str().trim_end_matches('/')}),
            Some(json!(token)),
        )
    } else {
        Err("unsupported_connection_type".into())
    }
}
pub fn execute(i: &Value) -> Result<Value> {
    let dir = t3_home().join("userdata");
    fs::create_dir_all(&dir).map_err(|_| "catalog_write_failed")?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(dir.join(".ot3-connection-catalog.lock"))
        .map_err(|_| "catalog_write_failed")?;
    if unsafe { libc::flock(std::os::fd::AsRawFd::as_raw_fd(&lock), libc::LOCK_EX) } != 0 {
        return Err("catalog_write_failed".into());
    }
    let c = load()?;
    if i["action"] == "read" {
        return snapshot(&c);
    }
    if i["action"]=="preferences-read"{return preferences_snapshot(&c)}
    if i["action"]=="client-update"{
        if let Some(id)=i["patch"]["environmentId"].as_str(){
            if !preferences_snapshot(&c)?["github"]["environments"].as_array().is_some_and(|rows|rows.iter().any(|r|r["environmentId"]==id)){return Err("unknown_environment".into())}
        }
        crate::connection_preferences::update_client_preferences(text(i,"clientRevision"),&i["patch"])?;
        return preferences_snapshot(&c)
    }
    if i["action"]=="github-update"{
        let expected=text(i,"revision");if expected!=self::revision()?{return Err("catalog_changed_reload".into())}
        let primary=native_primary();let env=text(i,"environmentId");
        if !preferences_snapshot(&c)?["github"]["environments"].as_array().is_some_and(|rows|rows.iter().any(|r|r["environmentId"]==env)){return Err("unknown_environment".into())}
        let key=crate::connection_preferences::github_connection_key(&c,env,primary.as_ref()).ok_or("untrusted_connection_endpoint")?;
        let next=crate::connection_preferences::set_github_permission(&c,primary.as_ref(),env,&key,text(i,"permission"))?;
        persist(&next,expected)?;return preferences_snapshot(&next)
    }

    if i["action"] == "restart-server" {
        let id = text(i, "environmentId");
        if i["confirmed"] != true { return Err("confirmation_required".into()); }
        if !c["targets"].as_array().into_iter().flatten().any(|t| t["environmentId"] == id && t["_tag"] == "SshConnectionTarget") {
            return Err("ssh_environment_required".into());
        }
        restart_remote(id)?;
        return snapshot(&c);
    }
    if i["action"] == "reconnect" {
        let id = text(i, "environmentId");
        if !c["targets"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|t| t["environmentId"] == id)
        {
            return Err("unknown_environment".into());
        }
        // Applied when the next rebuild starts, so this never waits for one in progress.
        request_reconnect(id);
        return snapshot(&c);
    }
    let revision = text(i, "revision");
    if revision != self::revision()? {
        return Err("catalog_changed_reload".into());
    }
    if i["action"] == "local-enabled" {
        let enabled = i["enabled"].as_bool().ok_or("invalid_command")?;
        let path = dir.join("desktop-settings.json");
        let mut d = if path.exists() {
            read_json(&path)?
        } else {
            json!({})
        };
        if !d.is_object() {
            return Err("invalid_t3_catalog".into());
        }
        d["localEnvironmentEnabled"] = json!(enabled);
        save_json(&path, &d)?;
        refresh_connections();
        return snapshot(&c);
    }
    let next = if i["action"] == "save-route" {
        prepare(&c, i)?
    } else {
        protocol::connection_catalog::mutate(&c, i)?
    };
    persist(&next, revision)?;
    refresh_connections();
    snapshot(&next)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encrypted_roundtrip_preserves_native_prefix() {
        for prefix in [b"v10", b"v11"] {
            let c = protocol::connection_catalog::empty();
            let d = encrypt(&c, prefix, b"password").unwrap();
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(text(&d, "encryptedCatalog"))
                .unwrap();
            assert!(bytes.starts_with(prefix));
            assert_eq!(decrypt_catalog(&bytes, b"password").unwrap(), c);
            assert!(!d.to_string().contains("targets"));
        }
    }
}
