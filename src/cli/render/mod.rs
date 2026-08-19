//! Human-readable rendering for the command line.
//!
//! Every function here is pure: it takes catalog data and returns a string.
//! That keeps the terminal output under test without a process, and keeps the
//! command implementations in `mod.rs` down to argument handling and a call.

// Every `write!` here targets a `String`, whose `fmt::Write` implementation
// cannot fail; the results are discarded deliberately rather than unwrapped,
// which library code must not do.
use std::fmt::Write as _;

use serde_json::Value;

use crate::catalog::{self, Capability, Category};

/// A one-line summary of a capability, aligned for a listing.
#[must_use]
pub fn line(capability: &Capability) -> String {
    format!(
        "{:<34} {:<10} {}",
        capability.id,
        capability.category.slug(),
        capability.summary
    )
}

/// A listing of capabilities, or a note when there are none.
#[must_use]
pub fn listing(capabilities: &[&Capability]) -> String {
    if capabilities.is_empty() {
        return "No capability matches. Run `portal catalog` to see every category.".to_owned();
    }
    let mut out = String::new();
    for capability in capabilities {
        out.push_str(&line(capability));
        out.push('\n');
    }
    let count = capabilities.len();
    let _ = write!(
        out,
        "\n{count} {}\n",
        if count == 1 {
            "capability"
        } else {
            "capabilities"
        }
    );
    out
}

/// The category overview shown by `portal catalog` with no filter.
#[must_use]
pub fn categories() -> String {
    let mut out = String::from("Portal capability categories\n\n");
    for category in Category::ALL {
        let count = catalog::in_category(*category).count();
        let _ = writeln!(
            out,
            "  {:<11} {:>3}  {}",
            category.slug(),
            count,
            category.description()
        );
    }
    let _ = write!(
        out,
        "\n{} capabilities in total. Filter with `portal catalog --category <name>`,\n\
         search with `portal search <words>`, then read one with `portal describe <id>`.\n",
        catalog::len()
    );
    out
}

/// The full, human-readable description of one capability.
#[must_use]
pub fn describe(capability: &Capability) -> String {
    let mut out = format!(
        "{}\n\n  {}\n\n  provider   {}\n  category   {}\n  route      {}\n  returns    {}\n  auth       {}\n",
        capability.id,
        capability.summary,
        capability.provider,
        capability.category.slug(),
        capability.route(),
        if capability.is_binary() {
            "bytes (use --out <path>)"
        } else {
            "json"
        },
        if capability.needs_auth() {
            "credential required"
        } else {
            "public"
        },
    );
    if !capability.aliases.is_empty() {
        let _ = writeln!(out, "  aliases    {}", capability.aliases.join(", "));
    }
    if capability.params.is_empty() {
        out.push_str("\n  No arguments.\n");
        return out;
    }
    out.push_str("\nArguments\n");
    for param in capability.params {
        let _ = writeln!(
            out,
            "  {:<24} {:<8} {:<9} {}",
            param.name,
            format!("{:?}", param.location).to_lowercase(),
            if param.required {
                "required"
            } else {
                "optional"
            },
            summarize(param.description, param.kind.describe()),
        );
    }
    let _ = write!(out, "\nExample\n  {}\n", example(capability));
    out
}

/// A parameter's documentation, falling back to its type when it has none.
fn summarize(description: &str, kind: &str) -> String {
    if description.trim().is_empty() {
        kind.to_owned()
    } else {
        format!("{kind}. {}", description.trim())
    }
}

/// An invocation an agent can copy, filled with placeholders.
#[must_use]
pub fn example(capability: &Capability) -> String {
    let mut command = format!("portal call {}", capability.id);
    for param in capability.required_params() {
        let _ = write!(command, " --arg {}=<{}>", param.name, param.name);
    }
    if capability.is_binary() {
        command.push_str(" --out ./downloaded");
    }
    command
}

/// A JSON value as the CLI prints it.
#[must_use]
pub fn json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

#[cfg(test)]
mod test;
