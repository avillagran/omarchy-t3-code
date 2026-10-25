//! `preview.*`: the thread's preview browser. `list` reads; the rest needs
//! `preview:operate`.
use super::reader::{bounded, one_of, put, Reader};
use super::{api_fn, ids, send, Function};
use crate::{Connection, Result};
use serde_json::{json, Value};

/// T3 `preview.ts` bounds.
const URL_MAX: usize = 2048;
const TITLE_MAX: usize = 512;
const TAB_ID_MAX: usize = 128;
const PROFILE_ID_MAX: usize = 64;
const VIEWPORT_MIN: u64 = 240;
const VIEWPORT_MAX: u64 = 3840;
const VIEWPORT_AREA_MAX: u64 = 3840 * 2160;
pub const RUNTIMES: &[&str] = &["desktop", "server"];
pub const COLOR_SCHEMES: &[&str] = &["system", "light", "dark"];
pub const CLEAR_TARGETS: &[&str] = &["cookies", "cache"];
/// T3 `PREVIEW_ZOOM_FACTORS`: exact values, not a range.
pub const ZOOM_FACTORS: &[f64] = &[0.25, 0.33, 0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0, 5.0];
/// Current and legacy `presetId`s of a `preset` viewport.
pub const VIEWPORT_PRESETS: &[&str] = &[
    "iphone-se", "iphone-xr", "iphone-12-pro", "iphone-14-pro-max", "pixel-7", "samsung-galaxy-s8-plus",
    "samsung-galaxy-s20-ultra", "ipad-mini", "ipad-air", "ipad-pro", "surface-pro-7", "surface-duo",
    "galaxy-z-fold-5", "asus-zenbook-fold", "samsung-galaxy-a51-71", "nest-hub", "nest-hub-max",
    "desktop-1920x1080", "desktop-1440x900", "laptop-1366x768", "laptop-1280x800", "ipad-pro-11", "iphone-15-pro",
    "pixel-8", "galaxy-s24",
];

