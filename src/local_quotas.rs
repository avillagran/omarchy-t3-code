use base64::Engine;
use chrono::Utc;
use omarchy_t3_client::*;
use serde_json::{json, Value};
use std::{io::Read, time::Duration};

// Supplemental accounts already available to local Hermes/CLI installations.
// Enumerate every credential, not just the first entry in a provider pool.
fn tokens_in(a: &Value, c: &Value, provider: &str) -> Vec<String> {
    let mut result = Vec::new();
    for entry in a["credential_pool"][provider]
        .as_array()
        .into_iter()
        .flatten()
    {
        for key in ["access_token", "api_key", "key"] {
            if let Some(t) = entry[key].as_str().filter(|s| !s.is_empty()) {
                result.push(t.to_owned());
                break;
            }
        }
    }
    for v in [
        &a["providers"][provider]["tokens"]["access_token"],
        &a["providers"][provider]["access_token"],
        &a["providers"][provider]["api_key"],
        &a["providers"][provider]["key"],
    ] {
        if let Some(t) = v.as_str().filter(|s| !s.is_empty()) {
            result.push(t.to_owned());
        }
    }
    if provider == "openai-codex" {
        for v in [&c["tokens"]["access_token"], &c["access_token"]] {
            if let Some(t) = v.as_str().filter(|s| !s.is_empty()) {
                result.push(t.to_owned());
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    result.retain(|t| seen.insert(t.clone()));
    result
}
fn tokens(provider: &str) -> Vec<String> {
    tokens_in(
        &read_json(&home().join(".hermes/auth.json")).unwrap_or(Value::Null),
        &read_json(&home().join(".codex/auth.json")).unwrap_or(Value::Null),
        provider,
    )
}
fn claims(token: &str) -> Value {
    token
        .split('.')
        .nth(1)
        .and_then(|p| {
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(p.trim_end_matches('='))
                .ok()
        })
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Value::Null)
}
fn fingerprint(provider: &str, token: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(format!("{provider}\0{token}")))
}
fn number(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
}
fn window(used: f64, seconds: i64, reset: f64, label: Option<String>) -> Value {
    let label = label.unwrap_or_else(|| {
        if seconds == 604800 {
            "Semanal".into()
        } else if seconds % 3600 == 0 && seconds > 0 {
            format!("{} h", seconds / 3600)
        } else {
            "Cuota".into()
        }
    });
    json!({"label":label,"usedPercent":used.clamp(0.,100.),"availablePercent":100.-used.clamp(0.,100.),"resetAt":reset,"windowSeconds":seconds})
}
pub fn openai_windows(d: &Value) -> Vec<Value> {
    let mut result = vec![];
    let mut limits = vec![("".to_string(), &d["rate_limit"])];
    for e in d["additional_rate_limits"].as_array().into_iter().flatten() {
        limits.push((text(e, "limit_name").to_string(), &e["rate_limit"]));
    }
    for (name, rate) in limits {
        for key in ["primary_window", "secondary_window"] {
            let w = &rate[key];
            if let Some(used) = number(&w["used_percent"]) {
                let sec = w["limit_window_seconds"].as_i64().unwrap_or(0);
                result.push(window(
                    used,
                    sec,
                    number(&w["reset_at"]).unwrap_or(0.),
                    if name.is_empty() {
                        None
                    } else {
                        Some(format!("{name} · {} h", sec / 3600))
                    },
                ));
            }
        }
    }
    result
}
pub fn kimi_windows(d: &Value) -> Vec<Value> {
    let mut result = vec![];
    if let Some(used) = number(&d["usage"]["used"]) {
        result.push(window(
            used,
            604800,
            epoch(&d["usage"]["resetTime"]) as f64 / 1000.,
            None,
        ));
    }
    for w in d["limits"].as_array().into_iter().flatten() {
        if let Some(used) = number(&w["detail"]["used"]).or_else(|| {
            let limit = number(&w["detail"]["limit"])?;
            let remaining = number(&w["detail"]["remaining"])?;
            if limit > 0. {
                Some((limit - remaining) / limit * 100.)
            } else {
                None
            }
        }) {
            let unit = text(&w["window"], "timeUnit");
            let sec = w["window"]["duration"].as_i64().unwrap_or(0)
                * if unit.ends_with("MINUTE") {
                    60
                } else if unit.ends_with("HOUR") {
                    3600
                } else {
                    1
                };
            result.push(window(
                used,
                sec,
                epoch(&w["detail"]["resetTime"]) as f64 / 1000.,
                None,
            ));
        }
    }
    result
}
fn read_env(key: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            std::fs::read_to_string(home().join(".hermes/.env"))
                .unwrap_or_default()
                .lines()
                .filter(|l| !l.trim_start().starts_with('#'))
                .filter_map(|l| l.split_once('='))
                .find(|(k, _)| k.trim() == key)
                .map(|(_, v)| v.trim().trim_matches(['\'', '"']).to_string())
                .unwrap_or_default()
        })
}
fn provider(client: &ExternalHttp, name: &str, token: &str) -> Value {
    let mut status = "unauthenticated".to_string();
    let mut windows = vec![];
    let mut details = vec![];
    let mut plan = String::new();
    if !token.is_empty() {
        let data = match name {
            "openai-codex" => {
                let claims = token
                    .split('.')
                    .nth(1)
                    .and_then(|p| {
                        base64::engine::general_purpose::URL_SAFE_NO_PAD
                            .decode(p.trim_end_matches('='))
                            .ok()
                    })
                    .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                    .unwrap_or(Value::Null);
                let account = text(&claims["https://api.openai.com/auth"], "chatgpt_account_id");
                client.fetch_headers(
                    "https://chatgpt.com/backend-api/wham/usage",
                    token,
                    false,
                    &if account.is_empty() {
                        vec![]
                    } else {
                        vec![("ChatGPT-Account-Id", account)]
                    },
                )
            }
            "kimi-coding" => client.fetch("https://api.kimi.com/coding/v1/usages", token, false),
            _ => client.fetch(
                "https://portal.nousresearch.com/api/oauth/account",
                token,
                false,
            ),
        };
        match data {
            Ok(d) => {
                status = "ok".into();
                match name {
                    "openai-codex" => {
                        windows = openai_windows(&d);
                        plan = text(&d, "plan_type").to_owned();
                    }
                    "kimi-coding" => windows = kimi_windows(&d),
                    _ => {
                        let s = &d["subscription"];
                        plan = text(s, "plan").into();
                        if let (Some(cap), Some(left)) = (
                            number(&s["monthly_credits"]),
                            number(&s["credits_remaining"]),
                        ) {
                            if cap > 0. {
                                windows.push(window(
                                    (cap - left) / cap * 100.,
                                    0,
                                    epoch(&s["current_period_end"]) as f64 / 1000.,
                                    Some("Mensual".into()),
                                ));
                            }
                        }
                        if let Some(total) =
                            number(&d["paid_service_access"]["total_usable_credits"])
                        {
                            details.push(format!("US${total:.2} disponible"));
                        }
                    }
                }
            }
            Err(e) => status = e,
        }
    }
    json!({"quota":{"status":status,"windows":windows,"plan":plan,"details":details}})
}

