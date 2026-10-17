//! T3's completed single-line shell fence actions. Data only; nothing executes here.
use pulldown_cmark::{CodeBlockKind,Event,Parser,Tag,TagEnd};
use serde_json::{json,Value};
/// Match native Cc/Cf rejection, including bidi and invisible format characters.
pub fn valid_command(command:&str)->bool{
 !command.trim().is_empty()&&command.len()<=16384&&!command.ends_with('\\')&&!command.chars().any(|c|c.is_control()||matches!(c as u32,0x00ad|0x0600..=0x0605|0x061c|0x06dd|0x070f|0x0890..=0x0891|0x08e2|0x180e|0x200b..=0x200f|0x2028..=0x202e|0x2060..=0x206f|0xfeff|0xfff9..=0xfffb|0x110bd|0x110cd|0x13430..=0x13455|0x1bca0..=0x1bca3|0x1d173..=0x1d17a|0xe0001|0xe0020..=0xe007f))
}
fn closed(source:&str)->bool{
 let opening=source.trim_start().chars().take_while(|c|*c=='`'||*c=='~').collect::<String>();if opening.len()<3{return false}
 let closing=source.lines().last().unwrap_or("").trim_start_matches([' ','\t','>']);let n=closing.chars().take_while(|c|Some(*c)==opening.chars().next()).count();n>=opening.len()&&closing[n..].trim().is_empty()&&source.contains('\n')
}
pub fn parse(markdown:&str)->Vec<Value>{
 let mut blocks=vec![];let mut current:Option<(String,String,bool)>=None;
 for (event,range) in Parser::new(markdown).into_offset_iter(){match event{
  Event::Start(Tag::CodeBlock(kind))=>{let (language,fenced)=match kind{CodeBlockKind::Fenced(info)=>(info.split_whitespace().next().unwrap_or("").to_string(),true),CodeBlockKind::Indented=>(String::new(),false)};current=Some((language,String::new(),fenced&&closed(&markdown[range])));},
  Event::Text(code) if current.is_some()=>{if let Some((_,text,_))=&mut current{text.push_str(&code)}},
  Event::End(TagEnd::CodeBlock)=>{if let Some((language,code,complete))=current.take(){let command=code.strip_suffix('\n').unwrap_or(&code);let runnable=complete&&code.ends_with('\n')&&matches!(language.as_str(),"sh"|"bash"|"zsh"|"fish"|"shell"|"powershell"|"pwsh")&&valid_command(command);blocks.push(json!({"index":blocks.len(),"language":language,"code":code,"command":if runnable{Some(command.trim())}else{None}}));if blocks.len()>=128{break}}},
  _=>{}
 }}blocks
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn same_closed_single_line_policy_as_native_t3(){for text in ["```bash\necho hello\n```","~~~sh\necho tilde\n~~~","> ```bash\n> echo quote\n> ```","````bash\necho four\n````"]{let b=parse(text);assert!(b[0]["command"].is_string(),"{text}: {b:?}")}
 for text in ["```bash\necho one\necho two\n```","```typescript\necho nope\n```","```bash\n\n```","```bash\necho hi\n\n```","```bash\necho hi\\\n```","```bash\necho hi\u{202e}\n```","```bash\necho hi\u{200b}\n```","```bash\necho incomplete","~~~bash\necho incomplete","````bash\necho incomplete\n```","<pre><code class=\"language-bash\">echo html</code></pre>"]{assert!(parse(text).iter().all(|b|b["command"].is_null()),"{text}")}}
}
