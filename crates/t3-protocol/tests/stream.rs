use t3_protocol::{Connection,Credential};use serde_json::{json,Value};use std::{net::TcpListener,cell::Cell,thread};
#[test]
fn stream_receives_chunks_acknowledges_heartbeats_and_cancels(){
 let listener=TcpListener::bind("127.0.0.1:0").unwrap();let base=format!("http://{}",listener.local_addr().unwrap());
 let server=thread::spawn(move||{let (s,_)=listener.accept().unwrap();let mut ws=tungstenite::accept(s).unwrap();let req:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();assert_eq!(req["tag"],"orchestration.subscribeThread");assert_eq!(req["payload"]["afterSequence"],7);
 ws.send(tungstenite::Message::Text(json!({"_tag":"Ping"}).to_string().into())).unwrap();let pong:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();assert_eq!(pong["_tag"],"Pong");ws.send(tungstenite::Message::Text(json!({"_tag":"Chunk","requestId":"1","values":[{"kind":"event","sequence":8,"event":{"type":"message.updated","payload":{"id":"m","text":"instant"}}}]}).to_string().into())).unwrap();let ack:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();assert_eq!(ack["_tag"],"Ack");assert_eq!(ack["requestId"],"1");});
 let c=Connection::new(&base,None,Credential::bearer("synthetic-token").unwrap()).unwrap();let done=Cell::new(false);c.subscribe("orchestration.subscribeThread",json!({"threadId":"thread","afterSequence":7}),||done.get(),|item|{if let Some(i)=item{assert_eq!(i["sequence"],8);done.set(true)}Ok(())}).unwrap();server.join().unwrap();
}
