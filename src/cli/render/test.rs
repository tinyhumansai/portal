//! Unit tests for command-line rendering.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::catalog;

fn capability(id: &str) -> &'static catalog::Capability {
    catalog::find(id).unwrap_or_else(|| panic!("{id} is in the catalog"))
}

#[test]
fn a_capability_line_leads_with_its_identifier_and_category() {
    let line = super::line(capability("search.web"));
    assert!(line.starts_with("search.web"));
    assert!(line.contains("search"));
    assert!(line.contains(capability("search.web").summary));
}

#[test]
fn a_listing_counts_what_it_printed() {
    let hits = vec![capability("search.web"), capability("models.chat")];
    let listing = super::listing(&hits);
    assert!(listing.contains("search.web"));
    assert!(listing.contains("models.chat"));
    assert!(listing.ends_with("2 capabilities\n"));
    assert!(super::listing(&hits[..1]).ends_with("1 capability\n"));
}

#[test]
fn an_empty_listing_points_at_the_catalog() {
    assert!(super::listing(&[]).contains("portal catalog"));
}

#[test]
fn the_category_overview_counts_every_category() {
    let overview = super::categories();
    for category in catalog::Category::ALL {
        assert!(overview.contains(category.slug()), "{category} is missing");
        assert!(overview.contains(category.description()));
    }
    assert!(overview.contains(&format!("{} capabilities in total", catalog::len())));
}

#[test]
fn describing_a_capability_shows_its_route_arguments_and_example() {
    let described = super::describe(capability("search.web"));
    assert!(described.contains("POST /agent-integrations/parallel/search"));
    assert!(described.contains("Parallel AI"));
    assert!(described.contains("objective"));
    assert!(described.contains("required"));
    assert!(described.contains("credential required"));
    // The alias line is present because search.web is an alias for the
    // derived identifier.
    assert!(described.contains("aliases"));
    assert!(described.contains("portal call search.web --arg objective="));
}

#[test]
fn describing_an_argumentless_capability_says_so() {
    let described = super::describe(capability("credits.balance"));
    assert!(described.contains("No arguments."));
    assert!(!described.contains("Arguments\n"));
}

#[test]
fn a_public_capability_is_marked_public() {
    assert!(super::describe(capability("health.check")).contains("public"));
}

#[test]
fn a_binary_capability_advertises_the_output_path() {
    let described = super::describe(capability("files.download"));
    assert!(described.contains("bytes (use --out <path>)"));
    assert!(super::example(capability("files.download")).ends_with("--out ./downloaded"));
}

#[test]
fn an_example_names_every_required_argument() {
    let example = super::example(capability("search.web"));
    assert!(example.contains("--arg objective=<objective>"));
    assert!(example.contains("--arg search_queries=<search_queries>"));
}

#[test]
fn json_is_pretty_printed() {
    let rendered = super::json(&serde_json::json!({ "a": 1 }));
    assert_eq!(rendered, "{\n  \"a\": 1\n}");
}
