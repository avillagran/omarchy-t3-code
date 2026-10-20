use omarchy_t3_client::browser_address;
use serde_json::{json, Value};
#[test]
fn address_bar_matches_original_t3_results() {
    let data: Value =
        serde_json::from_str(include_str!("../data/browser-address-cases.json")).unwrap();
    for case in data["cases"].as_array().unwrap() {
        let input = case["input"].as_str().unwrap();
        let result = browser_address::resolve(input);
        if case["error"] == true {
            assert!(result.is_err(), "input={input:?}");
        } else {
            assert_eq!(
                result.unwrap(),
                case["url"].as_str().unwrap(),
                "input={input:?}"
            );
        }
    }
}
#[test]
fn search_engine_templates_follow_native_preview_ts() {
    use omarchy_t3_client::browser_address::{resolve_with, search_template};
    let google = search_template(&json!({"browserSearchEngine":"google","browserCustomSearchUrl":""}));
    assert_eq!(
        resolve_with("how to center a div", &google).unwrap(),
        "https://www.google.com/search?q=how%20to%20center%20a%20div"
    );
    assert_eq!(resolve_with("cnn.com", &google).unwrap(), "https://cnn.com/");
    assert_eq!(
        resolve_with("a&b", "https://search.example/?q=%s&lang=en").unwrap(),
        "https://search.example/?q=a%26b&lang=en"
    );
    assert_eq!(
        search_template(&json!({"browserSearchEngine":"custom","browserCustomSearchUrl":" https://search.example/?q=%s "})),
        "https://search.example/?q=%s"
    );
    for url in [
        "https://search.example/?q=",
        "javascript:alert('%s')",
        "https://%s.example.com/",
        "https://%s@search.example/?q=%s",
    ] {
        assert_eq!(
            search_template(&json!({"browserSearchEngine":"custom","browserCustomSearchUrl":url})),
            "https://duckduckgo.com/?q=%s"
        );
    }
    assert_eq!(search_template(&json!({})), "https://duckduckgo.com/?q=%s");
}
