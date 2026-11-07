//! Flat locale → English-source → translation catalogs. No OS or framework dependency.
use serde_json::Value;
pub fn normalize(locale:&str)->String{locale.split(':').next().unwrap_or("").split(['.','@']).next().unwrap_or("").replace('_',"-").to_lowercase()}
pub fn translate(catalog:&Value,locale:&str,source:&str)->String{
 for requested in locale.split(':'){
  let locale=normalize(requested);let base=locale.split('-').next().unwrap_or("en");let base=if matches!(base,"nb"|"nn"){"no"}else{base};
  for l in [locale.as_str(),base]{if let Some(s)=catalog[l][source].as_str().filter(|s|!s.is_empty()){return s.to_string()}}
 }
 catalog["en"][source].as_str().unwrap_or(source).to_string()}
#[cfg(test)]
mod tests {
 use super::*;
 use serde_json::json;
 #[test]
 fn flat_catalog_uses_regional_base_and_english_fallback() {
  let c=json!({"en":{"Close":"Close","Tasks":"Tasks"},"fr":{"Close":"Fermer"},"no":{"Close":"Lukk"}});
  assert_eq!(normalize("fr_CA.UTF-8:en"),"fr-ca");
  assert_eq!(translate(&c,"fr_CA.UTF-8","Close"),"Fermer");
  assert_eq!(translate(&c,"zz:fr_CA.UTF-8","Close"),"Fermer");
  assert_eq!(translate(&c,"fr","Tasks"),"Tasks");
  assert_eq!(translate(&c,"zz","Unknown"),"Unknown");
  assert_eq!(translate(&c,"nb_NO.UTF-8","Close"),"Lukk");
 }
}

// ot3's own catalog and its lookup (`ui/qml/T3I18n.qml`), shipped with the library.

/// Desktop string catalog (55c0ce3), 20 locales with `{name}` placeholders.
/// Retains previously exported sources, including awareness card/alert strings.
pub const CATALOG_JSON: &str = include_str!("../data/i18n.json");
/// ot3's language picker: the catalog's locales and their own names.
pub const LANGUAGES: &[(&str, &str)] = &[("en","English"),("es","Español"),("pt","Português"),("fr","Français"),("de","Deutsch"),("it","Italiano"),("nl","Nederlands"),("pl","Polski"),("ru","Русский"),("uk","Українська"),("ja","日本語"),("ko","한국어"),("zh","中文"),("ar","العربية"),("tr","Türkçe"),("sv","Svenska"),("da","Dansk"),("no","Norsk"),("fi","Suomi"),("cs","Čeština")];
/// The catalog, parsed once.
pub fn catalog()->&'static Value{
 static CATALOG:std::sync::OnceLock<Value>=std::sync::OnceLock::new();
 CATALOG.get_or_init(||serde_json::from_str(CATALOG_JSON).expect("data/i18n.json"))}
fn base(locale:&str)->&str{let b=locale.split('-').next().unwrap_or("");if matches!(b,"nb"|"nn"){"no"}else{b}}
/// ot3's locale choice for a POSIX or BCP 47 locale list (`fr_CA.UTF-8:en`):
/// the first one the catalog has, by itself or by its language, else the
/// first one. `(locale, language, rtl)`, `rtl` for Arabic.
pub fn resolve(locales:&str)->(String,String,bool){
 let c=catalog();
 let candidates=locales.split(':').map(normalize).collect::<Vec<_>>();
 let found=candidates.iter().find(|l|c.get(l.as_str()).is_some()||c.get(base(l)).is_some()).cloned();
 let locale=found.or_else(||candidates.first().filter(|l|!l.is_empty()).cloned()).unwrap_or_else(||"en".into());
 let language=base(&locale).to_string();let rtl=language=="ar";
 (locale,language,rtl)}
