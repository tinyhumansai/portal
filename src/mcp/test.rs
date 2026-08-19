//! Unit tests for the MCP server, against a local mock backend.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{Value, json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{PROTOCOL_VERSION, Server};
use crate::client::Portal;
use crate::config::Settings;

/// A server pointed at `base_url`, carrying a credential.
fn server(base_url: &str) -> Server {
    Server::new(Portal::new(
        Settings::new(base_url).with_api_key(Some("th-test-key".to_owned())),
    ))
}

/// Send one request and parse the response line.
async fn ask(server: &Server, request: &Value) -> Value {
    let line = server
        .handle(&request.to_string())
        .await
        .expect("a request is answered");
    serde_json::from_str(&line).expect("the response is json")
}

/// The text block of a tool result.
fn text(response: &Value) -> String {
    response["result"]["content"][0]["text"]
        .as_str()
        .expect("a text block")
        .to_owned()
}

fn call(name: &str, arguments: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": name, "arguments": arguments },
    })
}

#[tokio::test]
async fn initialize_announces_the_protocol_and_the_catalog_size() {
    let response = ask(
        &server("http://localhost:1"),
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
    )
    .await;
    assert_eq!(response["result"]["protocolVersion"], PROTOCOL_VERSION);
    assert_eq!(response["result"]["serverInfo"]["name"], "portal");
    assert_eq!(response["result"]["serverInfo"]["version"], crate::VERSION);
    let instructions = response["result"]["instructions"]
        .as_str()
        .expect("instructions");
    assert!(instructions.contains("portal_search"));
}

#[tokio::test]
async fn ping_is_answered_with_an_empty_result() {
    let response = ask(
        &server("http://localhost:1"),
        &json!({ "jsonrpc": "2.0", "id": 9, "method": "ping" }),
    )
    .await;
    assert_eq!(response["result"], json!({}));
    assert_eq!(response["id"], 9);
}

#[tokio::test]
async fn tools_list_advertises_the_four_entry_points() {
    let response = ask(
        &server("http://localhost:1"),
        &json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
    )
    .await;
    let names: Vec<&str> = response["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert_eq!(
        names,
        vec![
            "portal_search",
            "portal_describe",
            "portal_invoke",
            "portal_status"
        ]
    );
    for tool in response["result"]["tools"].as_array().expect("tools") {
        assert_eq!(tool["inputSchema"]["type"], "object");
    }
}

