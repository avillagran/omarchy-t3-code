mod auto_compact;
mod document_session;
mod live;
mod shell_watch;
mod keybinding_watch;
mod device_watch;
mod device_screen;
mod preview_watch;
mod preview_command;
mod prompt_stash;
mod localization;
mod ui_state;
mod quotas;
mod local_quotas;
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
        "draft-load" => {let result=prompt_stash::recover_before_load(key,text(cmd,"projectId")).and_then(|_|checkpoint_operations::recover_before_load(key)).and_then(|_|drafts::load(key));emit("draft-loaded",json!({"key":key,"draft":result.as_ref().ok(),"error":result.as_ref().err()}));true}
        "draft-save" => {let result=drafts::save(key,&cmd["draft"]);let recovery=if result.as_ref().err().is_some_and(|e|e=="draft_recovery_changed"){drafts::load(key).ok().and_then(|d|d["recoveryIds"].as_array().and_then(|ids|ids.iter().find(|id|!cmd["draft"]["recoveryIds"].as_array().is_some_and(|old|old.contains(id)))).and_then(Value::as_str).and_then(|id|checkpoint_journal::action(id).ok().flatten()).as_ref().map(checkpoint_journal::receipt))}else{None};emit("draft-saved",json!({"key":key,"revision":cmd["revision"],"error":result.as_ref().err(),"recovery":recovery}));true}
        "prompt-stash" => {let result=prompt_stash::execute(cmd);emit("prompt-stash",json!({"requestId":cmd["requestId"],"environment":cmd["environment"],"projectId":cmd["projectId"],"key":key,"result":result.as_ref().ok(),"error":result.as_ref().err()}));true}
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
    // T3 notificationMode: desktop notifications, a sound, both or neither.
    let mode = ui_state::load()["threadNotifications"].as_str().unwrap_or("notifications").to_string();
    let banners = matches!(mode.as_str(), "notifications" | "notifications-and-sound");
    let sound = matches!(mode.as_str(), "sound" | "notifications-and-sound");
    let mut chime: Option<&str> = None;
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
                // Threads that finish together play one sound; a question outranks a completion.
                let waiting = !matches!(text(t, "reason"), "response" | "review" | "failed");
                if sound && (chime.is_none() || waiting) {
                    chime = Some(if waiting { "notification-input.mp3" } else { "notification-completion.mp3" });
                }
            }
            if changed && previous[&initialized] == true && banners {
                let icon=notification_icon::path(snapshot,t).ok();
                let delivered = Command::new("timeout")
                    .args([
                        "4s",
                        "notify-send",
                        "--app-name=Omarchy T3",
                        "--icon",
                        icon.as_ref().and_then(|p|p.to_str()).unwrap_or("t3-code"),
                        "--",
                        &format!("T3 · {}",localization::label(match text(t,"reason"){"response"|"review"=>"Done","failed"=>"Failed",_=>"Waiting"})),
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
    if let Some(name) = chime {
        play_sound(name);
    }
    save_json(&p, &previous)
}
fn play_sound(name: &str) {
    let Ok(exe) = std::env::current_exe() else { return };
    // bin/omarchy-t3-client sits next to ui/ in the plugin directory.
    let Some(file) = exe.parent().and_then(|bin| bin.parent()).map(|root| root.join("ui/assets/sounds").join(name)).filter(|f| f.is_file()) else { return };
    for player in ["pw-play", "paplay"] {
        if let Ok(mut child) = Command::new(player).arg(&file).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            std::thread::spawn(move || child.wait());
            return;
        }
    }
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
// Native tool downloads must not block messages, draft persistence or state.
// Read/action pools are independent and allocated lazily, with bounded admission.
fn device_pool(browser:browser_sessions::Manager)->mpsc::SyncSender<Value> {
    let (sender,receiver)=mpsc::sync_channel::<Value>(64);
    let receiver=std::sync::Arc::new(std::sync::Mutex::new(receiver));
    for _ in 0..4 {
        let receiver=receiver.clone();
        let browser=browser.clone();
        std::thread::spawn(move||loop {
            let input={let receiver=receiver.lock().unwrap();receiver.recv()};
            let Ok(v)=input else{break};
            let result=if v["op"]=="browser-import-sources"{browser_sources::list_at(&home())}else if matches!(text(&v,"op"),"browser-import-read"|"browser-import-action"){browser_import::execute(&v,&browser)}else if text(&v,"op").starts_with("browser-profiles-"){browser_profiles::execute_with(&v,Some(&browser))}else{write::execute(&v)};
            emit("action",json!({"op":v["op"],"environment":v["environment"],"requestId":v["requestId"],"result":result.as_ref().ok(),"error":result.as_ref().err()}));
        });
    }
    sender
}
fn watch(client: Client) -> Result<()> {
    emit("ui-state",ui_state::load());
    let (tx, rx) = mpsc::channel::<Value>();
    let compactor=auto_compact::Control::new(tx.clone());
    let (action_tx, action_rx) = mpsc::channel::<Value>();
    let input_action_tx=action_tx.clone();
    let mut document_manager=document_session::Pool::new();
    let cloud_manager=connection_cloud::CloudManager::default();
    let (image_tx,image_rx)=mpsc::channel::<Value>();
    let image_generation=std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let image_worker_generation=image_generation.clone();
    std::thread::spawn(move||{for input in image_rx{let generation=input["generation"].as_u64().unwrap_or(0);if image_worker_generation.load(std::sync::atomic::Ordering::Relaxed)!=generation{continue}let result=images::load(&input);if image_worker_generation.load(std::sync::atomic::Ordering::Relaxed)!=generation{continue}emit("action",json!({"op":"image","requestId":input["requestId"],"result":result.as_ref().ok(),"error":result.as_ref().err()}));}});
    // Bounded read worker: detail requests never wait behind sends or settings mutations.
    let (detail_tx,detail_rx)=mpsc::sync_channel::<Value>(2);
    let detail_generation=image_generation.clone();
    std::thread::spawn(move||{for v in detail_rx{
        let generation=v["generation"].as_u64().unwrap_or(0);
        if detail_generation.load(std::sync::atomic::Ordering::Relaxed)!=generation{continue}
        let result=write::execute(&v);
        if detail_generation.load(std::sync::atomic::Ordering::Relaxed)!=generation{continue}
        emit("action",json!({"op":v["op"],"environment":v["environment"],"thread":v["thread"],"requestId":v["requestId"],"result":result.as_ref().ok(),"error":result.as_ref().err()}));
    }});
    // Native preference reads and lifecycle commands must not block sends or draft persistence.
    let (archive_tx,archive_rx)=mpsc::sync_channel::<Value>(16);
    std::thread::spawn(move||{for v in archive_rx{let result=write::execute(&v);emit("action",json!({"op":v["op"],"environment":v["environment"],"thread":v["thread"],"requestId":v["requestId"],"result":result.as_ref().ok(),"error":result.as_ref().err()}));}});
    // Initialized only if the Integrations device UI is used.
    let mut device_read_tx:Option<mpsc::SyncSender<Value>>=None;
    let mut device_action_tx:Option<mpsc::SyncSender<Value>>=None;
    // Native Git actions are streams and must not block message/draft workers.
    let (git_tx,git_rx)=mpsc::sync_channel::<Value>(2);
    std::thread::spawn(move||{for v in git_rx{let result=write::execute(&v);emit("action",json!({"op":v["op"],"environment":v["environment"],"thread":v["thread"],"requestId":v["requestId"],"result":result.as_ref().ok(),"error":result.as_ref().err()}));}});
    // Rewinds remain attached to their original thread across navigation. Their
    // observations must not be discarded by the selected-detail generation.
    let (rewind_tx,rewind_rx)=mpsc::sync_channel::<Value>(8);
    std::thread::spawn(move||{for v in rewind_rx{let result=write::execute(&v);emit("action",json!({"op":v["op"],"environment":v["environment"],"thread":v["thread"],"requestId":v["requestId"],"operationId":v["operationId"],"result":result.as_ref().ok(),"error":result.as_ref().err()}));}});
    let input_tx = tx.clone();
    let live=live::Control::new(client.clone());
    shell_watch::start(tx.clone());
    let input_live=live.clone();
    let browser_manager=browser_sessions::Manager::start(browser_guest::Config::native()?,|id|environments()?.into_iter().find(|e|e.id==id).ok_or("unknown_environment".into()),connection_preferences::read_client_preferences,|data|emit("browser-surface",data));
    std::thread::spawn(move || {
        let mut input_visible=false;
        let mut keybinding_watches=keybinding_watch::Control::default();
        let mut device_watches=device_watch::Control::default();
        let mut device_screens=device_screen::Manager::default();
        let browser_observer=browser_manager.clone();
        let preview_watches=preview_watch::Control::with_observer(move|data|browser_observer.observe(data));
        let browser_actions=browser_manager.clone();
        let mut preview_commands=preview_command::Control::with_observer(move|input,result|browser_actions.action(input,result));
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
                if matches!(text(&v,"op"),"connection-cloud"|"connection-cloud-cancel"|"connection-cloud-install"){
                    let result=match text(&v,"op"){
                        "connection-cloud-cancel"=>cloud_manager.cancel(text(&v,"requestId")),
                        "connection-cloud-install"=>cloud_manager.confirm_install(text(&v,"requestId"),v["approved"]==true),
                        _=>{
                            if v["environment"]!="local"{Err("local_environment_only".into())}
                            else if matches!(text(&v,"action"),"link"|"link-publish-only")&&v["confirmed"]!=true{Err("confirmation_required".into())}
                            else{cloud_manager.start(connection_cloud::NativeCli::new(home().join(".t3/bin/t3"),t3_home()),text(&v,"requestId"),text(&v,"action"),v["confirmed"]==true,|event|emit("connection-cloud",event))}
                        }
                    };
                    if let Err(error)=result{emit("connection-cloud",json!({"environment":"local","requestId":v["requestId"],"action":v["action"],"busy":false,"stage":"failed","error":error}));}continue
                }

                if matches!(text(&v,"op"),"document-open"|"document-input"|"document-close"){
                    if v["op"]=="document-open"&&!input_visible{emit("action",json!({"op":"document-open","requestId":v["requestId"],"error":"panel_hidden"}));continue}
                    document_manager.send(v);continue
                }
                if v["op"]=="select" {let key=format!("{}/{}",text(&v,"environment"),text(&v,"thread"));if key!=document_thread_key{document_manager.send(json!({"op":"document-close"}));preview_watches.clear();browser_manager.hide();device_screens.stop();document_thread_key=key;}}
                if v["op"]=="clear-select" || (v["op"]=="visible" && v["value"]==false){document_manager.send(json!({"op":"document-close"}));}
                if v["op"]=="clear-select"{image_generation.fetch_add(1,std::sync::atomic::Ordering::Relaxed);input_live.select(None);}
                if v["op"]=="visible"{input_visible=v["value"]==true;if !input_visible{keybinding_watches.clear();device_watches.clear();device_screens.stop();preview_watches.clear();browser_manager.hide();}}
                if matches!(text(&v,"op"),"browser-show"|"browser-hide"|"browser-input"|"browser-control"){browser_manager.request(v,input_visible);continue}
                if v["op"]=="preview-action"{preview_commands.send(v,input_visible);continue}
                if v["op"]=="preview-watch"{preview_watches.select(&v,input_visible);continue}
                if v["op"]=="clear-select"{preview_watches.clear();browser_manager.hide();device_screens.stop();}
                if v["op"]=="devices-watch"{device_watches.select(&v,input_visible);continue}
                if matches!(text(&v,"op"),"device-screen-open"|"device-screen-input"|"device-screen-close"|"device-screen-action"|"device-screen-detail"){device_screens.request(&v,input_visible);continue}
                if v["op"]=="keybindings-watch"{keybinding_watches.select(&v,input_visible);continue}
                if v["op"]=="select" || (v["op"]=="visible"&&v["value"]==false){image_generation.fetch_add(1,std::sync::atomic::Ordering::Relaxed);}
                if v["op"]=="select"&&input_visible{input_live.select(Some((text(&v,"environment").into(),text(&v,"thread").into(),v["serial"].as_i64().unwrap_or(0))));}
                if v["op"]=="visible"&&v["value"]==false{input_live.select(None);}
                if v["op"]=="image"{if !input_visible{continue}v["generation"]=json!(image_generation.load(std::sync::atomic::Ordering::Relaxed));let _=image_tx.send(v);continue}
                if matches!(text(&v,"op"),"diff"|"tool-inspector"|"plan-followup"|"git-read"|"working-review"|"review-refs"|"rewind-preview"){
                    v["generation"]=json!(image_generation.load(std::sync::atomic::Ordering::Relaxed));
                    let error=if !input_visible{Some("panel_hidden")}else if detail_tx.try_send(v.clone()).is_err(){Some("detail_read_busy")}else{None};
                    if let Some(error)=error{emit("action",json!({"op":v["op"],"environment":v["environment"],"thread":v["thread"],"requestId":v["requestId"],"error":error}));}continue
                }
                if matches!(text(&v,"op"),"rewind-dispatch"|"rewind-observe"|"rewind-resolve"|"rewind-list"){
                    if rewind_tx.try_send(v.clone()).is_err(){emit("action",json!({"op":v["op"],"environment":v["environment"],"thread":v["thread"],"requestId":v["requestId"],"operationId":v["operationId"],"error":"revert_busy"}));}continue
                }
                if v["op"]=="rewind-recover"{let result=write::execute(&v);emit("action",json!({"op":v["op"],"environment":v["environment"],"thread":v["thread"],"requestId":v["requestId"],"operationId":v["operationId"],"result":result.as_ref().ok(),"error":result.as_ref().err()}));continue}
                if matches!(text(&v,"op"),"archive-read"|"archive-action"|"archive-state"|"keybindings-read"|"keybindings-write"|"keybindings-open"|"licenses-read"|"licenses-notice"){
                    if archive_tx.try_send(v.clone()).is_err(){emit("action",json!({"op":v["op"],"environment":v["environment"],"thread":v["thread"],"requestId":v["requestId"],"error":if text(&v,"op").starts_with("keybindings-"){"keybindings_busy"}else if text(&v,"op").starts_with("licenses-"){"licenses_busy"}else{"archive_busy"}}));}continue
                }
                if matches!(text(&v,"op"),"devices-read"|"devices-action"|"browser-profiles-read"|"browser-profiles-action"|"browser-import-sources"|"browser-import-read"|"browser-import-action") {
                    let sender=if matches!(text(&v,"op"),"devices-read"|"browser-profiles-read"|"browser-import-sources"|"browser-import-read"){device_read_tx.get_or_insert_with(||device_pool(browser_manager.clone()))}else{device_action_tx.get_or_insert_with(||device_pool(browser_manager.clone()))};
                    if sender.try_send(v.clone()).is_err(){emit("action",json!({"op":v["op"],"environment":v["environment"],"requestId":v["requestId"],"error":if text(&v,"op").starts_with("browser-profiles-"){"browser_profiles_busy"}else{"device_busy"}}));}
                    continue
                }
                if matches!(text(&v,"op"),"git-action"|"git-resolve"|"git-terminal"){
                    if git_tx.try_send(v.clone()).is_err(){emit("action",json!({"op":v["op"],"environment":v["environment"],"thread":v["thread"],"requestId":v["requestId"],"error":"git_operation_busy"}));}continue
                }
                if v["op"]=="git-state"{let result=write::execute(&v);emit("action",json!({"op":v["op"],"environment":v["environment"],"thread":v["thread"],"requestId":v["requestId"],"result":result.as_ref().ok(),"error":result.as_ref().err()}));continue}
                // Draft durability must never wait behind a slow remote snapshot.
                if v["op"]=="backend-status"{emit("backend-status",backend_service::status());continue}
                // Lifecycle controls must not wait behind a remote snapshot.
                if matches!(text(&v,"op"),"backend-start"|"backend-control") {
                    let service_tx=input_tx.clone();
                    std::thread::spawn(move||{
                        static BUSY: std::sync::Mutex<()> = std::sync::Mutex::new(());
                        let guard=BUSY.try_lock();
                        let result=match guard {Ok(_guard)=>write::execute(&v),Err(_)=>Err("backend_control_busy".into())};
                        emit("action",json!({"op":v["op"],"requestId":v["requestId"],"result":result.as_ref().ok(),"error":result.as_ref().err()}));
                        let _=service_tx.send(json!({"op":"refresh"}));
                    });continue
                }
                if v["op"]=="quit"{let _=cloud_manager.cancel_active();}
                if draft_command(&v) {continue}
                if v["op"]=="ui-state-set"{if let Err(error)=ui_state::save(&v["state"]){emit("ui-state-error",json!({"error":error}));}continue}
                // SSH onboarding can take a minute; keep sends and draft saves responsive.
                if matches!(text(&v,"op"),"connections-manage"|"connection-access"|"connection-network"|"connection-route"){std::thread::spawn(move||{let result=write::execute(&v);emit("action",json!({"op":v["op"],"requestId":v["requestId"],"result":result.as_ref().ok(),"error":result.as_ref().err()}));});continue}
                if matches!(text(&v,"op"),"branch-refs"|"branch-switch"|"branch-check"|"branch-resolve"|"thread-search"|"secret-answer"|"pull-request-action"|"thread-pull-request"|"usage-summary"|"scheduled-tasks"|"storage-cleanup"|"mcp-app"|"provider-setup"|"agent-sessions"){std::thread::spawn(move||{let result=write::execute(&v);emit("action",json!({"op":v["op"],"environment":v["environment"],"thread":v["thread"],"requestId":v["requestId"],"result":result.as_ref().ok(),"error":result.as_ref().err()}));});continue}
                if matches!(text(&v,"op"),"suggest"|"copy"|"client-settings"|"settings"|"provider-instances"|"window-layout"|"connections-manage"|"terminal-open"|"git-action"|"git-resolve"|"git-terminal") {let _=input_action_tx.send(v);continue}
                if input_tx.send(v).is_err() {
                    return;
                }
            }
        }
        let _=cloud_manager.cancel_active();
        let _=browser_manager.shutdown();
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
            if matches!(text(&input,"op"),"suggest"|"catalog"|"model-favorite"|"scroll-config"|"copy"|"client-settings"|"settings"|"provider-instances"|"window-layout"|"connections-manage") {emit("action",json!({"op":input["op"],"environment":input["environment"],"thread":input["thread"],"requestId":input["requestId"],"result":result.as_ref().ok(),"error":result.as_ref().err()}));continue}
            let _ = result_tx.send(json!({"op":"action_result","input":input,"result":result.as_ref().ok(),"error":result.as_ref().err()}));
        }
    });
    let mut state = json!({"threads":[],"environments":[],"working":0,"pending":0,"stale":true});
    let mut visible = false;
    let mut selected: Option<(String, String, i64)> = None;
    let mut query = String::new();
    let mut next_poll = Instant::now();
    let mut last_snapshot = Instant::now();
    let mut quotas = quotas::Control::default();
    let mut next_quota = Instant::now();
    let mut quota_running = false;
    let mut message_cache: Option<(String, String, i64, Value)> = None;
    loop {
        if Instant::now() >= next_quota && !quota_running {
            quota_running = true;
            next_quota = Instant::now() + Duration::from_secs(300);
            let tx = tx.clone();
            let c = client.clone();
            std::thread::spawn(move || {
                local_quotas::collect_with(&c, |data,complete|{let _=tx.send(json!({"op":"quota_result","data":data,"complete":complete}));});
            });
        }
        if Instant::now() >= next_poll {
            last_snapshot = Instant::now();
            if let Ok(environments)=environments(){quotas.sync(environments);}
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
            .min(if quota_running {next_poll}else{next_quota})
            .saturating_duration_since(Instant::now())
            .max(Duration::from_millis(1));
        let cmd = match rx.recv_timeout(timeout) {
            Ok(v) => v,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => return Ok(()),
        };
        match text(&cmd, "op") {
            "ui-state-set" => { if let Err(error)=ui_state::save(&cmd["state"]) {emit("ui-state-error",json!({"error":error}));} }
            "window-layout" | "project-sources" | "browse-folders" | "history" | "pull-requests" | "approval" | "question" | "scroll-config" | "model-favorite" | "diff" | "clipboard" | "queue" | "suggest" | "catalog" | "upload" | "send" | "stop" | "delete-upload" | "manage" => { let _ = action_tx.send(cmd); }
            "action_result" => {
                emit("action",json!({"op":cmd["input"]["op"],"environment":cmd["input"]["environment"],"thread":cmd["input"]["thread"],"requestId":cmd["input"]["requestId"],"result":cmd["result"],"error":cmd["error"]}));
                if matches!(text(&cmd["input"],"op"),"backend-start"|"backend-control"|"approval"|"question"|"queue"|"send"|"stop"|"manage") { message_cache=None; next_poll=Instant::now(); }
            }
            "quit" => return Ok(()),
            "refresh" => { refresh_connections(); next_poll = Instant::now(); },
            // Coalesce bursts of shell events into one snapshot.
            "shell-changed" => { next_poll = next_poll.min((Instant::now() + Duration::from_millis(200)).max(last_snapshot + Duration::from_secs(1))); },
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
            "quotas-refresh" => {quotas.refresh();next_quota=Instant::now();}
            "quota_result" => {
                quota_running = cmd["complete"]!=true;
                if !quota_running {next_quota=Instant::now()+Duration::from_secs(300);}
                quotas.local(cmd["data"].as_array().cloned().unwrap_or_default(),cmd["complete"]==true);
            }
            _ => {}
        }
    }
}
fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s|s=="super-enter") {
        // Compatibility for old installations: this command only opens a terminal.
        use std::os::unix::process::CommandExt;
        let _=Command::new("omarchy-launch-terminal").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).exec();
        return Err("terminal_launch_failed".into())
    }
    let client = Client::new()?;
    let result = match args.first().map(String::as_str).unwrap_or("watch") {
        "watch" => return watch(client),
        "licenses-read" => third_party_licenses::read()?,
        "browser-import-sources" => browser_sources::list_at(&home())?,
        "browser-profiles-read" if args.len()>1 => browser_profiles::execute(&json!({"op":"browser-profiles-read","environments":args[1..]}))?,
        "preview-read" if args.len()==3 => {let e=environments()?.into_iter().find(|e|e.id==args[1]).ok_or("unknown_environment")?;preview_transport::read(&e,&args[2])?},
        "devices-read" if args.len()==2 => write::execute(&json!({"op":"devices-read","environment":args[1]}))?,
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
        "rewind-preview" if args.len()==4 => write::execute(&json!({"op":"rewind-preview","environment":args[1],"thread":args[2],"messageId":args[3]}))?,
        "keybindings-read" if args.len()==2 =>write::execute(&json!({"op":"keybindings-read","environment":args[1]}))?,
        "archive-read" if args.len()==2 =>write::execute(&json!({"op":"archive-read","environment":args[1],"requestId":uuid::Uuid::new_v4().to_string(),"projectIds":null}))?,
        "review-refs" if args.len()>=5 => write::execute(&json!({"op":"review-refs","environment":args[1],"thread":args[2],"projectId":args[3],"cwd":args[4],"refKind":args.get(5).map(String::as_str).unwrap_or("local"),"query":args.get(6).map(String::as_str).unwrap_or(""),"requestId":uuid::Uuid::new_v4().to_string()}))?,
        "working-review" if args.len()>=5 => write::execute(&json!({"op":"working-review","environment":args[1],"thread":args[2],"projectId":args[3],"cwd":args[4],"sourceKind":args.get(5).map(String::as_str).unwrap_or("working-tree"),"filePath":args.get(6),"requestId":uuid::Uuid::new_v4().to_string()}))?,
        "git-read" if args.len()==5 => write::execute(&json!({"op":"git-read","environment":args[1],"thread":args[2],"projectId":args[3],"cwd":args[4],"requestId":uuid::Uuid::new_v4().to_string()}))?,
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
    if matches!(std::env::args().nth(1).as_deref(),Some("--version"|"-V")){println!("omarchy-t3-client {}",env!("CARGO_PKG_VERSION"));return}
    if std::env::args().nth(1).as_deref()==Some("browser-render"){if let Err(error)=browser_runtime::renderer(){eprintln!("{error}");std::process::exit(1)}return}
    if std::env::args().nth(1).as_deref()==Some("terminal-session"){let args:Vec<_>=std::env::args().skip(2).collect();let result=if args.len()==3{env_by_id(&args[0]).and_then(|e|terminal::run_session(&e,&args[1],&args[2]))}else{Err("invalid_command".into())};if let Err(e)=result{eprintln!("T3 terminal: {e}");std::process::exit(1)}return}
    if std::env::args().nth(1).as_deref()==Some("terminal") {let args:Vec<_>=std::env::args().skip(2).collect();let result=if args.len()==2 || (args.len()==4&&args[2]=="--run-file"){env_by_id(&args[0]).and_then(|e|terminal::run_command(&e,&args[1],args.get(3).map(std::path::Path::new)))}else{Err("invalid_command".into())};if let Err(e)=result{eprintln!("T3 terminal: {e}");std::process::exit(1)}return}
    if std::env::args().nth(1).as_deref()==Some("document-render") {if let Err(e)=documents::renderer(){eprintln!("{e}");std::process::exit(1)}return}
    if std::env::args().nth(1).as_deref()==Some("document-view") {if let Err(e)=documents::viewer(){eprintln!("{e}");std::process::exit(1)}return}
    if let Err(e) = run() {
        println!("{}", json!({"error":e}));
        std::process::exit(1);
    }
}

