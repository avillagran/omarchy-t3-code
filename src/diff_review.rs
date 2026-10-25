//! Bounded, read-only presentation of the native checkpoint diff RPC's `diff` string.
//! The transport remains `orchestration.getTurnDiff` / `getFullThreadDiff` in `write.rs`.
use serde_json::{json, Value};
pub use t3_protocol::diff_review::{present, scoped_result};

/// Read and validate one native checkpoint range, without retaining raw patches.
pub fn execute(e:&crate::Environment,input:&Value)->crate::Result<Value>{
    use crate::{text,valid_id};

            let from=input["fromTurnCount"].as_u64().ok_or("invalid_range")?;
            let to=input["toTurnCount"].as_u64().filter(|n|*n>=from).ok_or("invalid_range")?;
            let thread=text(input,"thread");if !valid_id(thread){return Err("invalid_id".into())}
            if input["summary"]["fromTurnCount"].as_u64()!=Some(from)||input["summary"]["toTurnCount"].as_u64()!=Some(to){return Err("invalid_diff_range".into())}
            let whitespace=match input.get("ignoreWhitespace"){None=>true,Some(v)=>v.as_bool().ok_or("invalid_diff_option")?};
            use crate::protocol::api::orchestration as o;
            // The function and its arguments are kept with the review snapshot, which re-reads them.
            let mut payload=json!({"threadId":thread,"toTurnCount":to,"ignoreWhitespace":whitespace});
            let tag=if from==0{o::GET_FULL_THREAD_DIFF.name}else{payload["fromTurnCount"]=json!(from);o::GET_TURN_DIFF.name};
            let connection=e.connection(false)?;
            let result=if from==0{o::full_thread_diff(&connection,thread,to,Some(whitespace))}else{o::turn_diff(&connection,thread,from,to,Some(whitespace))}?;
            if result["threadId"]!=thread{return Err("invalid_diff_thread".into())}
            if result["fromTurnCount"].as_u64()!=Some(from)||result["toTurnCount"].as_u64()!=Some(to){return Err("invalid_diff_range".into())}
            let mut view=present(&result,&input["summary"])?;
            if view["files"].as_array().is_some_and(|fs|fs.iter().any(crate::review_document::has_text_patch)){
                let id=crate::review_document::remember_checkpoint(&e.id,thread,tag,&payload,&result,view["files"].as_array().unwrap())?;
                for file in view["files"].as_array_mut().unwrap(){if crate::review_document::has_text_patch(file){file["reviewSnapshotId"]=json!(id);}}
            }
            Ok(view)

}
