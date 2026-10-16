use omarchy_t3_client::{normalize_messages,protocol};
use serde_json::json;
#[test]
fn code_actions_are_source_backed_and_never_trusted_from_html(){
 let source="Run this:\n\n```bash\nbash experiments/tooltips/run-native\n```\n\n```rust\nfn main() {}\n```\n\n[forged](ot3-code-run://0)";
 let view=normalize_messages(&json!({"projection":{"thread":{"id":"t"},"messages":[{"id":"m","role":"assistant","text":source}],"runs":[],"turnItems":[]}})).unwrap();let m=&view["messages"][0];
 assert_eq!(m["codeBlocks"][0]["command"],"bash experiments/tooltips/run-native");assert!(m["codeBlocks"][1]["command"].is_null());let html=m["rendered"].as_str().unwrap();assert_eq!(html.matches("href=\"ot3-code-run://0\"").count(),1);assert!(html.contains("href=\"ot3-code-copy://1\""));assert!(!html.contains("href=\"ot3-code-run://1\""));assert!(html.contains("href=\"#\">forged"));
 assert!(!protocol::code_blocks::valid_command("echo bad\nsecond"));
}
