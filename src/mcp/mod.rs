//! A Model Context Protocol server over stdio.
//!
//! The server exposes four tools rather than one per capability. Two hundred
//! tool definitions would crowd out the conversation they are meant to serve,
//! so the catalog is *searched* at runtime instead of enumerated up front:
//!
//! | Tool | Purpose |
//! | --- | --- |
//! | `portal_search` | find capabilities in plain words |
//! | `portal_describe` | read one capability's arguments and schema |
//! | `portal_invoke` | call a capability with JSON arguments |
//! | `portal_status` | check configuration, credentials, and reachability |
//!
//! `portal_describe` returns the same JSON Schema the CLI prints, so an agent
//! that reads a capability can invoke it without guessing.
//!
//! # Running
//!
//! ```sh
//! portal mcp
//! ```
//!
//! Register that command as an MCP server in any client; it speaks
//! line-delimited JSON-RPC on stdin and stdout and writes nothing else to
//! stdout, so diagnostics never corrupt the stream.

mod types;

pub use types::PROTOCOL_VERSION;

use std::path::{Component, Path, PathBuf};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::catalog::{self, Capability, Category};
use crate::client::Portal;
use crate::error::Result;
use types::{
    INVALID_PARAMS, INVALID_REQUEST, Incoming, METHOD_NOT_FOUND, PARSE_ERROR, failure, success,
    tool_result,
};

/// How many capabilities a search returns when the caller does not say.
const DEFAULT_SEARCH_LIMIT: usize = 20;

/// An MCP server bound to one configured [`Portal`].
#[derive(Debug, Clone)]
pub struct Server {
    portal: Portal,
}

impl Server {
    /// Build a server over an existing client.
    #[must_use]
    pub fn new(portal: Portal) -> Self {
        Self { portal }
    }

    /// Serve line-delimited JSON-RPC on stdin and stdout until stdin closes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`](crate::Error::Io) when the streams fail.
    pub async fn serve(&self) -> Result<()> {
        let mut lines = BufReader::new(tokio::io::stdin()).lines();
        let mut stdout = tokio::io::stdout();
        while let Some(line) = lines.next_line().await? {
            if line.trim().is_empty() {
                continue;
            }
            if let Some(response) = self.handle(&line).await {
                stdout.write_all(response.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
            }
        }
        Ok(())
    }

    /// Handle one JSON-RPC line, returning the line to write back.
    ///
    /// `None` means the message was a notification and takes no response.
    pub async fn handle(&self, line: &str) -> Option<String> {
        let message: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(error) => {
                return Some(render(&failure(
                    &Value::Null,
                    PARSE_ERROR,
                    format!("invalid json: {error}"),
                )));
            }
        };
        // An absent `id` key is a notification; an explicit `null`, an
        // object, an array, or a boolean is not a valid id at all. `raw_id`
        // keeps that distinction; `echo_id` is only ever used to shape an
        // error response and defaults to `null` when there is nothing valid
        // to echo, per JSON-RPC convention.
        let raw_id = message.get("id").cloned();
        let echo_id = raw_id.clone().unwrap_or(Value::Null);
        if !types::envelope_is_valid(&message, raw_id.as_ref()) {
            return Some(render(&failure(
                &echo_id,
                INVALID_REQUEST,
                "a jsonrpc request needs \"jsonrpc\": \"2.0\" and, if present, a non-null \
                 string or integer id",
            )));
        }
        let Ok(request) = serde_json::from_value::<Incoming>(message) else {
            return Some(render(&failure(
                &echo_id,
                INVALID_REQUEST,
                "a jsonrpc request needs a method",
            )));
        };
        raw_id.as_ref()?;
        Some(render(&self.respond(&echo_id, &request).await))
    }

    /// Dispatch one request to its method.
    async fn respond(&self, id: &Value, request: &Incoming) -> Value {
        match request.method.as_str() {
            "initialize" => success(id, &instructions()),
            "ping" => success(id, &json!({})),
            "tools/list" => success(id, &json!({ "tools": tool_definitions() })),
            "tools/call" => self.call_tool(id, &request.params).await,
            other => failure(id, METHOD_NOT_FOUND, format!("unknown method {other}")),
        }
    }

