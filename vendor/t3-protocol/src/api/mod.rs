//! Typed T3 API: one Rust function per RPC tag and one static registry that the
//! JSON ABI (`call`, `describe`) reads. Inputs are typed and validated before a
//! socket opens; responses stay `serde_json::Value`, because T3 declares them
//! forward compatible. No function retries: after `rpc_outcome_unknown` the host
//! decides from the function's `Kind` (see `Replay`). The library publishes the
//! scope each function needs; the host picks the token.
use crate::{Connection, Result};
use serde_json::{json, Map, Value};
use std::time::Duration;

pub mod ids;
pub(crate) mod reader;
pub mod assets;
pub mod orchestration;
pub mod guest_share;
pub mod preview;
pub mod projects;
pub mod review;
pub mod server;
pub mod vcs;
// Tanda 2: the remaining unary RPCs.
pub mod agent_sessions;
pub mod device;
pub mod diagnostics;
pub mod filesystem;
pub mod mcp_apps;
pub mod project_clone;
pub mod provider;
pub mod pull_requests;
pub mod scheduled_tasks;
pub mod secrets;
pub mod shell;
pub mod terminal;
pub mod worktree_setup;

pub mod streams;

pub use orchestration::{Command, ThreadLaunch};

/// T3 checkout the registry was verified against (`tests/fixtures/t3-contract.json`).
pub const CONTRACT: &str = "t3code@3f9448747";
/// Version of the `describe` document.
pub const API_VERSION: u64 = 1;
/// Deadline of `Connection::rpc`; functions with another deadline declare it.
pub const DEFAULT_TIMEOUT_MS: u64 = 20_000;
/// Upper bound of `Connection::rpc_with_timeout` and of a caller's `timeoutMs`.
pub const MAX_TIMEOUT_MS: u64 = 300_000;

