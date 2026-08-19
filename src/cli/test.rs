//! Unit tests for the command line, against a local mock backend.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use clap::Parser;
use serde_json::{Value, json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{Cli, Command, collect_arguments, execute, execute_with, parse_method, resolve};
use crate::catalog;
use crate::client::Portal;
use crate::config::Settings;
use crate::error::Error;

/// Parse a command line, as the binary would.
fn parse(args: &[&str]) -> Cli {
    Cli::try_parse_from(std::iter::once("portal").chain(args.iter().copied()))
        .expect("the command line parses")
}

/// A client pointed at `base_url`, carrying a test credential.
fn portal(base_url: &str) -> Portal {
    Portal::new(Settings::new(base_url).with_api_key(Some("th-test-key".to_owned())))
}

/// Run a command against `base_url` and return everything it printed.
async fn run(base_url: &str, args: &[&str]) -> String {
    let cli = parse(args);
    let mut out = Vec::new();
    execute_with(&portal(base_url), cli.command, cli.json, &mut out)
        .await
        .expect("the command succeeds");
    String::from_utf8(out).expect("output is utf-8")
}

/// Run a command expected to fail.
async fn run_err(base_url: &str, args: &[&str]) -> Error {
    let cli = parse(args);
    let mut out = Vec::new();
    execute_with(&portal(base_url), cli.command, cli.json, &mut out)
        .await
        .expect_err("the command fails")
}

fn capability(id: &str) -> &'static catalog::Capability {
    catalog::find(id).unwrap_or_else(|| panic!("{id} is in the catalog"))
}

#[test]
fn the_command_tree_parses() {
    assert!(matches!(
        parse(&["catalog"]).command,
        Command::Catalog { .. }
    ));
    assert!(matches!(
        parse(&["search", "web", "search"]).command,
        Command::Search { .. }
    ));
    assert!(matches!(parse(&["mcp"]).command, Command::Mcp));
    assert!(matches!(parse(&["status"]).command, Command::Status));
    assert!(matches!(parse(&["skill"]).command, Command::Skill));
    let cli = parse(&["--json", "describe", "search.web"]);
    assert!(cli.json);
    // A subcommand is mandatory; there is no useful default.
    assert!(Cli::try_parse_from(["portal"]).is_err());
}

#[test]
fn verify_the_command_tree_matches_its_help() {
    // `debug_assert` inside clap catches a malformed command definition.
    <Cli as clap::CommandFactory>::command().debug_assert();
}

#[tokio::test]
async fn execute_builds_its_client_from_the_environment_and_the_base_url_flag() {
    let mut cli = parse(&["catalog"]);
    cli.base_url = Some("http://localhost:1".to_owned());
    let mut out = Vec::new();
    execute(cli, &mut out).await.expect("the command succeeds");
    assert!(
        String::from_utf8(out)
            .expect("output is utf-8")
            .contains("Portal capability categories")
    );
}

#[tokio::test]
async fn catalog_prints_a_category_overview_by_default() {
    let out = run("http://localhost:1", &["catalog"]).await;
    assert!(out.contains("Portal capability categories"));
    assert!(out.contains("models"));
}

#[tokio::test]
async fn catalog_filters_by_category_and_group() {
    let out = run("http://localhost:1", &["catalog", "--category", "models"]).await;
    assert!(out.contains("models.chat"));
    assert!(!out.contains("teams.list"));

    let out = run("http://localhost:1", &["catalog", "--group", "medulla"]).await;
    assert!(out.contains("medulla.sessions.list"));
    assert!(!out.contains("models.chat"));
}

#[tokio::test]
async fn catalog_rejects_an_unknown_category() {
    let error = run_err(
        "http://localhost:1",
        &["catalog", "--category", "telepathy"],
    )
    .await;
    match error {
        Error::InvalidArgument { param, .. } => assert_eq!(param, "category"),
        other => panic!("unexpected error: {other}"),
    }
}

#[tokio::test]
async fn catalog_as_json_lists_every_capability() {
    let out = run("http://localhost:1", &["--json", "catalog"]).await;
    let listed: Value = serde_json::from_str(&out).expect("the listing is json");
    assert_eq!(listed.as_array().expect("an array").len(), catalog::len());
}

#[tokio::test]
async fn search_honours_its_limit() {
    let out = run("http://localhost:1", &["search", "--limit", "3", "search"]).await;
    assert!(out.contains("3 capabilities"));
}

#[tokio::test]
async fn describe_prints_the_arguments_and_the_schema() {
    let out = run("http://localhost:1", &["describe", "search.web"]).await;
    assert!(out.contains("objective"));

    let out = run("http://localhost:1", &["--json", "describe", "search.web"]).await;
    let described: Value = serde_json::from_str(&out).expect("the description is json");
    assert_eq!(described["input_schema"]["type"], "object");
}

#[tokio::test]
async fn an_unknown_capability_suggests_the_closest_matches() {
    let error = run_err("http://localhost:1", &["describe", "models.chatt"]).await;
    match error {
        Error::UnknownCapability(message) => assert!(message.contains("did you mean")),
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn an_unrecognisable_capability_is_reported_without_suggestions() {
    let error = resolve("zzzzzz").expect_err("nothing matches");
    match error {
        Error::UnknownCapability(message) => assert_eq!(message, "zzzzzz"),
        other => panic!("unexpected error: {other}"),
    }
}

#[tokio::test]
async fn a_dry_run_prints_the_request_without_sending_it() {
    let out = run(
        "http://localhost:1",
        &[
            "call",
            "search.web",
            "--arg",
            "objective=rust",
            "--arg",
            r#"search_queries=["rust"]"#,
            "--dry-run",
        ],
    )
    .await;
    assert!(out.contains("POST /agent-integrations/parallel/search"));
    assert!(out.contains("\"searchQueries\""));
}

#[tokio::test]
async fn call_invokes_the_backend() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/openai/v1/models"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "data": [{ "id": "tiny-1" }] })),
        )
        .mount(&server)
        .await;

    assert!(
        run(&server.uri(), &["call", "models.list"])
            .await
            .contains("tiny-1")
    );
    // The `models` shorthand is the same call.
    assert!(run(&server.uri(), &["models"]).await.contains("tiny-1"));
}

