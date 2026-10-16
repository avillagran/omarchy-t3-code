mod auto_compact;
mod document_session;
mod live;
mod drafts;
mod localization;
mod ui_state;
mod quotas;
use omarchy_t3_client::write;
use omarchy_t3_client::*;
use serde_json::{json, Value};
use std::{
    fs::{self, OpenOptions},
    io::{self, BufRead, Write},
    os::fd::AsRawFd,
    os::unix::fs::OpenOptionsExt,
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

fn emit(kind: &str, data: Value) {
    let out = json!({"type":kind,"data":data});
    let mut stdout = io::stdout().lock();
    let _ = writeln!(stdout, "{out}");
    let _ = stdout.flush();
}
fn draft_command(cmd:&Value)->bool {
    let key=text(cmd,"key");
    match text(cmd,"op") {
        "draft-load" => {let result=drafts::load(key);emit("draft-loaded",json!({"key":key,"draft":result.as_ref().ok(),"error":result.as_ref().err()}));true}
        "draft-save" => {let result=drafts::save(key,&cmd["draft"]);emit("draft-saved",json!({"key":key,"revision":cmd["revision"],"error":result.as_ref().err()}));true}
        _=>false
    }
}
fn env_by_id(id: &str) -> Result<Environment> {
    environments()?
        .into_iter()
        .find(|e| e.id == id)
        .ok_or("unknown_environment".into())
}
fn notify(snapshot: &Value) -> Result<()> {
    let dir = xdg("XDG_STATE_HOME", ".local/state");
    fs::create_dir_all(&dir).map_err(|_| "local_error")?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .open(dir.join("poll.lock"))
        .map_err(|_| "local_error")?;
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err("local_error".into());
    }
    let p = dir.join("seen-rust.json");
    let mut previous = read_json(&p).unwrap_or(json!({}));
    for env in snapshot["environments"].as_array().into_iter().flatten() {
        if env["state"] != "ready" {
            continue;
        }
        let id = text(env, "id");
        let initialized = format!("@initialized:{id}");
        let keys: Vec<String> = previous
            .as_object()
            .unwrap()
            .keys()
            .filter(|k| k.starts_with(&format!("{id}/")))
            .cloned()
            .collect();
        let mut next = json!({});
        for t in snapshot["threads"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|t| text(t, "environmentId") == id)
        {
            let key = text(t, "key");
            let old = &previous[key];
            let changed = t["pending"] == true
                && (old["pending"] != true || old["revision"] != t["revision"]);
            if changed && previous[&initialized] == true {
                let delivered = Command::new("timeout")
                    .args([
                        "4s",
                        "notify-send",
                        "--app-name=Omarchy T3",
                        "--",
                        &format!("T3 · {}",localization::label(match text(t,"reason"){"response"=>"Done","failed"=>"Failed",_=>"Waiting"})),
                        text(t, "title"),
                    ])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
                if !delivered {
                    if !old.is_null() {
                        next[key] = old.clone();
                    }
                    continue;
                }
            }
            next[key] = json!({"pending":t["pending"],"revision":t["revision"]});
        }
        for k in keys {
            previous.as_object_mut().unwrap().remove(&k);
        }
        for (k, v) in next.as_object().unwrap() {
            previous[k] = v.clone();
        }
        previous[initialized] = json!(true);
    }
    save_json(&p, &previous)
}
fn open_thread(client: &Client, env: &str, id: &str) -> Result<Value> {
    if !valid_id(id) {
        return Err("invalid_id".into());
    }
    let e = env_by_id(env)?;
    let d = client.get(&e, "/.well-known/t3/environment", false)?;
    let eid = text(&d, "environmentId");
    if !valid_id(eid) {
        return Err("malformed".into());
    }
    let mut u = reqwest::Url::parse(&e.base_url).map_err(|_| "invalid_endpoint")?;
    // Fragment router components are encoded separately; never place a token in the URL.
    let encode = |s: &str| {
        let mut u = reqwest::Url::parse("https://example.invalid/").unwrap();
        u.path_segments_mut().unwrap().push(s);
        u.path().trim_start_matches('/').to_string()
    };
    u.set_fragment(Some(&format!("/{}/{}", encode(eid), encode(id))));
    let target=desktop_open::open(eid,id,u.as_str())?;
    Ok(json!({"ok":true,"target":target}))
}
fn view(snapshot: &Value, query: &str) -> Value {
    let mut v = snapshot.clone();
    let q = query.to_lowercase();
    if !q.is_empty() {
        v["threads"] = json!(snapshot["threads"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|t| format!(
                "{} {} {}",
                text(t, "title"),
                text(t, "project"),
                text(t, "environmentId")
            )
            .to_lowercase()
            .contains(&q))
            .cloned()
            .collect::<Vec<_>>());
    }
    v
}
fn watch(client: Client) -> Result<()> {
    emit("ui-state",ui_state::load());
    let (tx, rx) = mpsc::channel::<Value>();
    let compactor=auto_compact::Control::new(tx.clone());
    let (action_tx, action_rx) = mpsc::channel::<Value>();
    let input_action_tx=action_tx.clone();
    let document_manager=document_session::start();
    let (image_tx,image_rx)=mpsc::channel::<Value>();
    let image_generation=std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let image_worker_generation=image_generation.clone();
    std::thread::spawn(move||{for input in image_rx{let generation=input["generation"].as_u64().unwrap_or(0);if image_worker_generation.load(std::sync::atomic::Ordering::Relaxed)!=generation{continue}let result=images::load(&input);if image_worker_generation.load(std::sync::atomic::Ordering::Relaxed)!=generation{continue}emit("action",json!({"op":"image","requestId":input["requestId"],"result":result.as_ref().ok(),"error":result.as_ref().err()}));}});
    let input_tx = tx.clone();
    let live=live::Control::new(client.clone());
    let input_live=live.clone();
    std::thread::spawn(move || {
        let mut input_visible=false;
        let mut document_thread_key=String::new();
        for line in io::stdin()
            .lock()
            .lines()
            .map_while(std::result::Result::ok)
        {
            if line.len() > 1024 * 1024 {
                continue;
            }
            if let Ok(mut v) = serde_json::from_str::<Value>(&line) {
                if matches!(text(&v,"op"),"document-open"|"document-input"|"document-close"){
                    if v["op"]=="document-open"&&!input_visible{emit("action",json!({"op":"document-open","requestId":v["requestId"],"error":"panel_hidden"}));continue}
                    document_manager.send(v);continue
                }
                if v["op"]=="select" {let key=format!("{}/{}",text(&v,"environment"),text(&v,"thread"));if key!=document_thread_key{document_manager.send(json!({"op":"document-close"}));document_thread_key=key;}}
                if v["op"]=="clear-select" || (v["op"]=="visible" && v["value"]==false){document_manager.send(json!({"op":"document-close"}));}
                if v["op"]=="clear-select"{image_generation.fetch_add(1,std::sync::atomic::Ordering::Relaxed);input_live.select(None);}
                if v["op"]=="visible"{input_visible=v["value"]==true;}
                if v["op"]=="select" || (v["op"]=="visible"&&v["value"]==false){image_generation.fetch_add(1,std::sync::atomic::Ordering::Relaxed);}
                if v["op"]=="select"&&input_visible{input_live.select(Some((text(&v,"environment").into(),text(&v,"thread").into(),v["serial"].as_i64().unwrap_or(0))));}
                if v["op"]=="visible"&&v["value"]==false{input_live.select(None);}
                if v["op"]=="image"{if !input_visible{continue}v["generation"]=json!(image_generation.load(std::sync::atomic::Ordering::Relaxed));let _=image_tx.send(v);continue}
                // Draft durability must never wait behind a slow remote snapshot.
                if v["op"]=="backend-status"{emit("backend-status",backend_service::status());continue}
                if draft_command(&v) {continue}
                if v["op"]=="ui-state-set"{if let Err(error)=ui_state::save(&v["state"]){emit("ui-state-error",json!({"error":error}));}continue}
                if matches!(text(&v,"op"),"suggest"|"copy") {let _=input_action_tx.send(v);continue}
                if input_tx.send(v).is_err() {
                    return;
                }
            }
        }
        let _ = input_tx.send(json!({"op":"quit"}));
    });
    let result_tx = tx.clone();
    std::thread::spawn(move || {
        let mut pending=None;
        loop {
            let mut input=match pending.take(){Some(v)=>v,None=>match action_rx.recv(){Ok(v)=>v,Err(_)=>break}};
            if input["op"]=="suggest" {
                while let Ok(next)=action_rx.try_recv(){if next["op"]=="suggest"{input=next}else{pending=Some(next);break}}
            }
            let result = write::execute(&input);
            if matches!(text(&input,"op"),"suggest"|"catalog"|"model-favorite"|"scroll-config"|"copy") {emit("action",json!({"op":input["op"],"environment":input["environment"],"thread":input["thread"],"requestId":input["requestId"],"result":result.as_ref().ok(),"error":result.as_ref().err()}));continue}
            let _ = result_tx.send(json!({"op":"action_result","input":input,"result":result.as_ref().ok(),"error":result.as_ref().err()}));
        }
    });
    let mut state = json!({"threads":[],"environments":[],"working":0,"pending":0,"stale":true});
    let mut visible = false;
    let mut selected: Option<(String, String, i64)> = None;
    let mut query = String::new();
    let mut next_poll = Instant::now();
    let mut next_quota = Instant::now();
    let mut quota_running = false;
    let mut service_starting = false;
    let mut message_cache: Option<(String, String, i64, Value)> = None;
    loop {
        if Instant::now() >= next_quota && !quota_running {
            quota_running = true;
            next_quota = Instant::now() + Duration::from_secs(300);
            let tx = tx.clone();
            let c = client.clone();
            std::thread::spawn(move || {
                let d = quotas::collect(&c);
                let _ = tx.send(json!({"op":"quota_result","data":d}));
            });
        }
        if Instant::now() >= next_poll {
            match snapshot(&client) {
                Ok(d) => {
                    let _ = notify(&d);
                    compactor.observe(&d);
                    let mut before = state.clone();
                    let mut after = d.clone();
                    if let Some(o) = before.as_object_mut() {
                        o.remove("observedAt");
                    }
                    if let Some(o) = after.as_object_mut() {
                        o.remove("observedAt");
                    }
                    let changed = before != after;
                    state = d;
                    if changed {
                        emit("snapshot", view(&state, &query));
                    }
                }
                Err(e) => {
                    state["stale"] = json!(true);
                    emit("error", json!({"error":e}));
                }
            }
            let active =
                visible || (state["stale"] == false && state["working"].as_u64().unwrap_or(0) > 0);
            next_poll = Instant::now() + Duration::from_secs(if active { 10 } else { 60 });
            if visible {
                if let Some((env, id, serial)) = &selected {
                    if live.ready(){continue}

                    let rev = state["threads"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .find(|t| text(t, "id") == id && text(t, "environmentId") == env)
                        .map(|t| t["updatedAt"].as_i64().unwrap_or(0))
                        .unwrap_or(0);
                    let selected_working=state["threads"].as_array().into_iter().flatten().any(|t|text(t,"id")==id && text(t,"environmentId")==env && t["working"]==true);
                    if selected_working || message_cache
                        .as_ref()
                        .is_none_or(|(e, i, r, _)| e != env || i != id || *r != rev)
                    {
                        let result = env_by_id(env).and_then(|e| client.messages(&e, id));
                        match result {
                            Ok(d) => {
                                message_cache = Some((env.clone(), id.clone(), rev, d.clone()));
                                if !live.ready(){emit("messages", json!({"serial":serial,"result":d}));}
                            }
                            Err(e) => {
                                if !live.ready(){emit("messages", json!({"serial":serial,"result":{"error":e}}))}
                            }
                        }
                    }
                }
            }
        }
        let timeout = next_poll
            .min(next_quota)
            .saturating_duration_since(Instant::now())
            .max(Duration::from_millis(1));
        let cmd = match rx.recv_timeout(timeout) {
            Ok(v) => v,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => return Ok(()),
        };
        match text(&cmd, "op") {
            "ui-state-set" => { if let Err(error)=ui_state::save(&cmd["state"]) {emit("ui-state-error",json!({"error":error}));} }
            "backend-start" | "backend-control" => {
                if !service_starting {
                    service_starting=true;let service_tx=tx.clone();
                    std::thread::spawn(move||{let result=write::execute(&cmd);let _=service_tx.send(json!({"op":"action_result","input":cmd,"result":result.as_ref().ok(),"error":result.as_ref().err()}));});
                }
            }
            "pull-requests" | "question" | "scroll-config" | "model-favorite" | "diff" | "clipboard" | "queue" | "suggest" | "catalog" | "upload" | "send" | "stop" | "delete-upload" | "manage" => { let _ = action_tx.send(cmd); }
            "action_result" => {
                if matches!(text(&cmd["input"],"op"),"backend-start"|"backend-control"){service_starting=false;}
                emit("action",json!({"op":cmd["input"]["op"],"environment":cmd["input"]["environment"],"thread":cmd["input"]["thread"],"requestId":cmd["input"]["requestId"],"result":cmd["result"],"error":cmd["error"]}));
                if matches!(text(&cmd["input"],"op"),"backend-start"|"backend-control") || cmd["input"]["op"]=="question" || cmd["input"]["op"]=="queue" || cmd["input"]["op"]=="send" || cmd["input"]["op"]=="stop" || cmd["input"]["op"]=="manage" { message_cache=None; next_poll=Instant::now(); }
            }
            "quit" => return Ok(()),
            "refresh" => { refresh_connections(); next_poll = Instant::now(); },
            "compaction-deadline"=>{if let Some(at)=cmd["at"].as_i64(){let remaining=at.saturating_sub(chrono::Utc::now().timestamp_millis()).max(0) as u64;next_poll=next_poll.min(Instant::now()+Duration::from_millis(remaining));}},
            "visible" => {
                visible = cmd["value"] == true;
                if visible {
                    next_poll = Instant::now();
                } else {
                    selected = None;
                    message_cache = None;
                }
            }
            "search" => {
                query = text(&cmd, "query").chars().take(256).collect();
                emit("snapshot", view(&state, &query));
            }
            "clear-select" => {selected=None;message_cache=None;}
            "select" => {
                let env = text(&cmd, "environment").to_string();
                let id = text(&cmd, "thread").to_string();
                let serial = cmd["serial"].as_i64().unwrap_or(0);
                if !visible {
                    continue;
                }
                selected = Some((env.clone(), id.clone(), serial));

            }
            "open" => {
                if let Err(e) =
                    open_thread(&client, text(&cmd, "environment"), text(&cmd, "thread"))
                {
                    emit("error", json!({"error":e}));
                }
            }
            "quota_result" => {
                quota_running = false;
                emit("quotas", cmd["data"].clone());
            }
            _ => {}
        }
    }
}
fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s|s=="super-enter") {
        let consumed=Command::new("timeout").args(["2","omarchy-shell","io.github.avillagran.omarchy-t3-code","submitMessage"]).stdin(Stdio::null()).stderr(Stdio::null()).output().ok().is_some_and(|o|o.status.success()&&String::from_utf8_lossy(&o.stdout).trim()=="true");
        if !consumed {use std::os::unix::process::CommandExt;let _=Command::new("omarchy-launch-terminal").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).exec();return Err("terminal_launch_failed".into())}
        return Ok(())
    }
    let client = Client::new()?;
    let result = match args.first().map(String::as_str).unwrap_or("watch") {
        "watch" => return watch(client),
        "relay-status" => if let Some(id)=args.get(1){check_native_environment(id)?}else{check_native_session()?},
        "connections" | "setup-local" | "setup-write" => json!({"source":"t3","automatic":true,"environments":environments()?.iter().map(|e|json!({"id":e.id,"name":e.name,"state":e.connection_error})).collect::<Vec<_>>()}),
        "snapshot" => {
            let d = snapshot(&client)?;
            if args.iter().any(|s| s == "--notify") {
                notify(&d)?;
            }
            d
        }
        "quotas" => quotas::collect(&client),
        "backend-status" => backend_service::status(),
        "backend-control" if args.len()==2 => backend_service::control(&args[1])?,
        "scroll-config" => scroll::read(),
        "catalog" if args.len()==2 => write::execute(&json!({"op":"catalog","environment":args[1]}))?,
        "suggest" if args.len()==6 => write::execute(&json!({"op":"suggest","environment":args[1],"thread":args[2],"kind":args[3],"query":args[4],"instanceId":args[5]}))?,
        "messages" if args.len() == 3 => client.messages(&env_by_id(&args[1])?, &args[2])?,
        "open" if args.len() == 3 => open_thread(&client, &args[1], &args[2])?,
        _ => return Err("invalid_command".into()),
    };
    println!("{result}");
    Ok(())
}
fn main() {
    if std::env::args().nth(1).as_deref()==Some("document-render") {if let Err(e)=documents::renderer(){eprintln!("{e}");std::process::exit(1)}return}
    if std::env::args().nth(1).as_deref()==Some("document-view") {if let Err(e)=documents::viewer(){eprintln!("{e}");std::process::exit(1)}return}
    if let Err(e) = run() {
        println!("{}", json!({"error":e}));
        std::process::exit(1);
    }
}