/// What a function does to server state, and so what a host may do after
/// `rpc_outcome_unknown`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// L: a read. Repeating it is free.
    Read,
    /// M·id: a mutation carrying a `commandId` the server deduplicates.
    /// Resend the same payload, with the same `commandId`.
    MutationWithCommandId,
    /// M·set: sets a state by value. Repeating it is harmless.
    IdempotentMutation,
    /// M!: not repeatable. Read the state back before deciding.
    UnrepeatableMutation,
}
/// The host's replay rule after `rpc_outcome_unknown`. The library never replays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Replay {
    Free,
    Id,
    Set,
    Never,
}
impl Kind {
    pub fn replay(self) -> Replay {
        match self {
            Kind::Read => Replay::Free,
            Kind::MutationWithCommandId => Replay::Id,
            Kind::IdempotentMutation => Replay::Set,
            Kind::UnrepeatableMutation => Replay::Never,
        }
    }
    /// `read` or `mutation`, as `describe` publishes it.
    pub fn as_str(self) -> &'static str {
        if self == Kind::Read {
            "read"
        } else {
            "mutation"
        }
    }
}
impl Replay {
    pub fn as_str(self) -> &'static str {
        match self {
            Replay::Free => "free",
            Replay::Id => "id",
            Replay::Set => "set",
            Replay::Never => "never",
        }
    }
}
/// The T3 scope a call needs. `Dynamic` lists every scope the payload can require.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Fixed(&'static str),
    Dynamic(&'static [&'static str]),
}
/// Argument types, as `describe` publishes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgType {
    /// `ids::id`
    Id,
    /// `ids::slug`
    Slug,
    /// `ids::token`
    Token,
    /// `ids::opaque`
    Opaque,
    /// `ids::path`
    Path,
    /// `ids::text`
    Text,
    Bool,
    Integer,
    /// A JSON number that may have a fraction.
    Number,
    Object,
    Array,
    Enum(&'static [&'static str]),
    /// An `orchestration.dispatchCommand` command type (the args are the command).
    Command,
}
impl ArgType {
    pub fn as_str(self) -> &'static str {
        match self {
            ArgType::Id => "id",
            ArgType::Slug => "slug",
            ArgType::Token => "token",
            ArgType::Opaque => "opaque",
            ArgType::Path => "path",
            ArgType::Text => "text",
            ArgType::Bool => "bool",
            ArgType::Integer => "integer",
            ArgType::Number => "number",
            ArgType::Object => "object",
            ArgType::Array => "array",
            ArgType::Enum(_) => "enum",
            ArgType::Command => "command",
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Arg {
    pub name: &'static str,
    pub ty: ArgType,
    pub required: bool,
}
/// One registry row. Built with `api_fn!`.
#[derive(Clone, Copy)]
pub struct Function {
    pub name: &'static str,
    pub module: &'static str,
    /// Wire tag, or `None` for a composed function.
    pub tag: Option<&'static str>,
    pub kind: Kind,
    pub scope: Scope,
    pub timeout_ms: u64,
    pub stream: bool,
    pub args: &'static [Arg],
    /// When true, `args` lists only the members every call shares and the
    /// payload builder rejects the rest itself (`orchestration.dispatchCommand`).
    pub open_args: bool,
    /// Extra `describe` members (e.g. the command types).
    pub variants: &'static [&'static str],
    payload: fn(&Args) -> Result<Value>,
}
impl Function {
    pub fn replay(&self) -> Replay {
        self.kind.replay()
    }
    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_ms)
    }
    /// The validated wire payload for JSON `args`, without any network.
    /// Unknown members are `invalid_input`; a missing or malformed one is `invalid_<field>`.
    pub fn payload(&self, args: &Value) -> Result<Value> {
        (self.payload)(&Args::checked(args, self.args, self.open_args)?)
    }
    /// Validate, then send once with this function's deadline (or `timeout`).
    pub fn call(&self, conn: &Connection, args: &Value, timeout: Option<Duration>) -> Result<Value> {
        let payload = self.payload(args)?;
        send_with(conn, self, payload, timeout.unwrap_or(self.timeout()))
    }
    pub fn describe(&self) -> Value {
        let args = self
            .args
            .iter()
            .map(|a| {
                let mut v = json!({"name":a.name,"type":a.ty.as_str(),"required":a.required});
                if let ArgType::Enum(values) = a.ty {
                    v["values"] = json!(values);
                }
                v
            })
            .collect::<Vec<_>>();
        let mut v = json!({"fn":self.name,"module":self.module,"tag":self.tag,"kind":self.kind.as_str(),
            "replay":self.replay().as_str(),"timeoutMs":self.timeout_ms,"stream":self.stream,
            "openArgs":self.open_args,"args":args});
        match self.scope {
            Scope::Fixed(s) => v["scope"] = json!(s),
            Scope::Dynamic(all) => {
                v["scope"] = json!("dynamic");
                v["scopes"] = json!(all);
            }
        }
        if !self.variants.is_empty() {
            v["variants"] = json!(self.variants);
        }
        v
    }
}

/// Build a registry row:
/// `api_fn!{name: "vcs.refreshStatus", module: "vcs", tag: Some("vcs.refreshStatus"),
/// kind: Read, scope: Fixed("orchestration:read"), args: [(cwd, Path, required)],
/// payload: |a| refresh_status_payload(a.req("cwd")?)}`.
/// Optional keys after `payload`: `timeout_ms`, `open_args`, `variants`.
macro_rules! api_fn {
    (name: $name:literal, module: $module:literal, tag: $tag:expr, kind: $kind:ident, scope: $scope:expr,
     args: [$(($arg:ident, $ty:expr, $req:ident)),* $(,)?], payload: $payload:expr
     $(, timeout_ms: $timeout:expr)? $(, open_args: $open:expr)? $(, variants: $variants:expr)? $(,)?) => {
        $crate::api::Function {
            name: $name,
            module: $module,
            tag: $tag,
            kind: $crate::api::Kind::$kind,
            scope: {
                #[allow(unused_imports)]
                use $crate::api::Scope::*;
                $scope
            },
            timeout_ms: $crate::api::api_fn!(@or $($timeout)?, $crate::api::DEFAULT_TIMEOUT_MS),
            stream: false,
            args: &[$($crate::api::Arg {
                name: stringify!($arg),
                ty: {
                    #[allow(unused_imports)]
                    use $crate::api::ArgType::*;
                    $ty
                },
                required: $crate::api::api_fn!(@required $req),
            }),*],
            open_args: $crate::api::api_fn!(@or $($open)?, false),
            variants: $crate::api::api_fn!(@or $($variants)?, &[]),
            payload: $payload,
        }
    };
    (@or , $default:expr) => { $default };
    (@or $value:expr, $default:expr) => { $value };
    (@required required) => { true };
    (@required optional) => { false };
}
pub(crate) use api_fn;

