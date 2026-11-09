//! Real cross-environment search with lossless native JSON request/results.
use crate::{
    environments, json,
    protocol::{api::orchestration, wtf16::Wtf16String, RawJson, RawRpcExit},
    search_json::{self, Object},
    Result,
};
use serde_json::Value;
use std::{collections::HashMap, sync::mpsc};

/// Scalar callers keep their existing API. The Qt pipe uses the raw path below.
pub fn search(input: &Value) -> Result<Value> {
    search_with_progress(input, |_| {})
}
pub fn search_with_progress(input: &Value, mut progress: impl FnMut(Value)) -> Result<Value> {
    let mut invalid = false;
    let result = search_raw(input, |value| match serde_json::from_str(value.get()) {
        Ok(value) => progress(value),
        Err(_) => invalid = true,
    })?;
    if invalid {
        return Err("search_requires_raw_json".into());
    }
    serde_json::from_str(result.get()).map_err(|_| "search_requires_raw_json".into())
}
/// The pipe wraps validated JSON as syntax so scalar host framing never touches
/// native text. Qt parses that syntax back into its original UTF-16 domain.
pub fn search_packed(input: &Value, mut progress: impl FnMut(Value)) -> Result<Value> {
    let result = search_raw(input, |value| progress(json!({"resultJson":value.get()})))?;
    Ok(json!({"resultJson":result.get()}))
}
fn empty(query: &Wtf16String) -> Result<Box<RawJson>> {
    let mut result = Object::new();
    result.insert("query".into(), query.to_json());
    for name in ["matches", "rows"] {
        result.insert(name.into(), search_json::encode(&Vec::<Value>::new())?);
    }
    result.insert("pending".into(), search_json::encode(&0)?);
    search_json::encode(&result)
}
/// Every producer is joined through actual terminal. A slower environment never
/// cancels an admitted sibling or causes its result to be reissued.
pub fn search_raw(input: &Value, mut progress: impl FnMut(Box<RawJson>)) -> Result<Box<RawJson>> {
    let options = search_json::request(input)?;
    let query = Wtf16String::from_json(search_json::field(&options, "query")?)
        .map_err(|_| "invalid_search_query")?
        .trim_ecmascript();
    if !(2..=200).contains(&query.units().len()) {
        return empty(&query);
    }
    let selected = options
        .get("environmentIds")
        .map(|raw| search_json::scalar::<Vec<String>>(raw))
        .transpose()?;
    let environments: Vec<_> = environments()?
        .into_iter()
        .filter(|environment| {
            selected
                .as_ref()
                .is_none_or(|ids| ids.contains(&environment.id))
        })
        .collect();
    let order: Vec<_> = environments
        .iter()
        .map(|environment| environment.id.clone())
        .collect();
    let (sender, receiver) = mpsc::channel();
    let mut handles = Vec::with_capacity(environments.len());
    for environment in environments {
        let id = environment.id.clone();
        let sender = sender.clone();
        let query = query.clone();
        let options = options.clone();
        handles.push((
            id.clone(),
            std::thread::spawn(move || {
                let found = (|| -> Result<RawRpcExit> {
                    let connection = environment.connection(true)?;
                    let search_messages = options
                        .get("searchMessages")
                        .map(|raw| search_json::scalar::<bool>(raw))
                        .transpose()?
                        .unwrap_or(true);
                    let mut args = Object::new();
                    args.insert("query".into(), query.to_json());
                    let mut older = false;
                    if crate::server_capabilities::get(&environment.id, &connection)?
                        .shell_older_settled
                    {
                        let shell = crate::shell_state::live(&environment.id)
                            .map(Ok)
                            .unwrap_or_else(|| {
                                crate::nightly::shell(&crate::Client::shared()?, &environment)
                            })?;
                        if let Some(before) = shell["olderSettled"]["before"].as_str() {
                            older = true;
                            args.insert("olderSettledBefore".into(), search_json::encode(&before)?);
                            if let Some(projects) = options.get("projectIds") {
                                let ids: Vec<String> = search_json::scalar(projects)?;
                                args.insert("projectIds".into(), search_json::encode(&ids)?);
                            }
                            if let Some(labels) = options.get("environmentLabels") {
                                let labels = search_json::object(labels)?;
                                if let Some(label) = labels.get(&environment.id) {
                                    Wtf16String::from_json(label)
                                        .map_err(|_| "invalid_search_options")?;
                                    args.insert("environmentLabel".into(), label.clone());
                                }
                            }
                            if !search_messages {
                                args.insert("titlesOnly".into(), search_json::encode(&true)?);
                            }
                        }
                    }
                    if !search_messages && !older {
                        return Ok(RawRpcExit::Success(Some(search_json::encode(
                            &json!({"matches":[]}),
                        )?)));
                    }
                    // Preserve all existing typed option validators. A raw
                    // label has the same proven native String type, without
                    // forcing its units through a scalar Value.
                    let mut checked = serde_json::Map::new();
                    for (key, value) in &args {
                        if key != "query" && key != "environmentLabel" {
                            checked.insert(key.clone(), search_json::scalar::<Value>(value)?);
                        }
                    }
                    if let Some(label) = args.get("environmentLabel") {
                        if let Ok(label) = search_json::scalar::<String>(label) {
                            checked.insert("environmentLabel".into(), Value::String(label));
                        }
                    }
                    let validated = orchestration::search_threads_utf16_payload(
                        &query,
                        &Value::Object(checked),
                    )?;
                    let mut validated = search_json::object(&validated)?;
                    if let Some(label) = args.get("environmentLabel") {
                        validated.insert("environmentLabel".into(), label.clone());
                    }
                    connection.rpc_raw_with_timeout(
                        orchestration::SEARCH_THREADS.name,
                        &search_json::encode(&validated)?,
                        orchestration::SEARCH_THREADS.timeout(),
                    )
                })();
                let _ = sender.send((id, found));
            }),
        ));
    }
    drop(sender);
    let mut pending = handles.len();
    let mut answers = HashMap::new();
    let mut errors = Object::new();
    let assemble = |answers: &HashMap<String, Box<RawJson>>,
                    errors: &Object,
                    pending: usize|
     -> Result<Box<RawJson>> {
        let mut matches = Vec::new();
        let mut rows = Vec::new();
        for environment in &order {
            let Some(value) = answers.get(environment) else {
                continue;
            };
            let value = search_json::object(value)?;
            if let Some(found) = value.get("matches") {
                for matched in search_json::array(found)? {
                    let mut matched = search_json::object(&matched)?;
                    matched.insert("environmentId".into(), search_json::encode(environment)?);
                    matches.push(search_json::encode(&matched)?);
                }
            }
            if let Some(threads) = value.get("threads") {
                if !search_json::array(threads)?.is_empty() {
                    let mut read = Object::new();
                    read.insert("threads".into(), threads.clone());
                    let sequence = value
                        .get("snapshotSequence")
                        .map(|raw| search_json::scalar::<u64>(raw))
                        .transpose()?
                        .unwrap_or(0);
                    read.insert("snapshotSequence".into(), search_json::encode(&sequence)?);
                    let mut page = Object::new();
                    page.insert("environmentId".into(), search_json::encode(environment)?);
                    page.insert("read".into(), search_json::encode(&read)?);
                    rows.push(page);
                }
            }
        }
        let mut result = Object::new();
        result.insert("query".into(), query.to_json());
        result.insert("matches".into(), search_json::encode(&matches)?);
        result.insert("rows".into(), search_json::encode(&rows)?);
        result.insert("errors".into(), search_json::encode(errors)?);
        result.insert("pending".into(), search_json::encode(&pending)?);
        result.insert("partial".into(), search_json::encode(&(pending > 0))?);
        search_json::encode(&result)
    };
    let mut assembly_error = None;
    for (environment, found) in receiver {
        pending -= 1;
        match found {
            Ok(RawRpcExit::Success(Some(value))) => {
                answers.insert(environment, value);
            }
            Ok(RawRpcExit::Success(None)) => {
                errors.insert(environment, search_json::encode(&"invalid_search_result")?);
            }
            Ok(RawRpcExit::Failure(cause)) => {
                let mut error = Object::new();
                error.insert("code".into(), search_json::encode(&"rpc_rejected")?);
                error.insert("cause".into(), cause);
                errors.insert(environment, search_json::encode(&error)?);
            }
            Err(error) => {
                errors.insert(environment, search_json::encode(&error)?);
            }
        }
        if pending > 0 {
            match assemble(&answers, &errors, pending) {
                Ok(value) => progress(value),
                Err(error) => assembly_error = Some(error),
            }
        }
    }
    for (environment, handle) in handles {
        if handle.join().is_err() {
            errors.insert(
                environment,
                search_json::encode(&"thread_search_worker_failed")?,
            );
        }
    }
    if let Some(error) = assembly_error {
        return Err(error);
    }
    assemble(&answers, &errors, 0)
}

