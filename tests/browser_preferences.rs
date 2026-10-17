use omarchy_t3_client::{browser_preferences as browser, connection_preferences as preferences};
use serde_json::{json, Value};

#[test]
fn native_default_profile_resolution_excludes_incognito_and_missing_profiles() {
    for (id, rows, expected) in [
        ("work", json!([{"id":"work","kind":"persistent"}]), "work"),
        ("work", json!([{"id":"work","kind":"incognito"}]), "work"),
        ("deleted", json!([]), "default"),
        (
            "incognito",
            json!([{"id":"incognito","kind":"persistent"}]),
            "default",
        ),
        ("default", json!([]), "default"),
        ("bad\nprofile", json!([{"id":"bad\nprofile"}]), "default"),
    ] {
        assert_eq!(
            browser::default_profile(&json!({"browserDefaultProfileId":id,"browserProfiles":rows})),
            expected
        );
    }
}
use std::fs;
#[test]
fn native_viewport_zoom_and_profile_contract_preserve_safe_values_and_boundaries() {
    let c = browser::contract();
    assert_eq!(c["presetIds"].as_array().unwrap().len(), 17);
    assert_eq!(c["zoomLevels"].as_array().unwrap().len(), 17);
    for p in c["presets"].as_array().unwrap() {
        let v = json!({"_tag":"preset","presetId":p["id"],"width":p["height"],"height":p["width"],"private":"PRIVATE"});
        let validated = browser::validate("browserDefaultViewport", &v).unwrap();
        assert_eq!(validated["presetId"], p["id"]);
        assert!(!validated.to_string().contains("PRIVATE"));
    }
    assert_eq!(
        browser::validate(
            "browserDefaultViewport",
            &json!({"_tag":"freeform","width":3840.0,"height":2160.0})
        )
        .unwrap()["width"],
        3840
    );
    for bad in [
        json!({"_tag":"freeform","width":3840,"height":2161}),
        json!({"_tag":"freeform","width":239,"height":800}),
        json!({"_tag":"freeform","width":800.5,"height":600}),
        json!({"_tag":"preset","width":800,"height":600,"presetId":"unknown"}),
    ] {
        assert!(browser::validate("browserDefaultViewport", &bad).is_err())
    }
    assert!(browser::validate(
        "browserDefaultViewport",
        &json!({"_tag":"preset","width":1080,"height":1920,"presetId":"desktop-1920x1080"})
    )
    .is_ok());
    for zoom in c["zoomLevels"].as_array().unwrap() {
        assert!(browser::validate("browserDefaultZoomFactor", zoom).is_ok())
    }
    assert!(browser::validate("browserDefaultZoomFactor", &json!(1.01)).is_err());
    assert!(browser::validate("browserRecordingFrameRate", &json!(60.0)).is_ok());
    assert!(browser::validate("browserRecordingFrameRate", &json!(45)).is_err());
    let profile = |id: &str, name: &str| json!([{"id":id,"name":name,"kind":"persistent","credential":"PRIVATE"}]);
    let valid = browser::validate("browserProfiles", &profile("profile-ñ", "Trabajo 🦀")).unwrap();
    assert!(!valid.to_string().contains("PRIVATE"));
    for p in [
        profile("bad\nkey", "Name"),
        profile(" space", "Name"),
        profile("id", &"🦀".repeat(25)),
        profile("id", ""),
    ] {
        assert!(browser::validate("browserProfiles", &p).is_err())
    }
    let v = browser::projection(
        &json!({"browserDefaultViewport":{"_tag":"bad"},"browserDefaultZoomFactor":19,"browserDefaultAppearance":"private","browserProfiles":[],"credential":"PRIVATE"}),
    );
    assert_eq!(v["browserDefaultViewport"], json!({"_tag":"fill"}));
    assert_eq!(v["browserDefaultZoomFactor"], 1.0);
    assert_eq!(v["browserDefaultAppearance"], "system");
    assert_eq!(v["browserAutoShowFloatingPreview"], true);
    assert!(!v.to_string().contains("PRIVATE"));
}
#[test]
fn native_client_file_cas_keeps_profiles_unknown_fields_and_legacy_envelope() {
    let root =
        std::env::temp_dir().join(format!("ot3-browser-preferences-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let file = root.join("client-settings.json");
    for legacy in [false, true] {
        let original = json!({"browserProfiles":[{"id":"profile-a","name":"Work","kind":"persistent"}],"browserDefaultProfileId":"profile-a","future":{"keep":true},"credential":"PRIVATE"});
        fs::write(
            &file,
            if legacy {
                json!({"schemaVersion":1,"settings":original})
            } else {
                original
            }
            .to_string(),
        )
        .unwrap();
        let before = preferences::read_client_preferences_at(&file).unwrap();
        assert!(!before.to_string().contains("PRIVATE"));
        let patch = json!({"browserDefaultViewport":{"_tag":"freeform","width":1280,"height":800},"browserDefaultZoomFactor":1.25,"browserDefaultAppearance":"dark","browserRecordingFrameRate":60,"browserRecordingShowKeyPresses":true,"browserRecordingShowMousePresses":true,"browserLinkTarget":"app","browserAutoShowFloatingPreview":false});
        let after = preferences::update_client_preferences_at(
            &file,
            before["revision"].as_str().unwrap(),
            &patch,
        )
        .unwrap();
        for (k, v) in patch.as_object().unwrap() {
            assert_eq!(&after[k], v)
        }
        assert_eq!(after["browserDefaultProfileId"], "profile-a");
        let stored: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        let stored = if legacy { &stored["settings"] } else { &stored };
        assert_eq!(stored["credential"], "PRIVATE");
        assert_eq!(stored["future"]["keep"], true);
        assert_eq!(stored["browserProfiles"][0]["name"], "Work");
        assert_eq!(
            preferences::update_client_preferences_at(
                &file,
                before["revision"].as_str().unwrap(),
                &json!({"browserDefaultZoomFactor":2})
            )
            .unwrap_err(),
            "client_settings_changed_reload"
        );
        let bytes = fs::read(&file).unwrap();
        assert!(preferences::update_client_preferences_at(
            &file,
            after["revision"].as_str().unwrap(),
            &json!({"browserDefaultZoomFactor":1.01})
        )
        .is_err());
        assert_eq!(fs::read(&file).unwrap(), bytes);
        assert!(preferences::update_client_preferences_at(
            &file,
            after["revision"].as_str().unwrap(),
            &json!({"browserSecret":"PRIVATE"})
        )
        .is_err());
        assert_eq!(fs::read(&file).unwrap(), bytes);
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn generated_original_helpers_and_schema_constants_match_their_provenance() {
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let provenance: Value = serde_json::from_slice(
        &fs::read(root.join("ui/js/NativeBrowserPreferences.sources.json")).unwrap(),
    )
    .unwrap();
    for (path, key) in [
        ("ui/js/NativeBrowserPreferences.js", "outputSha256"),
        ("data/browser-preferences.json", "contractSha256"),
    ] {
        assert_eq!(
            format!("{:x}", Sha256::digest(fs::read(root.join(path)).unwrap())),
            provenance[key].as_str().unwrap()
        );
    }
    assert_eq!(provenance["commit"], browser::contract()["commit"]);
    assert_eq!(provenance["files"].as_object().unwrap().len(), 5);
}
