//! The JSON-RPC 2.0 envelope the Model Context Protocol rides on.
//!
//! Only the framing lives here. It is small, stable, and fully described by the
//! specification, which is why it is written out rather than pulled in: the
//! server speaks four methods over line-delimited stdio, and a hand-written
//! envelope keeps that surface auditable and testable without a runtime.

use serde::Deserialize;
use serde_json::{Value, json};

/// The MCP revision this server implements.
pub const PROTOCOL_VERSION: &str = "2025-06-18";

/// JSON-RPC error code for a malformed message.
pub(super) const PARSE_ERROR: i32 = -32700;
/// JSON-RPC error code for a message that is not a valid request.
pub(super) const INVALID_REQUEST: i32 = -32600;
/// JSON-RPC error code for an unimplemented method.
pub(super) const METHOD_NOT_FOUND: i32 = -32601;
/// JSON-RPC error code for parameters the method cannot accept.
pub(super) const INVALID_PARAMS: i32 = -32602;

/// One incoming JSON-RPC message, once its envelope has passed
/// [`envelope_is_valid`].
///
/// The request identifier is not carried here: whether the caller sent one
/// at all (a request) or omitted it (a notification, per the specification)
/// is a property of the raw message, not of a value that collapses an absent
/// field and an explicit `null` into the same `None`. The caller extracts and
/// validates `id` from the raw [`Value`] before deserializing into this type.
#[derive(Debug, Clone, Deserialize)]
pub(super) struct Incoming {
    /// The method being called.
    pub(super) method: String,
    /// The method's parameters, defaulting to null.
    #[serde(default)]
    pub(super) params: Value,
}

/// Whether `message`'s JSON-RPC envelope is well-formed.
///
/// `jsonrpc` must be exactly `"2.0"`. `id`, when the key is present at all,
/// must be a non-null string or integer: the specification permits `null`
/// in base JSON-RPC but MCP forbids it, and an object, array, or boolean id
/// is never valid. Pass the raw `message.get("id")` so an absent key (a
/// notification) is distinguished from an explicit `null` (invalid).
#[must_use]
pub(super) fn envelope_is_valid(message: &Value, id: Option<&Value>) -> bool {
    if message.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return false;
    }
    match id {
        None | Some(Value::String(_)) => true,
        Some(Value::Number(number)) => number.is_i64() || number.is_u64(),
        Some(_) => false,
    }
}

/// Build a successful JSON-RPC response.
#[must_use]
pub(super) fn success(id: &Value, result: &Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// Build a JSON-RPC error response.
#[must_use]
pub(super) fn failure(id: &Value, code: i32, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message.into() },
    })
}

/// Build a tool result carrying one block of text.
///
/// `is_error` marks a failure the model is expected to read and react to, which
/// the specification distinguishes from a protocol-level error.
#[must_use]
pub(super) fn tool_result(text: impl Into<String>, is_error: bool) -> Value {
    json!({
        "content": [{ "type": "text", "text": text.into() }],
        "isError": is_error,
    })
}
