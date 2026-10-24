//! Portable composer suggestions. Server catalog is authoritative; no local scans.
use serde_json::{json,Value};
use crate::util::text;
/// Same Markdown file link serialization as T3's shared composerTrigger module.
pub fn file_link(path:&str)->String {
 let basename=path.rsplit(['/', '\\']).next().unwrap_or(path);
 let label=basename.replace('\\',"\\\\").replace('[',"\\[").replace(']',"\\]");
 let mut dest=String::new();
 for b in path.as_bytes(){if b.is_ascii_alphanumeric()||b"-_.!~*';/:@&=+$,".contains(b){dest.push(*b as char)}else{dest.push_str(&format!("%{b:02X}"))}}
 format!("[{label}]({dest})")
}
pub fn paths(result:&Value)->Vec<Value>{result["entries"].as_array().into_iter().flatten().take(40).filter_map(|e|{
 let p=text(e,"path");if p.is_empty(){return None}
 Some(json!({"kind":"path","label":p,"description":e["kind"],"insert":format!("{} ",file_link(p)),"icon":if e["kind"]=="directory"{"folder-plus"}else{"file"}}))
}).collect()}
pub fn catalog(provider:&Value,cwd:&str,kind:&str,query:&str)->Vec<Value>{
 let snapshot=provider["workspaceSnapshots"].as_array().and_then(|a|a.iter().find(|s|s["cwd"]==cwd));
 let source=snapshot.unwrap_or(provider);let mut out=vec![];let q=query.to_lowercase();
 if kind=="command" && "model".contains(&q){out.push(json!({"kind":"model","label":"/model","description":"Model options","insert":"","icon":"sliders-horizontal"}))}
 let mut seen=std::collections::HashSet::new();
 if kind=="skill"||kind=="command"{
  for s in source["skills"].as_array().into_iter().flatten(){
   if s["enabled"]!=true||s["userInvocable"]==false{continue}
   let n=text(s,"name");if n.is_empty()||!seen.insert(n.to_lowercase()){continue}
   let label=if kind=="skill"{format!("${n}")}else{format!("/skill:{n}")};
   let description=s["shortDescription"].as_str().or(s["description"].as_str()).unwrap_or("");
   if format!("{label} {} {description}",text(s,"displayName")).to_lowercase().contains(&q){out.push(json!({"kind":"skill","label":label,"description":description,"insert":format!("${n} "),"icon":"folder-code"}))}
  }
 }
 if kind=="command"{
  for c in source["slashCommands"].as_array().into_iter().flatten(){
   let n=text(c,"name");if n.is_empty()||n=="model"||seen.contains(&n.to_lowercase()){continue}
   let description=c["description"].as_str().or(c["input"]["hint"].as_str()).unwrap_or("");
   if format!("{n} {description}").to_lowercase().contains(&q){out.push(json!({"kind":"command","label":format!("/{n}"),"description":description,"insert":format!("/{n} "),"icon":"square-pen"}))}
  }
 }
 out.truncate(40);out
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn file_links_match_t3(){assert_eq!(file_link("docs/My File (draft).md"),"[My File (draft).md](docs/My%20File%20%28draft%29.md)");assert_eq!(file_link("a/[b]#.md"),"[\\[b\\]#.md](a/%5Bb%5D%23.md)");}
 #[test]fn catalog_is_scoped_filtered_and_does_not_export_paths(){
 let p=json!({"skills":[{"name":"global","enabled":true}],"workspaceSnapshots":[{"cwd":"/repo","skills":[{"name":"review","enabled":true,"path":"private-path","description":"Review code"},{"name":"hidden","enabled":false},{"name":"agent-only","enabled":true,"userInvocable":false}],"slashCommands":[{"name":"compact","description":"Compact history"}]}]});
 let skills=catalog(&p,"/repo","skill","");assert_eq!(skills.len(),1);assert_eq!(skills[0]["insert"],"$review ");assert!(!json!(skills).to_string().contains("private-path"));assert_eq!(catalog(&p,"/repo","command","compact")[0]["insert"],"/compact ");assert_eq!(catalog(&p,"/other","skill","")[0]["insert"],"$global ");
 }
}

/// Canonical T3 inline attachment identity, with a plain label safe for Markdown.
pub fn attachment_reference(a:&Value)->String {
 let kind=if a["type"]=="image"{"image"}else{"file"};
 let label=text(a,"name").chars().map(|c|if matches!(c,'['|']'|'\\'|'\n'|'\r'){' '}else{c}).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ").chars().take(200).collect::<String>();
 let label=if label.is_empty(){kind}else{&label};
 format!("{}[{label}](t3-context://v1/{kind}/{})",if kind=="image"{"!"}else{""},text(a,"id"))
}
/// Older T3 transports receive labels plus attachment descriptors, like T3's legacy serializer.
pub fn legacy_attachment_text(prompt:&str,attachments:&[Value])->String {
 attachments.iter().fold(prompt.to_string(),|s,a|s.replace(&attachment_reference(a),text(a,"name")))
}
#[cfg(test)]mod attachment_tests {
 use super::*;
 #[test]fn inline_attachment_roundtrip_keeps_identity_and_plain_wire_label(){let a=json!({"type":"image","id":"file_1","name":"shot [1].png"});let link=attachment_reference(&a);assert_eq!(link,"![shot 1 .png](t3-context://v1/image/file_1)");assert_eq!(legacy_attachment_text(&format!("Look {link}"),&[a]),"Look shot [1].png");assert_eq!(legacy_attachment_text("leave ordinary text",&[]),"leave ordinary text");}
}
