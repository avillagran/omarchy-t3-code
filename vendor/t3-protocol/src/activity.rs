//! Compact, safe tool presentation shared with mobile. Never returns tool inputs/outputs.
use serde_json::{json, Value};
fn text<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("")
}
pub fn item(row: &Value) -> &Value {
    if row["item"].is_object() {
        &row["item"]
    } else {
        row
    }
}
// Inspect shell wrappers without executing or returning arguments. Bounded input
// and depth keep malformed provider strings cheap and fail closed.
fn words(raw:&str)->Vec<String>{
 let mut out=Vec::new();let mut token=String::new();let mut quote='\0';let mut escaped=false;
 for c in raw.chars().take(8192){
  if escaped{token.push(c);escaped=false;continue}
  if c=='\\' && quote!='\''{escaped=true;continue}
  if quote!='\0'{if c==quote{quote='\0'}else{token.push(c)};continue}
  if c=='\''||c=='"'{quote=c;continue}
  if c.is_whitespace(){if !token.is_empty(){out.push(std::mem::take(&mut token));if out.len()>=128{break}}}
  else{token.push(c)}
 }
 if !token.is_empty(){out.push(token)}out
}
fn command_program(raw:&str,depth:usize)->Option<String>{
 if depth>4{return None}let args=words(raw);let mut i=0;
 while args.get(i).is_some_and(|s|s.split_once('=').is_some_and(|(k,_)|!k.is_empty()&&k.chars().all(|c|c.is_ascii_alphanumeric()||c=='_'))){i+=1}
 let first=args.get(i)?.rsplit('/').next()?;
 if matches!(first,"bash"|"sh"|"zsh"|"dash"|"ksh"){
  if let Some(n)=args.iter().enumerate().skip(i+1).find(|(_,a)|a.starts_with('-')&&a.contains('c')).map(|(n,_)|n){return command_program(args.get(n+1)?,depth+1)}
 }
 if matches!(first,"env"|"command"|"exec"){
  i+=1;while args.get(i).is_some_and(|a|a.starts_with('-')||a.contains('=')){i+=1}
  return command_program(&args[i..].join(" "),depth+1)
 }
 if !first.is_empty()&&first.len()<=64&&first.chars().all(|c|c.is_ascii_alphanumeric()||"._-".contains(c)){Some(first.into())}else{None}
}
pub fn describe(row: &Value) -> Option<Value> {
    let i = item(row);
    let typ = text(i, "type");
    if matches!(
        typ,
        "checkpoint" | "todo_list" | "user_message" | "assistant_message" | "system_notice" | "error" | ""
    ) {
        return None;
    }
    let name = text(i, "toolName").to_ascii_lowercase();
    let raw = i["input"].as_str().unwrap_or("");
    let command = i["input"]["command"]
        .as_str()
        .or_else(|| i["input"]["cmd"].as_str())
        .unwrap_or(raw)
        .trim_start();
    let program=command_program(command,0).unwrap_or_default();
    let first=program.as_str();
    let executable=if typ=="command_execution" {first}else{""};
    let category = if typ == "compaction" {
        "compact"
    } else if typ == "reasoning" {
        "think"
    } else if typ == "file_change"
        || name.contains("write")
        || name.contains("edit")
        || name.contains("patch")
    {
        "edit"
    } else if typ == "file_search"
        || name.contains("search")
        || name.contains("grep")
        || matches!(first, "rg" | "grep" | "find" | "fd")
    {
        if typ == "web_search" || name.contains("web") {
            "web"
        } else {
            "search"
        }
    } else if name.contains("read") || matches!(first, "cat" | "sed" | "head" | "tail" | "less") {
        "read"
    } else if typ == "web_search" {
        "web"
    } else if typ == "subagent" {
        "agent"
    } else if typ == "command_execution" || name.contains("exec") || name.contains("shell") {
        "execute"
    } else {
        "tool"
    };
    let title = if category == "think" {
        String::new()
    } else {
        i["input"]["title"]
            .as_str()
            .filter(|_| typ == "dynamic_tool")
            .unwrap_or_else(||{let title=text(i,"title");if !title.is_empty(){title}else if typ=="file_change"{text(i,"fileName")}else if typ=="dynamic_tool"{text(i,"toolName")}else{""}})
            .chars()
            .take(180)
            .collect()
    };
    Some(
        json!({"id":i["id"],"type":typ,"category":category,"title":title,"command":executable,"runId":i["runId"],"status":i["status"],"failed":text(i,"status")=="failed"||i["outputIndicatesFailure"]==true,"active":matches!(text(i,"status"),"running"|"in_progress"|"inProgress"|"started")}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shell_wrappers_report_program_without_arguments(){
      for raw in ["bash -lc 'rg private.txt'", "env LANG=C /bin/sh -c \"sed -n 1p secret.txt\"", "FOO=private /usr/bin/cat secret.txt"] {
        let a=describe(&json!({"type":"command_execution","input":raw,"status":"running"})).unwrap();
        assert!(matches!(a["command"].as_str(),Some("rg"|"sed"|"cat")));assert!(!a.to_string().contains("private"));assert!(!a.to_string().contains("secret"));
      }
    }
    #[test]
    fn tool_context_is_classified_without_exporting_payloads() {
        assert_eq!(describe(&json!({"type":"compaction","status":"running"})).unwrap()["category"],"compact");
        let read=describe(&json!({"item":{"id":"a","type":"command_execution","input":"sed -n '1,5p' secret.txt","output":"private output","status":"running"}})).unwrap();
        assert_eq!(read["category"], "read");
        assert!(!read.to_string().contains("secret.txt"));
        assert!(!read.to_string().contains("private output"));
        assert_eq!(
            describe(
                &json!({"type":"dynamic_tool","toolName":"read_file","input":{"path":"file.txt"}})
            )
            .unwrap()["category"],
            "read"
        );
    }
}
