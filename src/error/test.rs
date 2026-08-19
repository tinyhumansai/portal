//! Unit tests for the crate-wide error type.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::Error;

#[test]
fn missing_credentials_names_both_environment_variables() {
    let message = Error::MissingCredentials.to_string();
    assert!(message.contains("TINYHUMANS_API_KEY"));
    assert!(message.contains("TINYHUMANS_TOKEN"));
}

#[test]
fn unknown_capability_reports_the_identifier() {
    let error = Error::UnknownCapability("search.wob".to_owned());
    assert_eq!(error.to_string(), "unknown capability: search.wob");
}

#[test]
fn missing_argument_names_the_capability_and_parameter() {
    let error = Error::MissingArgument {
        capability: "search.web".to_owned(),
        param: "objective".to_owned(),
    };
    assert_eq!(
        error.to_string(),
        "capability search.web requires the argument objective"
    );
}

#[test]
fn invalid_argument_reports_the_expected_type() {
    let error = Error::InvalidArgument {
        param: "max_results".to_owned(),
        expected: "an integer".to_owned(),
    };
    assert_eq!(error.to_string(), "argument max_results must be an integer");
}

#[test]
fn unexpected_argument_reports_the_capability() {
    let error = Error::UnexpectedArgument {
        capability: "models.list".to_owned(),
        param: "quiery".to_owned(),
    };
    assert_eq!(
        error.to_string(),
        "capability models.list has no argument named quiery"
    );
}

#[test]
fn invalid_arguments_reports_the_type_that_was_supplied() {
    let error = Error::InvalidArguments { got: "array" };
    assert_eq!(
        error.to_string(),
        "arguments must be a json object, got array"
    );
}

#[test]
fn a_binary_response_points_at_the_output_path_form() {
    let error = Error::BinaryResponse {
        capability: "files.download".to_owned(),
    };
    assert_eq!(
        error.to_string(),
        "capability files.download returns bytes: invoke it with an output path"
    );
}

#[test]
fn an_unsupported_byte_request_names_the_capability() {
    let error = Error::UnsupportedByteRequest {
        capability: "models.speak".to_owned(),
    };
    let message = error.to_string();
    assert!(message.contains("models.speak"));
    assert!(message.contains("byte transport"));
}

#[test]
fn insecure_credentials_names_the_origin() {
    let error = Error::InsecureCredentials {
        base_url: "http://example.com".to_owned(),
    };
    let message = error.to_string();
    assert!(message.contains("http://example.com"));
    assert!(message.contains("https"));
}

#[test]
fn json_failures_convert_into_the_json_variant() {
    let failure = serde_json::from_str::<serde_json::Value>("{");
    let error = Error::from(failure.expect_err("truncated json does not parse"));
    assert!(matches!(error, Error::Json(_)));
    assert!(error.to_string().starts_with("json error:"));
}

#[test]
fn io_failures_convert_into_the_io_variant() {
    let error = Error::from(std::io::Error::other("stdin closed"));
    assert!(matches!(error, Error::Io(_)));
    assert!(error.to_string().starts_with("io error:"));
}

#[test]
fn backend_failures_convert_into_the_backend_variant() {
    let error = Error::from(tinyhumans_sdk::Error::Status {
        status: 402,
        body: serde_json::json!({ "error": "insufficient credits" }),
    });
    assert!(matches!(error, Error::Backend(_)));
    assert!(error.to_string().contains("402"));
}