    /// Run one tool call.
    async fn call_tool(&self, id: &Value, params: &Value) -> Value {
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return failure(id, INVALID_PARAMS, "a tool call needs a name");
        };
        let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
        let result = match name {
            "portal_search" => tool_search(&arguments),
            "portal_describe" => tool_describe(&arguments),
            "portal_status" => self.tool_status().await,
            "portal_invoke" => self.tool_invoke(&arguments).await,
            other => {
                return failure(id, METHOD_NOT_FOUND, format!("unknown tool {other}"));
            }
        };
        success(id, &result)
    }

    /// Report configuration and reachability.
    async fn tool_status(&self) -> Value {
        let settings = self.portal.settings();
        let reachable = self
            .portal
            .invoke("health.check", &Value::Null)
            .await
            .map_or_else(
                |error| json!({ "ok": false, "error": error.to_string() }),
                |_| json!({ "ok": true }),
            );
        tool_result(
            render_pretty(&json!({
                "version": crate::VERSION,
                "base_url": settings.base_url(),
                "credential": settings.credential_kind(),
                "capabilities": catalog::len(),
                "backend": reachable,
            })),
            false,
        )
    }

    /// Invoke a capability, optionally saving a byte response to a file.
    async fn tool_invoke(&self, arguments: &Value) -> Value {
        let Some(id) = arguments.get("capability").and_then(Value::as_str) else {
            return tool_result("portal_invoke needs a capability identifier", true);
        };
        let call_arguments = arguments.get("arguments").cloned().unwrap_or(Value::Null);
        let save_to = arguments.get("save_to").and_then(Value::as_str);

        if let Some(destination) = save_to {
            let destination = match confine_to_working_directory(destination) {
                Ok(path) => path,
                Err(message) => return tool_result(message, true),
            };
            return match self.portal.invoke_bytes(id, &call_arguments).await {
                Ok(bytes) => match tokio::fs::write(&destination, &bytes).await {
                    Ok(()) => tool_result(
                        format!("wrote {} bytes to {}", bytes.len(), destination.display()),
                        false,
                    ),
                    Err(error) => tool_result(
                        format!("could not write {}: {error}", destination.display()),
                        true,
                    ),
                },
                Err(error) => tool_result(error.to_string(), true),
            };
        }
        match self.portal.invoke(id, &call_arguments).await {
            Ok(value) => tool_result(render_pretty(&value), false),
            Err(error) => tool_result(error.to_string(), true),
        }
    }
}

/// Confine an MCP-supplied `save_to` path beneath the process's current
/// working directory.
///
/// `save_to` comes from whatever is driving the MCP tool call — potentially a
/// model acting on prompt-injected content — so it is never trusted as an
/// arbitrary filesystem path. An absolute path, a path that traverses out of
/// the working directory with `..`, or one that resolves through a symlink to
/// outside it is refused before any network call is made.
///
/// # Errors
///
/// Returns a human-readable rejection message, suitable for a tool error
/// result, when `save_to` is unsafe.
fn confine_to_working_directory(save_to: &str) -> std::result::Result<PathBuf, String> {
    let requested = Path::new(save_to);
    if requested.is_absolute() {
        return Err(format!("save_to must be a relative path, got {save_to}"));
    }
    if requested
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(format!("save_to may not contain .. segments: {save_to}"));
    }
    let working_directory = std::env::current_dir()
        .map_err(|error| format!("could not resolve the working directory: {error}"))?;
    let destination = working_directory.join(requested);
    // A relative path with no `..` segment can still escape the working
    // directory if a path component is a symlink to somewhere else. Compare
    // canonical forms when both resolve. A destination whose parent does not
    // exist yet has nothing to canonicalize against; the write that follows
    // fails on its own in that case, so it is not treated as unsafe here.
    let canonical_root = working_directory.canonicalize().ok();
    let canonical_parent = destination
        .parent()
        .and_then(|parent| parent.canonicalize().ok());
    if let (Some(root), Some(parent)) = (canonical_root, canonical_parent)
        && !parent.starts_with(&root)
    {
        return Err(format!("save_to escapes the working directory: {save_to}"));
    }
    Ok(destination)
}

/// The handshake response, naming the tools an agent should reach for first.
fn instructions() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": "portal", "version": crate::VERSION },
        "instructions": format!(
            "Portal exposes {} TinyHumans capabilities — providers, search engines, models, \
             and agents — behind four tools. Start with portal_search to find a capability, \
             portal_describe to read its arguments, then portal_invoke to call it.",
            catalog::len()
        ),
    })
}