#[tokio::test]
async fn call_writes_bytes_to_an_output_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/agent-integrations/file-storage/files/f1/download"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"%PDF-1.7".to_vec()))
        .mount(&server)
        .await;

    let destination = std::env::temp_dir().join("portal-cli-download.pdf");
    let out = run(
        &server.uri(),
        &[
            "call",
            "files.download",
            "--arg",
            "file_id=f1",
            "--out",
            &destination.to_string_lossy(),
        ],
    )
    .await;
    assert!(out.contains("wrote 8 bytes"));
    assert_eq!(
        std::fs::read(&destination).expect("the file exists"),
        b"%PDF-1.7"
    );
    std::fs::remove_file(&destination).expect("the fixture is removable");
}

#[tokio::test]
async fn the_chat_and_web_shorthands_call_their_capabilities() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/openai/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "chat_1" })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/agent-integrations/parallel/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "data": { "results": [] },
        })))
        .mount(&server)
        .await;

    assert!(
        run(&server.uri(), &["chat", "hello", "there"])
            .await
            .contains("chat_1")
    );
    assert!(
        run(&server.uri(), &["web", "tokio", "1.40"])
            .await
            .contains("results")
    );
}

#[tokio::test]
async fn raw_calls_an_arbitrary_public_route() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/feedback"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "data": { "id": "fb_1" },
        })))
        .mount(&server)
        .await;

    let out = run(
        &server.uri(),
        &["raw", "post", "/feedback", "--body", r#"{"title":"hi"}"#],
    )
    .await;
    assert!(out.contains("fb_1"));
}

#[tokio::test]
async fn raw_rejects_a_method_it_does_not_know() {
    let error = run_err("http://localhost:1", &["raw", "TRACE", "/"]).await;
    match error {
        Error::InvalidArgument { param, .. } => assert_eq!(param, "method"),
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn methods_are_parsed_case_insensitively() {
    assert_eq!(parse_method("get").expect("get"), catalog::Method::Get);
    assert_eq!(parse_method("Post").expect("post"), catalog::Method::Post);
    assert_eq!(parse_method("PUT").expect("put"), catalog::Method::Put);
    assert_eq!(
        parse_method("patch").expect("patch"),
        catalog::Method::Patch
    );
    assert_eq!(
        parse_method("DELETE").expect("delete"),
        catalog::Method::Delete
    );
}

#[tokio::test]
async fn status_reports_the_backend_and_the_credential() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "success": true, "data": "ok" })),
        )
        .mount(&server)
        .await;

    let out = run(&server.uri(), &["status"]).await;
    assert!(out.contains("reachability reachable"));

    let out = run(&server.uri(), &["--json", "status"]).await;
    let status: Value = serde_json::from_str(&out).expect("the status is json");
    assert_eq!(status["capabilities"], catalog::len());
}

#[tokio::test]
async fn status_reports_an_unreachable_backend() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    assert!(
        run(&server.uri(), &["status"])
            .await
            .contains("unreachable")
    );
}

#[tokio::test]
async fn the_skill_is_printed_with_its_frontmatter() {
    let out = run("http://localhost:1", &["skill"]).await;
    assert!(out.starts_with("---\nname: portal\n"));
    assert!(out.contains("portal_invoke"));
}

#[test]
fn arguments_take_strings_literally_and_parse_everything_else() {
    let arguments = collect_arguments(
        capability("search.web"),
        &[
            "objective=3 blind mice".to_owned(),
            r#"search_queries=["mice"]"#.to_owned(),
        ],
        None,
    )
    .expect("the arguments collect");
    assert_eq!(arguments["objective"], json!("3 blind mice"));
    assert_eq!(arguments["search_queries"], json!(["mice"]));
}

#[test]
fn an_unparseable_structured_argument_falls_back_to_a_string() {
    let arguments = collect_arguments(
        capability("apify.run"),
        &["input=not json".to_owned()],
        None,
    )
    .expect("the arguments collect");
    assert_eq!(arguments["input"], json!("not json"));
}

#[test]
fn json_arguments_merge_under_repeated_arg_flags() {
    let arguments = collect_arguments(
        capability("search.web"),
        &["objective=override".to_owned()],
        Some(r#"{"objective":"base","searchQueries":["x"]}"#),
    )
    .expect("the arguments collect");
    assert_eq!(arguments["objective"], json!("override"));
    assert_eq!(arguments["searchQueries"], json!(["x"]));
}

#[test]
fn an_argument_without_an_equals_sign_is_rejected() {
    let error = collect_arguments(capability("search.web"), &["objective".to_owned()], None)
        .expect_err("name=value is required");
    match error {
        Error::InvalidArgument { param, expected } => {
            assert_eq!(param, "objective");
            assert_eq!(expected, "written as name=value");
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn args_json_must_be_an_object() {
    let error = collect_arguments(capability("search.web"), &[], Some("[1,2]"))
        .expect_err("an array is not an argument object");
    assert!(matches!(error, Error::InvalidArguments { got: "array" }));

    let error = collect_arguments(capability("search.web"), &[], Some("{"))
        .expect_err("truncated json is rejected");
    assert!(matches!(error, Error::Json(_)));
}
