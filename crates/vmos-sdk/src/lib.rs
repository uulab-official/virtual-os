//! C ABI for talking to a running virtual-os daemon.
//!
//! The ABI is intentionally small and string-based: callers pass method names
//! and JSON parameters, and receive a heap-allocated JSON string back. Use
//! `vmos_free` to release returned strings.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use vmos_client::Client;

/// Library version.
#[no_mangle]
pub extern "C" fn vmos_version() -> *const c_char {
    static VERSION: &[u8] = concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes();
    VERSION.as_ptr() as *const c_char
}

/// Call any daemon method.
///
/// `method` is a NUL-terminated method name (for example `"vm.list"`).
/// `params_json` is a NUL-terminated JSON object, or NULL/empty for `{}`.
/// Returns a heap-allocated NUL-terminated JSON string. The caller must free
/// it with `vmos_free`. Returns NULL only on allocation failure.
/// # Safety
///
/// `method` must be a valid NUL-terminated pointer (or null). `params_json`,
/// if non-null, must be a valid NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn vmos_invoke(
    method: *const c_char,
    params_json: *const c_char,
) -> *mut c_char {
    let method = if method.is_null() {
        return error_json("method pointer is null");
    } else {
        match unsafe { CStr::from_ptr(method) }.to_str() {
            Ok(value) => value.to_string(),
            Err(error) => return error_json(&format!("invalid method: {error}")),
        }
    };

    let params_raw = if params_json.is_null() {
        String::new()
    } else {
        match unsafe { CStr::from_ptr(params_json) }.to_str() {
            Ok(value) => value.to_string(),
            Err(error) => return error_json(&format!("invalid params: {error}")),
        }
    };
    let params: serde_json::Value = if params_raw.trim().is_empty() {
        serde_json::json!({})
    } else {
        match serde_json::from_str(&params_raw) {
            Ok(value) => value,
            Err(error) => return error_json(&format!("invalid JSON params: {error}")),
        }
    };

    let socket = vmos_core::VmStore::default_root().join("daemon.sock");
    let mut client = match Client::connect(&socket) {
        Ok(client) => client,
        Err(error) => return error_json(&format!("cannot connect to daemon: {error}")),
    };

    let result = client.request(&method, &params, vmos_client::LONG_TIMEOUT);
    let payload = match result {
        Ok(value) => value,
        Err(error) => serde_json::json!({ "error": error.to_string() }),
    };
    match serde_json::to_string(&payload) {
        Ok(text) => match CString::new(text) {
            Ok(value) => value.into_raw(),
            Err(_) => error_json("result contained an interior NUL byte"),
        },
        Err(error) => error_json(&format!("cannot serialize result: {error}")),
    }
}

/// # Safety
///
/// `ptr` must be a pointer previously returned by `vmos_invoke`, or null.
#[no_mangle]
pub unsafe extern "C" fn vmos_free(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let _ = CString::from_raw(ptr);
    }
}

fn error_json(message: &str) -> *mut c_char {
    let payload = serde_json::json!({ "error": message });
    let text =
        serde_json::to_string(&payload).unwrap_or_else(|_| "{\"error\":\"internal\"}".into());
    CString::new(text)
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}
