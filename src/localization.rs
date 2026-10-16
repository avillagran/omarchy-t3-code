//! Native notification labels share the plugin's flat catalog and language preference.
use omarchy_t3_client::*;
use serde_json::Value;
use std::sync::OnceLock;
static CATALOG:OnceLock<Value>=OnceLock::new();
pub fn label(source:&str)->String{
 let prefs=crate::ui_state::load();
 let explicit=text(&prefs,"language");
 let system=std::fs::read_to_string("/etc/locale.conf").unwrap_or_default().lines().find_map(|l|l.trim().strip_prefix("LANG=").map(|s|s.trim_matches(['\'','"']).to_string())).unwrap_or_default();
 let locale=if !explicit.is_empty()&&explicit!="system"{explicit.to_string()}else{["OMARCHY_T3_LANGUAGE","LANGUAGE","LC_ALL","LC_MESSAGES"].iter().filter_map(|key|std::env::var(key).ok()).find(|v|!v.is_empty()).unwrap_or_else(||if !system.is_empty(){system}else{std::env::var("LANG").unwrap_or_else(|_|"en".into())})};
 protocol::i18n::translate(CATALOG.get_or_init(||serde_json::from_str(include_str!("../i18n.json")).unwrap_or_default()),&locale,source)
}
