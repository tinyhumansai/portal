//! Unit tests for the client, against a local mock backend.
//!
//! Nothing here touches the network or a real credential: `wiremock` serves the
//! backend contract on a loopback port, which keeps the transport, the envelope
//! handling, and the multipart path under test and deterministic.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{Value, json};
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::Portal;
use crate::catalog::Method;
use crate::config::Settings;
use crate::error::Error;

/// A client pointed at `server`, carrying a credential.
fn portal(server: &MockServer) -> Portal {
    Portal::new(Settings::new(server.uri()).with_api_key(Some("th-test-key".to_owned())))
}

#[tokio::test]
async fn invokes_a_capability_and_unwraps_the_envelope() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/agent-integrations/parallel/search"))
        .and(body_json(
            json!({ "objective": "rust", "searchQueries": ["rust"] }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "data": { "results": [{ "url": "https://rust-lang.org" }] },
        })))
        .mount(&server)
        .await;

    let result = portal(&server)
        .invoke(
            "search.web",
            &json!({ "objective": "rust", "search_queries": ["rust"] }),
        )
        .await
        .expect("the call succeeds");
    assert_eq!(result["results"][0]["url"], "https://rust-lang.org");
}

#[tokio::test]
async fn returns_openai_compatible_bodies_unwrapped() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/openai/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{ "id": "tiny-1" }],
        })))
        .mount(&server)
        .await;

    let result = portal(&server)
        .invoke("models.list", &Value::Null)
        .await
        .expect("the call succeeds");
    // No `data` unwrapping: the OpenAI-shaped body is returned as sent.
    assert_eq!(result["object"], "list");
    assert_eq!(result["data"][0]["id"], "tiny-1");
}

#[tokio::test]
async fn sends_query_arguments() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/agent-integrations/apify/runs/r1/results"))
        .and(query_param("limit", "5"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "success": true, "data": [] })),
        )
        .mount(&server)
        .await;

    let result = portal(&server)
        .invoke("apify.runs.results", &json!({ "run_id": "r1", "limit": 5 }))
        .await
        .expect("the call succeeds");
    assert_eq!(result, json!([]));
}

#[tokio::test]
async fn uploads_a_local_file_as_multipart() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/agent-integrations/file-storage/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "data": { "id": "file_1" },
        })))
        .mount(&server)
        .await;

    let file = std::env::temp_dir().join("portal-upload-test.txt");
    std::fs::write(&file, b"portal").expect("the fixture is writable");
    let result = portal(&server)
        .invoke(
            "files.create",
            &json!({ "file": file.to_string_lossy(), "visibility": "private" }),
        )
        .await
        .expect("the upload succeeds");
    std::fs::remove_file(&file).expect("the fixture is removable");
    assert_eq!(result["id"], "file_1");
}

#[tokio::test]
async fn reports_an_unreadable_upload_before_calling_the_backend() {
    let server = MockServer::start().await;
    let error = portal(&server)
        .invoke(
            "files.create",
            &json!({ "file": "/nonexistent/portal.txt" }),
        )
        .await
        .expect_err("the file cannot be read");
    assert!(matches!(error, Error::Io(_)));
}

#[tokio::test]
async fn downloads_bytes_through_the_byte_entry_point() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/agent-integrations/file-storage/files/f1/download"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"%PDF-1.7".to_vec()))
        .mount(&server)
        .await;

    let bytes = portal(&server)
        .invoke_bytes("files.download", &json!({ "file_id": "f1" }))
        .await
        .expect("the download succeeds");
    assert_eq!(bytes, b"%PDF-1.7");
}

#[tokio::test]
async fn refuses_a_binary_capability_through_the_json_entry_point() {
    let server = MockServer::start().await;
    let error = portal(&server)
        .invoke("files.download", &json!({ "file_id": "f1" }))
        .await
        .expect_err("bytes are not returned as json");
    match error {
        Error::BinaryResponse { capability } => assert_eq!(capability, "files.download"),
        other => panic!("unexpected error: {other}"),
    }
}