/// A call's JSON arguments, already checked against the function's `args`.
pub struct Args<'a> {
    value: &'a Value,
    map: &'a Map<String, Value>,
}
impl<'a> Args<'a> {
    /// `args` checked against the declared members: an object, without unknown
    /// members unless `open`, and with every required one present and not null.
    pub(crate) fn checked(args: &'a Value, declared: &[Arg], open: bool) -> Result<Args<'a>> {
        let map = args.as_object().ok_or("invalid_input")?;
        if !open && map.keys().any(|k| !declared.iter().any(|a| a.name == k)) {
            return Err("invalid_input".into());
        }
        for arg in declared.iter().filter(|a| a.required) {
            if map.get(arg.name).is_none_or(Value::is_null) {
                return Err(ids::invalid(arg.name));
            }
        }
        Ok(Args { value: args, map })
    }
    /// The whole argument object (for functions with `open_args`).
    pub fn all(&self) -> &'a Value {
        self.value
    }
    /// A member, treating JSON `null` as absent.
    pub fn get(&self, name: &str) -> Option<&'a Value> {
        self.map.get(name).filter(|v| !v.is_null())
    }
    /// An optional string; another JSON type is `invalid_<name>`.
    pub fn str(&self, name: &str) -> Result<Option<&'a str>> {
        self.get(name)
            .map(|v| v.as_str().ok_or_else(|| ids::invalid(name)))
            .transpose()
    }
    /// A required string.
    pub fn req(&self, name: &str) -> Result<&'a str> {
        self.str(name)?.ok_or_else(|| ids::invalid(name))
    }
    pub fn bool(&self, name: &str) -> Result<Option<bool>> {
        self.get(name)
            .map(|v| v.as_bool().ok_or_else(|| ids::invalid(name)))
            .transpose()
    }
    pub fn object(&self, name: &str) -> Result<Option<&'a Value>> {
        self.get(name)
            .map(|v| Some(v).filter(|v| v.is_object()).ok_or_else(|| ids::invalid(name)))
            .transpose()
    }
    /// A required object.
    pub fn req_object(&self, name: &str) -> Result<&'a Value> {
        self.object(name)?.ok_or_else(|| ids::invalid(name))
    }
    /// An optional non-negative integer.
    pub fn u64(&self, name: &str) -> Result<Option<u64>> {
        self.get(name)
            .map(|v| v.as_u64().ok_or_else(|| ids::invalid(name)))
            .transpose()
    }
    pub fn req_u64(&self, name: &str) -> Result<u64> {
        self.u64(name)?.ok_or_else(|| ids::invalid(name))
    }
    /// An optional integer that may be negative (a timestamp in milliseconds).
    pub fn i64(&self, name: &str) -> Result<Option<i64>> {
        self.get(name)
            .map(|v| v.as_i64().ok_or_else(|| ids::invalid(name)))
            .transpose()
    }
    /// An optional number (fractions allowed).
    pub fn f64(&self, name: &str) -> Result<Option<f64>> {
        self.get(name)
            .map(|v| v.as_f64().ok_or_else(|| ids::invalid(name)))
            .transpose()
    }
    pub fn array(&self, name: &str) -> Result<Option<&'a Vec<Value>>> {
        self.get(name)
            .map(|v| v.as_array().ok_or_else(|| ids::invalid(name)))
            .transpose()
    }
    pub fn req_array(&self, name: &str) -> Result<&'a Vec<Value>> {
        self.array(name)?.ok_or_else(|| ids::invalid(name))
    }
}

