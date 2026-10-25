//! `secrets.answerRequest`: answer an agent's request for a secret. The secret
//! never appears in `Debug` output or in an error.
use super::reader::Reader;
use super::{api_fn, ids, send, Function};
use crate::{Connection, Result};
use serde_json::{json, Value};
use std::fmt;

const SECRET_MAX: usize = 65_536;

pub const ANSWER_REQUEST: Function = api_fn! {
    name: "secrets.answerRequest", module: "secrets", tag: Some("secrets.answerRequest"),
    kind: UnrepeatableMutation, scope: Fixed("orchestration:operate"),
    args: [(threadId, Id, required), (turnItemId, Id, required), (answer, Object, required)],
    payload: |a| answer_payload(a.req("threadId")?, a.req("turnItemId")?, &SecretAnswer::from_json(a.req_object("answer")?)?),
};
pub const FUNCTIONS: &[Function] = &[ANSWER_REQUEST];

/// `{type:"save",secret}` or `{type:"decline"}`.
#[derive(Clone, PartialEq, Eq)]
pub enum SecretAnswer {
    Save(String),
    Decline,
}
impl fmt::Debug for SecretAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Save(_) => f.write_str("Save([redacted])"),
            Self::Decline => f.write_str("Decline"),
        }
    }
}
impl SecretAnswer {
    pub fn to_json(&self) -> Result<Value> {
        Ok(match self {
            Self::Save(secret) => {
                let s = secret.trim();
                if s.is_empty() || s.len() > SECRET_MAX || s.contains('\0') {
                    return Err(ids::invalid("answer"));
                }
                json!({"type":"save","secret":s})
            }
            Self::Decline => json!({"type":"decline"}),
        })
    }
    pub fn from_json(v: &Value) -> Result<Self> {
        let invalid = || ids::invalid("answer");
        let mut r = Reader::of("answer", v)?;
        let answer = match r.req_str("type").map_err(|_| invalid())?.as_str() {
            "save" => Self::Save(r.raw("secret").and_then(Value::as_str).ok_or_else(invalid)?.to_string()),
            "decline" => Self::Decline,
            _ => return Err(invalid()),
        };
        r.done().map_err(|_| invalid())?;
        answer.to_json()?;
        Ok(answer)
    }
}
/// `secrets.answerRequest`: `null`. Not repeatable.
pub fn answer(conn: &Connection, thread_id: &str, turn_item_id: &str, answer: &SecretAnswer) -> Result<Value> {
    send(conn, &ANSWER_REQUEST, answer_payload(thread_id, turn_item_id, answer)?)
}
fn answer_payload(thread_id: &str, turn_item_id: &str, answer: &SecretAnswer) -> Result<Value> {
    Ok(json!({"threadId":ids::id("threadId", thread_id)?,"turnItemId":ids::id("turnItemId", turn_item_id)?,"answer":answer.to_json()?}))
}
