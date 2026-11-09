pub use omarchy_t3_client::file_metadata_cache;
#[path = "../src/file_metadata_admission.rs"]
pub mod file_metadata_admission;
use file_metadata_admission::*;
use file_metadata_cache::ThreadRef;
use serde_json::Value;
#[test]
fn complete_native_query_and_path_functions_match_admission() {
    assert!(std::path::Path::new("/run/.containerenv").is_file());
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/file-metadata-admission-4288.json")).unwrap();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 480);
    for case in fixture["cases"].as_array().unwrap() {
        let reference = case["ref"].as_object().map(|_| ThreadRef {
            environment: case["ref"]["environmentId"].as_str().unwrap().into(),
            thread: case["ref"]["threadId"].as_str().unwrap().into(),
        });
        assert_eq!(
            serde_json::to_value(query(
                "env",
                case["raw"].as_str().unwrap(),
                case["cwd"].as_str(),
                reference.as_ref()
            ))
            .unwrap(),
            case["result"],
            "{case}"
        );
    }
    let reference = ThreadRef {
        environment: "env".into(),
        thread: "thread".into(),
    };
    for case in fixture["terminalCases"].as_array().unwrap() {
        let target = resolve_link(case["raw"].as_str().unwrap(), case["cwd"].as_str().unwrap());
        assert_eq!(target, case["target"]);
        assert_eq!(
            serde_json::to_value(query(
                "env",
                &target,
                case["cwd"].as_str(),
                Some(&reference)
            ))
            .unwrap(),
            case["result"],
            "{case}"
        );
    }
    for case in fixture["workspaceCases"].as_array().unwrap() {
        assert_eq!(
            workspace(case["raw"].as_str().unwrap(), case["cwd"].as_str().unwrap()),
            case["result"],
            "{case}"
        );
    }
}

pub use omarchy_t3_client::Result;
#[path = "../src/file_metadata_wire.rs"]
pub mod file_metadata_wire;
#[test]
fn complete_native_metadata_contract_codec_matches() {
    assert!(std::path::Path::new("/run/.containerenv").is_file());
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/file-metadata-wire-4288.json")).unwrap();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 30);
    for case in fixture["cases"].as_array().unwrap() {
        let actual = file_metadata_wire::decode(&case["input"]);
        assert_eq!(actual.is_ok(), case["valid"] == true, "{case}");
        if let Ok(entries) = actual {
            let entries = entries
                .into_iter()
                .map(|metadata| {
                    metadata.map(|metadata| {
                        let mut value = serde_json::json!({"kind":metadata.kind});
                        if let Some(bytes) = metadata.byte_length {
                            value["byteLength"] = bytes.into();
                        }
                        if let Some(mime) = metadata.mime_type {
                            value["mimeType"] = mime.into();
                        }
                        value
                    })
                })
                .collect::<Vec<_>>();
            assert_eq!(
                serde_json::json!({"entries":entries}),
                case["result"],
                "{case}"
            );
        }
    }
}

#[test]
fn real_message_projection_exposes_checkpoint_files_without_seeding_claims() {
    assert!(std::path::Path::new("/run/.containerenv").is_file());
    let files =
        serde_json::json!([{"path":"src/a.ts","kind":"modified","additions":1,"deletions":2}]);
    let input = serde_json::json!({"projection":{"messages":[],"checkpoints":[{"id":"checkpoint","status":"ready","files":files,"scopeId":"scope","capturedAt":"2026-11-07T00:00:00Z"}]}});
    let output = omarchy_t3_client::normalize_messages(&input).unwrap();
    assert_eq!(
        output["metadataCheckpoints"],
        serde_json::json!([{"id":"checkpoint","status":"ready","files":files}])
    );
    assert!(output["threadState"].get("metadataCheckpoints").is_none());
    assert!(output["threadState"].get("files").is_none());
    let output =
        omarchy_t3_client::normalize_messages(&serde_json::json!({"projection":{"messages":[]}}))
            .unwrap();
    assert!(
        output["metadataCheckpoints"].is_null(),
        "absent projection resets provider baseline"
    );
}

#[test]
fn whole_native_project_result_contracts_guard_real_read_seeding() {
    assert!(std::path::Path::new("/run/.containerenv").is_file());
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/file-metadata-project-wire-4288.json"
    ))
    .unwrap();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 40);
    assert_eq!(fixture["hashes"].as_object().unwrap().len(), 9);
    for case in fixture["cases"].as_array().unwrap() {
        let actual = file_metadata_wire::decode_project(&case["input"], case["schema"] == "read");
        assert_eq!(actual.is_ok(), case["valid"] == true, "{case}");
        if let Ok(actual) = actual {
            assert_eq!(actual, case["result"], "{case}");
        }
    }
}
