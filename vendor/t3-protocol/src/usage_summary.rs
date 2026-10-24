//! T3's usage page data (server.getUsageSummary), summed in Rust so the panel only
//! draws totals, a daily series and the top models.
use crate::util::{text, Result};
use chrono::{Datelike, Duration, NaiveDate};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Period windows of T3's usage page, ending today.
pub fn window(period: &str, today: NaiveDate) -> Result<(NaiveDate, NaiveDate)> {
    let since = match period {
        "day" => today,
        "week" => today - Duration::days(6),
        "month" => today - Duration::days(29),
        "quarter" => today - Duration::days(89),
        "year" => NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap(),
        _ => return Err("invalid_usage_period".into()),
    };
    Ok((since, today))
}
/// The window and `server.getUsageSummary` payload for a period ending `today`.
/// `time_zone` is the host's IANA zone, as the server buckets days in it.
pub fn request(period: &str, today: NaiveDate, time_zone: &str) -> Result<(NaiveDate, NaiveDate, Value)> {
    let (since, until) = window(period, today)?;
    let payload = json!({"sinceDay":since.format("%Y-%m-%d").to_string(),"untilDay":until.format("%Y-%m-%d").to_string(),"timeZone":time_zone,"resolution":"day"});
    Ok((since, until, payload))
}
fn tokens(t: &Value) -> u64 {
    ["uncachedInputTokens", "cachedInputTokens", "cacheCreationTokens", "outputTokens", "reasoningTokens"]
        .iter()
        .map(|k| t[*k].as_u64().unwrap_or(0))
        .sum()
}
pub fn aggregate(summary: &Value, since: NaiveDate, until: NaiveDate) -> Value {
    let mut days: BTreeMap<String, (f64, u64)> = BTreeMap::new();
    let mut day = since;
    while day <= until {
        days.insert(day.format("%Y-%m-%d").to_string(), (0.0, 0));
        day += Duration::days(1);
    }
    let mut models: BTreeMap<(String, String), (f64, u64, u64)> = BTreeMap::new();
    let mut providers: BTreeMap<String, (f64, u64)> = BTreeMap::new();
    let (mut cost, mut total, mut sessions, mut savings, mut unpriced) = (0.0, 0u64, 0u64, 0.0, 0u64);
    let mut kinds = json!({"input":0,"cached":0,"cacheWrite":0,"output":0,"reasoning":0});
    for b in summary["buckets"].as_array().into_iter().flatten() {
        let c = b["costUsd"].as_f64().unwrap_or(0.0);
        let t = tokens(&b["totals"]);
        cost += c;
        total += t;
        sessions += b["sessions"].as_u64().unwrap_or(0);
        savings += b["cacheSavingsUsd"].as_f64().unwrap_or(0.0);
        unpriced += b["unpricedRecords"].as_u64().unwrap_or(0);
        for (key, field) in [("input", "uncachedInputTokens"), ("cached", "cachedInputTokens"), ("cacheWrite", "cacheCreationTokens"), ("output", "outputTokens"), ("reasoning", "reasoningTokens")] {
            kinds[key] = json!(kinds[key].as_u64().unwrap_or(0) + b["totals"][field].as_u64().unwrap_or(0));
        }
        if let Some(d) = days.get_mut(text(b, "day")) {
            d.0 += c;
            d.1 += t;
        }
        let m = models.entry((text(b, "provider").into(), text(b, "model").into())).or_default();
        m.0 += c;
        m.1 += t;
        m.2 += b["records"].as_u64().unwrap_or(0);
        let p = providers.entry(text(b, "provider").into()).or_default();
        p.0 += c;
        p.1 += t;
    }
    let mut models: Vec<Value> = models
        .into_iter()
        .map(|((provider, model), (c, t, r))| json!({"provider":provider,"model":model,"cost":c,"tokens":t,"records":r}))
        .collect();
    models.sort_by(|a, b| b["cost"].as_f64().partial_cmp(&a["cost"].as_f64()).unwrap().then(b["tokens"].as_u64().cmp(&a["tokens"].as_u64())));
    models.truncate(12);
    let sources: Vec<Value> = summary["sources"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| json!({"provider":s["fingerprint"]["provider"],"status":s["status"],"message":s["message"],"refreshing":s["refreshing"]==true}))
        .collect();
    json!({
        "sinceDay":since.format("%Y-%m-%d").to_string(),"untilDay":until.format("%Y-%m-%d").to_string(),
        "cost":cost,"tokens":total,"sessions":sessions,"cacheSavings":savings,"unpricedRecords":unpriced,"kinds":kinds,
        "days":days.into_iter().map(|(day,(c,t))|json!({"day":day,"cost":c,"tokens":t})).collect::<Vec<_>>(),
        "models":models,
        "providers":providers.into_iter().map(|(p,(c,t))|json!({"provider":p,"cost":c,"tokens":t})).collect::<Vec<_>>(),
        "sources":sources,"pricing":summary["pricing"]["status"],"readAt":summary["readAt"]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sums_days_models_and_kinds() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 20).unwrap();
        let (since, until) = window("week", today).unwrap();
        assert_eq!(since, NaiveDate::from_ymd_opt(2026, 10, 14).unwrap());
        let t = |i: u64, o: u64| json!({"uncachedInputTokens":i,"cachedInputTokens":0,"cacheCreationTokens":0,"outputTokens":o,"reasoningTokens":0});
        let s = json!({"buckets":[
            {"day":"2026-10-20","provider":"claude","model":"opus","totals":t(100,50),"costUsd":1.5,"cacheSavingsUsd":0.2,"records":3,"unpricedRecords":0,"sessions":1},
            {"day":"2026-10-19","provider":"codex","model":"gpt","totals":t(10,5),"costUsd":0.5,"cacheSavingsUsd":0,"records":1,"unpricedRecords":1,"sessions":2},
            {"day":"2026-10-20","provider":"claude","model":"opus","totals":t(1,1),"costUsd":0.25,"cacheSavingsUsd":0,"records":1,"unpricedRecords":0,"sessions":1}
        ],"sources":[{"fingerprint":{"provider":"claude"},"status":"ok","message":null}],"pricing":{"status":"fresh"}});
        let a = aggregate(&s, since, until);
        assert_eq!(a["cost"], 2.25);
        assert_eq!(a["tokens"], 167);
        assert_eq!(a["sessions"], 4);
        assert_eq!(a["days"].as_array().unwrap().len(), 7);
        assert_eq!(a["days"][6]["cost"], 1.75);
        assert_eq!(a["models"][0]["model"], "opus");
        assert_eq!(a["kinds"]["output"], 56);
        assert!(window("decade", today).is_err());
        let (_, _, payload) = request("week", today, "America/Santiago").unwrap();
        assert_eq!(payload, json!({"sinceDay":"2026-10-14","untilDay":"2026-10-20","timeZone":"America/Santiago","resolution":"day"}));
        assert!(request("decade", today, "UTC").is_err());
    }
}