/// Native visit strings belong to DateTime.make and pickedQuery, not an RFC
/// parser or a Rust scalar conversion. The existing action queue owns ordering.
pub fn visit_raw(input: &Value) -> Result<RawRpcExit> {
    let fields = search_json::request(input)?;
    let environment: String = search_json::scalar(search_json::field(&fields, "environment")?)?;
    let thread: String = search_json::scalar(search_json::field(&fields, "thread")?)?;
    if !crate::valid_id(&thread) {
        return Err("invalid_command".into());
    }
    let at = search_json::field(&fields, "visitedAt")?;
    Wtf16String::from_json(at).map_err(|_| "invalid_command")?;
    let mut command = Object::new();
    command.insert("type".into(), search_json::encode(&"thread.visit")?);
    command.insert(
        "commandId".into(),
        search_json::encode(&format!("omarchy-visit-{}", uuid::Uuid::new_v4()))?,
    );
    command.insert("threadId".into(), search_json::encode(&thread)?);
    command.insert("visitedAt".into(), fields["visitedAt"].clone());
    if let Some(opened) = fields.get("opened") {
        let opened: bool = search_json::scalar(opened).map_err(|_| "invalid_command")?;
        command.insert("opened".into(), search_json::encode(&opened)?);
    }
    if let Some(query) = fields.get("searchQuery") {
        let value = Wtf16String::from_json(query).map_err(|_| "invalid_command")?;
        if value.units().len() > 200 {
            return Err("invalid_command".into());
        }
        command.insert("searchQuery".into(), query.clone());
    }
    let environment = environments()?
        .into_iter()
        .find(|entry| entry.id == environment)
        .ok_or("unknown_environment")?;
    environment.connection(true)?.rpc_raw(
        orchestration::DISPATCH_COMMAND.name,
        &search_json::encode(&command)?,
    )
}
