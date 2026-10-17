// Compiles the owned module independently until the parent connects it to lib.rs/write.rs.
use omarchy_t3_client::diff_review;
use serde_json::json;

#[test]
fn native_multi_file_rename_delete_binary_quoted_paths_and_split_alignment() {
    let patch = concat!(
        "diff --git \"a/old name.txt\" \"b/new name.txt\"\n",
        "similarity index 90%\nrename from old name.txt\nrename to new name.txt\n",
        "--- \"a/old name.txt\"\n+++ \"b/new name.txt\"\n@@ -2,3 +2,4 @@ header\n",
        " keep\n-old\n+new\n+extra\n more\n",
        "diff --git a/gone.rs b/gone.rs\ndeleted file mode 100644\n",
        "--- a/gone.rs\n+++ /dev/null\n@@ -1 +0,0 @@\n-last\n",
        "diff --git a/logo.png b/logo.png\nindex 111..222 100644\nBinary files a/logo.png and b/logo.png differ\n",
        "diff --git \"a/quote\\\"path.txt\" \"b/quote\\\"path.txt\"\n",
        "--- \"a/quote\\\"path.txt\"\n+++ \"b/quote\\\"path.txt\"\n@@ -1 +1 @@\n-old\n+new\n",
        "diff --git \"a/caf\\303\\251.txt\" \"b/caf\\303\\251.txt\"\n",
        "--- \"a/caf\\303\\251.txt\"\n+++ \"b/caf\\303\\251.txt\"\n@@ -1 +1 @@\n-old\n+new\n"
    );
    let summary = json!({"files":[{"path":"new name.txt","kind":"renamed","additions":2,"deletions":1},
        {"path":"gone.rs","kind":"deleted","additions":0,"deletions":1},
        {"path":"logo.png","kind":"modified","additions":null,"deletions":null},
        {"path":"missing.txt","kind":"modified","additions":3,"deletions":4}],"fromTurnCount":1,"toTurnCount":2});
    let output = diff_review::present(&json!({"diff":patch,"fromTurnCount":1,"toTurnCount":2}), &summary).unwrap();
    let files = output["files"].as_array().unwrap();
    assert_eq!(files.len(), 6);
    assert_eq!(files[0]["kind"],"renamed");
    assert_eq!(files[0]["oldPath"],"old name.txt");
    assert_eq!(files[0]["path"],"new name.txt");
    assert_eq!(files[0]["additions"],2);
    let paired = files[0]["splitRows"].as_array().unwrap().iter()
        .find(|row| row["left"]["text"] == "-old").unwrap();
    assert_eq!(paired["right"]["text"],"+new");
    let extra = files[0]["splitRows"].as_array().unwrap().iter()
        .find(|row| row["right"]["text"] == "+extra").unwrap();
    assert_eq!(extra["left"],serde_json::Value::Null);
    assert_eq!(files[1]["kind"],"deleted"); assert_eq!(files[1]["path"],"gone.rs");
    assert_eq!(files[2]["binary"],true);
    assert_eq!(files[3]["path"],"quote\"path.txt");
    assert_eq!(files[4]["path"],"café.txt");
    assert_eq!(files[5]["missing"],true);
}

#[test]
fn scoped_late_failed_and_invalid_responses_do_not_leak_other_thread() {
    let summary = json!({"fromTurnCount":0,"toTurnCount":1,"files":[]});
    let expected = json!({"requestId":"latest","environment":"host-a","thread":"thread-a","fromTurnCount":0,"toTurnCount":1});
    let receipt = json!({"requestId":"latest","environment":"host-a","thread":"thread-a","result":{"threadId":"thread-a","diff":"","fromTurnCount":0,"toTurnCount":1}});
    assert_eq!(diff_review::scoped_result(&expected,&receipt,&summary).unwrap().unwrap()["files"],json!([]));
    for (field,value) in [("requestId","old"),("environment","host-b"),("thread","thread-b")] {
        let mut late=receipt.clone(); late[field]=json!(value);
        assert!(diff_review::scoped_result(&expected,&late,&summary).is_none());
    }
    let mut failed=receipt.clone(); failed["error"]=json!("checkpoint_unavailable");
    assert_eq!(diff_review::scoped_result(&expected,&failed,&summary).unwrap().unwrap_err(),"checkpoint_unavailable");
    let mut wrong=receipt.clone(); wrong["result"]["toTurnCount"]=json!(2);
    assert_eq!(diff_review::scoped_result(&expected,&wrong,&summary).unwrap().unwrap_err(),"invalid_diff_range");
    let mut wrong_thread=receipt.clone(); wrong_thread["result"]["threadId"]=json!("thread-b");
    assert_eq!(diff_review::scoped_result(&expected,&wrong_thread,&summary).unwrap().unwrap_err(),"invalid_diff_thread");
    let mut wrong_summary=summary.clone(); wrong_summary["toTurnCount"]=json!(2);
    assert!(diff_review::scoped_result(&expected,&receipt,&wrong_summary).is_none());
}

