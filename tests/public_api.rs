//! Integration tests against Portal's public API only.
//!
//! These are the regression suite for the crate's contract: the identifiers
//! agents type, the arguments they pass, and the errors they get back. Unit
//! tests may reach into private helpers; nothing here may.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use portal::catalog::{self, Category};
use portal::{Error, Portal, Settings};
use serde_json::{Value, json};

#[test]
fn the_catalog_covers_every_advertised_category() {
    assert!(catalog::len() > 100);
    for category in Category::ALL {
        assert!(
            catalog::in_category(*category).next().is_some(),
            "{category} has no capabilities"
        );
    }
}

#[test]
fn the_flagship_capabilities_keep_their_identifiers() {
    // These ids are documented in the README and the Agent Skill; renaming one
    // is a breaking change to the agent-facing contract, not a refactor.
    for id in [
        "search.web",
        "search.chat",
        "search.extract",
        "research.start",
        "research.status",
        "research.result",
        "models.list",
        "models.chat",
        "models.embed",
        "models.speak",
        "models.transcribe",
        "media.image",
        "media.video",
        "tools.list",
        "tools.execute",
        "files.create",
        "files.download",
        "credits.balance",
        "pricing.list",
        "health.check",
    ] {
        assert!(catalog::find(id).is_some(), "{id} is missing");
    }
}

#[test]
fn every_capability_documents_itself_well_enough_to_invoke() {
    for capability in catalog::all() {
        assert!(
            !capability.summary.is_empty(),
            "{} has no summary",
            capability.id
        );
        assert!(
            !capability.provider.is_empty(),
            "{} has no provider",
            capability.id
        );
        assert!(
            capability.path.starts_with('/'),
            "{} has a relative path",
            capability.id
        );
        let schema = capability.input_schema();
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(
            schema["required"].as_array().expect("required").len(),
            capability.required_params().count()
        );
    }
}

#[test]
fn planning_a_request_validates_arguments_before_any_network_call() {
    let capability = catalog::find("search.web").expect("search.web");
    let request = portal::invoke::plan(
        capability,
        &json!({ "objective": "rust", "search_queries": ["rust"] }),
    )
    .expect("the request plans");
    assert_eq!(request.path, "/agent-integrations/parallel/search");

    let error = portal::invoke::plan(capability, &json!({ "objective": "rust" }))
        .expect_err("search_queries is required");
    assert!(matches!(error, Error::MissingArgument { .. }));
}

#[tokio::test]
async fn invoking_without_a_credential_fails_locally() {
    let portal = Portal::new(Settings::new("http://localhost:1"));
    let error = portal
        .invoke("credits.balance", &Value::Null)
        .await
        .expect_err("a credential is required");
    assert!(matches!(error, Error::MissingCredentials));
}

#[tokio::test]
async fn invoking_an_unknown_capability_fails_locally() {
    let portal = Portal::new(Settings::new("http://localhost:1").with_api_key(Some("k".into())));
    let error = portal
        .invoke("search.wob", &Value::Null)
        .await
        .expect_err("the capability does not exist");
    assert!(matches!(error, Error::UnknownCapability(_)));
}

#[test]
fn settings_never_print_a_credential() {
    let settings =
        Settings::new("https://api.tinyhumans.ai").with_api_key(Some("sk-secret".into()));
    assert!(!format!("{settings:?}").contains("sk-secret"));
    assert_eq!(settings.credential_kind(), "api key");
}

#[test]
fn the_environment_variable_names_are_part_of_the_contract() {
    assert_eq!(portal::API_KEY_VAR, "TINYHUMANS_API_KEY");
    assert_eq!(portal::TOKEN_VAR, "TINYHUMANS_TOKEN");
    assert_eq!(portal::BASE_URL_VAR, "TINYHUMANS_BASE_URL");
    assert_eq!(portal::DEFAULT_BASE_URL, "https://api.tinyhumans.ai");
}