const MODULES: &[&[Function]] = &[
    server::FUNCTIONS,
    orchestration::FUNCTIONS, guest_share::FUNCTIONS,
    preview::FUNCTIONS,
    assets::FUNCTIONS,
    vcs::FUNCTIONS,
    review::FUNCTIONS,
    projects::FUNCTIONS,
    // Tanda 2.
    diagnostics::FUNCTIONS,
    provider::FUNCTIONS,
    project_clone::FUNCTIONS,
    filesystem::FUNCTIONS,
    pull_requests::FUNCTIONS,
    device::FUNCTIONS,
    scheduled_tasks::FUNCTIONS,
    agent_sessions::FUNCTIONS,
    mcp_apps::FUNCTIONS,
    shell::FUNCTIONS,
    secrets::FUNCTIONS,
    worktree_setup::FUNCTIONS,
    terminal::FUNCTIONS,

    streams::FUNCTIONS,
];
/// Every registered function, in a stable order.
pub fn functions() -> impl Iterator<Item = &'static Function> {
    MODULES.iter().flat_map(|m| m.iter())
}
pub fn function(name: &str) -> Option<&'static Function> {
    functions().find(|f| f.name == name)
}
/// The registry as JSON, for hosts that generate wrappers (Kotlin) or probe support.
pub fn describe() -> Value {
    json!({"apiVersion":API_VERSION,"contract":CONTRACT,
        "functions":functions().map(Function::describe).collect::<Vec<_>>(),
        "streams":streams::describe(),"domain":crate::domain::describe()})
}
/// `timeoutMs` of a call: 1..=300 000, else `invalid_rpc_timeout`.
pub fn timeout(ms: Option<&Value>) -> Result<Option<Duration>> {
    ms.filter(|v| !v.is_null())
        .map(|v| {
            v.as_u64()
                .filter(|ms| (1..=MAX_TIMEOUT_MS).contains(ms))
                .map(Duration::from_millis)
                .ok_or_else(|| "invalid_rpc_timeout".to_string())
        })
        .transpose()
}
/// Call a registered function by name with JSON arguments (what `{"op":"call"}` does).
pub fn call(conn: &Connection, name: &str, args: &Value, timeout: Option<Duration>) -> Result<Value> {
    function(name).ok_or("unknown_function")?.call(conn, args, timeout)
}
pub(crate) fn send(conn: &Connection, f: &Function, payload: Value) -> Result<Value> {
    send_with(conn, f, payload, f.timeout())
}
/// Exactly one request; never retried. The default deadline keeps `rpc`'s behavior.
pub(crate) fn send_with(conn: &Connection, f: &Function, payload: Value, timeout: Duration) -> Result<Value> {
    // Streams go through `subscribe`; composed reads have no tag of their own.
    if f.stream {
        return Err("stream_function".into());
    }
    let Some(tag) = f.tag else { return streams::read_first(conn, f, payload, timeout) };
    if timeout == Duration::from_millis(DEFAULT_TIMEOUT_MS) {
        conn.rpc(tag, payload)
    } else {
        conn.rpc_with_timeout(tag, payload, timeout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registry_names_are_unique_and_args_are_declared_once() {
        let mut names = functions().map(|f| f.name).collect::<Vec<_>>();
        let count = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), count);
        for f in functions() {
            let mut args = f.args.iter().map(|a| a.name).collect::<Vec<_>>();
            args.sort();
            args.dedup();
            assert_eq!(args.len(), f.args.len(), "{}", f.name);
            assert!(f.timeout_ms >= 1 && f.timeout_ms <= MAX_TIMEOUT_MS);
        }
    }
    #[test]
    fn args_reject_unknown_members_and_name_missing_ones() {
        let f = function("vcs.refreshStatus").unwrap();
        assert_eq!(f.payload(&json!({"cwd":"/r"})).unwrap(), json!({"cwd":"/r"}));
        assert_eq!(f.payload(&json!({"cwd":"/r","cdw":"/r"})).unwrap_err(), "invalid_input");
        assert_eq!(f.payload(&json!([])).unwrap_err(), "invalid_input");
        assert_eq!(f.payload(&json!({})).unwrap_err(), "invalid_cwd");
        assert_eq!(f.payload(&json!({"cwd":null})).unwrap_err(), "invalid_cwd");
        assert_eq!(f.payload(&json!({"cwd":7})).unwrap_err(), "invalid_cwd");
        assert_eq!(timeout(Some(&json!(0))).unwrap_err(), "invalid_rpc_timeout");
        assert_eq!(timeout(Some(&json!(300_001))).unwrap_err(), "invalid_rpc_timeout");
        assert_eq!(timeout(None).unwrap(), None);
    }
}