pub const LIST: Function = api_fn! {
    name: "preview.list", module: "preview", tag: Some("preview.list"),
    kind: Read, scope: Fixed("orchestration:read"),
    args: [(threadId, Id, required)],
    payload: |a| thread_payload(a.req("threadId")?),
};
pub const OPEN: Function = api_fn! {
    name: "preview.open", module: "preview", tag: Some("preview.open"),
    kind: UnrepeatableMutation, scope: Fixed("preview:operate"),
    args: [
        (threadId, Id, required), (url, Text, optional), (viewport, Object, optional), (profileId, Token, optional),
        (runtime, Enum(RUNTIMES), optional), (reveal, Bool, optional),
    ],
    payload: |a| {
        let open = PreviewOpen {
            url: a.str("url")?.map(Into::into),
            viewport: a.object("viewport")?.cloned(),
            profile_id: a.str("profileId")?.map(Into::into),
            runtime: a.str("runtime")?.map(Into::into),
            reveal: a.bool("reveal")?,
        };
        open_payload(a.req("threadId")?, &open)
    },
};
pub const NAVIGATE: Function = api_fn! {
    name: "preview.navigate", module: "preview", tag: Some("preview.navigate"),
    kind: IdempotentMutation, scope: Fixed("preview:operate"),
    args: [(threadId, Id, required), (tabId, Token, required), (url, Text, required), (resolvedTitle, Text, optional)],
    payload: |a| navigate_payload(a.req("threadId")?, a.req("tabId")?, a.req("url")?, a.str("resolvedTitle")?),
};
pub const RESIZE: Function = api_fn! {
    name: "preview.resize", module: "preview", tag: Some("preview.resize"),
    kind: IdempotentMutation, scope: Fixed("preview:operate"),
    args: [(threadId, Id, required), (tabId, Token, required), (viewport, Object, required)],
    payload: |a| resize_payload(a.req("threadId")?, a.req("tabId")?, a.req_object("viewport")?),
};
pub const ADJUST: Function = api_fn! {
    name: "preview.adjust", module: "preview", tag: Some("preview.adjust"),
    kind: IdempotentMutation, scope: Fixed("preview:operate"),
    args: [
        (threadId, Id, required), (tabId, Token, required), (colorScheme, Enum(COLOR_SCHEMES), optional),
        (zoomFactor, Number, optional), (hardReload, Bool, optional), (clear, Enum(CLEAR_TARGETS), optional),
    ],
    payload: |a| {
        let adjust = PreviewAdjust {
            color_scheme: a.str("colorScheme")?.map(Into::into),
            zoom_factor: a.f64("zoomFactor")?,
            hard_reload: a.bool("hardReload")?,
            clear: a.str("clear")?.map(Into::into),
        };
        adjust_payload(a.req("threadId")?, a.req("tabId")?, &adjust)
    },
};
pub const REFRESH: Function = api_fn! {
    name: "preview.refresh", module: "preview", tag: Some("preview.refresh"),
    kind: IdempotentMutation, scope: Fixed("preview:operate"),
    args: [(threadId, Id, required), (tabId, Token, required)],
    payload: |a| tab_payload(a.req("threadId")?, a.req("tabId")?),
};
pub const CLOSE: Function = api_fn! {
    name: "preview.close", module: "preview", tag: Some("preview.close"),
    kind: IdempotentMutation, scope: Fixed("preview:operate"),
    args: [(threadId, Id, required), (tabId, Token, optional)],
    payload: |a| close_payload(a.req("threadId")?, a.str("tabId")?),
};
/// ot3 waits up to 60 s while the server wipes a browser profile.
pub const CLEAR_PROFILE: Function = api_fn! {
    name: "preview.clearProfile", module: "preview", tag: Some("preview.clearProfile"),
    kind: IdempotentMutation, scope: Fixed("preview:operate"),
    args: [(profileId, Token, required)],
    payload: |a| clear_profile_payload(a.req("profileId")?),
    timeout_ms: 60_000,
};
pub const REPORT_STATUS: Function = api_fn! {
    name: "preview.reportStatus", module: "preview", tag: Some("preview.reportStatus"),
    kind: IdempotentMutation, scope: Fixed("preview:operate"),
    args: [(threadId, Id, required), (tabId, Token, required), (navStatus, Object, required), (canGoBack, Bool, required), (canGoForward, Bool, required)],
    payload: |a| report_status_payload(
        a.req("threadId")?, a.req("tabId")?, a.req_object("navStatus")?,
        a.bool("canGoBack")?.unwrap_or_default(), a.bool("canGoForward")?.unwrap_or_default(),
    ),
};
pub const FUNCTIONS: &[Function] = &[LIST, OPEN, NAVIGATE, RESIZE, ADJUST, REFRESH, CLOSE, CLEAR_PROFILE, REPORT_STATUS];

fn thread_payload(thread_id: &str) -> Result<Value> {
    Ok(json!({"threadId":ids::id("threadId", thread_id)?}))
}
fn tab_payload(thread_id: &str, tab_id: &str) -> Result<Value> {
    Ok(json!({"threadId":ids::id("threadId", thread_id)?,"tabId":ids::token("tabId", tab_id, TAB_ID_MAX)?}))
}
fn url(field: &str, value: &str) -> Result<String> {
    let url = ids::text(field, value, URL_MAX)?;
    if url.chars().any(char::is_control) {
        return Err(ids::invalid(field));
    }
    Ok(url.into())
}
/// `PreviewViewportSetting`: `{_tag:"fill"}`, `{_tag:"freeform",width,height}` or
/// `{_tag:"preset",width,height,presetId}`; sides 240..=3840, area ≤ 3840×2160.
pub fn viewport(v: &Value) -> Result<Value> {
    let invalid = || ids::invalid("viewport");
    let mut r = Reader::of("viewport", v)?;
    let tag = r.req_str("_tag").map_err(|_| invalid())?;
    let size = |r: &mut Reader| -> Result<(u64, u64)> {
        let side = |r: &mut Reader, k: &'static str| r.req_u64(k).ok().filter(|s| (VIEWPORT_MIN..=VIEWPORT_MAX).contains(s)).ok_or_else(invalid);
        let (w, h) = (side(r, "width")?, side(r, "height")?);
        if w * h > VIEWPORT_AREA_MAX {
            return Err(invalid());
        }
        Ok((w, h))
    };
    let out = match tag.as_str() {
        "fill" => json!({"_tag":"fill"}),
        "freeform" => {
            let (w, h) = size(&mut r)?;
            json!({"_tag":"freeform","width":w,"height":h})
        }
        "preset" => {
            let (w, h) = size(&mut r)?;
            let preset = r.req_str("presetId").map_err(|_| invalid())?;
            json!({"_tag":"preset","width":w,"height":h,"presetId":one_of("viewport", &preset, VIEWPORT_PRESETS)?})
        }
        _ => return Err(invalid()),
    };
    r.done().map_err(|_| invalid())?;
    Ok(out)
}

