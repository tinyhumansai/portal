//! The client that actually calls the backend.
//!
//! [`Portal`] is the one type a library caller needs. It owns a
//! [`Settings`] and a vendored [`TinyHumansClient`], resolves a capability
//! identifier through the [`catalog`](crate::catalog), plans the request with
//! [`invoke::plan`], and sends it.
//!
//! Three guards run before anything leaves the process, because each of them
//! turns a confusing remote failure into an obvious local one:
//!
//! 1. an unknown identifier is rejected against the catalog;
//! 2. a capability that needs a credential is refused when none is configured;
//! 3. a byte-returning capability refuses the JSON entry point, so an audio
//!    file never lands in an agent's context as a wall of encoded text.
//!
//! # Example
//!
//! ```no_run
//! use portal::Portal;
//! use serde_json::json;
//!
//! # async fn run() -> Result<(), portal::Error> {
//! let portal = Portal::from_env();
//! let results = portal
//!     .invoke("search.web", &json!({
//!         "objective": "who ships the fastest rust http client",
//!         "search_queries": ["fastest rust http client"],
//!     }))
//!     .await?;
//! println!("{results:#}");
//! # Ok(())
//! # }
//! ```

use serde_json::Value;
use tinyhumans_sdk::TinyHumansClient;

use crate::catalog::{self, BodyKind, Capability, Envelope};
use crate::config::Settings;
use crate::error::{Error, Result};
use crate::invoke::{self, Request};

/// A configured door to every backend capability.
#[derive(Clone)]
pub struct Portal {
    settings: Settings,
    client: TinyHumansClient,
}

impl std::fmt::Debug for Portal {
    // The vendored client is not `Debug`, and its credentials must not be
    // printed even if it were; the redacted settings say everything useful.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Portal")
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}

impl Portal {
    /// Build a client from explicit settings.
    #[must_use]
    pub fn new(settings: Settings) -> Self {
        let client = TinyHumansClient::new(settings.base_url())
            .with_api_key(settings.api_key().map(str::to_owned))
            .with_token(settings.token().map(str::to_owned));
        Self { settings, client }
    }

    /// Build a client from the process environment.
    ///
    /// See [`Settings::from_env`] for the variables that are read.
    #[must_use]
    pub fn from_env() -> Self {
        Self::new(Settings::from_env())
    }

    /// The settings this client was built from.
    #[must_use]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// The vendored SDK client, for anything Portal does not model.
    #[must_use]
    pub fn sdk(&self) -> &TinyHumansClient {
        &self.client
    }

    /// Resolve a capability and check it can be invoked as JSON.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownCapability`], [`Error::MissingCredentials`], or
    /// [`Error::BinaryResponse`].
    pub fn prepare(&self, id: &str, arguments: &Value) -> Result<Request> {
        let capability = self.resolve(id)?;
        if capability.is_binary() {
            return Err(Error::BinaryResponse {
                capability: capability.id.to_owned(),
            });
        }
        invoke::plan(capability, arguments)
    }

    /// Invoke a capability and return its JSON result.
    ///
    /// # Errors
    ///
    /// Returns any error from [`Portal::prepare`], [`Error::InsecureCredentials`]
    /// when a credential is configured over a non-loopback `http://` origin,
    /// [`Error::Io`] when an upload file cannot be read, and [`Error::Backend`]
    /// when the backend rejects the call or the transport fails.
    pub async fn invoke(&self, id: &str, arguments: &Value) -> Result<Value> {
        let request = self.prepare(id, arguments)?;
        self.send(&request).await
    }

    /// Invoke a capability whose response is bytes.
    ///
    /// This is the entry point for downloads and generated audio. The byte
    /// transport only sends the path, so a capability that also needs query or
    /// body arguments is refused locally rather than sending an incomplete
    /// request.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownCapability`], [`Error::MissingCredentials`],
    /// [`Error::InsecureCredentials`], [`Error::UnsupportedByteRequest`] when
    /// the capability needs query or body arguments, argument errors from
    /// [`invoke::plan`], or [`Error::Backend`].
    pub async fn invoke_bytes(&self, id: &str, arguments: &Value) -> Result<Vec<u8>> {
        let capability = self.resolve(id)?;
        let request = invoke::plan(capability, arguments)?;
        if request_carries_unsent_arguments(&request) {
            return Err(Error::UnsupportedByteRequest {
                capability: capability.id.to_owned(),
            });
        }
        self.check_transport_security()?;
        Ok(self
            .client
            .raw()
            .send_bytes(capability.method.into(), &request.path)
            .await?)
    }

