//! Unit tests for settings resolution.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::{DEFAULT_BASE_URL, Settings};

fn lookup(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |name| {
        pairs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_owned())
    }
}

#[test]
fn defaults_to_the_public_backend_without_credentials() {
    let settings = Settings::from_lookup(lookup(&[]));
    assert_eq!(settings.base_url(), DEFAULT_BASE_URL);
    assert_eq!(settings.api_key(), None);
    assert_eq!(settings.token(), None);
    assert!(!settings.is_authenticated());
    assert_eq!(settings.credential_kind(), "none");
}

#[test]
fn reads_every_variable_from_the_environment() {
    let settings = Settings::from_lookup(lookup(&[
        ("TINYHUMANS_BASE_URL", "https://staging-api.tinyhumans.ai/"),
        ("TINYHUMANS_API_KEY", "th-key"),
        ("TINYHUMANS_TOKEN", "jwt"),
    ]));
    assert_eq!(settings.base_url(), "https://staging-api.tinyhumans.ai");
    assert_eq!(settings.api_key(), Some("th-key"));
    assert_eq!(settings.token(), Some("jwt"));
    assert_eq!(settings.credential_kind(), "api key and bearer token");
}

#[test]
fn treats_blank_variables_as_absent() {
    let settings = Settings::from_lookup(lookup(&[
        ("TINYHUMANS_BASE_URL", "   "),
        ("TINYHUMANS_API_KEY", ""),
        ("TINYHUMANS_TOKEN", "  jwt  "),
    ]));
    assert_eq!(settings.base_url(), DEFAULT_BASE_URL);
    assert_eq!(settings.api_key(), None);
    assert_eq!(settings.token(), Some("jwt"));
    assert_eq!(settings.credential_kind(), "bearer token");
}

#[test]
fn trims_a_trailing_slash_from_an_explicit_base_url() {
    assert_eq!(
        Settings::new("http://localhost:5005/").base_url(),
        "http://localhost:5005"
    );
}

#[test]
fn builders_replace_each_credential_independently() {
    let settings = Settings::default().with_api_key(Some("th-key".to_owned()));
    assert_eq!(settings.credential_kind(), "api key");
    let settings = settings
        .with_api_key(None)
        .with_token(Some("jwt".to_owned()));
    assert_eq!(settings.credential_kind(), "bearer token");
    assert!(settings.is_authenticated());
}

#[test]
fn from_env_reads_the_real_process_environment() {
    // Only the default is asserted: the test process must not depend on, or
    // mutate, whatever credentials the developer has exported.
    let settings = Settings::from_env();
    assert!(!settings.base_url().ends_with('/'));
}

#[test]
fn debug_output_never_contains_a_credential() {
    let settings = Settings::default()
        .with_api_key(Some("th-secret-key".to_owned()))
        .with_token(Some("jwt-secret".to_owned()));
    let rendered = format!("{settings:?}");
    assert!(!rendered.contains("th-secret-key"));
    assert!(!rendered.contains("jwt-secret"));
    assert!(rendered.contains("api key and bearer token"));
}
