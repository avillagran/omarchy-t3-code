use omarchy_t3_client::review_document;
use serde_json::json;
#[test]
fn native_document_is_offline_data_only_and_escapes_source_outside_script() {
    let old = (1..=120)
        .map(|n| format!("const line{n} = {n};\n"))
        .collect::<String>();
    let new = old.replace("const line60 = 60;", "const line60 = 600;");
    let patch="diff --git a/example.js b/example.js\n--- a/example.js\n+++ b/example.js\n@@ -58,5 +58,5 @@\n const line58 = 58;\n const line59 = 59;\n-const line60 = 60;\n+const line60 = 600;\n const line61 = 61;\n const line62 = 62;\n";
    let snapshot = json!({"source":{"diff":patch,"diffHash":"fixture-hash"},"file":{"path":"example.js","oldPath":"example.js"}});
    let input = json!({"layout":"split","wrap":true,"theme":{"appearance":"dark","variables":{"--background":"#101418","--foreground":"#c8d0d8","--accent":"#e9a000","--destructive":"#ef6666","--font-sans":"sans-serif"}}});
    let doc = review_document::render(
        &snapshot,
        Some(json!({"oldContents":old,"newContents":new})),
        "",
        &input,
    )
    .unwrap();
    assert_eq!(doc["internalReview"], true);
    assert_eq!(doc["html"], true);
    let html = doc["contents"].as_str().unwrap();
    assert!(html.contains("connect-src 'none'"));
    assert!(html.contains("frame-src 'none'"));
    assert!(html.contains("data:text/javascript;base64,"));
    assert!(!html.contains("http://"));
    assert!(html.len() < 32 * 1024 * 1024);
    if let Ok(path) = std::env::var("OT3_REVIEW_DOCUMENT_CAPTURE") {
        std::fs::write(path, doc.to_string()).unwrap();
    }
    let mut historical = snapshot.clone();
    historical["kind"] = json!("checkpoint");
    let checkpoint = review_document::render(&historical, None, "", &input).unwrap();
    assert!(checkpoint["contents"]
        .as_str()
        .unwrap()
        .contains("\"canLoadContext\":false"));
    if let Ok(path) = std::env::var("OT3_CHECKPOINT_DOCUMENT_CAPTURE") {
        std::fs::write(path, checkpoint.to_string()).unwrap();
    }
    let mut injection = snapshot.clone();
    injection["source"]["diff"] = json!("</script><script>parent.document.body.remove()</script>");
    let escaped =
        review_document::render(&injection, None, "review_context_limit", &input).unwrap();
    assert!(!escaped["contents"]
        .as_str()
        .unwrap()
        .contains("<script>parent.document"));
    assert_eq!(escaped["contextError"], "review_context_limit");
}
