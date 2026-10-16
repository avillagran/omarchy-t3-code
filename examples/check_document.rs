//! Read-only smoke check of the native environment file-grant path.
use omarchy_t3_client::*;
use serde_json::json;
fn main(){let args=std::env::args().collect::<Vec<_>>();let e=environments().unwrap().into_iter().find(|e|e.id==args[1]).unwrap();let c=if let Some(c)=&e.connection{c.as_ref().clone()}else{protocol::Connection::new(&e.base_url,None,protocol::Credential::bearer(&environment_token(&e,true).unwrap()).unwrap()).unwrap()};let r=protocol::documents::resource(&args[2],&args[3]).and_then(|r|c.document(r));match r{Ok(d)=>{std::fs::write("/tmp/t3-reference-document.html",text(&d,"contents")).unwrap();println!("{}",json!({"bytes":text(&d,"contents").len(),"saved":"/tmp/t3-reference-document.html"}))},Err(e)=>println!("{}",json!({"error":e}))}}
