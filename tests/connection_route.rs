use omarchy_t3_client::{connection_route as route,connection_github as github};
use serde_json::json;
#[test]
fn logical_project_matching_uses_native_fork_grouping_not_local_paths(){
 let projects=vec![json!({"id":"a","environmentId":"one","repositoryIdentity":{"canonicalKey":"upstream","rootPath":"/one","origin":{"canonicalKey":"fork"}}}),json!({"id":"b","environmentId":"two","repositoryIdentity":{"canonicalKey":"fork","rootPath":"/elsewhere"}}),json!({"id":"c","environmentId":"three","repositoryIdentity":{"canonicalKey":"upstream"}}),json!({"id":"d","environmentId":"four","repositoryIdentity":null})];
 let matched=route::eligible_projects(&projects,"one","a");assert_eq!(matched.len(),2);assert_eq!(matched[1]["id"],"b");assert!(route::eligible_projects(&projects,"four","d").is_empty());
}
#[test]
fn providers_must_be_authenticated_and_enabled_for_the_destination_project(){
 let mut config=json!({"providers":[{"instanceId":"work","enabled":true,"installed":true,"status":"ready","auth":{"status":"authenticated"},"models":[{"slug":"model"}]}],"settings":{"projectSettingsOverrides":{"p":{"providerInstances":{"work":{"enabled":false}}}}}});
 assert!(!route::provider_eligible(&config,"p","work","model"));assert!(route::provider_eligible(&config,"other","work","model"));config["providers"][0]["auth"]["status"]=json!("unauthenticated");assert!(!route::provider_eligible(&config,"other","work","model"));assert!(!route::provider_eligible(&config,"other","missing","model"));
}
#[test]
fn github_read_routing_requires_matching_saved_endpoints_and_account_identity(){
 let primary=json!({"_tag":"PrimaryConnectionTarget","environmentId":"primary","httpBaseUrl":"http://127.0.0.1:3773","wsBaseUrl":"ws://127.0.0.1:3773"});let mut c=json!({"targets":[],"profiles":[],"githubRoutingPermissions":[]});
 let key=omarchy_t3_client::connection_preferences::github_connection_key(&c,"primary",Some(&primary)).unwrap();c["githubRoutingPermissions"]=json!([{"environmentId":"primary","connectionKey":key,"permission":"read"}]);assert_eq!(github::permission(&c,Some(&primary),"primary"),"read");let mut moved=primary.clone();moved["httpBaseUrl"]=json!("http://127.0.0.1:9999");assert_eq!(github::permission(&c,Some(&moved),"primary"),"off");
 let identity=json!({"provider":"github","host":"GitHub.com","accountId":"123"});assert!(github::identities_match(&identity,&json!({"provider":"github","host":"github.com","accountId":"123"})));assert!(!github::identities_match(&identity,&json!({"provider":"github","host":"github.com","accountId":"456"})));assert!(!github::identities_match(&json!({"provider":"github","host":"github.com"}),&json!({"provider":"github","host":"github.com"})));
}
