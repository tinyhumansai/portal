//! Turning capability arguments into an outgoing backend request.
//!
//! Planning is deliberately separate from sending. [`plan`] is a pure function
//! from a [`Capability`] and a JSON object to a [`Request`], so every argument
//! rule — required, typed, known — is enforced and tested without a network,
//! and an agent's mistake comes back as a precise local error instead of an
//! opaque `400` from three services away.
//!
//! # Example
//!
//! ```
//! use portal::{catalog, invoke};
//! use serde_json::json;
//!
//! let capability = catalog::find("research.status").expect("research.status");
//! let request = invoke::plan(capability, &json!({ "run_id": "run 42" }))?;
//! assert_eq!(request.path, "/agent-integrations/parallel/research/run%2042");
//! # Ok::<(), portal::Error>(())
//! ```

use std::path::PathBuf;

use serde_json::{Map, Value};
use tinyhumans_sdk::enc;

use crate::catalog::{BodyKind, Capability, ParamIn, ParamKind};
use crate::error::{Error, Result};

/// A fully resolved backend request, ready to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The capability the request was planned from.
    pub capability: &'static Capability,
    /// The path with every placeholder substituted and percent-encoded.
    pub path: String,
    /// Query string pairs, in the catalog's parameter order.
    pub query: Vec<(&'static str, Option<String>)>,
    /// The JSON body, when the capability sends one.
    pub body: Option<Value>,
    /// `multipart/form-data` text fields.
    pub form: Vec<(&'static str, String)>,
    /// `multipart/form-data` file fields, as local paths.
    pub files: Vec<(&'static str, PathBuf)>,
}

impl Request {
    /// The request as a single line, for logs and dry runs.
    #[must_use]
    pub fn describe(&self) -> String {
        let mut line = format!("{} {}", self.capability.method.as_str(), self.path);
        let query: Vec<String> = self
            .query
            .iter()
            .filter_map(|(name, value)| value.as_ref().map(|value| format!("{name}={value}")))
            .collect();
        if !query.is_empty() {
            line.push('?');
            line.push_str(&query.join("&"));
        }
        line
    }
}

/// Validate `arguments` against `capability` and build the request it implies.
///
/// # Errors
///
/// Returns [`Error::InvalidArguments`] when `arguments` is not an object or
/// null, [`Error::UnexpectedArgument`] for an argument the capability does not
/// declare, [`Error::MissingArgument`] for an absent required argument, and
/// [`Error::InvalidArgument`] when a value has the wrong JSON type.
pub fn plan(capability: &'static Capability, arguments: &Value) -> Result<Request> {
    let empty = Map::new();
    let arguments = match arguments {
        Value::Object(map) => map,
        Value::Null => &empty,
        other => {
            return Err(Error::InvalidArguments {
                got: json_type_name(other),
            });
        }
    };

    for name in arguments.keys() {
        if capability.param(name).is_none() {
            return Err(Error::UnexpectedArgument {
                capability: capability.id.to_owned(),
                param: name.clone(),
            });
        }
    }

    let mut path = capability.path.to_owned();
    let mut query = Vec::new();
    let mut body = Map::new();
    let mut form = Vec::new();
    let mut files = Vec::new();
    // A capability the contract does not describe carries its whole body under
    // a synthetic `Object`-kind `body` parameter, which replaces the body map
    // rather than nesting under a `body` key. The `Object` kind is what makes
    // that safe: `feedback.create` and friends declare a real *string* field
    // called `body`, which must stay one field among several.
    //
    // Recorded here and applied after the loop rather than returned early, so
    // every remaining parameter is still validated.
    let mut free_form_body = None;

    for param in capability.params {
        let Some(value) =
            lookup(arguments, param.name).or_else(|| lookup(arguments, param.wire_name))
        else {
            if param.required {
                return Err(Error::MissingArgument {
                    capability: capability.id.to_owned(),
                    param: param.name.to_owned(),
                });
            }
            continue;
        };
        if !param.kind.accepts(value) {
            return Err(Error::InvalidArgument {
                param: param.name.to_owned(),
                expected: param.kind.describe().to_owned(),
            });
        }
        match param.location {
            ParamIn::Path => {
                path = path.replace(&format!("{{{}}}", param.wire_name), &enc(&scalar(value)));
            }
            ParamIn::Query => query.push((param.wire_name, Some(scalar(value)))),
            ParamIn::Body => match (capability.body, param.kind) {
                (BodyKind::Multipart, ParamKind::File) => {
                    files.push((param.wire_name, PathBuf::from(scalar(value))));
                }
                (BodyKind::Multipart, _) => form.push((param.wire_name, scalar(value))),
                (_, ParamKind::Object) if param.name == "body" && param.wire_name == "body" => {
                    free_form_body = Some(value.clone());
                }
                (_, _) => {
                    body.insert(param.wire_name.to_owned(), value.clone());
                }
            },
        }
    }

    let body = match (capability.body, free_form_body) {
        (BodyKind::None | BodyKind::Multipart, _) => None,
        (BodyKind::Json, Some(free_form)) => Some(free_form),
        (BodyKind::Json, None) => Some(Value::Object(body)),
    };
    Ok(Request {
        capability,
        path,
        query,
        body,
        form,
        files,
    })
}

/// Fetch an argument, treating an explicit `null` as absent.
fn lookup<'a>(arguments: &'a Map<String, Value>, name: &str) -> Option<&'a Value> {
    arguments.get(name).filter(|value| !value.is_null())
}

/// Render a value for a path segment, a query pair, or a form field.
///
/// Strings pass through unquoted; everything else keeps its JSON spelling so
/// `true` stays `true` rather than becoming `"true"`.
fn scalar(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// The JSON type name used in argument errors.
fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod test;