#[cfg(test)]
mod notification_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn delivery_child() {
        let Ok(root)=std::env::var("OT3_NOTIFY_TEST_ROOT") else{return};
        std::env::set_var("XDG_STATE_HOME",format!("{root}/state"));
        std::env::set_var("XDG_CACHE_HOME",format!("{root}/cache"));
        let mut snapshot=json!({"environments":[{"id":"fixture","state":"ready"}],"projects":[],"threads":[{"environmentId":"fixture","key":"fixture/one","title":"Input required","pending":false,"revision":"1"}]});
        notify(&snapshot).unwrap();
        snapshot["threads"][0]["pending"]=json!(true);snapshot["threads"][0]["revision"]=json!("2");snapshot["threads"][0]["reason"]=json!("request");
        notify(&snapshot).unwrap();notify(&snapshot).unwrap();
    }
    #[test]
    fn delivers_once_with_composed_icon_without_desktop_notification() {
        let root=std::env::temp_dir().join(format!("ot3-notify-test-{}",uuid::Uuid::new_v4()));fs::create_dir_all(root.join("bin")).unwrap();
        let script=root.join("bin/notify-send");fs::write(&script,"#!/bin/sh\nprintf '%s\\n' \"$@\" >> \"$OT3_NOTIFY_TEST_ROOT/delivered\"\n").unwrap();fs::set_permissions(&script,fs::Permissions::from_mode(0o700)).unwrap();
        let output=Command::new(std::env::current_exe().unwrap()).args(["--exact","notification_tests::delivery_child"]).env("OT3_NOTIFY_TEST_ROOT",&root).env("PATH",format!("{}:{}",root.join("bin").display(),std::env::var("PATH").unwrap_or_default())).output().unwrap();
        assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stdout));
        let args=fs::read_to_string(root.join("delivered")).unwrap();let lines=args.lines().collect::<Vec<_>>();assert_eq!(lines.iter().filter(|&&s|s=="--icon").count(),1);let position=lines.iter().position(|&s|s=="--icon").unwrap();let image=image::open(lines[position+1]).unwrap();assert_eq!((image.width(),image.height()),(96,96));assert!(args.contains("Input required"));
        fs::remove_dir_all(root).unwrap();
    }
}
