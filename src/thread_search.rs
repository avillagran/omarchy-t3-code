//! Server-wide thread search (T3 orchestration.searchThreads) across every connected
//! environment. Environments that cannot search are skipped, not fatal.
use crate::{environments, json, protocol::api::orchestration, text, Result};
use serde_json::Value;
use std::{collections::HashMap, sync::mpsc};

pub fn search(input: &Value) -> Result<Value> {
    search_with_progress(input, |_| {})
}

/// Publish each environment's actual answer while slower requests remain live.
/// The caller owns every worker through its terminal join; observers never
/// cancel or restart an in-flight transport merely because another one replied.
pub fn search_with_progress(input: &Value, mut progress: impl FnMut(Value)) -> Result<Value> {
    let query = text(input, "query").trim().to_string();
    let length = query.encode_utf16().count();
    if !(2..=200).contains(&length) {
        return Ok(json!({"query":query,"matches":[],"rows":[],"pending":0}));
    }
    let selected = input["environmentIds"].as_array();
    let environments: Vec<_> = environments()?
        .into_iter()
        .filter(|environment| {
            selected.is_none_or(|ids| ids.iter().any(|id| id.as_str() == Some(&environment.id)))
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
        let options = input.clone();
        handles.push((
            id.clone(),
            std::thread::spawn(move || {
                let found = environment.connection(true).and_then(|connection| {
                    let search_messages = options["searchMessages"].as_bool().unwrap_or(true);
                    let mut args = json!({"query":query});
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
                            args["olderSettledBefore"] = json!(before);
                            if options["projectIds"].is_array() {
                                args["projectIds"] = options["projectIds"].clone();
                            }
                            if let Some(label) =
                                options["environmentLabels"][&environment.id].as_str()
                            {
                                args["environmentLabel"] = json!(label);
                            }
                            if !search_messages {
                                args["titlesOnly"] = json!(true);
                            }
                        }
                    }
                    if !search_messages && !older {
                        return Ok(json!({"matches":[]}));
                    }
                    let result = orchestration::SEARCH_THREADS.call(&connection, &args, None)?;
                    Ok(result)
                });
                let _ = sender.send((id, found));
            }),
        ));
    }
    drop(sender);
    let mut pending = handles.len();
    let mut answers = HashMap::new();
    let mut errors = json!({});
    let assemble = |answers: &HashMap<String, Value>, errors: &Value, pending: usize| {
        let mut matches = Vec::new();
        let mut rows = Vec::new();
        for environment in &order {
            let Some(value) = answers.get(environment) else {
                continue;
            };
            for matched in value["matches"].as_array().into_iter().flatten() {
                let mut matched = matched.clone();
                matched["environmentId"] = json!(environment);
                matches.push(matched);
            }
            if value["threads"]
                .as_array()
                .is_some_and(|threads| !threads.is_empty())
            {
                rows.push(json!({"environmentId":environment,"read":{
                    "snapshotSequence":value["snapshotSequence"].as_u64().unwrap_or(0),"threads":value["threads"]}}));
            }
        }
        json!({"query":query,"matches":matches,"rows":rows,"errors":errors,"pending":pending,"partial":pending>0})
    };
    for (environment, found) in receiver {
        pending -= 1;
        match found {
            Ok(value) => {
                answers.insert(environment, value);
            }
            Err(error) => errors[environment] = json!(error),
        }
        if pending > 0 {
            progress(assemble(&answers, &errors, pending));
        }
    }
    for (environment, handle) in handles {
        if handle.join().is_err() {
            errors[environment] = json!("thread_search_worker_failed");
        }
    }
    Ok(assemble(&answers, &errors, 0))
}