/// T3I18n's isolates around the composer's literal tokens in Arabic.
const ISOLATED:&[&str]=&["@files/folders","$skills","/commands","Ctrl+Enter"];
/// ot3's `tr(source, args)` for `locales`: the locale's string, its
/// language's, English's or the source; `{name}` placeholders replaced by
/// `args` (JavaScript's String of each value) and left as they are without one;
/// in Arabic the composer's tokens and every argument wrapped in U+2068/U+2069.
pub fn t(locales:&str,source:&str,args:Option<&Value>)->String{render(&resolve(locales),source,args)}
/// Every catalog string for `locales` as `t` gives it without arguments, with
/// `resolve`'s `{locale, language, rtl}`.
pub fn strings(locales:&str)->Value{
 let resolved=resolve(locales);
 let strings=catalog()["en"].as_object().into_iter().flatten().map(|(k,_)|(k.clone(),Value::String(render(&resolved,k,None)))).collect::<serde_json::Map<_,_>>();
 serde_json::json!({"locale":resolved.0,"language":resolved.1,"rtl":resolved.2,"strings":strings})}
fn render((locale,language,rtl):&(String,String,bool),source:&str,args:Option<&Value>)->String{
 let (rtl,c)=(*rtl,catalog());
 let found=[locale.as_str(),language.as_str(),"en"].iter().find_map(|l|c[*l][source].as_str().filter(|s|!s.is_empty()));
 let mut value=found.unwrap_or(source).to_string();
 if rtl{
  let mut out=String::new();let mut rest=value.as_str();
  'scan:while !rest.is_empty(){
   for token in ISOLATED{if let Some(after)=rest.strip_prefix(token){out.push('\u{2068}');out.push_str(token);out.push('\u{2069}');rest=after;continue 'scan}}
   let ch=rest.chars().next().unwrap();out.push(ch);rest=&rest[ch.len_utf8()..];
  }
  value=out;
 }
 placeholders(&value,args,rtl)}
/// `/\{([A-Za-z_][A-Za-z0-9_]*)\}/g` replaced from `args`.
fn placeholders(value:&str,args:Option<&Value>,rtl:bool)->String{
 let mut out=String::with_capacity(value.len());let mut rest=value;
 while let Some(open)=rest.find('{'){
  out.push_str(&rest[..open]);let tail=&rest[open+1..];
  let end=tail.find(|ch:char|!(ch.is_ascii_alphanumeric()||ch=='_')).unwrap_or(tail.len());
  let name=&tail[..end];
  let valid=!name.is_empty()&&!name.starts_with(|ch:char|ch.is_ascii_digit())&&tail[end..].starts_with('}');
  match args.and_then(|a|a.get(name)).filter(|_|valid){
   Some(v)=>{let v=crate::js::string(v);if rtl{out.push('\u{2068}');out.push_str(&v);out.push('\u{2069}')}else{out.push_str(&v)}rest=&tail[end+1..]}
   None=>{out.push('{');rest=tail}
  }
 }
 out.push_str(rest);out}
#[cfg(test)]
mod catalog_tests {
 use super::*;
 use serde_json::json;
 #[test]
 fn ot3_catalog_lookup_placeholders_and_isolates(){
  assert_eq!(catalog().as_object().unwrap().len(),20);
  assert_eq!(resolve("fr_CA.UTF-8:en"),("fr-ca".into(),"fr".into(),false));
  assert_eq!(resolve("zz:de"),("de".into(),"de".into(),false));
  assert_eq!(resolve("zz"),("zz".into(),"zz".into(),false));
  assert_eq!(resolve(""),("en".into(),"en".into(),false));
  assert_eq!(resolve("nb_NO").1,"no");
  assert_eq!(t("es","Close",None),"Cerrar");
  assert_eq!(t("en","{n} working",Some(&json!({"n":2}))),"2 working");
  assert_eq!(t("en","{n} working",None),"{n} working");
  assert_eq!(t("en","Not in the catalog {x} {{y}} {1a}",Some(&json!({"x":1.5,"y":[1,2],"1a":"no"}))),"Not in the catalog 1.5 {1,2} {1a}");
  let ar=t("ar","{n} working",Some(&json!({"n":3})));
  assert!(ar.contains("\u{2068}3\u{2069}"),"{ar}");
 }
}