#[tokio::test]
async fn a_notification_is_not_answered() {
    let answered = server("http://localhost:1")
        .handle(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
        .await;
    assert!(answered.is_none());
}

#[tokio::test]
async fn malformed_input_is_reported_as_a_parse_error() {
    let response: Value = serde_json::from_str(
        &server("http://localhost:1")
            .handle("{not json")
            .await
            .expect("a parse error is answered"),
    )
    .expect("the error is json");
    assert_eq!(response["error"]["code"], -32700);
}

#[tokio::test]
async fn a_message_without_a_method_is_an_invalid_request() {
    let response: Value = serde_json::from_str(
        &server("http://localhost:1")
            .handle(r#"{"jsonrpc":"2.0","id":4}"#)
            .await
            .expect("an invalid request is answered"),
    )
    .expect("the error is json");
    assert_eq!(response["error"]["code"], -32600);
    assert_eq!(response["id"], 4);
}

#[tokio::test]
async fn an_unknown_method_is_reported() {
    let response = ask(
        &server("http://localhost:1"),
        &json!({ "jsonrpc": "2.0", "id": 5, "method": "resources/list" }),
    )
    .await;
    assert_eq!(response["error"]["code"], -32601);
}

#[tokio::test]
async fn a_tool_call_without_a_name_is_invalid() {
    let response = ask(
        &server("http://localhost:1"),
        &json!({ "jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {} }),
    )
    .await;
    assert_eq!(response["error"]["code"], -32602);
}

#[tokio::test]
async fn an_unknown_tool_is_reported() {
    let response = ask(
        &server("http://localhost:1"),
        &call("portal_teleport", &json!({})),
    )
    .await;
    assert_eq!(response["error"]["code"], -32601);
}

#[tokio::test]
async fn search_returns_ranked_matches() {
    let response = ask(
        &server("http://localhost:1"),
        &call(
            "portal_search",
            &json!({ "query": "web search", "limit": 5 }),
        ),
    )
    .await;
    assert_eq!(response["result"]["isError"], false);
    let matches: Value = serde_json::from_str(&text(&response)).expect("the matches are json");
    let ids: Vec<&str> = matches["matches"]
        .as_array()
        .expect("matches")
        .iter()
        .filter_map(|entry| entry["id"].as_str())
        .collect();
    assert!(ids.len() <= 5);
    assert!(ids.contains(&"search.web"));
}

#[tokio::test]
async fn search_filters_by_category() {
    let response = ask(
        &server("http://localhost:1"),
        &call(
            "portal_search",
            &json!({ "query": "", "category": "models" }),
        ),
    )
    .await;
    let matches: Value = serde_json::from_str(&text(&response)).expect("the matches are json");
    for entry in matches["matches"].as_array().expect("matches") {
        assert_eq!(entry["category"], "models");
    }
}

#[tokio::test]
async fn search_rejects_an_unknown_category() {
    let response = ask(
        &server("http://localhost:1"),
        &call("portal_search", &json!({ "category": "telepathy" })),
    )
    .await;
    assert_eq!(response["result"]["isError"], true);
    assert!(text(&response).contains("unknown category"));
}

#[tokio::test]
async fn search_says_so_when_nothing_matches() {
    let response = ask(
        &server("http://localhost:1"),
        &call("portal_search", &json!({ "query": "kumquat telepathy" })),
    )
    .await;
    assert_eq!(response["result"]["isError"], false);
    assert!(text(&response).contains("no capability matches"));
}

#[tokio::test]
async fn describe_returns_an_invocable_schema() {
    let response = ask(
        &server("http://localhost:1"),
        &call("portal_describe", &json!({ "capability": "search.web" })),
    )
    .await;
    let described: Value = serde_json::from_str(&text(&response)).expect("the description is json");
    assert_eq!(
        described["route"],
        "POST /agent-integrations/parallel/search"
    );
    assert_eq!(described["returns"], "json");
    assert_eq!(described["read_only"], false);
    assert_eq!(described["input_schema"]["type"], "object");
}

#[tokio::test]
async fn describe_suggests_alternatives_for_an_unknown_capability() {
    let response = ask(
        &server("http://localhost:1"),
        &call("portal_describe", &json!({ "capability": "models.chatt" })),
    )
    .await;
    assert_eq!(response["result"]["isError"], true);
    assert!(text(&response).contains("unknown capability"));
}

#[tokio::test]
async fn describe_requires_a_capability() {
    let response = ask(
        &server("http://localhost:1"),
        &call("portal_describe", &json!({})),
    )
    .await;
    assert_eq!(response["result"]["isError"], true);
}

#[tokio::test]
async fn invoke_requires_a_capability() {
    let response = ask(
        &server("http://localhost:1"),
        &call("portal_invoke", &json!({})),
    )
    .await;
    assert_eq!(response["result"]["isError"], true);
    assert!(text(&response).contains("needs a capability"));
}

#[tokio::test]
async fn invoke_returns_the_backend_result() {
    let backend = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/openai/v1/models"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "data": [{ "id": "tiny-1" }] })),
        )
        .mount(&backend)
        .await;

    let response = ask(
        &server(&backend.uri()),
        &call("portal_invoke", &json!({ "capability": "models.list" })),
    )
    .await;
    assert_eq!(response["result"]["isError"], false);
    assert!(text(&response).contains("tiny-1"));
}

#[tokio::test]
async fn invoke_reports_an_argument_mistake_as_a_tool_error() {
    let response = ask(
        &server("http://localhost:1"),
        &call(
            "portal_invoke",
            &json!({ "capability": "search.web", "arguments": { "objective": "x" } }),
        ),
    )
    .await;
    assert_eq!(response["result"]["isError"], true);
    assert!(text(&response).contains("requires the argument search_queries"));
}

#[tokio::test]
async fn invoke_saves_bytes_to_a_file_when_asked() {
    let backend = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/agent-integrations/file-storage/files/f1/download"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"%PDF-1.7".to_vec()))
        .mount(&backend)
        .await;

    let destination = std::env::temp_dir().join("portal-mcp-download.pdf");
    let response = ask(
        &server(&backend.uri()),
        &call(
            "portal_invoke",
            &json!({
                "capability": "files.download",
                "arguments": { "file_id": "f1" },
                "save_to": destination.to_string_lossy(),
            }),
        ),
    )
    .await;
    assert_eq!(response["result"]["isError"], false);
    assert!(text(&response).contains("wrote 8 bytes"));
    assert_eq!(
        std::fs::read(&destination).expect("the file exists"),
        b"%PDF-1.7"
    );
    std::fs::remove_file(&destination).expect("the fixture is removable");
}

#[tokio::test]
async fn invoke_reports_a_download_that_cannot_be_written() {
    let backend = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/agent-integrations/file-storage/files/f1/download"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"x".to_vec()))
        .mount(&backend)
        .await;

    let response = ask(
        &server(&backend.uri()),
        &call(
            "portal_invoke",
            &json!({
                "capability": "files.download",
                "arguments": { "file_id": "f1" },
                "save_to": "/nonexistent-directory/portal.pdf",
            }),
        ),
    )
    .await;
    assert_eq!(response["result"]["isError"], true);
    assert!(text(&response).contains("could not write"));
}

#[tokio::test]
async fn invoke_reports_a_failed_download() {
    let backend = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/agent-integrations/file-storage/files/f1/download"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({ "error": "gone" })))
        .mount(&backend)
        .await;

    let response = ask(
        &server(&backend.uri()),
        &call(
            "portal_invoke",
            &json!({
                "capability": "files.download",
                "arguments": { "file_id": "f1" },
                "save_to": std::env::temp_dir().join("portal-missing.pdf").to_string_lossy(),
            }),
        ),
    )
    .await;
    assert_eq!(response["result"]["isError"], true);
    assert!(text(&response).contains("404"));
}

#[tokio::test]
async fn status_reports_configuration_and_reachability() {
    let backend = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "success": true, "data": "ok" })),
        )
        .mount(&backend)
        .await;

    let response = ask(&server(&backend.uri()), &call("portal_status", &json!({}))).await;
    let status: Value = serde_json::from_str(&text(&response)).expect("the status is json");
    assert_eq!(status["credential"], "api key");
    assert_eq!(status["backend"]["ok"], true);
    assert_eq!(status["capabilities"], crate::catalog::len());
    // The key itself is never echoed.
    assert!(!text(&response).contains("th-test-key"));
}

#[tokio::test]
async fn status_reports_an_unreachable_backend() {
    let backend = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({ "error": "down" })))
        .mount(&backend)
        .await;

    let response = ask(&server(&backend.uri()), &call("portal_status", &json!({}))).await;
    let status: Value = serde_json::from_str(&text(&response)).expect("the status is json");
    assert_eq!(status["backend"]["ok"], false);
}

#[tokio::test]
async fn a_blank_line_handed_straight_to_the_handler_is_a_parse_error() {
    // `serve` skips blank lines before they reach the handler; a caller that
    // does not still gets a well-formed error rather than a panic.
    let response: Value = serde_json::from_str(
        &server("http://localhost:1")
            .handle("   ")
            .await
            .expect("a blank line is answered"),
    )
    .expect("the error is json");
    assert_eq!(response["error"]["code"], -32700);
}
