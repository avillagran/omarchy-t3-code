pub mod backend_service;
pub mod images;
pub mod documents;
pub mod scroll;
pub mod message_text;
pub mod desktop_open;
pub mod clipboard;
mod desktop_cloud;
mod connections;
pub mod write;
pub mod suggestions;
pub use t3_protocol as protocol;
pub use connections::{read_catalog, t3_home, refresh_connections};
pub use desktop_cloud::{check_native_session,check_native_environment};
use chrono::{DateTime, Utc};
use reqwest::blocking::Client as Http;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::Duration,
};

pub type Result<T> = std::result::Result<T, String>;
pub const MAX_BYTES: u64 = 8 * 1024 * 1024;
pub fn text<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("")
}
pub fn truth(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
        Value::Number(n) => n.as_f64().unwrap_or(0.) != 0.,
    }
}
pub fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 2048
        && s.as_bytes()[0].is_ascii_alphanumeric()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_:%-".contains(&b))
}
pub fn epoch(v: &Value) -> i64 {
    if let Some(s) = v.as_str() {
        return DateTime::parse_from_rfc3339(s)
            .map(|d| d.timestamp_millis().max(0))
            .unwrap_or(0);
    }
    v.as_f64()
        .filter(|n| n.is_finite() && *n >= 0.)
        .map(|n| {
            if n < 100_000_000_000. {
                (n * 1000.) as i64
            } else {
                n as i64
            }
        })
        .unwrap_or(0)
}
pub fn rendered_markdown(input: &str) -> String {
    use pulldown_cmark::{html, Event, Options, Parser, Tag, TagEnd};
    let events = Parser::new_ext(
        input,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH,
    )
    .filter_map(|event| match event {
        Event::Start(Tag::Image { .. })
        | Event::End(TagEnd::Image)
        => None,
        Event::Start(Tag::Link { link_type, dest_url, title, id }) => {
            let url=dest_url.to_ascii_lowercase();
            let safe=!url.contains('\0') && (!url.contains(':') || ["http://","https://","file://","t3-thread://"].iter().any(|s|url.starts_with(s)));
            Some(Event::Start(Tag::Link {link_type,dest_url:if safe {dest_url}else{"#".into()},title,id}))
        },
        Event::Html(value) | Event::InlineHtml(value) => Some(Event::Text(value)),
        other => Some(other),
    });
    let mut out = String::new();
    html::push_html(&mut out, events);
    out
}
pub fn file_references(input:&str)->Vec<Value>{
 use pulldown_cmark::{Event,Parser,Tag};
 let mut paths=HashSet::new();
 Parser::new(input).filter_map(|e|match e{Event::Start(Tag::Link{dest_url,..})=>Some(dest_url.to_string()),Event::Code(path) if path.starts_with('/')||path.starts_with("./")=>Some(path.to_string()),_=>None})
 .filter(|p|!p.starts_with('#')&&(!p.contains(':')||p.starts_with("file://")))
 .filter(|p|paths.insert(p.clone())).take(30).map(|p|json!({"name":p.rsplit('/').next().unwrap_or(&p),"path":p})).collect()
}
pub fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}
pub fn xdg(name: &str, fallback: &str) -> PathBuf {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(fallback))
        .join("omarchy-t3-code")
}
pub fn read_json(p: &Path) -> Result<Value> {
    serde_json::from_slice(&fs::read(p).map_err(|_| "local_error")?).map_err(|_| "malformed".into())
}
pub fn save_json(p: &Path, v: &Value) -> Result<()> {
    let parent = p.parent().ok_or("local_error")?;
    fs::create_dir_all(parent).map_err(|_| "local_error")?;
    let tmp = parent.join(format!(".tmp-{}-{}", std::process::id(),uuid::Uuid::new_v4()));
    let result = (|| {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(|_| "local_error")?;
        use std::io::Write;
        f.write_all(v.to_string().as_bytes())
            .map_err(|_| "local_error")?;
        f.sync_all().map_err(|_| "local_error")?;
        fs::rename(&tmp, p).map_err(|_| "local_error".to_string())?;
        fs::File::open(parent).and_then(|d|d.sync_all()).map_err(|_| "local_error".to_string())
    })();
    let _ = fs::remove_file(tmp);
    result
}
pub fn private_token(path: &Path) -> Result<String> {
    let f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| "unauthorized")?;
    let m = f.metadata().map_err(|_| "unauthorized")?;
    if !m.is_file()
        || m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o077 != 0
        || m.len() > 4097
    {
        return Err("unauthorized".into());
    }
    let mut s = String::new();
    f.take(4097)
        .read_to_string(&mut s)
        .map_err(|_| "unauthorized")?;
    let s = s.trim();
    if !(8..=4096).contains(&s.len()) || s.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("unauthorized".into());
    }
    Ok(s.to_string())
}
#[derive(Clone, Deserialize)]
pub struct Environment {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub base_url: String,
    pub token_file: PathBuf,
    #[serde(default)]
    pub write_token_file: Option<PathBuf>,
    #[serde(skip)]
    pub bearer: Option<String>,
    #[serde(skip)]
    pub connection_error: Option<String>,
    #[serde(skip)]
    pub connection: Option<std::sync::Arc<protocol::Connection>>,
}
pub fn environments() -> Result<Vec<Environment>> {
    if let Some(path) = std::env::var_os("OMARCHY_T3_FIXTURE_ENVIRONMENTS") {
        return fixture_environments(&PathBuf::from(path));
    }
    connections::inherited_environments()
}
fn fixture_environments(p: &Path) -> Result<Vec<Environment>> {
    if !p.exists() {
        return Ok(vec![]);
    }
    let d = read_json(&p)?;
    let envs: Vec<Environment> =
        serde_json::from_value(d["environments"].clone()).map_err(|_| "invalid_config")?;
    let mut ids = HashSet::new();
    if envs.len() > 16 {
        return Err("invalid_config".into());
    }
    for e in &envs {
        if e.id.is_empty()
            || e.id.len() > 64
            || !e.id.as_bytes()[0].is_ascii_alphanumeric()
            || !e
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            || !ids.insert(&e.id)
            || !e.token_file.is_absolute()
        {
            return Err("invalid_config".into());
        }
        validate_base(&e.base_url)?;
    }
    Ok(envs)
}
pub fn environment_token(e: &Environment, write: bool) -> Result<String> {
    if let Some(error) = &e.connection_error {return Err(error.clone())}
    if let Some(token) = &e.bearer {return Ok(token.clone())}
    private_token(if write {e.write_token_file.as_ref().unwrap_or(&e.token_file)} else {&e.token_file})
}
pub fn validate_base(base: &str) -> Result<()> {
    let u = reqwest::Url::parse(base).map_err(|_| "invalid_endpoint")?;
    if !["http", "https"].contains(&u.scheme())
        || u.host_str().is_none()
        || !u.username().is_empty()
        || u.password().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
        || !["", "/"].contains(&u.path())
        || u.port() == Some(0)
    {
        return Err("invalid_endpoint".into());
    }
    Ok(())
}
#[derive(Clone)]
pub struct Client {
    http: Http,
}
impl Client {
    pub fn new() -> Result<Self> {
        Ok(Self {
            http: Http::builder()
                .timeout(Duration::from_secs(8))
                .connect_timeout(Duration::from_secs(3))
                .pool_idle_timeout(Duration::from_secs(90))
                .redirect(reqwest::redirect::Policy::none())
                .no_proxy()
                .build()
                .map_err(|_| "local_error")?,
        })
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
    pub fn get(&self, e: &Environment, path: &str, protocol: bool) -> Result<Value> {
        if let Some(error) = &e.connection_error {return Err(error.clone())}
        if let Some(connection) = &e.connection {return connection.request_json("GET",path,None,protocol)}
        validate_base(&e.base_url)?;
        protocol::Connection::with_http_client(self.http.clone(), &e.base_url, None,
            protocol::Credential::bearer(&environment_token(e,false)?)?)?.request_json("GET",path,None,protocol)
    }
    pub fn messages(&self, e: &Environment, id: &str) -> Result<Value> {
        if !valid_id(id) {
            return Err("invalid_id".into());
        }
        let mut u = reqwest::Url::parse(e.base_url.trim_end_matches('/'))
            .map_err(|_| "invalid_endpoint")?;
        u.path_segments_mut()
            .map_err(|_| "invalid_endpoint")?
            .extend(["api", "orchestration", "threads", id, "bounded"]);
        let d = self.get(e,u.path(),true)?;
        normalize_messages(&d)
    }
}
pub type RenderCache=HashMap<String,(String,String,String)>;
pub fn normalize_messages(d:&Value)->Result<Value>{normalize_messages_cached(d,&mut RenderCache::new())}
pub fn normalize_messages_cached(d:&Value,cache:&mut RenderCache)->Result<Value>{
        let rows = d["projection"]["messages"].as_array().ok_or("malformed")?;
        let queue=protocol::queue::derive(&d["projection"]);
        let hidden=queue["hiddenMessageIds"].as_array().unwrap();
        let mut messages = vec![];
        for m in rows {
            if hidden.contains(&m["id"]){continue}
            if !m["text"].is_string() || !m["role"].is_string() {
                return Err("malformed".into());
            }
            let raw=text(m,"text");let id=text(m,"id").to_string();
            if !cache.get(&id).is_some_and(|c|c.0==raw){let display=message_text::plain_qt(raw).unwrap_or_else(||raw.to_string());let rendered=rendered_markdown(&display);cache.insert(id.clone(),(raw.to_string(),display,rendered));}
            let (_,display,rendered)=&cache[&id];
            messages.push(json!({"id":text(m,"id"),"role":m["role"],"streaming":m["streaming"]==true,"text":display,"attachments":m["attachments"].as_array().cloned().unwrap_or_default(),"rendered":rendered,"fileReferences":file_references(display),"createdAt":epoch(&m["createdAt"])}));
        }
        let retained=messages.iter().map(|m|text(m,"id").to_string()).collect::<HashSet<_>>();cache.retain(|id,_|retained.contains(id));
        let p=&d["projection"];let active=active_run_id(p);let status=p["runs"].as_array().and_then(|r|r.last()).map(|r|text(r,"status")).unwrap_or("idle");
        let started=p["runs"].as_array().into_iter().flatten().find(|r|text(r,"id")==active).map(|r|epoch(&r["startedAt"]).max(epoch(&r["createdAt"]))).unwrap_or(0);
        Ok(
            json!({"threadState":{"working":!active.is_empty(),"startedAt":started,"activeRunId":if active.is_empty(){Value::Null}else{json!(active)},"status":status},"questions":protocol::questions::pending(&d["projection"]),"tasks":protocol::tasks::derive(&d["projection"]),"queue":queue,"timeline":conversation_timeline(&d["projection"],&messages),"messages":messages,"activity":live_activity(&d["projection"]),"hasMoreHistory":truth(&d["hasMoreHistory"]),"payloadBudgetExceeded":truth(&d["payloadBudgetExceeded"])}),
        )
}

/// Read only the desktop client's grouping preferences; never touch its store.
pub fn native_project_grouping()->Value{
 let p=t3_home().join("userdata/client-settings.json");let mut out=json!({"mode":"repository","overrides":{}});
 if let Ok(f)=std::fs::File::open(p){if let Ok(v)=serde_json::from_reader::<_,Value>(f.take(1024*1024)){
  if matches!(v["sidebarProjectGroupingMode"].as_str(),Some("repository"|"repository_path"|"separate")){out["mode"]=v["sidebarProjectGroupingMode"].clone()}
  if let Some(m)=v["sidebarProjectGroupingOverrides"].as_object(){for(k,v)in m.iter().take(512){if k.len()<=2200&&matches!(v.as_str(),Some("repository"|"repository_path"|"separate")){out["overrides"][k]=v.clone()}}}
 }}out
}
pub fn is_settled(t: &Value) -> bool {
    t["settledOverride"] == "settled" || t["settledOverride"] == true
        || (t["settledOverride"] != "active" && truth(&t["settledAt"]))
}
/// Summaries of real in-flight turn items; never expose tool inputs or reasoning bodies.
pub fn active_run_id(p:&Value)->&str {p["runs"].as_array().into_iter().flatten().rev().find(|r|matches!(text(r,"status"),"preparing"|"starting"|"running"|"waiting")).map(|r|text(r,"id")).unwrap_or_else(||if p["runs"].as_array().is_some_and(|r|!r.is_empty()){""}else{text(&p["thread"],"activeRunId")} )}
pub fn live_activity(projection: &Value) -> Value {
    let run=active_run_id(projection);
    json!(projection["turnItems"].as_array().into_iter().flatten().filter_map(protocol::activity::describe)
      .filter(|a| !run.is_empty() && text(a,"runId")==run && a["active"]==true).rev().take(3).collect::<Vec<_>>())
}
pub fn conversation_timeline(projection:&Value,messages:&[Value])->Vec<Value>{
 let mut rows=messages.to_vec();
 let first=messages.first().map(|m|epoch(&m["createdAt"])).unwrap_or(0);
 let items=projection["turnItems"].as_array().map(Vec::as_slice).unwrap_or(&[]);
 for row in &items[items.len().saturating_sub(600)..]{
  let native=protocol::activity::item(row);
  if let Some(a)=protocol::documents::html_render(native){let time=epoch(&native["startedAt"]).max(epoch(&row["createdAt"]));if time>=first{rows.push(json!({"id":format!("html/{}",text(native,"id")),"role":"assistant","createdAt":time,"text":"","attachments":[a]}))}}
  if let Some(boundary)=protocol::lifecycle::describe(row,projection["runs"].as_array().map(Vec::as_slice).unwrap_or(&[])){
   let time=epoch(&native["startedAt"]).max(epoch(&row["createdAt"]));
   if time>=first{rows.push(json!({"id":format!("context/{}",text(native,"id")),"role":"context","createdAt":time,"context":boundary,"text":""}))}continue
  }
  let Some(mut activity)=protocol::activity::describe(row) else{continue};
  let item=protocol::activity::item(row);let time=epoch(&item["startedAt"]).max(epoch(&row["createdAt"]));
  if time<first{continue}
  activity["active"]=json!(activity["active"]==true && text(&activity,"runId")==active_run_id(projection) && !active_run_id(projection).is_empty());
  rows.push(json!({"id":format!("work/{}",text(item,"id")),"role":"activity","createdAt":time,"activity":activity,"text":""}));
 }
 for c in protocol::changes::summaries(projection){let time=epoch(&c["capturedAt"]);if time>=first{rows.push(json!({"id":format!("changes/{}",text(&c,"id")),"role":"changes","createdAt":time,"changes":c,"text":""}))}}
 rows.sort_by_key(|r|epoch(&r["createdAt"]));
 let mut grouped:Vec<Value>=vec![];
 for row in rows{
  if let Some(last)=grouped.last_mut(){
   if row["role"]=="activity"&&last["role"]=="activity"&&row["activity"]["runId"]==last["activity"]["runId"]{
    let count=last["activity"]["count"].as_u64().unwrap_or(1)+1;
    let mut entries=last["activity"]["entries"].as_array().cloned().unwrap_or_else(||vec![last["activity"].clone()]);if entries.len()<600{entries.push(row["activity"].clone())}
    let id=last["id"].clone();let active=if last["activity"]["active"]==true && row["activity"]["active"]!=true {Some(last["activity"].clone())}else{None};*last=row;if let Some(a)=active{last["activity"]=a;}last["id"]=id;last["activity"]["count"]=json!(count);last["activity"]["entries"]=json!(entries);continue
   }
  }grouped.push(row)
 }grouped
}
pub fn normalize(d: &Value, env: &str, now: i64) -> Result<Vec<Value>> {
    let rows = d["threads"].as_array().ok_or("malformed")?;
    let projects: HashMap<&str, &str> = d["projects"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| (text(p, "id"), text(p, "title")))
        .collect();
    let active = |s: &str| ["preparing", "queued", "starting", "running"].contains(&s);
    let mut result = vec![];
    let mut ids = HashSet::new();
    for t in rows {
        let id = text(t, "id");
        if !valid_id(id) || !ids.insert(id) || !t["title"].is_string() {
            return Err("malformed".into());
        }
        if truth(&t["deletedAt"]) || truth(&t["archivedAt"]) {
            continue;
        }
        let status = t
            .get("status")
            .unwrap_or(&Value::Null)
            .as_str()
            .unwrap_or("unknown");
        if !t["status"].is_null() && !t["status"].is_string() {
            return Err("malformed".into());
        }
        let working = active(status)
            || active(text(t, "activityRunStatus"))
            || truth(&t["pendingBackgroundTasks"]);
        let mut reason = if !t["pendingRuntimeRequest"].is_null() {
            "request"
        } else if truth(&t["hasActionableProposedPlan"]) {
            "plan"
        } else if status == "waiting" {
            "waiting"
        } else if status == "failed" {
            "failed"
        } else {
            ""
        };
        let completed = epoch(&t["latestRunCompletedAt"]);
        if reason.is_empty()
            && !working
            && status == "completed"
            && completed > epoch(&t["lastVisitedAt"])
        {
            reason = "review";
        }
        if is_settled(t) {
            reason = "";
        }
        let snoozed = epoch(&t["snoozedUntil"]) > now;
        let req = &t["pendingRuntimeRequest"];
        let request_id = if truth(req) {
            req["id"]
                .as_str()
                .filter(|s| !s.is_empty())
                .or_else(|| req["requestId"].as_str())
                .unwrap_or("pending")
        } else {
            ""
        };
        let stamp = if completed != 0 {
            completed.to_string()
        } else if !text(t, "latestRunId").is_empty() {
            String::new()
        } else {
            t["updatedAt"]
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| t["updatedAt"].to_string())
        };
        let revision = format!(
            "{:x}",
            Sha256::digest(
                [reason, text(t, "latestRunId"), request_id, &stamp]
                    .join("\0")
                    .as_bytes()
            )
        );
        let project = text(t, "projectId");
        let mut row=json!({"id":id,"environmentId":env,"key":format!("{env}/{id}"),"title":t["title"],"project":projects.get(project).copied().unwrap_or(project),"status":status,"working":working,"pending":!reason.is_empty()&&!snoozed,"reason":if reason.is_empty(){Value::Null}else{json!(reason)},"snoozed":snoozed,"updatedAt":epoch(&t["updatedAt"]),"revision":revision,"parentThreadId":t["lineage"]["parentThreadId"]});
        if let Some(relationship)=t["lineage"]["relationshipToParent"].as_str(){row["relationshipToParent"]=json!(relationship);}
        result.push(row);
    }
    result.sort_by(|a, b| {
        epoch(&b["updatedAt"])
            .cmp(&epoch(&a["updatedAt"]))
            .then_with(|| text(a, "key").cmp(text(b, "key")))
    });
    Ok(result)
}
// Keep T3 monogram geometry; all color choices belong to the live Omarchy theme.
pub fn project_identity(name: &str) -> String {
    let words: Vec<&str> = name
        .trim()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect();
    let first = words.first().copied().unwrap_or("PR");
    let initial = first.chars().next().unwrap_or('P');
    let second = first
        .chars()
        .skip(1)
        .find(|c| c.is_numeric())
        .or_else(|| {
            if words.len() > 1 {
                words.last().and_then(|s| s.chars().next())
            } else {
                first.chars().last()
            }
        })
        .unwrap_or(initial);
    let monogram = format!("{initial}{second}")
        .to_uppercase()
        .chars()
        .take(2)
        .collect();
    monogram
}
pub fn snapshot(client: &Client) -> Result<Value> {
    let mut threads = vec![];
    let mut states = vec![];
    let mut projects = vec![];
    for e in environments()? {
        match client
            .get(&e, "/api/orchestration/shell", true)
            .and_then(|d| {
                for p in d["projects"].as_array().into_iter().flatten() {
                    projects.push(json!({"id":p["id"],"environmentId":e.id,"key":format!("{}/{}",e.id,text(p,"id")),"title":p["title"],"workspaceRoot":p["workspaceRoot"],"repositoryIdentity":p["repositoryIdentity"],"defaultThreadEnvMode":p["defaultThreadEnvMode"],"autoPull":p["autoPull"],"isScratch":p["isScratch"],"defaultModelSelection":p["defaultModelSelection"]}));
                }
                let mut items = normalize(&d, &e.id, Utc::now().timestamp_millis())?;
                let raw: HashMap<&str, &Value> = d["threads"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|t| (text(t, "id"), t))
                    .collect();
                for item in &mut items {
                    if let Some(t) = raw.get(text(item, "id")) {
                        for (key,value) in protocol::sidebar::metadata(t,Utc::now().timestamp_millis()).as_object().unwrap(){item[key]=value.clone();}
                        if item["snoozed"]==false && !text(item,"reason").is_empty(){item["pending"]=json!(true);}
                        item["compactActivityAt"]=json!(epoch(&t["latestUserAuthoredMessageAt"]).max(epoch(&t["latestRunCompletedAt"])));
                        item["compactRunId"]=t["latestRunId"].clone();
                        item["lastSentAt"]=json!(epoch(t.get("latestUserAuthoredMessageAt").unwrap_or(&t["latestRunRequestedAt"])).max(epoch(&t["createdAt"])));
                        item["environmentName"] = json!(e.name);
                        item["projectIsScratch"]=json!(d["projects"].as_array().into_iter().flatten().find(|p|p["id"]==t["projectId"]).is_some_and(|p|p["isScratch"]==true));
                        item["projectId"] = t["projectId"].clone();
                        item["projectKey"] = json!(format!("{}/{}",e.id,text(t,"projectId")));
                        item["settled"] = json!(is_settled(t));
                        item["pinned"] = json!(truth(&t["pinnedAt"]));
                        item["startedAt"] = json!(epoch(&t["activityRunStartedAt"]).max(epoch(&t["latestRunStartedAt"])));
                        item["provider"] = t["providerInstanceId"].clone();
                        item["branch"] = t["branch"].clone();
                        item["model"] = t["modelSelection"]["model"].clone();
                        for field in ["modelSelection", "runtimeMode", "interactionMode", "activeRunId","worktreePath","pinnedAt","pinOrderKey","autoSettleDisabledAt","titleRegeneration"] { item[field] = t[field].clone(); }
                        let monogram = project_identity(text(item, "project"));
                        item["projectMonogram"] = json!(monogram);
                    }
                }
                Ok(items)
            }) {
            Ok(mut items) => {
                threads.append(&mut items);
                states.push(json!({"id":e.id,"name":e.name,"state":"ready"}));
            }
            Err(err) => {
                if ["unreachable","unauthorized"].contains(&err.as_str()) { connections::connection_failed(&e.id); }
                states.push(json!({"id":e.id,"name":e.name,"state":err}));
            },
        }
    }
    threads.sort_by(|a, b| {
        b["updatedAt"]
            .as_i64()
            .cmp(&a["updatedAt"].as_i64())
            .then_with(|| text(a, "key").cmp(text(b, "key")))
    });
    let working = threads.iter().filter(|t| t["working"] == true).count();
    let pending = threads.iter().filter(|t| t["pending"] == true).count();
    Ok(
        json!({"protocol":"omarchy.t3.v1","observedAt":Utc::now().timestamp_millis(),"stale":states.iter().any(|e|e["state"]!="ready"),"environments":states,"nativeProjectGrouping":native_project_grouping(),"projects":projects,"threads":threads,"working":working,"pending":pending}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_contract_vectors() {
        let vectors: Value =
            serde_json::from_str(include_str!("../docs/protocol/fixtures/shell.json")).unwrap();
        for v in vectors.as_array().unwrap() {
            assert_eq!(
                json!(normalize(&v["shell"], "fixture", v["nowMs"].as_i64().unwrap()).unwrap()),
                v["expected"],
                "{}",
                v["name"]
            );
        }
    }
    #[test]
    fn lineage_remains_available_without_conflating_forks_and_subagents() {
        let shell=json!({"threads":[
            {"id":"root","title":"Root"},
            {"id":"child","title":"Child","lineage":{"parentThreadId":"root","relationshipToParent":"subagent"}},
            {"id":"fork","title":"Fork","lineage":{"parentThreadId":"root","relationshipToParent":"fork"}}
        ]});
        let rows=normalize(&shell,"fixture",0).unwrap();assert_eq!(rows.len(),3);
        assert_eq!(rows.iter().find(|r|r["id"]=="child").unwrap()["relationshipToParent"],"subagent");
        assert_eq!(rows.iter().find(|r|r["id"]=="fork").unwrap()["relationshipToParent"],"fork");
    }
    #[test]
    fn live_state_keeps_run_clock_and_latest_tool_context(){
      let base=1700000000000_i64;let mut items=vec![];for n in 0..650 {items.push(json!({"id":format!("t{n}"),"runId":"r","type":"command_execution","input":"rg file","status":if n==649{"running"}else{"completed"},"startedAt":base+1000+n}));}
      let p=json!({"thread":{"activeRunId":"r"},"runs":[{"id":"r","status":"running","createdAt":base+500,"startedAt":base+600}],"messages":[{"id":"m","role":"user","text":"hello","createdAt":base+500}],"turnItems":items});
      let view=normalize_messages(&json!({"projection":p})).unwrap();
      assert_eq!(view["threadState"]["startedAt"],base+600);assert_eq!(view["activity"][0]["command"],"rg");
      let rows=view["timeline"].as_array().unwrap();let last=rows.last().unwrap();assert_eq!(last["activity"]["active"],true);assert_eq!(last["activity"]["entries"].as_array().unwrap().len(),600);
    }
    #[test]
    fn settlement_and_live_activity_do_not_invent_work() {
        assert!(is_settled(&json!({"settledOverride":"settled"})));
        assert!(!is_settled(&json!({"settledOverride":"active","settledAt":"2026-10-15T00:00:00Z"})));
        let p=json!({"thread":{"activeRunId":"run-one"},"turnItems":[
            {"id":"old","runId":"run-old","type":"command_execution","status":"running"},
            {"id":"finished","runId":"run-one","type":"command_execution","status":"completed"},
            {"id":"live","runId":"run-one","type":"command_execution","status":"running","input":"private command must not be exposed","title":"Build"}
        ]});
        let activity=live_activity(&p);
        assert_eq!(activity.as_array().unwrap().len(),1);
        assert_eq!(activity[0]["id"],"live");
        assert!(!activity.to_string().contains("private command"));
        let mut stopped=p.clone();stopped["thread"]["activeRunId"]=Value::Null;
        assert_eq!(live_activity(&stopped),json!([]));
    }
    #[test]
    fn contextual_timeline_keeps_active_work_and_message_order() {
        let base=1792036800000i64;
        let messages=vec![json!({"id":"u","role":"user","createdAt":base}),json!({"id":"a","role":"assistant","createdAt":base+4000})];
        let p=json!({"thread":{"activeRunId":"run"},"turnItems":[
          {"id":"read","runId":"run","type":"command_execution","status":"running","startedAt":base+1000,"input":"cat secret.txt"},
          {"id":"done","runId":"run","type":"command_execution","status":"completed","startedAt":base+2000,"output":"private output"}
        ]});
        let rows=conversation_timeline(&p,&messages);
        assert_eq!(rows.len(),3);assert_eq!(rows[0]["id"],"u");assert_eq!(rows[2]["id"],"a");
        assert_eq!(rows[1]["activity"]["category"],"read");assert_eq!(rows[1]["activity"]["active"],true);assert_eq!(rows[1]["activity"]["count"],2);assert_eq!(rows[1]["activity"]["command"],"cat");assert_eq!(rows[1]["activity"]["entries"].as_array().unwrap().len(),2);
        assert!(!json!(rows).to_string().contains("secret.txt"));assert!(!json!(rows).to_string().contains("private output"));
    }
    #[test]
    fn dates_and_path_boundaries() {
        assert_eq!(epoch(&json!(1792036800)), 1792036800000);
        assert_eq!(epoch(&json!("2026-10-15T04:00:00Z")), 1792036800000);
        assert_eq!(epoch(&json!(true)), 0);
        assert!(!valid_id("../escape"));
        assert!(valid_id("thread:command%3Ax"));
        assert!(validate_base("http://user:secret@host/").is_err());
        assert!(validate_base("https://host/path").is_err());
    }
}
