use base64::Engine;
use chrono::Utc;
use omarchy_t3_client::*;
use serde_json::{json, Value};

fn auth(provider: &str) -> String {
    let a = read_json(&home().join(".hermes/auth.json")).unwrap_or(Value::Null);
    for entry in a["credential_pool"][provider]
        .as_array()
        .into_iter()
        .flatten()
    {
        if let Some(t) = entry["access_token"].as_str().filter(|s| !s.is_empty()) {
            return t.into();
        }
    }
    for v in [
        &a["providers"][provider]["tokens"]["access_token"],
        &a["providers"][provider]["access_token"],
    ] {
        if let Some(t) = v.as_str().filter(|s| !s.is_empty()) {
            return t.into();
        }
    }
    if provider == "openai-codex" {
        let c = read_json(&home().join(".codex/auth.json")).unwrap_or(Value::Null);
        for v in [&c["tokens"]["access_token"], &c["access_token"]] {
            if let Some(t) = v.as_str().filter(|s| !s.is_empty()) {
                return t.into();
            }
        }
    }
    String::new()
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
fn provider(client: &Client, name: &str) -> Value {
    let token = if name == "kimi-coding" {
        read_env("KIMI_CODING_API_KEY")
    } else {
        auth(name)
    };
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
                    &token,
                    false,
                    &if account.is_empty() {
                        vec![]
                    } else {
                        vec![("ChatGPT-Account-Id", account)]
                    },
                )
            }
            "kimi-coding" => client.fetch("https://api.kimi.com/coding/v1/usages", &token, false),
            _ => client.fetch(
                "https://portal.nousresearch.com/api/oauth/account",
                &token,
                false,
            ),
        };
        match data {
            Ok(d) => {
                status = "ok".into();
                match name {
                    "openai-codex" => windows = openai_windows(&d),
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
    // Preserve Ares' window semantics and calculate the pace once per refresh, outside QML.
    let weekly = windows
        .iter()
        .find(|w| w["windowSeconds"] == 604800)
        .or_else(|| {
            windows
                .iter()
                .max_by_key(|w| w["windowSeconds"].as_i64().unwrap_or(0))
        })
        .cloned()
        .unwrap_or(Value::Null);
    let short = windows
        .iter()
        .find(|w| w["windowSeconds"] == 18000)
        .cloned()
        .unwrap_or(Value::Null);
    let seconds = weekly["windowSeconds"].as_f64().unwrap_or(0.);
    let reset = weekly["resetAt"].as_f64().unwrap_or(0.);
    let expected = if seconds > 0. && reset > 0. {
        ((reset - Utc::now().timestamp() as f64) / seconds * 100.).clamp(0., 100.)
    } else {
        -1.
    };
    json!({"provider":name,"label":match name{"openai-codex"=>"Codex","kimi-coding"=>"Kimi",_=>"Nous"},"quota":{"status":status,"windows":windows,"plan":plan,"details":details},"weekly":weekly,"shortWindow":short,"expectedAvailable":expected,"onPace":expected<0.||weekly["availablePercent"].as_f64().unwrap_or(0.)>=expected})
}
pub fn collect(client: &Client) -> Value {
    let providers: Vec<Value> = ["openai-codex", "kimi-coding", "nous"]
        .iter()
        .map(|name| provider(client, name))
        .collect();
    json!({"updatedAt":Utc::now().timestamp(),"providers":providers})
}
#[cfg(test)]
mod tests {
    use super::*;
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
