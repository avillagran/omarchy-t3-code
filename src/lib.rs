pub mod quota_sources;
pub mod notification_icon;
pub mod backend_service;
pub mod images;
pub mod markdown_images;
pub mod documents;
pub mod scroll;
pub mod message_text;
pub mod desktop_open;
pub mod terminal;
pub mod clipboard;
mod desktop_cloud;
mod connections;
pub mod connection_access;
pub mod connection_network;
pub mod connection_preferences;
pub mod connection_cloud;
pub mod connection_route;
pub mod connection_github;
pub mod native_settings;
pub mod provider_instances;
pub mod scoped_settings;
pub mod plan_workflow;
pub mod checkpoint_revert;
pub mod checkpoint_journal;
pub mod checkpoint_operations;
pub mod drafts;
pub mod prompt_recall;
pub mod git_controls;
pub mod branch_picker;
pub mod latex;
pub mod thread_search;
pub mod pull_request_actions;
pub mod usage_summary;
pub mod scheduled_tasks;
pub mod storage_cleanup;
pub mod mcp_apps;
pub mod provider_setup;
pub mod agent_sessions;
pub mod process_diagnostics;
pub mod local_servers;
pub mod git_operations;
pub mod diff_review;
pub mod working_review;
pub mod review_document;
pub mod archive_management;
pub mod bounded_snapshot;
pub mod keybinding_settings;
pub mod third_party_licenses;
pub mod integration_devices;
pub mod tool_inspector;
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
pub fn rendered_markdown(input:&str)->String{markdown_with_code_actions(input,None)}
/// T3's mathRenderingEnabled client setting (off by default), re-read when the file changes.
pub fn math_enabled()->bool{
    static CACHE:std::sync::Mutex<Option<(std::time::SystemTime,bool)>>=std::sync::Mutex::new(None);
    let path=t3_home().join("userdata/client-settings.json");
    let Ok(modified)=fs::metadata(&path).and_then(|m|m.modified()) else {return false};
    let mut cache=CACHE.lock().unwrap();
    if let Some((at,value))=*cache{if at==modified{return value}}
    let value=connection_preferences::client_settings().map(|s|s["mathRenderingEnabled"]==true).unwrap_or(false);
    *cache=Some((modified,value));value
}
fn markdown_with_code_actions(input: &str, blocks:Option<&[Value]>) -> String {
    markdown_render(input,blocks,math_enabled())
}
pub fn markdown_render(input: &str, blocks:Option<&[Value]>, math:bool) -> String {
    use pulldown_cmark::{html, Event, Options, Parser, Tag, TagEnd};
    let mut image_index=0;
    let normalized;
    let input=if math{normalized=latex::normalize_delimiters(input);normalized.as_str()}else{input};
    let options=Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | if math{Options::ENABLE_MATH}else{Options::empty()};
    let events = Parser::new_ext(
        input,
        options,
    )
    .filter_map(|event| match event {
        Event::InlineMath(tex) => Some(Event::Html(latex::to_rich_text(&tex,false).into())),
        Event::DisplayMath(tex) => Some(Event::Html(latex::to_rich_text(&tex,true).into())),
        Event::Start(Tag::Image { .. }) if blocks.is_some() => {
            let marker=format!("<a href=\"ot3-image-open://{image_index}\">");image_index+=1;Some(Event::Html(marker.into()))
        },
        Event::End(TagEnd::Image) if blocks.is_some() => Some(Event::Html("</a>".into())),
        Event::Start(Tag::Image { .. }) | Event::End(TagEnd::Image) => None,
        Event::Start(Tag::Link { link_type, dest_url, title, id }) => {
            let url=dest_url.to_ascii_lowercase();
            let safe=!url.contains('\0') && (!url.contains(':') || ["http://","https://","file://","t3-thread://"].iter().any(|s|url.starts_with(s)));
            Some(Event::Start(Tag::Link {link_type,dest_url:if safe {dest_url}else{"#".into()},title,id}))
        },
        Event::Html(value) | Event::InlineHtml(value) => Some(Event::Text(value)),
        other => Some(other),
    });
    let mut out = String::new();
    let mut index=0;
    let mut table_index=0;
    let events=events.flat_map(|event|{let mut batch=Vec::new();
        // Copy actions after each table, as in T3's table menu (Markdown or CSV).
        if blocks.is_some()&&matches!(&event,Event::End(TagEnd::Table)){batch.push(event);batch.push(Event::Html(format!("<p align=\"right\"><a href=\"ot3-table-copy://{table_index}/markdown\">Copy as Markdown</a> · <a href=\"ot3-table-copy://{table_index}/csv\">Copy as CSV</a></p>").into()));table_index+=1;return batch}
        if matches!(&event,Event::Start(Tag::CodeBlock(_))){if let Some(block)=blocks.and_then(|b|b.get(index)){let run=if block["command"].is_string(){format!("<a href=\"ot3-code-run://{index}\">Run in terminal</a> · ")}else if text(block,"language").eq_ignore_ascii_case("mermaid"){format!("<a href=\"ot3-code-diagram://{index}\">Show diagram</a> · ")}else{String::new()};batch.push(Event::Html(format!("<table width=\"100%\"><tr><td>{}</td><td align=\"right\">{run}<a href=\"ot3-code-copy://{index}\">Copy code</a></td></tr></table>",text(block,"language").replace('&',"&amp;").replace('<',"&lt;").replace('>',"&gt;")).into()));}index+=1;}batch.push(event);batch});
    html::push_html(&mut out, events);
    out
}
/// Tables of a Markdown message as Markdown and CSV, for the copy actions.
pub fn markdown_tables(input:&str)->Vec<Value>{
    use pulldown_cmark::{Event,Options,Parser,Tag,TagEnd};
    let mut tables=vec![];let mut rows:Vec<Vec<String>>=vec![];let mut row:Vec<String>=vec![];let mut cell=String::new();let mut inside=false;
    for event in Parser::new_ext(input,Options::ENABLE_TABLES|Options::ENABLE_STRIKETHROUGH){
        match event{
            Event::Start(Tag::Table(_))=>{inside=true;rows.clear();}
            Event::End(TagEnd::TableCell)=>{row.push(cell.trim().to_string());cell.clear();}
            Event::End(TagEnd::TableHead)|Event::End(TagEnd::TableRow)=>{rows.push(std::mem::take(&mut row));}
            Event::End(TagEnd::Table)=>{inside=false;
                let md=rows.iter().enumerate().map(|(i,r)|{let line=format!("| {} |",r.iter().map(|c|c.replace('|',"\\|")).collect::<Vec<_>>().join(" | "));if i==0{format!("{line}\n|{}|",r.iter().map(|_|" --- ").collect::<Vec<_>>().join("|"))}else{line}}).collect::<Vec<_>>().join("\n");
                let csv=rows.iter().map(|r|r.iter().map(|c|if c.contains([',','"','\n']){format!("\"{}\"",c.replace('"',"\"\""))}else{c.clone()}).collect::<Vec<_>>().join(",")).collect::<Vec<_>>().join("\n");
                tables.push(json!({"markdown":md,"csv":csv}));
            }
            Event::Text(t)|Event::Code(t) if inside=>cell.push_str(&t),
            Event::SoftBreak|Event::HardBreak if inside=>cell.push(' '),
            _=>{}
        }
    }
    tables
}
pub fn markdown_search_text(input:&str)->String{
    use pulldown_cmark::{Event,Options,Parser,TagEnd};
    let mut out=String::new();
    for event in Parser::new_ext(input,Options::ENABLE_TABLES|Options::ENABLE_STRIKETHROUGH){
        match event{
            Event::Text(s)|Event::Code(s)|Event::Html(s)|Event::InlineHtml(s)=>out.push_str(&s),
            Event::SoftBreak|Event::HardBreak|Event::End(TagEnd::Paragraph|TagEnd::Heading(_)|TagEnd::CodeBlock|TagEnd::Item|TagEnd::TableCell|TagEnd::TableRow)=>out.push('\n'),
            _=>{}
        }
    }
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
    pub fn bounded_thread(&self, e: &Environment, id: &str) -> Result<Value> {
        if !valid_id(id) { return Err("invalid_id".into()); }
        let mut u = reqwest::Url::parse(e.base_url.trim_end_matches('/')).map_err(|_| "invalid_endpoint")?;
        u.path_segments_mut().map_err(|_| "invalid_endpoint")?.extend(["api", "orchestration", "threads", id, "bounded"]);
        // Native opt-in removes duplicated local timeline items without dropping history.
        u.query_pairs_mut().append_pair("compactTurnItems", "1");
        let path = format!("{}?{}", u.path(), u.query().unwrap());
        bounded_snapshot::restore(self.get(e, &path, true)?)
    }
    pub fn messages(&self, e: &Environment, id: &str) -> Result<Value> {
        normalize_messages(&self.bounded_thread(e, id)?)
    }
}
pub type RenderCache=HashMap<String,(String,String,String,String,Vec<Value>,Vec<Value>)>;
pub fn normalize_messages(d:&Value)->Result<Value>{normalize_messages_cached(d,&mut RenderCache::new())}
pub fn normalize_messages_cached(d:&Value,cache:&mut RenderCache)->Result<Value>{
        let rows = d["projection"]["messages"].as_array().ok_or("malformed")?;
        let rolled=rolled_back_runs(&d["projection"]);
        let queue=protocol::queue::derive(&d["projection"]);
        let hidden=queue["hiddenMessageIds"].as_array().unwrap();
        let mut messages = vec![];
        let math=math_enabled();
        for m in rows {
            if hidden.contains(&m["id"])||rolled.contains(text(m,"runId")){continue}
            if !m["text"].is_string() || !m["role"].is_string() {
                return Err("malformed".into());
            }
            let (attribution,raw)=prompt_recall::attribution(m);let id=text(m,"id").to_string();
            let cache_raw=if math{format!("\u{1}{raw}")}else{raw.to_string()};if !cache.get(&id).is_some_and(|c|c.0==cache_raw){let display=message_text::plain_qt(raw).unwrap_or_else(||raw.to_string());let blocks=protocol::code_blocks::parse(&display);let rendered=markdown_with_code_actions(&display,Some(&blocks));let search=markdown_search_text(&display);let images=markdown_images::descriptors(&display);cache.insert(id.clone(),(cache_raw,display,rendered,search,blocks,images));}
            let (_,display,rendered,search,blocks,images)=&cache[&id];
            messages.push(json!({"id":text(m,"id"),"role":m["role"],"revertReference":if m["role"]=="user"{checkpoint_revert::reference(&d["projection"],text(m,"id")).ok()}else{None},"canRewindMessage":checkpoint_revert::can_preview(&d["projection"],m),"recallableText":if m["role"]=="user"&&attribution.is_none(){json!(prompt_recall::recall(raw))}else{Value::Null},"attribution":attribution,"forkRunId":if m["role"]=="assistant"&&m["streaming"]!=true&&m["runId"].is_string(){m["runId"].clone()}else{Value::Null},"streaming":m["streaming"]==true,"text":display,"searchText":if m["role"]=="user"{display}else{search},"attachments":m["attachments"].as_array().cloned().unwrap_or_default(),"rendered":rendered,"markdownImages":if m["role"]=="assistant"{images.clone()}else{vec![]},"fileReferences":file_references(display),"codeBlocks":blocks,"tables":if m["role"]=="assistant"&&display.contains('|'){json!(markdown_tables(display))}else{json!([])},"createdAt":epoch(&m["createdAt"])}));
        }
        // One fork action per run: on its last response.
        let mut seen_runs=HashSet::new();for m in messages.iter_mut().rev(){if let Some(run)=m["forkRunId"].as_str().map(str::to_string){if !seen_runs.insert(run){m["forkRunId"]=Value::Null;}}}
        let retained=messages.iter().map(|m|text(m,"id").to_string()).collect::<HashSet<_>>();cache.retain(|id,_|retained.contains(id));
        let p=&d["projection"];let active=active_run_id(p);let status=p["runs"].as_array().and_then(|r|r.last()).map(|r|text(r,"status")).unwrap_or("idle");
        let started=p["runs"].as_array().into_iter().flatten().find(|r|text(r,"id")==active).map(|r|epoch(&r["startedAt"]).max(epoch(&r["createdAt"]))).unwrap_or(0);
        let mut sorting=p["thread"].as_object().cloned().map(Value::Object).unwrap_or_else(||json!({}));
        sorting["latestRun"]=p["runs"].as_array().and_then(|r|r.last()).cloned().unwrap_or(Value::Null);
        if sorting["latestRun"].is_object()&&sorting["latestRun"].get("requestedAt").is_none(){sorting["latestRun"]["requestedAt"]=sorting["latestRun"]["createdAt"].clone();}
        let mut thread_state=json!({"working":!active.is_empty(),"startedAt":started,"activeRunId":if active.is_empty(){Value::Null}else{json!(active)},"status":status,"providerInstanceId":p["thread"]["providerInstanceId"],"interactionMode":p["thread"]["interactionMode"],"latestRunSettled":plan_workflow::latest_run_settled(p)});
        for (key,value) in protocol::sidebar::sort_timestamps(&sorting).as_object().unwrap(){if key!="lastSentAt"&&value.as_i64().unwrap_or(0)>0{thread_state[key]=value.clone();}}
        for (key,value) in thread_workflows(p).as_object().unwrap(){thread_state[key]=value.clone();}
        thread_state["contextWindow"]=context_window(p);
        // Subagents still working for this thread (T3's subagent bar).
        thread_state["subagents"]=json!(p["subagents"].as_array().into_iter().flatten().filter(|s|matches!(text(s,"status"),"pending"|"running"|"waiting")).take(16).map(|s|json!({"id":s["id"],"runId":s["runId"],"title":s["title"].as_str().filter(|t|!t.trim().is_empty()).unwrap_or_else(||text(s,"prompt")).chars().take(120).collect::<String>(),"status":s["status"],"childThreadId":s["childThreadId"],"origin":s["origin"]})).collect::<Vec<_>>());
        Ok(json!({"threadState":thread_state,"approvals":protocol::approvals::pending(&d["projection"]),"questions":protocol::questions::pending(&d["projection"]),"tasks":protocol::tasks::derive(&d["projection"]),"queue":queue,"planView":plan_workflow::view(p,rendered_markdown),"timeline":conversation_timeline(&d["projection"],&messages),"messages":messages,"activity":live_activity(&d["projection"]),"historyCursor":d["historyCursor"],"hasMoreHistory":truth(&d["hasMoreHistory"]),"payloadBudgetExceeded":truth(&d["payloadBudgetExceeded"])}))
}

