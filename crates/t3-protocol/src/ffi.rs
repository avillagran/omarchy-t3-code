//! Small C ABI for Kotlin/JNI or other hosts. All I/O stays in the Rust library.
use super::*;
#[cfg(test)]
use std::ffi::CStr;
use std::{
    ffi::{c_char, CString},
    sync::Mutex,
};
struct Handle {
    connection: Option<Connection>,
    key: Arc<DpopKey>,
}
/// # Safety
/// Returned handle must be freed once, after all calls have completed.
#[no_mangle]
pub extern "C" fn t3_client_new() -> *mut std::ffi::c_void {
    Box::into_raw(Box::new(Mutex::new(Handle {
        connection: None,
        key: Arc::new(DpopKey::generate()),
    })))
    .cast()
}
fn call(handle: &Mutex<Handle>, request: Value) -> Result<Value> {
    let mut h = handle.lock().map_err(|_| "internal_error")?;
    match field(&request, "op")? {
        "connectBearer" => {
            let connection = Connection::new(
                field(&request, "httpBaseUrl")?,
                request["wsBaseUrl"].as_str(),
                Credential::bearer(field(&request, "token")?)?,
            )?;
            h.connection = Some(connection);
            Ok(json!({"connected":true}))
        }
        "connectRelay" => {
            let connection = connect_relay(
                field(&request, "relayUrl")?,
                field(&request, "cloudJwt")?,
                field(&request, "clientId")?,
                field(&request, "environmentId")?,
                h.key.clone(),
            )?;
            h.connection = Some(connection);
            Ok(json!({"connected":true}))
        }
        "disconnect" => {
            h.connection = None;
            Ok(json!({"connected":false}))
        }
        "request" => h.connection.as_ref().ok_or("not_connected")?.request_json(
            field(&request, "method")?,
            field(&request, "path")?,
            request.get("body"),
            request["protocol"].as_bool().unwrap_or(true),
        ),
        "rpc" => h
            .connection
            .as_ref()
            .ok_or("not_connected")?
            .rpc(field(&request, "tag")?, request["payload"].clone()),
        _ => Err("unknown_operation".into()),
    }
}
/// # Safety
/// `handle` is a live t3_client_new result; input addresses `len` readable bytes.
/// The UTF-8 response is owned by Rust and must be freed with t3_string_free.
/// Calls on one handle serialize. Do not free a handle while a call is active.
#[no_mangle]
pub unsafe extern "C" fn t3_client_call(
    handle: *mut std::ffi::c_void,
    input: *const u8,
    len: usize,
) -> *mut c_char {
    let result = std::panic::catch_unwind(|| {
        if handle.is_null() || input.is_null() || len > MAX_BYTES as usize {
            return Err("invalid_input".into());
        }
        let bytes = std::slice::from_raw_parts(input, len);
        let request = serde_json::from_slice(bytes).map_err(|_| "malformed")?;
        call(&*handle.cast::<Mutex<Handle>>(), request)
    })
    .unwrap_or_else(|_| Err("internal_error".into()));
    let response = match result {
        Ok(data) => json!({"ok":true,"data":data}),
        Err(code) => json!({"ok":false,"error":code}),
    };
    CString::new(response.to_string()).unwrap().into_raw()
}
/// # Safety
/// Free only a non-freed pointer returned by t3_client_call, or null.
#[no_mangle]
pub unsafe extern "C" fn t3_string_free(value: *mut c_char) {
    if !value.is_null() {
        drop(CString::from_raw(value));
    }
}
/// # Safety
/// Free only a live handle, once, with no concurrent calls; null is accepted.
#[no_mangle]
pub unsafe extern "C" fn t3_client_free(handle: *mut std::ffi::c_void) {
    if !handle.is_null() {
        drop(Box::from_raw(handle.cast::<Mutex<Handle>>()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn c_abi_ownership_and_errors() {
        unsafe {
            let h = t3_client_new();
            let input = br#"{"op":"request","method":"GET","path":"/api/orchestration/shell"}"#;
            let response = t3_client_call(h, input.as_ptr(), input.len());
            let v: Value = serde_json::from_slice(CStr::from_ptr(response).to_bytes()).unwrap();
            assert_eq!(v, json!({"ok":false,"error":"not_connected"}));
            t3_string_free(response);
            let input=br#"{"op":"connectBearer","httpBaseUrl":"https://t3.example","token":"fixture-secret"}"#;
            let response = t3_client_call(h, input.as_ptr(), input.len());
            assert_eq!(
                serde_json::from_slice::<Value>(CStr::from_ptr(response).to_bytes()).unwrap()["ok"],
                true
            );
            t3_string_free(response);
            let response = t3_client_call(std::ptr::null_mut(), input.as_ptr(), input.len());
            assert_eq!(
                serde_json::from_slice::<Value>(CStr::from_ptr(response).to_bytes()).unwrap()
                    ["error"],
                "invalid_input"
            );
            t3_string_free(response);
            t3_client_free(h);
        }
    }
}
