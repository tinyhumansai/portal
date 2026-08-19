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
    /// Returns any error from [`Portal::prepare`], [`Error::Io`] when an upload
    /// file cannot be read, and [`Error::Backend`] when the backend rejects the
    /// call or the transport fails.
    pub async fn invoke(&self, id: &str, arguments: &Value) -> Result<Value> {
        let request = self.prepare(id, arguments)?;
        self.send(&request).await
    }

    /// Invoke a capability whose response is bytes.
    ///
    /// This is the entry point for downloads and generated audio. Query and
    /// body arguments are still validated, but only the path is sent.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownCapability`], [`Error::MissingCredentials`],
    /// argument errors from [`invoke::plan`], or [`Error::Backend`].
    pub async fn invoke_bytes(&self, id: &str, arguments: &Value) -> Result<Vec<u8>> {
        let capability = self.resolve(id)?;
        let request = invoke::plan(capability, arguments)?;
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
    /// Returns [`Error::Io`] when an upload file cannot be read and
    /// [`Error::Backend`] when the call fails.
    pub async fn send(&self, request: &Request) -> Result<Value> {
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
    /// Returns [`Error::Backend`] when the route is not exposed, the backend
    /// rejects the call, or the transport fails.
    pub async fn raw(
        &self,
        method: catalog::Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value> {
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

#[cfg(test)]
mod test;
