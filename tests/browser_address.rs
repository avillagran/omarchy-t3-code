use omarchy_t3_client::browser_address;
use serde_json::Value;
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
