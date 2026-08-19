//! Discovery over the compiled-in capability table.
//!
//! The catalog is what makes Portal usable by an agent that does not already
//! know the backend: it can list the [`Category`] it wants, search in plain
//! words, and read a capability's arguments — all without a network round trip
//! and without any knowledge of the underlying routes.
//!
//! The table itself is generated from the deployed contract by
//! `scripts/sync-catalog.mjs`; nothing in this module is hand-maintained.
//!
//! # Example
//!
//! ```
//! use portal::catalog;
//!
//! let web = catalog::find("search.web").expect("the web search capability");
//! assert_eq!(web.category.slug(), "search");
//! assert!(web.param("objective").is_some());
//!
//! // Aliases resolve to the same capability as the derived identifier.
//! assert_eq!(catalog::find("research.search"), Some(web));
//!
//! // Plain-language lookup, ranked best first.
//! let hits = catalog::search("transcribe audio");
//! assert!(hits.iter().any(|c| c.id == "models.transcribe"));
//! ```

mod generated;
mod types;

pub use types::{
    Auth, BodyKind, Capability, Category, Envelope, Method, Param, ParamIn, ParamKind, ResponseKind,
};

use generated::CAPABILITIES;

/// Every capability Portal can invoke, sorted by identifier.
#[must_use]
pub fn all() -> &'static [Capability] {
    CAPABILITIES
}

/// The number of capabilities in the catalog.
#[must_use]
pub fn len() -> usize {
    CAPABILITIES.len()
}

/// Look up a capability by identifier or alias.
#[must_use]
pub fn find(id: &str) -> Option<&'static Capability> {
    let id = id.trim();
    CAPABILITIES
        .iter()
        .find(|capability| capability.answers_to(id))
}

/// Every capability in one category.
pub fn in_category(category: Category) -> impl Iterator<Item = &'static Capability> {
    CAPABILITIES
        .iter()
        .filter(move |capability| capability.category == category)
}

/// Every capability in one group, such as `medulla` or `files`.
pub fn in_group(group: &str) -> impl Iterator<Item = &'static Capability> + '_ {
    CAPABILITIES
        .iter()
        .filter(move |capability| capability.group == group)
}

/// Every distinct provider named in the catalog, sorted.
#[must_use]
pub fn providers() -> Vec<&'static str> {
    let mut providers: Vec<&'static str> = CAPABILITIES
        .iter()
        .map(|capability| capability.provider)
        .collect();
    providers.sort_unstable();
    providers.dedup();
    providers
}

/// Every distinct group in the catalog, sorted.
#[must_use]
pub fn groups() -> Vec<&'static str> {
    let mut groups: Vec<&'static str> = CAPABILITIES
        .iter()
        .map(|capability| capability.group)
        .collect();
    groups.sort_unstable();
    groups.dedup();
    groups
}

/// Rank the catalog against a plain-language query, best match first.
///
/// Every whitespace-separated term must appear somewhere in the capability for
/// it to match at all, which keeps a two-word query from returning half the
/// catalog. Matches on the identifier outrank matches on the summary, and a
/// match on a parameter name counts for least — it is the weakest evidence
/// that the caller meant this capability.
///
/// An empty query returns the whole catalog in identifier order.
#[must_use]
pub fn search(query: &str) -> Vec<&'static Capability> {
    let terms: Vec<String> = query
        .split_whitespace()
        .map(str::to_ascii_lowercase)
        .collect();
    if terms.is_empty() {
        return CAPABILITIES.iter().collect();
    }
    let mut scored: Vec<(u32, &'static Capability)> = CAPABILITIES
        .iter()
        .filter_map(|capability| score(capability, &terms).map(|score| (score, capability)))
        .collect();
    // Descending by score, then by identifier so equal matches stay stable.
    scored.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.id.cmp(right.1.id)));
    scored
        .into_iter()
        .map(|(_, capability)| capability)
        .collect()
}

/// Total match strength, or `None` when a term is missing entirely.
fn score(capability: &'static Capability, terms: &[String]) -> Option<u32> {
    let id = capability.id.to_ascii_lowercase();
    let summary = capability.summary.to_ascii_lowercase();
    let provider = capability.provider.to_ascii_lowercase();
    let path = capability.path.to_ascii_lowercase();
    let mut total = 0;
    for term in terms {
        let mut best = 0;
        if id == *term {
            best = 100;
        } else if id.split('.').any(|segment| segment == term) {
            best = best.max(40);
        } else if id.contains(term.as_str()) {
            best = best.max(25);
        }
        if summary.contains(term.as_str()) {
            best = best.max(15);
        }
        if provider.contains(term.as_str()) || capability.group.contains(term.as_str()) {
            best = best.max(10);
        }
        if path.contains(term.as_str()) {
            best = best.max(6);
        }
        if capability
            .params
            .iter()
            .any(|param| param.name.contains(term.as_str()))
        {
            best = best.max(3);
        }
        if best == 0 {
            return None;
        }
        total += best;
    }
    Some(total)
}

#[cfg(test)]
mod test;
