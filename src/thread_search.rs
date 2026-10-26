//! Server-wide thread search (T3 orchestration.searchThreads) across every connected
//! environment. Environments that cannot search are skipped, not fatal.
use crate::{environments, json, protocol::api::orchestration, text, Result};
use serde_json::Value;

pub fn search(input: &Value) -> Result<Value> {
    let query = text(input, "query").trim().to_string();
    if query.chars().count() < 2 || query.chars().count() > 200 {
        return Ok(json!({"query":query,"matches":[]}));
    }
    let handles: Vec<_> = environments()?
        .into_iter()
        .map(|e| {
            let query = query.clone();
            std::thread::spawn(move || {
                let found = e.connection(true).and_then(|c| {
                    let mut args=json!({"query":query,"limit":20});
                    if crate::server_capabilities::get(&e.id,&c)?.shell_older_settled {
                        let shell=crate::shell_state::live(&e.id).map(Ok).unwrap_or_else(||crate::nightly::shell(&crate::Client::shared()?,&e))?;
                        if let Some(before)=shell["olderSettled"]["before"].as_str(){args["olderSettledBefore"]=json!(before);args["environmentLabel"]=json!(e.name);}
                    }
                    let result=orchestration::SEARCH_THREADS.call(&c,&args,None)?;
                    if result["threads"].is_array(){crate::shell_state::supplement(&e.id,&result)?;}Ok(result)
                });
                (e.id, found)
            })
        })
        .collect();
    let mut matches = vec![];
    let mut errors = json!({});
    for handle in handles {
        let Ok((environment, found)) = handle.join() else { continue };
        match found {
            Ok(v) => {
                for m in v["matches"].as_array().into_iter().flatten().take(50) {
                    let mut m = m.clone();
                    m["environmentId"] = json!(environment);
                    matches.push(m);
                }
            }
            Err(error) => errors[environment] = json!(error),
        }
    }

    Ok(json!({"query":query,"matches":matches,"errors":errors}))
}
