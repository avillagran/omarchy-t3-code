//! Confirmed local browser import. Source cookies stay in Rust/private Qt pipes;
//! the UI receives only a captured view, counts, metadata and outcome tokens.
use crate::{
    browser_cleanup::{Runtime, Scope},
    browser_profiles, connection_preferences as settings, json, text, Environment, Result,
};
use serde_json::Value;
use std::{
    collections::HashMap,
    path::Path,
    sync::{Mutex, OnceLock},
};
static VIEWS: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
fn views() -> &'static Mutex<HashMap<String, Value>> {
    VIEWS.get_or_init(Default::default)
}
fn store(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_owned())
        .to_string_lossy()
        .into_owned()
}
fn target_path(path: &Path, environment: &str, profile: &str) -> Result<std::path::PathBuf> {
    Ok(path
        .parent()
        .ok_or("sessionUnavailable")?
        .join("ot3-browser-profiles")
        .join(
            text(
                &crate::browser_runtime::partition(environment, profile)?,
                "partition",
            )
            .trim_start_matches("persist:"),
        ))
}
fn profile_limit() -> usize {
    crate::browser_preferences::contract()["profileMaxCount"]
        .as_u64()
        .expect("pinned native profile limit") as usize
}
pub fn read_at(
    environments: &[Environment],
    input: &Value,
    home: &Path,
    path: &Path,
    runtime: &dyn Runtime,
) -> Result<Value> {
    if !runtime.can_import() {
        return Err("sessionUnavailable".into());
    }
    let e = environments
        .iter()
        .find(|e| e.id == text(input, "environment"))
        .ok_or("unknown_environment")?;
    let (_, session) = browser_profiles::inspect(e)?;
    let client = settings::read_client_preferences_at(path)?;
    let sources = crate::browser_sources::list_at(home)?;
    let profiles = browser_profiles::native_profiles(&client);
    let id = uuid::Uuid::new_v4().to_string();
    let cached = json!({"environment":e.id,"originKey":session["originKey"],"clientStore":store(path),"profiles":profiles,"sources":sources});
    let mut all = views().lock().map_err(|_| "local_error")?;
    if all.len() >= 128 {
        all.clear()
    }
    all.insert(id.clone(), cached);
    Ok(
        json!({"viewId":id,"environment":e.id,"environmentName":e.name,"profiles":profiles,"sources":sources,"canCreateProfile":client["browserProfiles"].as_array().is_some_and(|p|p.len()<profile_limit())}),
    )
}
fn input_target(input: &Value) -> Result<(&str, &str)> {
    let kind = text(&input["target"], "kind");
    let id = text(&input["target"], "profileId");
    if !matches!(kind, "new" | "existing") || id == "incognito" {
        return Err("readFailed".into());
    }
    crate::browser_runtime::partition(text(input, "environment"), id)?;
    if kind == "new"
        && id
            .strip_prefix("profile-")
            .and_then(|s| uuid::Uuid::parse_str(s).ok())
            .is_none()
    {
        return Err("readFailed".into());
    }
    Ok((kind, id))
}
fn blocked(
    input: &Value,
    reason: &str,
    state: &str,
    imported: usize,
    skipped: usize,
    domains: &[String],
    rolled_back: bool,
) -> Value {
    json!({"kind":"blocked","state":state,"environment":input["environment"],"profileId":input["target"]["profileId"],"reason":reason,"imported":imported,"skipped":skipped,"skippedDomains":domains.iter().take(20).collect::<Vec<_>>(),"rolledBack":rolled_back})
}
pub fn apply_at(
    environments: &[Environment],
    input: &Value,
    home: &Path,
    path: &Path,
    helper: &Path,
    runtime: &dyn Runtime,
) -> Result<Value> {
    let _lock = browser_profiles::ACTION
        .try_lock()
        .map_err(|_| "browser_profiles_busy")?;
    if input["confirmed"] != true {
        return Err("confirmation_required".into());
    }
    if !runtime.can_import() {
        return Err("sessionUnavailable".into());
    }
    let (kind, profile) = input_target(input)?;
    let cached = views()
        .lock()
        .map_err(|_| "local_error")?
        .get(text(input, "viewId"))
        .cloned()
        .ok_or("browser_import_view_expired")?;
    if cached["clientStore"] != store(path) || cached["environment"] != input["environment"] {
        return Err("browser_profile_scope_changed".into());
    }
    let e = environments
        .iter()
        .find(|e| e.id == text(input, "environment"))
        .ok_or("unknown_environment")?;
    let (_, session) = browser_profiles::inspect(e)?;
    if session["originKey"] != cached["originKey"] {
        return Err("browser_profile_scope_changed".into());
    }
    let before = settings::read_client_preferences_at(path)?;
    let profiles = browser_profiles::native_profiles(&before);
    if kind == "existing" {
        let current = profiles
            .iter()
            .find(|p| p["id"] == profile)
            .ok_or("readFailed")?;
        if !cached["profiles"]
            .as_array()
            .is_some_and(|p| p.contains(current))
        {
            return Err("browser_profile_changed_reload".into());
        }
    } else {
        let target_exists = match target_path(path, &e.id, profile)?.symlink_metadata() {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            _ => true,
        };
        if profiles.iter().any(|p| p["id"] == profile) || target_exists {
            return Err("profileNotSaved".into());
        }
        if before["browserProfiles"]
            .as_array()
            .is_none_or(|p| p.len() >= profile_limit())
        {
            return Err("profileLimitReached".into());
        }
    }
    let source = cached["sources"]
        .as_array()
        .and_then(|s| s.iter().find(|s| s["id"] == input["sourceId"]))
        .ok_or("unknownSource")?;
    if let Some(reason) = source["unavailable"].as_str() {
        return Err(reason.into());
    }
    if !source["profiles"].as_array().is_some_and(|ps| {
        ps.iter()
            .any(|p| p["directory"] == input["sourceProfileDirectory"])
    }) {
        return Err("unknownSourceProfile".into());
    }
    if views()
        .lock()
        .map_err(|_| "local_error")?
        .remove(text(input, "viewId"))
        .is_none()
    {
        return Err("browser_import_view_expired".into());
    }
    let mut lease = runtime.begin(
        profile,
        Path::new(text(&cached, "clientStore")),
        &[Scope {
            environment: e.id.clone(),
            origin_key: text(&session, "originKey").into(),
        }],
    )?;
    let read = match crate::browser_cookies::read_selected(
        home,
        text(input, "sourceId"),
        text(input, "sourceProfileDirectory"),
        true,
        helper,
    ) {
        Ok(read) => read,
        Err(reason) => return Ok(blocked(input, &reason, "failed", 0, 0, &[], false)),
    };
    let mut imported = 0usize;
    let mut skipped = read.undecryptable;
    let mut domains = read.undecryptable_hosts;
    for cookie in read.cookies {
        let host = crate::Url::parse(&cookie.url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_owned))
            .unwrap_or_default();
        // Native BrowserImportResult classifies expired cookies as skipped.
        // They cannot establish a session and must not clear an existing target cookie.
        if cookie
            .expiration_date
            .is_some_and(|expiry| expiry <= chrono::Utc::now().timestamp_millis() as f64 / 1000.)
        {
            skipped += 1;
            if !host.is_empty() && !domains.contains(&host) {
                domains.push(host)
            }
            continue;
        }
        match lease.write_cookie(&e.id, cookie) {
            Ok(true) => imported += 1,
            Ok(false) => {
                skipped += 1;
                if !host.is_empty() && !domains.contains(&host) {
                    domains.push(host)
                }
            }
            Err(error) => {
                let before_write = imported == 0
                    && matches!(
                        error.as_str(),
                        "browser_runtime_unavailable"
                            | "browser_guest_limit"
                            | "browser_profile_scope_changed"
                            | "invalid_browser_profile"
                            | "invalid_browser_cookie"
                    );
                let rollback = kind == "new" && lease.clear(&e.id).is_ok();
                return Ok(blocked(
                    input,
                    if before_write {
                        "sessionUnavailable"
                    } else {
                        "browser_cookie_import_unknown"
                    },
                    if before_write { "failed" } else { "uncertain" },
                    imported,
                    skipped,
                    &domains,
                    rollback,
                ));
            }
        }
    }
    if lease.flush_import(&e.id).is_err() {
        let rollback = kind == "new" && lease.clear(&e.id).is_ok();
        return Ok(blocked(
            input,
            "browser_cookie_import_unknown",
            "uncertain",
            imported,
            skipped,
            &domains,
            rollback,
        ));
    }
    let registration = (|| -> Result<(Value, String)> {
        if cached["clientStore"] != store(path) {
            return Err("profileNotSaved".into());
        }
        let current = settings::read_client_preferences_at(path)?;
        let all = browser_profiles::native_profiles(&current);
        if kind == "existing" {
            let selected = all
                .iter()
                .find(|p| p["id"] == profile)
                .ok_or("profileNotSaved")?;
            return Ok((current.clone(), text(selected, "name").into()));
        }
        if imported == 0 {
            return Ok((current, text(source, "name").into()));
        }
        if let Some(selected) = all.iter().find(|p| p["id"] == profile) {
            return Ok((current.clone(), text(selected, "name").into()));
        }
        let mut users = current["browserProfiles"]
            .as_array()
            .cloned()
            .ok_or("profileNotSaved")?;
        if users.len() >= profile_limit() {
            return Err("profileLimitReached".into());
        }
        let mut name = text(source, "name").to_owned();
        let mut suffix = 2;
        while all.iter().any(|p| p["name"] == name) {
            name = format!("{} {suffix}", text(source, "name"));
            suffix += 1
        }
        users.push(json!({"id":profile,"name":name,"kind":"persistent"}));
        let data = settings::update_client_preferences_at(
            path,
            text(&current, "revision"),
            &json!({"browserProfiles":users}),
        )
        .map_err(|_| "profileNotSaved")?;
        Ok((data, name))
    })();
    match registration {
        Ok((data, name)) => Ok(
            json!({"kind":"imported","state":"complete","environment":e.id,"profileId":profile,"imported":imported,"skipped":skipped,"skippedDomains":domains.iter().take(20).collect::<Vec<_>>(),"targetName":name,"data":data}),
        ),
        Err(reason) => {
            let rollback = kind == "new" && lease.clear(&e.id).is_ok();
            Ok(blocked(
                input,
                if reason == "profileLimitReached" {
                    "profileLimitReached"
                } else {
                    "profileNotSaved"
                },
                if kind == "new" && rollback {
                    "failed"
                } else {
                    "uncertain"
                },
                imported,
                skipped,
                &domains,
                rollback,
            ))
        }
    }
}
pub fn execute(input: &Value, runtime: &dyn Runtime) -> Result<Value> {
    let path = crate::t3_home().join("userdata/client-settings.json");
    let environments = crate::environments()?;
    match text(input, "op") {
        "browser-import-read" => read_at(&environments, input, &crate::home(), &path, runtime),
        "browser-import-action" => {
            let helper = std::env::current_exe()
                .map_err(|_| "sessionUnavailable")?
                .parent()
                .ok_or("sessionUnavailable")?
                .join("t3-browser-secret");
            apply_at(
                &environments,
                input,
                &crate::home(),
                &path,
                &helper,
                runtime,
            )
        }
        _ => Err("invalid_browser_import_action".into()),
    }
}
