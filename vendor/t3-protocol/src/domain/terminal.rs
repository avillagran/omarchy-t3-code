//! Terminal helpers through the existing JSON/JNI ABI; no new native handles.
use super::domain_fn;
use crate::terminal;
use serde_json::json;
pub const FUNCTIONS: &[super::DomainFn] = &[
    domain_fn! {name:"terminal.context", module:"terminal", args:[(shell,Object,required),(threadId,Id,required),(fallback,Path,required)], returns:"object {cwd,worktreePath?,providerInstanceId?}: selected server paths", run:|a| terminal::context(a.req_object("shell")?,a.req("threadId")?,a.req("fallback")?)},
    domain_fn! {name:"terminal.attach", module:"terminal", args:[(threadId,Id,required),(terminalId,Token,required),(context,Object,required),(cols,Integer,required),(rows,Integer,required),(resume,Bool,optional)], returns:"object: validated terminal.attach args; resume=true preserves stopped terminals, never use an ordering cursor as afterSequence", run:|a| {
     let cols=u16::try_from(a.req_u64("cols")?).map_err(|_|"invalid_terminal_size")?;
     let rows=u16::try_from(a.req_u64("rows")?).map_err(|_|"invalid_terminal_size")?;
     let mut p=terminal::attach(a.req("threadId")?,a.req("terminalId")?,a.req_object("context")?,cols,rows)?;
     if a.bool("resume")?.unwrap_or(false){p["restartIfNotRunning"]=json!(false);p["resume"]=json!(true);}
     // Validate optional context members with the wire builder before returning.
     crate::api::streams::TERMINAL_ATTACH.payload(&p)
    }},
    domain_fn! {name:"terminal.output", module:"terminal", args:[(state,Object,optional),(event,Object,required)], returns:"object {state,text,replace,finished,error,exitCode,exitSignal}: exact ordered ANSI/Unicode strings; snapshot replaces renderer history; remote error retained with state", run:|a| {
     let mut output=terminal::Output::from_state(a.get("state").unwrap_or(&json!({})))?;
     let event=a.req_object("event")?;
     let (text,error)=match output.apply(event){Ok(t)=>(t,None),Err(e)=>(None,Some(e))};
     Ok(json!({"state":output.state(),"text":text,"replace":matches!(event["type"].as_str(),Some("snapshot"|"started"|"restarted")),"finished":output.finished,"error":error,"exitCode":event.get("exitCode").or_else(||event["snapshot"].get("exitCode")),"exitSignal":event.get("exitSignal").or_else(||event["snapshot"].get("exitSignal"))}))
    }},
];
