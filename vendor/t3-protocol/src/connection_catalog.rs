//! Native T3 connection catalog transformations. Hosts own encrypted persistence.
use serde_json::{json, Value};
type Result<T> = std::result::Result<T, String>;
/// T3 direct/hosted pairing links and host+code forms. Credentials stay host-owned.
pub fn pairing_target(raw: &str, code: &str) -> Result<(String, String)> {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > 16384 || raw.chars().any(char::is_control) {
        return Err("invalid_endpoint".into());
    }
    let normalize = |s: &str| {
        reqwest::Url::parse(&if s.contains("://") {
            s.to_string()
        } else {
            format!("https://{s}")
        })
        .map_err(|_| "invalid_endpoint".to_string())
    };
    let mut url = normalize(raw)?;
    let fragment = url
        .fragment()
        .and_then(|f| reqwest::Url::parse(&format!("http://localhost/?{f}")).ok());
    let get = |u: &reqwest::Url, k: &str| {
        u.query_pairs()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.to_string())
    };
    let credential = if !code.trim().is_empty() {
        code.trim().to_string()
    } else {
        fragment
            .as_ref()
            .and_then(|u| get(u, "token"))
            .or_else(|| get(&url, "token"))
            .unwrap_or_default()
    };
    if credential.len() > 16384 || credential.chars().any(char::is_control) {
        return Err("invalid_pairing_code".into());
    }
    if let Some(host) = get(&url, "host").filter(|h| !h.is_empty() && !credential.is_empty()) {
        url = normalize(&host)?;
    }
    let scheme = match url.scheme() {
        "http" | "ws" => "http",
        "https" | "wss" => "https",
        _ => return Err("invalid_endpoint".into()),
    };
    url.set_scheme(scheme).map_err(|_| "invalid_endpoint")?;
    if url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port() == Some(0)
    {
        return Err("invalid_endpoint".into());
    }
    url.set_path("");
    url.set_query(None);
    url.set_fragment(None);
    Ok((url.as_str().trim_end_matches('/').to_string(), credential))
}
pub fn empty() -> Value {
    json!({"schemaVersion":1,"targets":[],"profiles":[],"credentials":[],"remoteDpopTokens":[],"disabledEnvironmentIds":[]})
}
fn validate(c: &Value) -> Result<()> {
    if c["schemaVersion"] != 1
        || !["targets", "profiles", "credentials"]
            .iter()
            .all(|k| c[*k].is_array())
    {
        return Err("invalid_t3_catalog".into());
    }
    Ok(())
}
pub fn view(c: &Value) -> Value {
    let disabled = c["disabledEnvironmentIds"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut environments: Vec<Value> = vec![];
    for t in c["targets"].as_array().into_iter().flatten() {
        let id = t["environmentId"].as_str().unwrap_or("");
        let p = c["profiles"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|p| p["connectionId"] == t["connectionId"]);
        let ssh=p.and_then(|p|p["target"].as_object().map(|_|json!({"alias":p["target"]["alias"],"hostname":p["target"]["hostname"],"username":p["target"]["username"],"port":p["target"]["port"]})));
        let url = p
            .and_then(|p| p["httpBaseUrl"].as_str())
            .and_then(|s| pairing_target(s, "").ok())
            .map(|(base, _)| base);
        let route = json!({"connectionId":t["connectionId"],"kind":t["_tag"],"ssh":ssh,"url":url});
        if let Some(e) = environments.iter_mut().find(|e| e["id"] == id) {
            e["routes"].as_array_mut().unwrap().push(route)
        } else {
            environments.push(json!({"id":id,"name":t["label"],"enabled":!disabled.contains(&t["environmentId"]),"routes":[route]}))
        }
    }
    json!(environments)
}
fn label(i: &Value) -> Result<String> {
    let s = i["label"].as_str().unwrap_or("").trim();
    if s.is_empty() || s.len() > 256 || s.chars().any(char::is_control) {
        return Err("invalid_connection_label".into());
    }
    Ok(s.to_string())
}
pub fn mutate(c: &Value, i: &Value) -> Result<Value> {
    validate(c)?;
    let mut c = c.clone();
    let env = i["environmentId"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("unknown_environment")?;
    if !c["targets"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|t| t["environmentId"] == env)
    {
        return Err("unknown_environment".into());
    }
    match i["action"].as_str().unwrap_or("") {
        "rename" => {
            let name = label(i)?;
            for key in ["targets", "profiles"] {
                for p in c[key].as_array_mut().unwrap() {
                    if p["environmentId"] == env {
                        p["label"] = json!(name)
                    }
                }
            }
        }
        "enabled" => {
            let enabled = i["enabled"].as_bool().ok_or("invalid_command")?;
            let mut ids = c["disabledEnvironmentIds"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            ids.retain(|id| id != env);
            if !enabled {
                ids.push(json!(env))
            }
            c["disabledEnvironmentIds"] = json!(ids);
        }
        "remove" | "remove-route" => {
            if i["confirmed"] != true {
                return Err("confirmation_required".into());
            }
            let route = i["connectionId"].as_str().unwrap_or("");
            let all = i["action"] == "remove";
            if !all
                && !c["targets"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|t| t["environmentId"] == env && t["connectionId"] == route)
            {
                return Err("unknown_route".into());
            }
            let removed: Vec<Value> = c["targets"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|t| t["environmentId"] == env && (all || t["connectionId"] == route))
                .map(|t| t["connectionId"].clone())
                .filter(|id| id.is_string())
                .collect();
            c["targets"]
                .as_array_mut()
                .unwrap()
                .retain(|t| !(t["environmentId"] == env && (all || t["connectionId"] == route)));
            for key in ["profiles", "credentials"] {
                c[key]
                    .as_array_mut()
                    .unwrap()
                    .retain(|p| !removed.contains(&p["connectionId"]));
            }
            if all
                || !c["targets"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|t| t["environmentId"] == env)
            {
                for key in ["remoteDpopTokens", "githubRoutingPermissions"] {
                    if let Some(a) = c[key].as_array_mut() {
                        a.retain(|p| p["environmentId"] != env)
                    }
                }
                if let Some(a) = c["disabledEnvironmentIds"].as_array_mut() {
                    a.retain(|v| v != env)
                }
            }
        }
        "prefer-route" => {
            let route = &i["connectionId"];
            let a = c["targets"].as_array_mut().unwrap();
            let pos = a
                .iter()
                .position(|t| t["environmentId"] == env && t["connectionId"] == *route)
                .ok_or("unknown_route")?;
            let t = a.remove(pos);
            let first = a
                .iter()
                .position(|t| t["environmentId"] == env)
                .unwrap_or(a.len());
            a.insert(first, t);
        }
        _ => return Err("invalid_command".into()),
    }
    Ok(c)
}
pub fn register(
    c: &Value,
    target: Value,
    profile: Value,
    credential: Option<Value>,
) -> Result<Value> {
    validate(c)?;
    if credential.as_ref().is_some_and(|v| {
        v.as_str()
            .is_none_or(|s| s.is_empty() || s.chars().any(char::is_control))
    }) {
        return Err("invalid_connection".into());
    }
    let mut c = c.clone();
    let id = target["connectionId"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("invalid_connection")?;
    let env = target["environmentId"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("invalid_connection")?;
    if profile["environmentId"] != env || profile["connectionId"] != id {
        return Err("invalid_connection".into());
    }
    if let Some(existing) = c["targets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["connectionId"] == id)
    {
        if existing["environmentId"] != env {
            return Err("t3_environment_mismatch".into());
        }
    }
    for (key, value) in [("targets", target.clone()), ("profiles", profile)] {
        let list = c[key].as_array_mut().unwrap();
        if let Some(p) = list.iter().position(|v| v["connectionId"] == id) {
            list[p] = value
        } else {
            list.push(value)
        }
    }
    if let Some(credential) = credential {
        let list = c["credentials"].as_array_mut().unwrap();
        list.retain(|v| v["connectionId"] != id);
        list.push(json!({"connectionId":id,"credential":{"_tag":"BearerConnectionCredential","token":credential}}));
    }
    if let Some(a) = c["disabledEnvironmentIds"].as_array_mut() {
        a.retain(|v| v != env)
    }
    Ok(c)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_pairing_links_hide_code_from_backend_endpoint() {
        assert_eq!(
            pairing_target(
                "https://t3.codes/connect?host=http%3A%2F%2Fnode%3A3773#token=secret%20code",
                ""
            )
            .unwrap(),
            ("http://node:3773".into(), "secret code".into())
        );
        assert_eq!(
            pairing_target("wss://node:3773/path?token=secret", "").unwrap(),
            ("https://node:3773".into(), "secret".into())
        );
        assert_eq!(
            pairing_target("node:3773", "code").unwrap(),
            ("https://node:3773".into(), "code".into())
        );
        assert!(pairing_target("file:///tmp", "code").is_err());
        assert!(pairing_target("https://user:password@node", "code").is_err());
    }
    #[test]
    fn routes_preserve_other_accounts_and_redact_secrets() {
        let mut c = empty();
        for id in ["a", "b"] {
            c=register(&c,json!({"_tag":"BearerConnectionTarget","connectionId":id,"environmentId":"node","label":"Node"}),json!({"_tag":"BearerConnectionProfile","connectionId":id,"environmentId":"node","label":"Node","httpBaseUrl":"http://node"}),Some(json!("private-token"))).unwrap();
        }
        c["unknownFutureField"] = json!({"keep":true});
        assert!(!view(&c).to_string().contains("private-token"));
        c = mutate(
            &c,
            &json!({"action":"enabled","environmentId":"node","enabled":false}),
        )
        .unwrap();
        assert_eq!(view(&c)[0]["enabled"], false);
        assert_eq!(c["credentials"].as_array().unwrap().len(), 2);
        c = mutate(
            &c,
            &json!({"action":"prefer-route","environmentId":"node","connectionId":"b"}),
        )
        .unwrap();
        assert_eq!(view(&c)[0]["routes"][0]["connectionId"], "b");
        assert!(mutate(&c, &json!({"action":"remove","environmentId":"node"})).is_err());
        c=mutate(&c,&json!({"action":"remove-route","environmentId":"node","connectionId":"b","confirmed":true})).unwrap();
        assert_eq!(c["credentials"].as_array().unwrap().len(), 1);
        assert_eq!(c["unknownFutureField"]["keep"], true);
        c = mutate(
            &c,
            &json!({"action":"rename","environmentId":"node","label":"New"}),
        )
        .unwrap();
        assert_eq!(view(&c)[0]["name"], "New");
        c = mutate(
            &c,
            &json!({"action":"remove","environmentId":"node","confirmed":true}),
        )
        .unwrap();
        assert_eq!(c["credentials"], json!([]));
        assert_eq!(view(&c), json!([]));
    }
}