    /// Send an already-planned request.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InsecureCredentials`] when a credential is configured
    /// over a non-loopback `http://` origin, [`Error::Io`] when an upload file
    /// cannot be read, and [`Error::Backend`] when the call fails.
    pub async fn send(&self, request: &Request) -> Result<Value> {
        self.check_transport_security()?;
        let capability = request.capability;
        if capability.body == BodyKind::Multipart {
            return self.send_multipart(request).await;
        }
        Ok(self
            .client
            .raw()
            .send(
                capability.method.into(),
                &request.path,
                &request.query,
                request.body.as_ref(),
                capability.envelope == Envelope::Unwrap,
            )
            .await?)
    }

    /// Call a public route directly, for anything the catalog does not cover.
    ///
    /// The vendored SDK still refuses administrative routes, so this widens the
    /// surface to newly deployed public routes only.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InsecureCredentials`] when a credential is configured
    /// over a non-loopback `http://` origin, [`Error::Backend`] when the route
    /// is not exposed, the backend rejects the call, or the transport fails.
    pub async fn raw(
        &self,
        method: catalog::Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value> {
        self.check_transport_security()?;
        Ok(self
            .client
            .raw()
            .send(method.into(), path, &[], body, true)
            .await?)
    }

    /// Look up a capability and confirm the client can authenticate it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownCapability`] or [`Error::MissingCredentials`].
    fn resolve(&self, id: &str) -> Result<&'static Capability> {
        let capability =
            catalog::find(id).ok_or_else(|| Error::UnknownCapability(id.to_owned()))?;
        if capability.needs_auth() && !self.settings.is_authenticated() {
            return Err(Error::MissingCredentials);
        }
        Ok(capability)
    }

    /// Refuse to send a configured credential over cleartext HTTP.
    ///
    /// `x-api-key` and `Authorization` are attached to every request once a
    /// credential is configured, regardless of whether the capability being
    /// called needs one. A non-loopback `http://` origin would put that
    /// credential on the wire unencrypted, so the call is refused locally.
    /// Loopback origins (`localhost`, `127.0.0.1`, `::1`) are exempt to keep
    /// local development against an unencrypted backend working.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InsecureCredentials`].
    fn check_transport_security(&self) -> Result<()> {
        if self.settings.is_authenticated()
            && is_insecure_credentialed_origin(self.settings.base_url())
        {
            return Err(Error::InsecureCredentials {
                base_url: self.settings.base_url().to_owned(),
            });
        }
        Ok(())
    }

    /// Read every declared file and post the request as `multipart/form-data`.
    async fn send_multipart(&self, request: &Request) -> Result<Value> {
        let mut form = reqwest::multipart::Form::new();
        for (field, path) in &request.files {
            let bytes = tokio::fs::read(path).await?;
            let name = path.file_name().map_or_else(
                || "upload".to_owned(),
                |name| name.to_string_lossy().into_owned(),
            );
            form = form.part(
                (*field).to_owned(),
                reqwest::multipart::Part::bytes(bytes).file_name(name),
            );
        }
        for (field, value) in &request.form {
            form = form.text((*field).to_owned(), value.clone());
        }
        Ok(self
            .client
            .raw()
            .post_multipart(&request.path, form)
            .await?)
    }
}

/// Backend origins the transport-security check treats as local development.
const LOOPBACK_HOSTS: &[&str] = &["localhost", "127.0.0.1", "::1", "[::1]"];

/// Whether `base_url` is cleartext HTTP to a non-loopback host.
///
/// A missing or non-`http://` scheme (including `https://` and anything
/// unrecognized) is not flagged here; only an explicit, non-loopback
/// `http://` origin is insecure enough to refuse a configured credential.
fn is_insecure_credentialed_origin(base_url: &str) -> bool {
    let Some(host_and_rest) = base_url.strip_prefix("http://") else {
        return false;
    };
    !LOOPBACK_HOSTS.iter().any(|host| {
        host_and_rest == *host
            || host_and_rest.starts_with(&format!("{host}/"))
            || host_and_rest.starts_with(&format!("{host}:"))
    })
}

/// Whether a planned byte request needs query or body arguments that
/// [`Portal::invoke_bytes`] cannot send.
fn request_carries_unsent_arguments(request: &Request) -> bool {
    let has_query = request.query.iter().any(|(_, value)| value.is_some());
    let has_body = match &request.body {
        None => false,
        Some(Value::Object(map)) => !map.is_empty(),
        Some(_) => true,
    };
    has_query || has_body
}

#[cfg(test)]
mod test;
