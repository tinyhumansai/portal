//! Unit tests for catalog discovery.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::{Category, Envelope, Method, ParamIn, ParamKind, ResponseKind};
use serde_json::json;

#[test]
fn the_catalog_is_not_empty_and_is_sorted_by_identifier() {
    let all = super::all();
    assert!(all.len() > 100, "expected the full backend surface");
    assert_eq!(all.len(), super::len());
    let mut sorted: Vec<&str> = all.iter().map(|capability| capability.id).collect();
    let original = sorted.clone();
    sorted.sort_unstable();
    assert_eq!(original, sorted);
}

#[test]
fn every_identifier_and_alias_is_unique() {
    let mut names: Vec<&str> = Vec::new();
    for capability in super::all() {
        names.push(capability.id);
        names.extend(capability.aliases.iter().copied());
    }
    let count = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(
        names.len(),
        count,
        "identifiers and aliases must not collide"
    );
}

#[test]
fn every_path_placeholder_has_a_required_path_parameter() {
    for capability in super::all() {
        for placeholder in capability.path.split('{').skip(1) {
            let wire = placeholder.split('}').next().expect("closing brace");
            let param = capability
                .param(wire)
                .unwrap_or_else(|| panic!("{} declares no parameter for {wire}", capability.id));
            assert_eq!(param.location, ParamIn::Path);
            assert!(param.required, "{} {wire} must be required", capability.id);
        }
    }
}

#[test]
fn only_openai_compatible_routes_bypass_the_envelope() {
    for capability in super::all() {
        let raw = capability.envelope == Envelope::Raw;
        assert_eq!(
            raw,
            capability.path.starts_with("/openai/"),
            "{} has the wrong envelope",
            capability.id
        );
    }
}

#[test]
fn finds_a_capability_by_identifier_and_by_alias() {
    let web = super::find("search.web").expect("search.web");
    assert_eq!(web.path, "/agent-integrations/parallel/search");
    assert_eq!(web.method, Method::Post);
    assert_eq!(super::find("research.search"), Some(web));
    assert_eq!(super::find("  search.web  "), Some(web));
    assert_eq!(super::find("search.wob"), None);
}

#[test]
fn categories_round_trip_through_their_slugs() {
    for category in Category::ALL {
        assert_eq!(Category::parse(category.slug()), Some(*category));
        assert_eq!(category.to_string(), category.slug());
        assert!(!category.description().is_empty());
    }
    assert_eq!(Category::parse("  MODELS "), Some(Category::Models));
    assert_eq!(Category::parse("nonsense"), None);
}

#[test]
fn every_category_and_group_has_at_least_one_capability() {
    for category in Category::ALL {
        assert!(
            super::in_category(*category).next().is_some(),
            "{category} is empty"
        );
    }
    for group in super::groups() {
        assert!(super::in_group(group).next().is_some());
    }
    assert!(super::providers().contains(&"Parallel AI"));
    assert!(super::providers().windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn search_ranks_an_exact_identifier_first() {
    let hits = super::search("models.chat");
    assert_eq!(hits.first().map(|c| c.id), Some("models.chat"));
}

#[test]
fn search_requires_every_term_to_match() {
    let hits = super::search("transcribe audio");
    assert!(hits.iter().any(|c| c.id == "models.transcribe"));
    assert!(super::search("transcribe kumquat").is_empty());
}

#[test]
fn an_empty_search_returns_the_whole_catalog() {
    assert_eq!(super::search("   ").len(), super::len());
}

#[test]
fn search_matches_provider_names() {
    let hits = super::search("tenor");
    assert!(hits.iter().any(|c| c.id == "gifs.search"));
}

#[test]
fn methods_report_their_spelling_and_safety() {
    assert_eq!(Method::Get.as_str(), "GET");
    assert!(Method::Get.is_read_only());
    for method in [Method::Post, Method::Put, Method::Patch, Method::Delete] {
        assert!(!method.is_read_only());
        assert_eq!(reqwest::Method::from(method).as_str(), method.as_str());
    }
}

#[test]
fn parameter_kinds_accept_only_their_own_json_type() {
    assert!(ParamKind::String.accepts(&json!("hello")));
    assert!(!ParamKind::String.accepts(&json!(1)));
    assert!(ParamKind::Integer.accepts(&json!(3)));
    assert!(!ParamKind::Integer.accepts(&json!(3.5)));
    assert!(ParamKind::Number.accepts(&json!(3.5)));
    assert!(ParamKind::Boolean.accepts(&json!(true)));
    assert!(ParamKind::Array.accepts(&json!([1])));
    assert!(ParamKind::Object.accepts(&json!({})));
    assert!(ParamKind::File.accepts(&json!("./a.pdf")));
    assert!(ParamKind::Any.accepts(&json!(null)));
    assert_eq!(ParamKind::Any.json_type(), None);
    assert_eq!(ParamKind::File.json_type(), Some("string"));
    assert_eq!(ParamKind::Integer.describe(), "an integer");
    assert_eq!(ParamKind::Number.describe(), "a number");
    assert_eq!(ParamKind::Boolean.describe(), "a boolean");
    assert_eq!(ParamKind::Array.describe(), "an array");
    assert_eq!(ParamKind::Object.describe(), "an object");
    assert_eq!(ParamKind::Any.describe(), "any json value");
    assert_eq!(ParamKind::String.describe(), "a string");
    assert_eq!(ParamKind::File.describe(), "a path to a local file");
}

#[test]
fn a_capability_describes_its_route_and_transport() {
    let upload = super::find("files.create").expect("files.create");
    assert_eq!(
        upload.route(),
        "POST /agent-integrations/file-storage/files"
    );
    assert!(upload.is_upload());
    assert!(upload.needs_auth());
    assert!(!upload.is_binary());
    assert_eq!(upload.param("file").map(|p| p.kind), Some(ParamKind::File));

    let download = super::find("files.download").expect("files.download");
    assert!(download.is_binary());
    assert_eq!(download.response, ResponseKind::Binary);

    let health = super::find("health.check").expect("health.check");
    assert!(!health.needs_auth());
    assert!(health.required_params().next().is_none());
}

#[test]
fn the_input_schema_names_required_arguments_and_rejects_extras() {
    let web = super::find("search.web").expect("search.web");
    let schema = web.input_schema();
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["additionalProperties"], false);
    assert!(schema["properties"]["objective"]["type"] == "string");
    let required: Vec<&str> = schema["required"]
        .as_array()
        .expect("required is an array")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    assert!(required.contains(&"objective"));
    assert!(!required.contains(&"mode"));
}

#[test]
fn the_input_schema_tells_the_caller_a_file_argument_is_a_local_path() {
    let upload = super::find("files.create").expect("files.create");
    let schema = upload.input_schema();
    let description = schema["properties"]["file"]["description"]
        .as_str()
        .expect("file has a description");
    assert!(description.contains("path to a file on this machine"));
}

#[test]
fn parameters_match_either_spelling() {
    let web = super::find("search.web").expect("search.web");
    let param = web.param("search_queries").expect("search_queries");
    assert_eq!(param.wire_name, "searchQueries");
    assert!(param.matches("searchQueries"));
    assert!(param.matches("search_queries"));
    assert!(!param.matches("queries"));
}