#[test]
fn rejects_oversized_and_malformed_input() {
    assert_eq!(diff_review::present(&json!({"diff":"x".repeat(2*1024*1024+1)}),&json!({})).unwrap_err(),"diff_limit_2MiB");
    assert_eq!(diff_review::present(&json!({"diff":"diff --git garbage"}),&json!({})).unwrap_err(),"invalid_diff_header");
    assert_eq!(diff_review::present(&json!({"diff":"diff --git a/a b/a\n".repeat(257)}),&json!({})).unwrap_err(),"diff_limit_files");
}

#[test]
fn git_unquoted_space_paths_and_required_scope() {
    let value=diff_review::present(&json!({"diff":"diff --git a/old space.txt b/new space.txt\nrename from old space.txt\nrename to new space.txt\n"}),&json!({})).unwrap();
    assert_eq!(value["files"][0]["oldPath"],"old space.txt");
    assert_eq!(value["files"][0]["path"],"new space.txt");
    assert!(diff_review::scoped_result(&json!({"requestId":"id","fromTurnCount":0,"toTurnCount":1}),&json!({"requestId":"id"}),&json!({})).is_none());
}

#[test]
fn production_read_uses_native_range_and_whitespace_and_rejects_wrong_result(){
 use std::{net::TcpListener,sync::Arc,thread};
 use omarchy_t3_client::{Environment,protocol};
 for (from,wrong) in [(0,false),(2,false),(2,true)]{
 let listener=TcpListener::bind("127.0.0.1:0").unwrap();let base=format!("http://{}",listener.local_addr().unwrap());
 let server=thread::spawn(move||{
 let (stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(std::time::Duration::from_secs(3))).unwrap();let mut ws=tungstenite::accept(stream).unwrap();
 let request:serde_json::Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
 assert_eq!(request["tag"],if from==0{"orchestration.getFullThreadDiff"}else{"orchestration.getTurnDiff"});
 assert_eq!(request["payload"]["ignoreWhitespace"],false);assert_eq!(request["payload"]["threadId"],"one");assert_eq!(request["payload"]["toTurnCount"],3);
 if from==0{assert!(request["payload"].get("fromTurnCount").is_none())}else{assert_eq!(request["payload"]["fromTurnCount"],from)}
 ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":{"threadId":if wrong{"other"}else{"one"},"fromTurnCount":from,"toTurnCount":3,"diff":"diff --git a/a b/a\n--- a/a\n+++ b/a\n@@ -1 +1 @@\n-old\n+new\n"}}}).to_string().into())).unwrap();
 });
 let conn=protocol::Connection::new(&base,None,protocol::Credential::bearer("synthetic").unwrap()).unwrap();
 let env=Environment{id:"fixture".into(),name:"Fixture".into(),base_url:base,token_file:"/unused".into(),write_token_file:None,bearer:None,connection_error:None,connection:Some(Arc::new(conn))};
 let input=json!({"environment":"fixture","thread":"one","fromTurnCount":from,"toTurnCount":3,"ignoreWhitespace":false,"summary":{"fromTurnCount":from,"toTurnCount":3,"files":[]}});
 let result=diff_review::execute(&env,&input);server.join().unwrap();
 if wrong{assert_eq!(result.unwrap_err(),"invalid_diff_thread")}else{let parsed=result.unwrap();assert_eq!(parsed["files"][0]["path"],"a");assert!(parsed.get("diff").is_none())}
 }
}
