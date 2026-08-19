//! Discover what Portal can do, without a credential or a network call.
//!
//! ```sh
//! cargo run --example basic
//! ```

// Examples may panic on a broken invariant; library code may not.
#![allow(clippy::expect_used)]

use portal::catalog::{self, Category};

fn main() {
    println!(
        "portal {} — {} capabilities\n",
        portal::VERSION,
        catalog::len()
    );

    for category in Category::ALL {
        println!(
            "{:<11} {:>3}  {}",
            category.slug(),
            catalog::in_category(*category).count(),
            category.description()
        );
    }

    println!("\nSearching for \"transcribe audio\":");
    for capability in catalog::search("transcribe audio").iter().take(3) {
        println!("  {:<24} {}", capability.id, capability.summary);
    }

    let transcribe = catalog::find("models.transcribe").expect("the transcription capability");
    println!("\n{} ({})", transcribe.id, transcribe.route());
    for param in transcribe.params {
        println!(
            "  {:<20} {}",
            param.name,
            if param.required {
                "required"
            } else {
                "optional"
            }
        );
    }
}