/// Search the catalog and return a compact, readable list.
fn tool_search(arguments: &Value) -> Value {
    let query = arguments
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let category = arguments.get("category").and_then(Value::as_str);
    let category = match category.map(Category::parse) {
        Some(None) => {
            return tool_result(
                format!("unknown category; expected one of {}", category_list()),
                true,
            );
        }
        Some(Some(category)) => Some(category),
        None => None,
    };
    let limit = arguments
        .get("limit")
        .and_then(Value::as_u64)
        .and_then(|limit| usize::try_from(limit).ok())
        .unwrap_or(DEFAULT_SEARCH_LIMIT)
        .clamp(1, catalog::len());

    let hits: Vec<&Capability> = catalog::search(query)
        .into_iter()
        .filter(|capability| category.is_none_or(|wanted| capability.category == wanted))
        .take(limit)
        .collect();
    if hits.is_empty() {
        return tool_result(
            format!(
                "no capability matches {query:?}. Categories: {}",
                category_list()
            ),
            false,
        );
    }
    let listing: Vec<Value> = hits
        .iter()
        .map(|capability| {
            json!({
                "id": capability.id,
                "category": capability.category,
                "provider": capability.provider,
                "summary": capability.summary,
                "required": capability
                    .required_params()
                    .map(|param| param.name)
                    .collect::<Vec<_>>(),
            })
        })
        .collect();
    tool_result(render_pretty(&json!({ "matches": listing })), false)
}

/// Describe one capability, including the schema for its arguments.
fn tool_describe(arguments: &Value) -> Value {
    let Some(id) = arguments.get("capability").and_then(Value::as_str) else {
        return tool_result("portal_describe needs a capability identifier", true);
    };
    let Some(capability) = catalog::find(id) else {
        let suggestions: Vec<&str> = catalog::search(id)
            .into_iter()
            .take(5)
            .map(|capability| capability.id)
            .collect();
        return tool_result(
            format!("unknown capability {id}. Closest matches: {suggestions:?}"),
            true,
        );
    };
    tool_result(render_pretty(&describe_value(capability)), false)
}

/// The full JSON view of a capability, shared with `portal describe --json`.
pub(crate) fn describe_value(capability: &'static Capability) -> Value {
    json!({
        "id": capability.id,
        "aliases": capability.aliases,
        "category": capability.category,
        "provider": capability.provider,
        "summary": capability.summary,
        "route": capability.route(),
        "read_only": capability.method.is_read_only(),
        "needs_auth": capability.needs_auth(),
        "returns": if capability.is_binary() { "bytes" } else { "json" },
        "input_schema": capability.input_schema(),
    })
}

/// The tool definitions advertised by `tools/list`.
fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "portal_search",
            "description": format!(
                "Search {} TinyHumans capabilities — search engines, models, media, data, \
                 automation, files, and agents — in plain words. Start here.",
                catalog::len()
            ),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "What you want to do, in plain words." },
                    "category": {
                        "type": "string",
                        "description": format!("Restrict to one category: {}.", category_list()),
                    },
                    "limit": { "type": "integer", "description": "Maximum matches to return. Defaults to 20." },
                },
                "required": [],
                "additionalProperties": false,
            },
        }),
        json!({
            "name": "portal_describe",
            "description": "Read one capability's arguments, route, and JSON Schema before invoking it.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "capability": { "type": "string", "description": "A capability identifier, such as search.web." },
                },
                "required": ["capability"],
                "additionalProperties": false,
            },
        }),
        json!({
            "name": "portal_invoke",
            "description": "Call a capability. Arguments must match the schema from portal_describe.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "capability": { "type": "string", "description": "A capability identifier, such as models.chat." },
                    "arguments": { "type": "object", "description": "The capability's arguments." },
                    "save_to": {
                        "type": "string",
                        "description": "For capabilities that return bytes, a local path to write them to.",
                    },
                },
                "required": ["capability"],
                "additionalProperties": false,
            },
        }),
        json!({
            "name": "portal_status",
            "description": "Report the configured backend, which credential is in use, and whether the backend answers.",
            "inputSchema": { "type": "object", "properties": {}, "required": [], "additionalProperties": false },
        }),
    ]
}

/// Every category slug, comma separated, for help text.
fn category_list() -> String {
    Category::ALL
        .iter()
        .map(|category| category.slug())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Serialize a response line, falling back to a protocol error it cannot fail.
fn render(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| {
        r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"unserializable response"}}"#
            .to_owned()
    })
}

/// Pretty-print a value for a human or a model to read.
fn render_pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

#[cfg(test)]
mod test;
