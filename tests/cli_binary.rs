//! End-to-end tests of the `portal` binary itself.
//!
//! Everything else drives the library directly; these run the built executable
//! so the entry point, the exit status, and the error stream are covered by the
//! same suite. No credential is set, so only offline commands are exercised.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::process::Command;

/// Run the built binary with `args` and no credentials in the environment.
fn portal(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_portal"))
        .args(args)
        .env_remove("TINYHUMANS_API_KEY")
        .env_remove("TINYHUMANS_TOKEN")
        .env("TINYHUMANS_BASE_URL", "http://127.0.0.1:1")
        .output()
        .expect("the binary runs")
}

#[test]
fn the_catalog_command_succeeds_and_lists_categories() {
    let output = portal(&["catalog"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf-8");
    assert!(stdout.contains("Portal capability categories"));
    assert!(stdout.contains("search"));
}

#[test]
fn the_skill_command_prints_the_agent_skill() {
    let output = portal(&["skill"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf-8");
    assert!(stdout.starts_with("---\nname: portal\n"));
}

#[test]
fn describe_prints_an_example_invocation() {
    let output = portal(&["describe", "search.web"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf-8");
    assert!(stdout.contains("portal call search.web"));
}

#[test]
fn a_failing_command_reports_on_stderr_and_exits_non_zero() {
    let output = portal(&["describe", "models.chatt"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr is utf-8");
    assert!(stderr.starts_with("portal: "));
    assert!(stderr.contains("did you mean"));
}

#[test]
fn invoking_without_a_credential_says_which_variables_to_set() {
    let output = portal(&["call", "credits.balance"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr is utf-8");
    assert!(stderr.contains("TINYHUMANS_API_KEY"));
}

#[test]
fn the_version_flag_reports_the_crate_version() {
    let output = portal(&["--version"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf-8");
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}
