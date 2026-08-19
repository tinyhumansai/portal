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

/// One incoming JSON-RPC message.
#[derive(Debug, Clone, Deserialize)]
pub(super) struct Incoming {
    /// The request identifier, absent on a notification.
    #[serde(default)]
    pub(super) id: Option<Value>,
    /// The method being called.
    pub(super) method: String,
    /// The method's parameters, defaulting to null.
    #[serde(default)]
    pub(super) params: Value,
}

impl Incoming {
    /// Whether the message is a notification, which must not be answered.
    #[must_use]
    pub(super) fn is_notification(&self) -> bool {
        self.id.is_none()
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