/// Port of T3 contextWindow.ts: the newest provider turn usage, else the active provider
/// thread's context usage, else the last compaction's token count.
pub fn context_window(p:&Value)->Value{
    let num=|v:&Value|v.as_f64().filter(|n|n.is_finite());
    let build=|used:f64,max:Option<f64>,u:&Value|{
        let pct=max.filter(|m|*m>0.0).map(|m|(used/m*100.0).min(100.0));
        json!({"usedTokens":used.max(0.0),"maxTokens":max,"usedPercentage":pct,"remainingTokens":max.map(|m|(m-used).round().max(0.0)),"inputTokens":num(&u["inputTokens"]),"cachedInputTokens":num(&u["cachedInputTokens"]),"outputTokens":num(&u["outputTokens"]),"reasoningOutputTokens":num(&u["reasoningOutputTokens"]),"totalProcessedTokens":num(&u["totalProcessedTokens"]),"compactsAutomatically":u.get("compactsAutomatically").cloned().unwrap_or(json!(true)),"autoCompactThreshold":num(&u["autoCompactThreshold"]),"cost":num(&u["cost"])})
    };
    let live=p["providerTurns"].as_array().into_iter().flatten().map(|t|&t["tokenUsage"]).filter(|u|u.is_object())
        .fold(None::<&Value>,|best,u|match best{Some(b) if protocol::compaction::timestamp(&u["updatedAt"])<protocol::compaction::timestamp(&b["updatedAt"])=>Some(b),_=>Some(u)});
    if let Some(u)=live{if let Some(used)=num(&u["usedTokens"]){let mut out=build(used,num(&u["maxTokens"]),u);out["totalProcessedTokens"]=Value::Null;out["autoCompactThreshold"]=Value::Null;out["cost"]=Value::Null;out["compactsAutomatically"]=json!(true);return out}}
    let active=&p["thread"]["activeProviderThreadId"];
    if let Some(u)=p["providerThreads"].as_array().into_iter().flatten().find(|t|!active.is_null()&&&t["id"]==active).map(|t|&t["contextUsage"]).filter(|u|u.is_object()){
        if let Some(used)=num(&u["usedTokens"]){return build(used,num(&u["maxTokens"]),u)}
    }
    if let Some(c)=p["turnItems"].as_array().into_iter().flatten().rev().find(|i|i["type"]=="compaction"&&num(&i["afterTokenCount"]).is_some_and(|n|n>=0.0)){
        let mut out=build(num(&c["afterTokenCount"]).unwrap(),None,&Value::Null);out["totalProcessedTokens"]=json!(num(&c["beforeTokenCount"]));out["compactsAutomatically"]=json!(true);return out
    }
    Value::Null
}
/// Ports of T3 threadWorkflows/threadRelationships: where a fork merges back and
/// which finished run it would carry.
pub fn thread_workflows(p:&Value)->Value{
    let thread=&p["thread"];
    let target=if thread["lineage"]["relationshipToParent"]!="fork"{Value::Null}else if thread["forkedFrom"]["type"]=="run"{thread["forkedFrom"]["threadId"].clone()}else{thread["lineage"]["parentThreadId"].clone()};
    let runs=p["runs"].as_array().cloned().unwrap_or_default();
    let ordinal=|r:&Value|r["ordinal"].as_i64().unwrap_or(-1);
    let latest=runs.iter().filter(|r|matches!(text(r,"status"),"waiting"|"completed")).max_by_key(|r|ordinal(r));
    let run=latest.filter(|l|!runs.iter().any(|r|ordinal(r)>ordinal(l)&&matches!(text(r,"status"),"preparing"|"starting"|"running"))).map(|r|r["id"].clone()).unwrap_or(Value::Null);
    json!({"mergeBackTargetThreadId":target,"mergeBackRunId":run})
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
fn inspectable_activity(row:&Value)->Option<Value>{
    let mut activity=protocol::activity::describe(row)?;
    if let Ok(reference)=tool_inspector::compact_reference(row){activity["inspector"]=reference;}
    Some(activity)
}
pub fn live_activity(projection: &Value) -> Value {
    let run=active_run_id(projection);
    json!(projection["turnItems"].as_array().into_iter().flatten().filter_map(inspectable_activity)
      .filter(|a| !run.is_empty() && text(a,"runId")==run && a["active"]==true).rev().take(3).collect::<Vec<_>>())
}
fn rolled_back_runs(projection:&Value)->HashSet<String>{projection["runs"].as_array().into_iter().flatten().filter(|r|r["status"]=="rolled_back").filter_map(|r|r["id"].as_str().map(str::to_owned)).collect()}
pub fn conversation_timeline(projection:&Value,messages:&[Value])->Vec<Value>{
 let rolled=rolled_back_runs(projection);
 let mut rows=messages.to_vec();
 let first=messages.first().map(|m|epoch(&m["createdAt"])).unwrap_or(0);
 let plan_view=plan_workflow::view(projection,rendered_markdown);
 let items=projection["turnItems"].as_array().map(Vec::as_slice).unwrap_or(&[]);
 for row in &items[items.len().saturating_sub(600)..]{
  let native=protocol::activity::item(row);
  if rolled.contains(text(native,"runId")){continue}
  if native["type"]=="proposed_plan" {
   if let Some(plan)=plan_view["plans"].as_array().into_iter().flatten().find(|p|p["id"]==native["planId"]){
    let time=epoch(&native["startedAt"]).max(epoch(&native["updatedAt"]));
    if time>=first{rows.push(json!({"id":format!("plan/{}",text(native,"id")),"role":"plan","createdAt":time,"plan":plan,"text":""}));}continue
   }
  }

  if native["type"]=="secret_request"{
   // An agent's request_secret: answered here, the value goes to the server only.
   let time=epoch(&native["startedAt"]).max(epoch(&row["createdAt"]));
   let local=native["threadId"].is_null()||native["threadId"]==projection["thread"]["id"];
   if time>=first{rows.push(json!({"id":format!("secret/{}",text(native,"id")),"role":"secret","createdAt":time,"text":"","secret":{"turnItemId":native["id"],"threadId":native["threadId"],"label":native["label"],"reason":native["reason"],"placeholder":native["placeholder"],"status":native["secretStatus"],"local":local}}))}
   continue
  }
  if let Some(mut a)=protocol::documents::mcp_app(native){
   // An MCP app is drawn like an HTML render; its requests go back through mcpApps.*.
   if a["mcpApp"]["threadId"].is_null(){a["mcpApp"]["threadId"]=projection["thread"]["id"].clone();}
   let time=epoch(&native["startedAt"]).max(epoch(&row["createdAt"]));
   if time>=first{rows.push(json!({"id":format!("app/{}",text(native,"id")),"role":"assistant","createdAt":time,"text":"","attachments":[a]}))}
  }
  if let Some(a)=protocol::documents::html_render(native){let time=epoch(&native["startedAt"]).max(epoch(&row["createdAt"]));if time>=first{rows.push(json!({"id":format!("html/{}",text(native,"id")),"role":"assistant","createdAt":time,"text":"","attachments":[a]}))}}
  if let Some(boundary)=protocol::lifecycle::describe(row,projection["runs"].as_array().map(Vec::as_slice).unwrap_or(&[])){
   let time=epoch(&native["startedAt"]).max(epoch(&row["createdAt"]));
   if time>=first{rows.push(json!({"id":format!("context/{}",text(native,"id")),"role":"context","createdAt":time,"context":boundary,"text":""}))}continue
  }
  let Some(mut activity)=inspectable_activity(row) else{continue};
  let item=protocol::activity::item(row);let time=epoch(&item["startedAt"]).max(epoch(&row["createdAt"]));
  if time<first{continue}
  activity["active"]=json!(activity["active"]==true && text(&activity,"runId")==active_run_id(projection) && !active_run_id(projection).is_empty());
  rows.push(json!({"id":format!("work/{}",text(item,"id")),"role":"activity","createdAt":time,"activity":activity,"text":""}));
 }
 for c in protocol::changes::summaries(projection){if rolled.contains(text(&c,"runId")){continue}let time=epoch(&c["capturedAt"]);if time>=first{rows.push(json!({"id":format!("changes/{}",text(&c,"id")),"role":"changes","createdAt":time,"changes":c,"text":""}))}}
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
 }
 // A content fingerprint per row lets the panel skip unchanged rows on each
 // streamed update without serialising the whole transcript on the UI thread.
 for row in grouped.iter_mut(){row["rev"]=json!(revision(row))}
 grouped
}
fn revision(row:&Value)->String{use std::hash::{Hash,Hasher};let mut h=std::collections::hash_map::DefaultHasher::new();row.to_string().hash(&mut h);format!("{:016x}",h.finish())}
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
        // Match native isProviderNativeSubagentThread; MCP children remain writable.
        if t["lineage"]["relationshipToParent"]=="subagent" && t["creationSource"]=="provider" {row["providerSubagent"]=json!(true);}
        for (key,value) in protocol::sidebar::sort_timestamps(t).as_object().unwrap(){row[key]=value.clone();}
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
                    projects.push(json!({"id":p["id"],"environmentId":e.id,"key":format!("{}/{}",e.id,text(p,"id")),"title":p["title"],"workspaceRoot":p["workspaceRoot"],"repositoryIdentity":p["repositoryIdentity"],"faviconPath":p["faviconPath"],"projectIcon":p["projectIcon"],"defaultThreadEnvMode":p["defaultThreadEnvMode"],"autoPull":p["autoPull"],"isScratch":p["isScratch"],"defaultModelSelection":p["defaultModelSelection"]}));
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
                        for field in ["modelSelection", "runtimeMode", "interactionMode", "activeRunId","worktreePath","pinnedAt","pinOrderKey","activeOrderKey","autoSettleDisabledAt","titleRegeneration"] { item[field] = t[field].clone(); }
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
        json!({"protocol":"omarchy.t3.v1","observedAt":Utc::now().timestamp_millis(),"stale":states.iter().any(|e|e["state"]!="ready"),"environments":states,"primaryEnvironmentId":if connections::native_primary().is_some(){json!("local")}else{Value::Null},"nativeProjectGrouping":native_project_grouping(),"clientPreferences":connection_preferences::read_client_preferences().ok(),"projects":projects,"threads":threads,"working":working,"pending":pending}),
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
            {"id":"fork","title":"Fork","creationSource":"provider","lineage":{"parentThreadId":"root","relationshipToParent":"fork"}},
            {"id":"native","title":"Native","creationSource":"provider","lineage":{"parentThreadId":"root","relationshipToParent":"subagent"}},
            {"id":"mcp","title":"MCP","creationSource":"mcp","lineage":{"parentThreadId":"root","relationshipToParent":"subagent"}}
        ]});
        let rows=normalize(&shell,"fixture",0).unwrap();assert_eq!(rows.len(),5);
        for row in &rows {assert_eq!(row["providerSubagent"]==true,row["id"]=="native");}
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
    fn live_completion_updates_sidebar_return_without_metadata_update(){
      let base=1700000000000_i64;
      let p=json!({"thread":{"createdAt":base,"updatedAt":base},"runs":[{"id":"r","status":"completed","createdAt":base+100,"completedAt":base+9000}],"messages":[],"turnItems":[]});
      let view=normalize_messages(&json!({"projection":p})).unwrap();
      assert_eq!(view["threadState"]["inboxAt"],base+9000);
      assert_eq!(view["threadState"]["working"],false);
      assert_eq!(view["threadState"]["activeAnchorAt"],base);
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
    fn timeline_rows_carry_a_content_revision(){
        let a=conversation_timeline(&json!({}),&[json!({"id":"m","role":"assistant","text":"one","createdAt":1})]);
        let b=conversation_timeline(&json!({}),&[json!({"id":"m","role":"assistant","text":"one","createdAt":1})]);
        let c=conversation_timeline(&json!({}),&[json!({"id":"m","role":"assistant","text":"two","createdAt":1})]);
        assert_eq!(a[0]["rev"],b[0]["rev"]);assert_ne!(a[0]["rev"],c[0]["rev"]);assert_eq!(a[0]["rev"].as_str().unwrap().len(),16);
    }
    #[test]
    fn grouped_activity_retains_each_native_source_without_output(){
        let rows=(0..2).map(|n|json!({"sourceThreadId":"ancestor","sourceItemId":format!("item{n}"),"item":{"id":format!("item{n}"),"threadId":"ancestor","runId":"r","providerTurnId":"p","type":"command_execution","status":"completed","startedAt":"2026-10-17T12:00:00Z","updatedAt":"2026-10-17T12:00:01Z","input":"printf private-input","output":"private-output"}})).collect::<Vec<_>>();
        let timeline=conversation_timeline(&json!({"turnItems":rows}),&[]);
        let entries=timeline[0]["activity"]["entries"].as_array().unwrap();assert_eq!(entries.len(),2);
        for (n,entry) in entries.iter().enumerate(){assert_eq!(entry["inspector"]["sourceThreadId"],"ancestor");assert_eq!(entry["inspector"]["sourceItemId"],format!("item{n}"));assert_eq!(entry["inspector"]["providerTurnId"],"p")}
        assert!(!json!(timeline).to_string().contains("private-"));
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

pub mod window_layout;

pub mod browser_preferences;

pub mod browser_profiles;

pub mod browser_runtime;

pub mod preview_transport;
pub mod preview_actions;
pub mod browser_guest;
pub mod browser_sessions;
pub mod browser_address;
pub mod browser_cleanup;

pub mod browser_sources;

mod browser_cookie_database;
pub mod browser_cookies;

pub mod browser_import;

pub mod storage_settings;
#[cfg(test)]
mod context_window_tests {
    use super::context_window;
    use serde_json::json;
    #[test]
    fn newest_turn_usage_then_provider_thread_then_compaction() {
        let p = json!({"thread":{"activeProviderThreadId":"pt"},"providerTurns":[{"tokenUsage":{"usedTokens":10,"maxTokens":100,"updatedAt":"2026-01-02T00:00:00Z"}},{"tokenUsage":{"usedTokens":50,"maxTokens":200,"updatedAt":"2026-01-01T00:00:00Z"}}],"providerThreads":[{"id":"pt","contextUsage":{"usedTokens":7,"maxTokens":70}}]});
        let w = context_window(&p);
        assert_eq!(w["usedTokens"], 10.0);
        assert_eq!(w["usedPercentage"], 10.0);
        assert_eq!(w["remainingTokens"], 90.0);
        let p = json!({"thread":{"activeProviderThreadId":"pt"},"providerTurns":[],"providerThreads":[{"id":"pt","contextUsage":{"usedTokens":7,"maxTokens":70,"cost":0.5}}]});
        assert_eq!(context_window(&p)["usedPercentage"], 10.0);
        assert_eq!(context_window(&p)["cost"], 0.5);
        let p = json!({"thread":{},"turnItems":[{"type":"compaction","afterTokenCount":1200,"beforeTokenCount":90000}]});
        assert_eq!(context_window(&p)["usedTokens"], 1200.0);
        assert!(context_window(&p)["maxTokens"].is_null());
        assert!(context_window(&json!({"thread":{}})).is_null());
    }
}
#[cfg(test)]
mod math_markdown_tests {
    use super::markdown_render;
    use serde_json::json;
    #[test]
    fn math_is_typeset_only_when_enabled_and_mermaid_blocks_offer_a_diagram() {
        let on = markdown_render("Energy $E=mc^2$ and\n\n$$\\sum_i x_i$$\n\nprice \\(a\\)", None, true);
        assert!(on.contains("mc<sup>2</sup>"), "{on}");
        assert!(on.contains("<p align=\"center\">"), "{on}");
        assert!(on.contains("∑<sub>i</sub>"), "{on}");
        let off = markdown_render("Energy $E=mc^2$", None, false);
        assert!(off.contains("$E=mc^2$"), "{off}");
        let blocks = vec![json!({"language":"mermaid","code":"graph TD; A-->B"})];
        let html = markdown_render("```mermaid\ngraph TD; A-->B\n```", Some(&blocks), false);
        assert!(html.contains("ot3-code-diagram://0"), "{html}");
    }
}
#[cfg(test)]
mod table_tests {
    #[test]
    fn tables_copy_as_markdown_and_csv() {
        let md = "| a | b |\n|---|---|\n| 1, x | `y` |\n";
        let t = super::markdown_tables(md);
        assert_eq!(t[0]["csv"], "a,b\n\"1, x\",y");
        assert_eq!(t[0]["markdown"], "| a | b |\n| --- | --- |\n| 1, x | y |");
        assert!(super::markdown_render(md, Some(&[]), false).contains("ot3-table-copy://0/csv"));
    }
}
