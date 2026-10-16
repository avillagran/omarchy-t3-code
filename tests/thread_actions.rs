use omarchy_t3_client::protocol::sidebar::thread_action;
use serde_json::json;
#[test]
fn thread_context_actions_use_native_commands_and_validate_destructive_intent(){
 for(action,typ)in [("pin","thread.pin"),("unpin","thread.unpin"),("mark-unread","thread.mark-unread"),("archive","thread.archive"),("regenerate-title","thread.metadata.update")]{let c=thread_action(&json!({"thread":"target","action":action})).unwrap();assert_eq!(c["type"],typ);assert_eq!(c["threadId"],"target")}
 assert!(thread_action(&json!({"thread":"target","action":"delete-thread"})).is_err());
 assert_eq!(thread_action(&json!({"thread":"target","action":"delete-thread","confirmed":true})).unwrap()["type"],"thread.delete");
 assert!(thread_action(&json!({"thread":"target","action":"archive","working":true})).is_err());
 assert_eq!(thread_action(&json!({"thread":"target","action":"thread-rename","title":" New title "})).unwrap()["title"],"New title");
 assert!(thread_action(&json!({"thread":"target","action":"thread-rename","title":" "})).is_err());
 assert_eq!(thread_action(&json!({"thread":"target","action":"auto-settle","enabled":false})).unwrap()["enabled"],false);
 assert!(thread_action(&json!({"thread":"target","action":"auto-settle","enabled":"false"})).is_err());
}
