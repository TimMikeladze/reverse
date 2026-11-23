//! A three-function C ABI over the toolchain, so the browser needs no
//! binding generator: the page writes a JSON request into memory it asked
//! for, calls `q_call`, and reads a length-prefixed JSON reply.
//!
//! Request:  `{"tool": "verse_check", "arguments": {"source": "..."}}`
//! Reply:    `{"ok": true, "text": "...", "report": {...}}`
//!           or `{"ok": false, "text": "what went wrong"}`
//!
//! The tools are the same ones the MCP server exposes.

use serde_json::{json, Value as Json};

/// Reserve `len` bytes for the caller to fill.
#[no_mangle]
pub extern "C" fn q_alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::<u8>::with_capacity(len.max(1));
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Release memory obtained from `q_alloc` or returned by `q_call`.
///
/// # Safety
/// `ptr` and `cap` must describe exactly one such allocation, freed once.
#[no_mangle]
pub unsafe extern "C" fn q_free(ptr: *mut u8, cap: usize) {
    if !ptr.is_null() {
        drop(Vec::from_raw_parts(ptr, 0, cap.max(1)));
    }
}

/// Run a request. Returns a buffer holding a little-endian `u32` length
/// followed by that many bytes of UTF-8 JSON; free it with
/// `q_free(ptr, length + 4)`.
///
/// # Safety
/// `ptr` must point to `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn q_call(ptr: *const u8, len: usize) -> *mut u8 {
    let input = std::slice::from_raw_parts(ptr, len);
    let reply = handle(input).to_string().into_bytes();
    let mut out = Vec::with_capacity(reply.len() + 4);
    out.extend_from_slice(&(reply.len() as u32).to_le_bytes());
    out.extend_from_slice(&reply);
    out.shrink_to_fit();
    let p = out.as_mut_ptr();
    std::mem::forget(out);
    p
}

/// The request handler, usable natively for tests.
pub fn handle(input: &[u8]) -> Json {
    let req: Json = match serde_json::from_slice(input) {
        Ok(r) => r,
        Err(e) => return json!({ "ok": false, "text": format!("bad request: {}", e) }),
    };
    let tool = req["tool"].as_str().unwrap_or("");
    let mut args = req.get("arguments").cloned().unwrap_or(json!({}));
    // There is no file system in the browser.
    if let Some(obj) = args.as_object_mut() {
        obj.remove("path");
    }
    match reverse_cli::mcp::call_tool(tool, &args) {
        Ok((text, report)) => json!({ "ok": true, "text": text, "report": report }),
        Err(message) => json!({ "ok": false, "text": message }),
    }
}
