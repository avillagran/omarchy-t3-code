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