#[tokio::test]
async fn surfaces_a_backend_failure_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/payments/credits/balance"))
        .respond_with(ResponseTemplate::new(402).set_body_json(json!({ "error": "no credits" })))
        .mount(&server)
        .await;

    let error = portal(&server)
        .invoke("credits.balance", &Value::Null)
        .await
        .expect_err("402 is an error");
    assert!(error.to_string().contains("402"));
}

#[tokio::test]
async fn calls_a_public_route_through_the_raw_escape_hatch() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/announcements/latest"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "data": { "title": "portal" },
        })))
        .mount(&server)
        .await;

    let result = portal(&server)
        .raw(Method::Get, "/announcements/latest", None)
        .await
        .expect("the call succeeds");
    assert_eq!(result["title"], "portal");
}

#[tokio::test]
async fn a_public_capability_needs_no_credential() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "success": true, "data": "ok" })),
        )
        .mount(&server)
        .await;

    let portal = Portal::new(Settings::new(server.uri()));
    assert_eq!(
        portal
            .invoke("health.check", &Value::Null)
            .await
            .expect("ok"),
        json!("ok")
    );
}

#[test]
fn refuses_an_authenticated_capability_without_a_credential() {
    let portal = Portal::new(Settings::new("http://localhost:5005"));
    let error = portal
        .prepare("credits.balance", &Value::Null)
        .expect_err("a credential is required");
    assert!(matches!(error, Error::MissingCredentials));
}

#[test]
fn refuses_an_unknown_capability() {
    let portal = Portal::new(Settings::new("http://localhost:5005"));
    let error = portal
        .prepare("search.wob", &Value::Null)
        .expect_err("the capability does not exist");
    match error {
        Error::UnknownCapability(id) => assert_eq!(id, "search.wob"),
        other => panic!("unexpected error: {other}"),
    }
}

#[tokio::test]
async fn refuses_credentials_over_cleartext_http_to_a_non_loopback_host() {
    let portal = Portal::new(
        Settings::new("http://backend.example.com").with_api_key(Some("th-test-key".into())),
    );
    let error = portal
        .invoke("credits.balance", &Value::Null)
        .await
        .expect_err("http to a non-loopback host is refused");
    match error {
        Error::InsecureCredentials { base_url } => {
            assert_eq!(base_url, "http://backend.example.com");
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[tokio::test]
async fn allows_credentials_over_cleartext_http_to_localhost() {
    let server = MockServer::start().await;
    // `server.uri()` is already a loopback origin (`http://127.0.0.1:<port>`);
    // `portal(&server)` carries a credential, and every mocked test above
    // already exercises this path. This test additionally covers the
    // `localhost` hostname spelling specifically.
    Mock::given(method("GET"))
        .and(path("/payments/credits/balance"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "success": true, "data": 5 })),
        )
        .mount(&server)
        .await;
    let base_url = server.uri().replacen("127.0.0.1", "localhost", 1);
    let portal = Portal::new(Settings::new(base_url).with_api_key(Some("th-test-key".into())));
    let result = portal
        .invoke("credits.balance", &Value::Null)
        .await
        .expect("localhost is exempt from the transport-security check");
    assert_eq!(result, json!(5));
}

#[tokio::test]
async fn refuses_a_byte_capability_that_needs_a_body_the_transport_cannot_send() {
    let server = MockServer::start().await;
    let error = portal(&server)
        .invoke_bytes("models.speak", &json!({ "text": "hello" }))
        .await
        .expect_err("the byte transport cannot send a body");
    match error {
        Error::UnsupportedByteRequest { capability } => assert_eq!(capability, "models.speak"),
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn exposes_its_settings_and_sdk_without_leaking_credentials() {
    let portal =
        Portal::new(Settings::new("http://localhost:5005/").with_token(Some("jwt".into())));
    assert_eq!(portal.settings().base_url(), "http://localhost:5005");
    let rendered = format!("{portal:?}");
    assert!(!rendered.contains("jwt"));
    assert!(rendered.contains("bearer token"));
    // The vendored client is reachable for anything Portal does not model.
    let _sdk = portal.sdk();
}

#[test]
fn from_env_builds_a_client() {
    let portal = Portal::from_env();
    assert!(!portal.settings().base_url().is_empty());
}