fn compatible(name: &str, token: &str, legacy: Value) -> Value {
    let checked = Utc::now().to_rfc3339();
    let windows=legacy["quota"]["windows"].as_array().into_iter().flatten().enumerate().map(|(i,w)| {
        let seconds=w["windowSeconds"].as_i64().unwrap_or(0);
        let kind=if seconds==604800{"weekly"}else if seconds==18000{"session"}else if name=="nous"{"monthly"}else{"other"};
        let mut out=json!({"id":format!("local_{kind}_{i}"),"kind":kind,"label":w["label"],"usedPercent":w["usedPercent"]});
        // Match native Codex and OpenCode keys through their own pooling rules.
        if seconds>0{out["windowDurationMins"]=json!(seconds/60);}
        if let Some(at)=w["resetAt"].as_f64().filter(|v|*v>0.) {if let Some(date)=chrono::DateTime::from_timestamp(at as i64,0){out["resetsAt"]=json!(date.to_rfc3339());}}
        out
    }).collect::<Vec<_>>();
    let driver = match name {
        "openai-codex" => "codex",
        other => other,
    };
    let mut usage = json!({"checkedAt":checked,"windows":windows,"credentialFingerprint":fingerprint(name,token)});
    if legacy["quota"]["status"] != "ok" {
        usage["unavailable"] = json!({"reason":"probeFailed"});
    }
    let mut account = json!({"id":fingerprint(name,token),"driver":driver,"usageLimits":usage});
    let decoded = claims(token);
    let mut email = decoded["email"]
        .as_str()
        .or(decoded["https://api.openai.com/profile"]["email"].as_str())
        .unwrap_or("")
        .to_owned();
    if email.is_empty() && name == "openai-codex" {
        let codex = read_json(&home().join(".codex/auth.json")).unwrap_or(Value::Null);
        if codex["tokens"]["access_token"] == token || codex["access_token"] == token {
            email = text(&claims(text(&codex["tokens"], "id_token")), "email").to_owned();
        }
    }
    if email.is_empty() {
        let auth = read_json(&home().join(".hermes/auth.json")).unwrap_or(Value::Null);
        for entry in auth["credential_pool"][name]
            .as_array()
            .into_iter()
            .flatten()
        {
            if ["access_token", "api_key", "key"]
                .iter()
                .any(|key| entry[*key] == token)
            {
                email = entry["email"]
                    .as_str()
                    .or(entry["account"]["email"].as_str())
                    .unwrap_or("")
                    .to_owned();
                break;
            }
        }
    }
    if !email.is_empty() {
        account["email"] = json!(email);
    }
    if let Some(plan) = legacy["quota"]["plan"].as_str().filter(|s| !s.is_empty()) {
        account["plan"] = json!(plan);
    }
    account
}
fn go_windows(body: &Value) -> Option<Vec<Value>> {
    let mut windows = vec![];
    for (field, id, kind, label, minutes) in [
        (
            "rolling",
            "go_rolling",
            "session",
            "Go · Session",
            Some(300),
        ),
        ("weekly", "go_weekly", "weekly", "Go · Weekly", Some(10080)),
        ("monthly", "go_monthly", "monthly", "Go · Monthly", None),
    ] {
        let w = &body["usage"][field];
        let percent = w["percent"].as_f64().filter(|v| v.is_finite())?;
        let mut window =
            json!({"id":id,"kind":kind,"label":label,"usedPercent":percent.clamp(0.,100.)});
        if let Some(duration) = minutes {
            window["windowDurationMins"] = json!(duration);
        }
        if let Some(reset) = w["resetsAt"]
            .as_str()
            .filter(|s| chrono::DateTime::parse_from_rfc3339(s).is_ok())
        {
            window["resetsAt"] = json!(reset);
        }
        if window.get("resetsAt").is_none() {
            return None;
        }
        windows.push(window);
    }

    Some(windows)
}
fn opencode(client: &ExternalHttp, token: &str) -> Value {
    let mut usage = json!({"checkedAt":Utc::now().to_rfc3339(),"windows":[],"credentialFingerprint":fingerprint("opencode-go",token)});
    match client.fetch_headers(
        "https://opencode.ai/zen/go/v1/usage",
        token,
        false,
        &[(
            "User-Agent",
            concat!("omarchy-t3-code/", env!("CARGO_PKG_VERSION")),
        )],
    ) {
        Ok(body) => {
            if let Some(windows) = go_windows(&body) {
                usage["windows"] = json!(windows);
            } else {
                usage["unavailable"] = json!({"reason":"probeFailed"});
            }
        }
        Err(_) => {
            usage["unavailable"] = json!({"reason":"probeFailed"});
        }
    }
    json!({"id":fingerprint("opencode-go",token),"driver":"opencode","usageLimits":usage})
}
pub fn collect_with(client: &ExternalHttp, mut receive: impl FnMut(Value, bool)) -> Value {
    let auth = read_json(&home().join(".hermes/auth.json")).unwrap_or(Value::Null);
    let mut names = std::collections::BTreeSet::new();
    for key in ["credential_pool", "providers"] {
        for name in auth[key].as_object().into_iter().flat_map(|m| m.keys()) {
            names.insert(name.clone());
        }
    }
    if home().join(".codex/auth.json").exists() {
        names.insert("openai-codex".into());
    }
    let kimi = read_env("KIMI_CODING_API_KEY");
    if !kimi.is_empty() {
        names.insert("kimi-coding".into());
    }
    let mut go = tokens("opencode-go");
    let stored = read_json(&xdg("XDG_DATA_HOME", ".local/share").join("opencode/auth.json"))
        .unwrap_or(Value::Null);
    if stored["opencode-go"]["type"] == "api" {
        if let Some(key) = stored["opencode-go"]["key"]
            .as_str()
            .filter(|s| !s.is_empty())
        {
            go.push(key.to_owned());
        }
    }
    for key in ["OPENCODE_GO_API_KEY", "OPENCODE_API_KEY"] {
        let value = read_env(key);
        if !value.is_empty() {
            go.push(value);
        }
    }
    let mut tasks = Vec::new();
    for name in names {
        // Adapters know endpoints; discovery and account counts are data driven.
        if !matches!(name.as_str(), "openai-codex" | "kimi-coding" | "nous") {
            continue;
        }
        let mut credentials = tokens(&name);
        if name == "kimi-coding" && !kimi.is_empty() {
            credentials.push(kimi.clone());
        }
        let mut seen = std::collections::HashSet::new();
        for token in credentials {
            if seen.insert(token.clone()) {
                tasks.push((name.clone(), token));
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    for token in go {
        if seen.insert(token.clone()) {
            tasks.push(("opencode-go".into(), token));
        }
    }
    let mut accounts = Vec::new();
    for chunk in tasks.chunks(4) {
        let mut results = std::thread::scope(|scope| {
            let jobs = chunk
                .iter()
                .map(|(name, token)| {
                    scope.spawn(move || {
                        if name == "opencode-go" {
                            opencode(client, token)
                        } else {
                            compatible(name, token, provider(client, name, token))
                        }
                    })
                })
                .collect::<Vec<_>>();
            jobs.into_iter()
                .filter_map(|job| job.join().ok())
                .collect::<Vec<_>>()
        });
        accounts.append(&mut results);
        receive(
            json!([{"id":"ot3-local-credentials","label":"Local accounts","accounts":accounts}]),
            false,
        );
    }
    let result = if accounts.is_empty() {
        json!([])
    } else {
        json!([{"id":"ot3-local-credentials","label":"Local accounts","accounts":accounts}])
    };
    receive(result.clone(), true);
    result
}
pub fn collect(client: &ExternalHttp) -> Value {
    collect_with(client, |_, _| {})
}
/// HTTP to the third-party usage APIs above (ChatGPT, Kimi, Nous, opencode), with
/// credentials that are not T3's. T3 traffic goes through `protocol::Connection`, which
/// only allows same-origin paths: 8 s per request, 3 s to connect, no proxy, no redirects,
/// JSON responses up to 8 MiB.
#[derive(Clone)]
pub struct ExternalHttp {
    http: reqwest::blocking::Client,
}
impl ExternalHttp {
    pub fn new() -> Result<Self> {
        Ok(Self {
            http: reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(8))
                .connect_timeout(Duration::from_secs(3))
                .pool_idle_timeout(Duration::from_secs(90))
                .redirect(reqwest::redirect::Policy::none())
                .no_proxy()
                .build()
                .map_err(|_| "local_error")?,
        })
    }
    /// The process-wide client. Clones share its pool.
    pub fn shared() -> Result<Self> {
        static SHARED: std::sync::OnceLock<ExternalHttp> = std::sync::OnceLock::new();
        if let Some(client) = SHARED.get() {return Ok(client.clone())}
        let client = Self::new()?;
        Ok(SHARED.get_or_init(|| client).clone())
    }
    pub fn fetch(&self, url: &str, token: &str, protocol: bool) -> Result<Value> {
        self.fetch_headers(url, token, protocol, &[])
    }
    pub fn fetch_headers(
        &self,
        url: &str,
        token: &str,
        protocol: bool,
        headers: &[(&str, &str)],
    ) -> Result<Value> {
        let mut req = self
            .http
            .get(url)
            .bearer_auth(token)
            .header("Accept", "application/json")
            .header("Accept-Encoding", "identity");
        for (name, value) in headers {
            req = req.header(*name, *value);
        }
        if protocol {
            req = req.header("x-t3-orchestration-protocol", "2");
        }
        let resp = req.send().map_err(|_| "unreachable")?;
        let status = resp.status();
        if [401, 403].contains(&status.as_u16()) {
            return Err("unauthorized".into());
        }
        if !status.is_success() {
            return Err(format!("http_{}", status.as_u16()));
        }
        if resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_lowercase()
            != "application/json"
        {
            return Err("not_json".into());
        }
        if !["identity", ""].contains(
            &resp
                .headers()
                .get("content-encoding")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("identity"),
        ) {
            return Err("unsupported_encoding".into());
        }
        let mut bytes = vec![];
        resp.take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "unreachable")?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err("too_large".into());
        }
        serde_json::from_slice(&bytes).map_err(|_| "malformed".into())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn serve(status: &str, typ: &str, body: &str) -> (String, std::thread::JoinHandle<String>) {
        use std::io::Write;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let (status, typ, body) = (status.to_owned(), typ.to_owned(), body.to_owned());
        let handle = std::thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            let mut buf = [0; 8192];
            let len = conn.read(&mut buf).unwrap();
            write!(conn,"HTTP/1.1 {status}\r\nContent-Type: {typ}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            String::from_utf8_lossy(&buf[..len]).into_owned()
        });
        (base, handle)
    }
    #[test]
    fn external_http_headers_and_failure_boundaries() {
        let c = ExternalHttp::new().unwrap();
        let (base, h) = serve("200 OK", "application/json", "{\"threads\":[]}");
        assert_eq!(
            c.fetch_headers(&base, "synthetic-test-token", false, &[("ChatGPT-Account-Id", "acct")]).unwrap(),
            json!({"threads":[]})
        );
        let req = h.join().unwrap().to_lowercase();
        assert!(req.contains("authorization: bearer synthetic-test-token"));
        assert!(req.contains("chatgpt-account-id: acct"));
        assert!(req.contains("accept-encoding: identity"));
        assert!(!req.contains("x-t3-orchestration-protocol"));
        for (status, typ, body, error) in [
            ("401 Unauthorized", "application/json", "{}", "unauthorized"),
            ("403 Forbidden", "application/json", "{}", "unauthorized"),
            ("200 OK", "text/html", "<html>", "not_json"),
            ("200 OK", "application/json", "{", "malformed"),
            ("302 Found", "text/html", "", "http_302"),
        ] {
            let (base, h) = serve(status, typ, body);
            assert_eq!(c.fetch(&base, "synthetic-test-token", false).unwrap_err(), error);
            h.join().unwrap();
        }
    }
    #[test]
    fn all_pool_credentials_are_discovered_and_deduplicated() {
        let auth = json!({"credential_pool":{"openai-codex":[{"access_token":"one"},{"access_token":"two"},{"access_token":"one"}],"opencode-go":[{"api_key":"go-one"},{"key":"go-two"}]},"providers":{"openai-codex":{"access_token":"two"}}});
        assert_eq!(
            tokens_in(
                &auth,
                &json!({"tokens":{"access_token":"three"}}),
                "openai-codex"
            ),
            vec!["one", "two", "three"]
        );
        assert_eq!(
            tokens_in(&auth, &Value::Null, "opencode-go"),
            vec!["go-one", "go-two"]
        );
        assert!(tokens_in(&auth, &Value::Null, "unconfigured").is_empty());
    }
    #[test]
    fn go_payload_preserves_native_ids_and_rejects_partial_or_invalid_resets() {
        let w = json!({"percent":25,"resetsAt":"2026-10-19T12:00:00Z"});
        let mut body = json!({"usage":{"rolling":w,"weekly":w,"monthly":w}});
        let windows = go_windows(&body).unwrap();
        assert_eq!(windows.len(), 3);
        assert_eq!(windows[0]["id"], "go_rolling");
        assert_eq!(windows[1]["windowDurationMins"], 10080);
        assert!(windows[2].get("windowDurationMins").is_none());
        body["usage"]["monthly"]["resetsAt"] = json!("bad");
        assert!(go_windows(&body).is_none());
        body["usage"].as_object_mut().unwrap().remove("monthly");
        assert!(go_windows(&body).is_none());
        assert_eq!(
            fingerprint("opencode-go", "same"),
            fingerprint("opencode-go", "same")
        );
        assert_ne!(
            fingerprint("opencode-go", "same"),
            fingerprint("nous", "same")
        );
    }
    #[test]
    fn provider_windows_preserve_available() {
        let w = openai_windows(
            &json!({"rate_limit":{"primary_window":{"used_percent":20,"limit_window_seconds":18000,"reset_at":1900000000}}}),
        );
        assert_eq!(w[0]["availablePercent"], 80.);
        assert_eq!(w[0]["windowSeconds"], 18000);
        let w = kimi_windows(
            &json!({"usage":{"used":"8","resetTime":"2026-10-15T04:00:00Z"},"limits":[{"window":{"duration":300,"timeUnit":"TIME_UNIT_MINUTE"},"detail":{"limit":"100","remaining":"92","resetTime":"2026-10-15T04:00:00Z"}}]}),
        );
        assert_eq!(w[1]["availablePercent"], 92.);
        assert_eq!(w[0]["availablePercent"], 92.);
        assert_eq!(w[0]["resetAt"], 1792036800.);
    }
}
