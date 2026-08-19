//! Unit tests for request planning.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;

use super::plan;
use crate::catalog;
use crate::error::Error;

fn capability(id: &str) -> &'static catalog::Capability {
    catalog::find(id).unwrap_or_else(|| panic!("{id} is in the catalog"))
}

#[test]
fn builds_a_json_body_from_declared_arguments() {
    let request = plan(
        capability("search.web"),
        &json!({ "objective": "rust async", "search_queries": ["rust async"] }),
    )
    .expect("plan");
    assert_eq!(request.path, "/agent-integrations/parallel/search");
    assert!(request.query.is_empty());
    assert_eq!(
        request.body,
        Some(json!({ "objective": "rust async", "searchQueries": ["rust async"] }))
    );
    assert_eq!(
        request.describe(),
        "POST /agent-integrations/parallel/search"
    );
}

#[test]
fn accepts_the_backend_spelling_of_an_argument() {
    let request = plan(
        capability("search.web"),
        &json!({ "objective": "x", "searchQueries": ["x"] }),
    )
    .expect("plan");
    assert_eq!(request.body.expect("body")["searchQueries"], json!(["x"]));
}

#[test]
fn percent_encodes_path_arguments() {
    let request = plan(
        capability("research.status"),
        &json!({ "run_id": "run 42/x" }),
    )
    .expect("plan");
    assert_eq!(
        request.path,
        "/agent-integrations/parallel/research/run%2042%2Fx"
    );
}

#[test]
fn places_query_arguments_in_the_query_string() {
    let request = plan(
        capability("apify.runs.results"),
        &json!({ "run_id": "r1", "limit": 5 }),
    )
    .expect("plan");
    assert_eq!(request.query, vec![("limit", Some("5".to_owned()))]);
    assert_eq!(
        request.describe(),
        "GET /agent-integrations/apify/runs/r1/results?limit=5"
    );
}

#[test]
fn omits_absent_and_null_optional_arguments() {
    let request = plan(
        capability("apify.runs.results"),
        &json!({ "run_id": "r1", "limit": null }),
    )
    .expect("plan");
    assert!(request.query.is_empty());
}

#[test]
fn splits_a_multipart_upload_into_files_and_form_fields() {
    let request = plan(
        capability("files.create"),
        &json!({ "file": "./notes.pdf", "visibility": "private" }),
    )
    .expect("plan");
    assert_eq!(request.body, None);
    assert_eq!(request.files, vec![("file", "./notes.pdf".into())]);
    assert_eq!(request.form, vec![("visibility", "private".to_owned())]);
}

#[test]
fn sends_a_free_form_body_without_nesting_it() {
    let request = plan(
        capability("search.agentic"),
        &json!({ "body": { "query": "flights to lisbon" } }),
    )
    .expect("plan");
    assert_eq!(request.body, Some(json!({ "query": "flights to lisbon" })));
}

#[test]
fn a_get_capability_sends_no_body() {
    let request = plan(capability("models.list"), &json!({})).expect("plan");
    assert_eq!(request.body, None);
    assert_eq!(request.path, "/openai/v1/models");
}

#[test]
fn null_arguments_are_treated_as_no_arguments() {
    let request = plan(capability("models.list"), &serde_json::Value::Null).expect("plan");
    assert_eq!(request.path, "/openai/v1/models");
}

#[test]
fn rejects_arguments_that_are_not_an_object() {
    let error = plan(capability("models.list"), &json!([1, 2])).expect_err("array is rejected");
    assert!(matches!(error, Error::InvalidArguments { got: "array" }));
}

#[test]
fn rejects_an_undeclared_argument() {
    let error =
        plan(capability("models.list"), &json!({ "quiery": "x" })).expect_err("typo is rejected");
    match error {
        Error::UnexpectedArgument { capability, param } => {
            assert_eq!(capability, "models.list");
            assert_eq!(param, "quiery");
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn rejects_a_missing_required_argument() {
    let error = plan(capability("search.web"), &json!({ "objective": "x" }))
        .expect_err("searchQueries is required");
    match error {
        Error::MissingArgument { capability, param } => {
            assert_eq!(capability, "search.web");
            assert_eq!(param, "search_queries");
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn rejects_an_argument_of_the_wrong_type() {
    let error = plan(
        capability("search.web"),
        &json!({ "objective": 7, "search_queries": ["x"] }),
    )
    .expect_err("objective must be a string");
    match error {
        Error::InvalidArgument { param, expected } => {
            assert_eq!(param, "objective");
            assert_eq!(expected, "a string");
        }
        other => panic!("unexpected error: {other}"),
    }
}
