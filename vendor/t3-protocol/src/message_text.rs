//! Repair only complete Qt rich-text documents; ordinary HTML/code remains literal.
pub fn plain_qt(input:&str)->Option<String>{
 let lower=input.to_ascii_lowercase();let trimmed=lower.trim_start();
 if !(trimmed.starts_with("<!doctype html")||trimmed.starts_with("<html"))||!(lower.contains("name=\"qrichtext\"")||lower.contains("name='qrichtext'")){return None}
 let html_end=lower.rfind("</html>")?+7;if !input[html_end..].trim().is_empty(){return None}
 let start=lower.find("<body")?;let start=start+lower[start..].find('>')?+1;let end=lower.rfind("</body>")?;
 let mut out=String::new();let body=&input[start..end];let mut at=0;
 while at<body.len(){if body.as_bytes()[at]==b'<'{let close=body[at..].find('>')?+at;let tag=body[at+1..close].split_whitespace().next().unwrap_or("").trim_end_matches('/').to_ascii_lowercase();if matches!(tag.as_str(),"br"|"/p"|"/div"|"/li"|"/tr"|"/pre"){out.push('\n')}else if tag=="li"{out.push_str("- ")}else if tag=="/td"{out.push('\t')}at=close+1;}else if body.as_bytes()[at]==b'&'{if let Some(end)=body[at..].find(';').filter(|n|*n<16){let e=&body[at+1..at+end];let ch=match e{"amp"=>Some('&'),"lt"=>Some('<'),"gt"=>Some('>'),"quot"=>Some('"'),"apos"=>Some('\''),"nbsp"=>Some(' '),_=>e.strip_prefix("#x").and_then(|s|u32::from_str_radix(s,16).ok()).or_else(||e.strip_prefix('#').and_then(|s|s.parse().ok())).and_then(char::from_u32)};if let Some(c)=ch{out.push(c);at+=end+1;continue}}
 out.push('&');at+=1;
 }else{let ch=body[at..].chars().next()?;out.push(ch);at+=ch.len_utf8();}}
 Some(out.trim().to_string())
}
#[cfg(test)]mod tests{use super::*;#[test]fn qt_document_is_text_and_code_stays_literal(){let doc="<!DOCTYPE HTML><html><head><meta name=\"qrichtext\" content=\"1\"/><style>private CSS</style></head><body><p>Hello &amp; &#x1f44b;</p><p>&lt;code&gt;</p><img src=\"https://never-load.invalid\"/></body></html>";assert_eq!(plain_qt(doc).unwrap(),"Hello & 👋\n<code>");assert!(plain_qt(&format!("{doc}\nKeep this trailing question")).is_none());assert!(plain_qt("<p>HTML example</p>").is_none());assert!(plain_qt("```html\n<html>example</html>\n```").is_none());}}
