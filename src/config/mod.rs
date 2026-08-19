//! Where Portal gets its backend address and credentials.
//!
//! [`Settings`] is the one place the environment is read. Everything else in
//! the crate takes a [`Settings`] (or an already-built client), which keeps the
//! rest of the code free of ambient state and testable without touching the
//! process environment.
//!
//! # Environment
//!
//! | Variable | Meaning | Default |
//! | --- | --- | --- |
//! | `TINYHUMANS_BASE_URL` | Backend origin to call | `https://api.tinyhumans.ai` |
//! | `TINYHUMANS_API_KEY` | Long-lived API key, sent as `x-api-key` | unset |
//! | `TINYHUMANS_TOKEN` | User bearer token, sent as `Authorization` | unset |
//!
//! At least one credential is required for every capability except the public
//! health check. Both may be set at once; the backend accepts either.
//!
//! # Example
//!
//! ```
//! use portal::Settings;
//!
//! let settings = Settings::new("https://api.tinyhumans.ai")
//!     .with_api_key(Some("th-key".to_owned()));
//! assert!(settings.is_authenticated());
//! ```

/// The environment variable naming the backend origin.
pub const BASE_URL_VAR: &str = "TINYHUMANS_BASE_URL";
/// The environment variable holding a long-lived API key.
pub const API_KEY_VAR: &str = "TINYHUMANS_API_KEY";
/// The environment variable holding a user bearer token.
pub const TOKEN_VAR: &str = "TINYHUMANS_TOKEN";
/// The backend used when [`BASE_URL_VAR`] is unset.
pub const DEFAULT_BASE_URL: &str = "https://api.tinyhumans.ai";

/// Backend address and credentials for a Portal client.
///
/// The [`Debug`] implementation prints the credential *kind* and never the
/// credential, so a settings value can be logged or echoed by `portal doctor`
/// without leaking a key into a terminal, a CI log, or an issue report.
#[derive(Clone, PartialEq, Eq)]
pub struct Settings {
    base_url: String,
    api_key: Option<String>,
    token: Option<String>,
}

impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Settings")
            .field("base_url", &self.base_url)
            .field("credential", &self.credential_kind())
            .finish_non_exhaustive()
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
}

impl Settings {
    /// Settings pointing at `base_url` with no credentials.
    ///
    /// A trailing slash is trimmed so path joining stays predictable.
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        let base_url = base_url.into().trim_end_matches('/').to_owned();
        Self {
            base_url,
            api_key: None,
            token: None,
        }
    }

    /// Read the settings from the process environment.
    ///
    /// Unset or blank variables are treated as absent, so an exported-but-empty
    /// `TINYHUMANS_TOKEN` does not become an `Authorization: Bearer ` header.
    #[must_use]
    pub fn from_env() -> Self {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Read the settings through a caller-supplied variable lookup.
    ///
    /// This is what [`Settings::from_env`] is built from, and what tests use to
    /// exercise resolution without mutating the process-wide environment.
    #[must_use]
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self {
        let clean = |name: &str| {
            lookup(name)
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        };
        Self {
            base_url: clean(BASE_URL_VAR)
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_owned())
                .trim_end_matches('/')
                .to_owned(),
            api_key: clean(API_KEY_VAR),
            token: clean(TOKEN_VAR),
        }
    }

    /// Replace the API key.
    #[must_use]
    pub fn with_api_key(mut self, api_key: Option<String>) -> Self {
        self.api_key = api_key;
        self
    }

    /// Replace the bearer token.
    #[must_use]
    pub fn with_token(mut self, token: Option<String>) -> Self {
        self.token = token;
        self
    }

    /// The backend origin, without a trailing slash.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// The configured API key, if any.
    #[must_use]
    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    /// The configured bearer token, if any.
    #[must_use]
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    /// Whether at least one credential is present.
    #[must_use]
    pub fn is_authenticated(&self) -> bool {
        self.api_key.is_some() || self.token.is_some()
    }

    /// Which credential the client will present, for diagnostics.
    ///
    /// Returns the credential *kind*, never the secret itself, so it is safe to
    /// print from `portal doctor` and to include in an MCP response.
    #[must_use]
    pub fn credential_kind(&self) -> &'static str {
        match (self.api_key.is_some(), self.token.is_some()) {
            (true, true) => "api key and bearer token",
            (true, false) => "api key",
            (false, true) => "bearer token",
            (false, false) => "none",
        }
    }
}

#[cfg(test)]
mod test;
