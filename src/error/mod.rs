//! Crate-wide error and result types.
//!
//! Every fallible public function in this crate returns [`Result`], and every
//! failure mode is a distinct [`Error`] variant. Add a variant rather than
//! encoding new context into an existing message: callers match on variants,
//! and message text is not a stable API.
//!
//! The variants split into three groups, which is the distinction an agent
//! needs in order to decide what to do next:
//!
//! - **Configuration** ([`Error::MissingCredentials`]) — nothing will work
//!   until the operator supplies a credential.
//! - **Request construction** ([`Error::UnknownCapability`],
//!   [`Error::MissingArgument`], [`Error::InvalidArgument`],
//!   [`Error::UnexpectedArgument`], [`Error::InvalidArguments`],
//!   [`Error::BinaryResponse`]) — the call was rejected locally, before any
//!   network traffic, and can be fixed by correcting the arguments.
//! - **Execution** ([`Error::Backend`], [`Error::Json`], [`Error::Io`]) — the
//!   call left the process and something downstream failed.

/// Errors returned by this crate.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// No bearer token and no API key were available.
    ///
    /// Set `TINYHUMANS_API_KEY` or `TINYHUMANS_TOKEN`, or build the client
    /// explicitly with a credential.
    #[error("no credential configured: set TINYHUMANS_API_KEY or TINYHUMANS_TOKEN")]
    MissingCredentials,

    /// The requested capability identifier is not in the catalog.
    #[error("unknown capability: {0}")]
    UnknownCapability(String),

    /// A required argument was absent.
    #[error("capability {capability} requires the argument {param}")]
    MissingArgument {
        /// The capability that was invoked.
        capability: String,
        /// The name of the missing argument.
        param: String,
    },

    /// An argument was present but had the wrong JSON type.
    #[error("argument {param} must be {expected}")]
    InvalidArgument {
        /// The name of the offending argument.
        param: String,
        /// The JSON type the capability declares for the argument.
        expected: String,
    },

    /// An argument was supplied that the capability does not declare.
    ///
    /// Rejected rather than ignored: a silently dropped argument reads as a
    /// successful call that quietly did something else.
    #[error("capability {capability} has no argument named {param}")]
    UnexpectedArgument {
        /// The capability that was invoked.
        capability: String,
        /// The name of the undeclared argument.
        param: String,
    },

    /// The top-level arguments were not a JSON object.
    #[error("arguments must be a json object, got {got}")]
    InvalidArguments {
        /// The JSON type that was supplied instead.
        got: &'static str,
    },

    /// A JSON capability was invoked but its response is raw bytes.
    ///
    /// Bytes are never inlined into a JSON result: an audio file or a stored
    /// document would swamp the caller's context. Use the byte-returning form
    /// instead — `Portal::invoke_bytes`, `portal call --out <path>`, or the MCP
    /// `save_to` argument.
    #[error("capability {capability} returns bytes: invoke it with an output path")]
    BinaryResponse {
        /// The capability that was invoked.
        capability: String,
    },

    /// A byte-returning capability was invoked with query or body arguments.
    ///
    /// The byte transport only sends the path: query and body arguments would
    /// be silently dropped rather than reaching the backend, so the call is
    /// refused locally instead of returning an incomplete or misleading
    /// response.
    #[error(
        "capability {capability} needs query or body arguments that the byte transport cannot \
         send; call it without those arguments or use `Portal::invoke` if it returns json"
    )]
    UnsupportedByteRequest {
        /// The capability that was invoked.
        capability: String,
    },

    /// Credentials are configured but the backend origin is cleartext HTTP.
    ///
    /// `x-api-key` and `Authorization` are sent on every request once a
    /// credential is configured, so a non-loopback `http://` origin would put
    /// them on the wire unencrypted. Use `https://`, or point at a loopback
    /// origin for local development.
    #[error(
        "refusing to send credentials to {base_url} over unencrypted http; use https, or omit \
         credentials for local development against localhost or 127.0.0.1"
    )]
    InsecureCredentials {
        /// The configured backend origin.
        base_url: String,
    },

    /// The backend call failed, or the SDK refused to send it.
    #[error("backend call failed: {0}")]
    Backend(#[from] tinyhumans_sdk::Error),

    /// A JSON payload could not be parsed or serialized.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Reading from or writing to a transport stream failed.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// The crate's standard result type.
///
/// Use this alias in public signatures instead of spelling out
/// `std::result::Result<T, Error>`.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod test;
