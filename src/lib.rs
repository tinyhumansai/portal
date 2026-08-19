//! One agent-friendly door to every provider, search engine, and model the
//! `TinyHumans` backend offers.
//!
//! Portal wraps the vendored [`TinyHumans` SDK](tinyhumans_sdk) in a *capability
//! catalog*: 200-odd backend operations, each with a stable identifier, a
//! category, typed arguments, and documentation taken from the deployed
//! contract. An agent that has never seen this API can search the catalog in
//! plain words, read a capability's arguments, and invoke it — without knowing
//! a single route.
//!
//! The same catalog powers four surfaces, so they cannot drift apart:
//!
//! | Surface | Entry point |
//! | --- | --- |
//! | Rust library | [`Portal`] |
//! | Command line | `portal …`, built from [`cli`] |
//! | MCP server | `portal mcp`, built from [`mcp`] |
//! | Agent Skill | `skills/portal/SKILL.md` |
//!
//! # Configuration
//!
//! Set `TINYHUMANS_API_KEY` or `TINYHUMANS_TOKEN`, and optionally
//! `TINYHUMANS_BASE_URL`. See [`Settings`].
//!
//! # Example
//!
//! ```
//! use portal::catalog;
//!
//! // Discovery needs no credential and no network.
//! let hits = catalog::search("web search");
//! assert!(hits.iter().any(|capability| capability.id == "search.web"));
//!
//! let web = catalog::find("search.web").expect("the web search capability");
//! assert_eq!(web.provider, "Parallel AI");
//! assert!(web.param("objective").is_some_and(|param| param.required));
//! ```
//!
//! Invoking one needs a credential:
//!
//! ```no_run
//! use portal::Portal;
//! use serde_json::json;
//!
//! # async fn run() -> Result<(), portal::Error> {
//! let models = Portal::from_env().invoke("models.list", &json!({})).await?;
//! println!("{models:#}");
//! # Ok(())
//! # }
//! ```

pub mod catalog;
pub mod cli;
pub mod invoke;
pub mod mcp;

mod client;
mod config;
mod error;

pub use client::Portal;
pub use config::{API_KEY_VAR, BASE_URL_VAR, DEFAULT_BASE_URL, Settings, TOKEN_VAR};
pub use error::{Error, Result};

/// The version of this crate, as reported by the CLI and the MCP server.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