/// `preview.list`: `{sessions[], serverEpoch, revision}` for one thread.
pub fn list(conn: &Connection, thread_id: &str) -> Result<Value> {
    send(conn, &LIST, thread_payload(thread_id)?)
}
/// `PreviewOpenInput` options; all optional.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PreviewOpen {
    pub url: Option<String>,
    /// See `viewport`.
    pub viewport: Option<Value>,
    pub profile_id: Option<String>,
    /// `desktop` or `server`.
    pub runtime: Option<String>,
    pub reveal: Option<bool>,
}
/// `preview.open`: the new `PreviewSession`. Creates a tab: read `list` before repeating.
pub fn open(conn: &Connection, thread_id: &str, open: &PreviewOpen) -> Result<Value> {
    send(conn, &OPEN, open_payload(thread_id, open)?)
}
fn open_payload(thread_id: &str, o: &PreviewOpen) -> Result<Value> {
    let mut v = thread_payload(thread_id)?;
    put(&mut v, "url", o.url.as_deref().map(|u| url("url", u)).transpose()?);
    put(&mut v, "viewport", o.viewport.as_ref().map(viewport).transpose()?);
    put(&mut v, "profileId", o.profile_id.as_deref().map(|p| ids::token("profileId", p, PROFILE_ID_MAX)).transpose()?);
    put(&mut v, "runtime", o.runtime.as_deref().map(|r| one_of("runtime", r, RUNTIMES)).transpose()?);
    put(&mut v, "reveal", o.reveal);
    Ok(v)
}
/// `preview.navigate`: `PreviewSession`.
pub fn navigate(conn: &Connection, thread_id: &str, tab_id: &str, url: &str, resolved_title: Option<&str>) -> Result<Value> {
    send(conn, &NAVIGATE, navigate_payload(thread_id, tab_id, url, resolved_title)?)
}
fn navigate_payload(thread_id: &str, tab_id: &str, to: &str, resolved_title: Option<&str>) -> Result<Value> {
    let mut v = tab_payload(thread_id, tab_id)?;
    v["url"] = json!(url("url", to)?);
    put(&mut v, "resolvedTitle", resolved_title.map(|t| bounded("resolvedTitle", t, TITLE_MAX)).transpose()?);
    Ok(v)
}
/// `preview.resize`: `PreviewSession`.
pub fn resize(conn: &Connection, thread_id: &str, tab_id: &str, viewport: &Value) -> Result<Value> {
    send(conn, &RESIZE, resize_payload(thread_id, tab_id, viewport)?)
}
fn resize_payload(thread_id: &str, tab_id: &str, size: &Value) -> Result<Value> {
    let mut v = tab_payload(thread_id, tab_id)?;
    v["viewport"] = viewport(size)?;
    Ok(v)
}
/// `PreviewAdjustInput` options; all optional.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PreviewAdjust {
    /// `system`, `light` or `dark`.
    pub color_scheme: Option<String>,
    /// One of `ZOOM_FACTORS`.
    pub zoom_factor: Option<f64>,
    pub hard_reload: Option<bool>,
    /// `cookies` or `cache`.
    pub clear: Option<String>,
}
/// `preview.adjust`: `PreviewSession`.
pub fn adjust(conn: &Connection, thread_id: &str, tab_id: &str, adjust: &PreviewAdjust) -> Result<Value> {
    send(conn, &ADJUST, adjust_payload(thread_id, tab_id, adjust)?)
}
fn adjust_payload(thread_id: &str, tab_id: &str, a: &PreviewAdjust) -> Result<Value> {
    let mut v = tab_payload(thread_id, tab_id)?;
    put(&mut v, "colorScheme", a.color_scheme.as_deref().map(|c| one_of("colorScheme", c, COLOR_SCHEMES)).transpose()?);
    if let Some(zoom) = a.zoom_factor {
        if !ZOOM_FACTORS.contains(&zoom) {
            return Err(ids::invalid("zoomFactor"));
        }
        v["zoomFactor"] = json!(zoom);
    }
    put(&mut v, "hardReload", a.hard_reload);
    put(&mut v, "clear", a.clear.as_deref().map(|c| one_of("clear", c, CLEAR_TARGETS)).transpose()?);
    Ok(v)
}
/// `preview.refresh`: `null`.
pub fn refresh(conn: &Connection, thread_id: &str, tab_id: &str) -> Result<Value> {
    send(conn, &REFRESH, tab_payload(thread_id, tab_id)?)
}
/// `preview.close`: `null`. Without `tab_id`, closes every tab of the thread.
pub fn close(conn: &Connection, thread_id: &str, tab_id: Option<&str>) -> Result<Value> {
    send(conn, &CLOSE, close_payload(thread_id, tab_id)?)
}
fn close_payload(thread_id: &str, tab_id: Option<&str>) -> Result<Value> {
    let mut v = thread_payload(thread_id)?;
    put(&mut v, "tabId", tab_id.map(|t| ids::token("tabId", t, TAB_ID_MAX)).transpose()?);
    Ok(v)
}
/// `preview.clearProfile`: `null`. Deletes a browser profile's data on the server.
pub fn clear_profile(conn: &Connection, profile_id: &str) -> Result<Value> {
    send(conn, &CLEAR_PROFILE, clear_profile_payload(profile_id)?)
}
fn clear_profile_payload(profile_id: &str) -> Result<Value> {
    Ok(json!({"profileId":ids::token("profileId", profile_id, PROFILE_ID_MAX)?}))
}
/// `PreviewNavStatus`: `{_tag:"Idle"}`, `{_tag:"Loading"|"Success",url,title}` or
/// `{_tag:"LoadFailed",url,title,code,description,download?:{id,fileName}}`.
fn nav_status(v: &Value) -> Result<Value> {
    let invalid = || ids::invalid("navStatus");
    let mut r = Reader::of("navStatus", v)?;
    let tag = r.req_str("_tag").map_err(|_| invalid())?;
    let page = |r: &mut Reader| -> Result<Value> {
        let to = r.req_str("url").map_err(|_| invalid())?;
        let title = r.raw("title").and_then(Value::as_str).ok_or_else(invalid)?;
        Ok(json!({"_tag":tag,"url":url("navStatus", &to)?,"title":bounded("navStatus", title, TITLE_MAX)?}))
    };
    let out = match tag.as_str() {
        "Idle" => json!({"_tag":"Idle"}),
        "Loading" | "Success" => page(&mut r)?,
        "LoadFailed" => {
            let mut v = page(&mut r)?;
            v["code"] = json!(r.raw("code").and_then(Value::as_i64).ok_or_else(invalid)?);
            v["description"] = json!(bounded("navStatus", r.raw("description").and_then(Value::as_str).ok_or_else(invalid)?, 4096)?);
            if let Some(d) = r.opt_value("download") {
                let mut d = Reader::of("navStatus", &d)?;
                let (id, name) = (d.req_str("id").map_err(|_| invalid())?, d.req_str("fileName").map_err(|_| invalid())?);
                d.done().map_err(|_| invalid())?;
                v["download"] = json!({"id":bounded("navStatus", &id, 256)?,"fileName":bounded("navStatus", &name, 1024)?});
            }
            v
        }
        _ => return Err(invalid()),
    };
    r.done().map_err(|_| invalid())?;
    Ok(out)
}
/// `preview.reportStatus`: `null`. A host-rendered preview reports its state.
pub fn report_status(conn: &Connection, thread_id: &str, tab_id: &str, nav: &Value, can_go_back: bool, can_go_forward: bool) -> Result<Value> {
    send(conn, &REPORT_STATUS, report_status_payload(thread_id, tab_id, nav, can_go_back, can_go_forward)?)
}
fn report_status_payload(thread_id: &str, tab_id: &str, nav: &Value, can_go_back: bool, can_go_forward: bool) -> Result<Value> {
    let mut v = tab_payload(thread_id, tab_id)?;
    v["navStatus"] = nav_status(nav)?;
    v["canGoBack"] = json!(can_go_back);
    v["canGoForward"] = json!(can_go_forward);
    Ok(v)
}
