//! End-to-end checks of the `--json` output contract documented in the README:
//! exactly one JSON document on stdout on every path, including argument
//! parsing failures, which happen before any command runs.

use std::process::{Command, Output};

/// Run the built binary with `args` and a clean environment.
///
/// `EC_SERVER` / `EC_ADMIN_TOKEN` are cleared so a developer's shell cannot
/// change what the CLI parses.
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_eccli"))
        .args(args)
        .env_remove("EC_SERVER")
        .env_remove("EC_ADMIN_TOKEN")
        .output()
        .expect("failed to run eccli")
}

#[test]
fn malformed_json_invocation_emits_one_json_error_and_exits_1() {
    // `cancel-election` requires `--election-id`, so clap rejects this before
    // the command ever runs.
    let out = run(&["--json", "cancel-election"]);

    assert_eq!(out.status.code(), Some(1));

    let stdout = String::from_utf8(out.stdout).unwrap();
    let value: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {stdout}"));

    assert_eq!(value["ok"], serde_json::json!(false));
    let error = value["error"].as_str().expect("error must be a string");
    assert!(
        error.contains("--election-id"),
        "error should name the missing argument, got: {error}"
    );
    assert!(
        !error.contains('\u{1b}'),
        "ANSI escapes must not leak into the JSON payload"
    );
}

#[test]
fn unknown_flag_in_json_mode_is_also_a_json_error() {
    let out = run(&["--json", "list-elections", "--nope"]);

    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8(out.stdout).unwrap();
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is not JSON");
    assert_eq!(value["ok"], serde_json::json!(false));
}

#[test]
fn human_mode_keeps_claps_usage_error_on_stderr() {
    let out = run(&["cancel-election"]);

    // Unchanged clap behaviour: usage errors exit 2 and print to stderr.
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty(), "usage errors must not touch stdout");
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("--election-id"), "got: {stderr}");
}

#[test]
fn help_and_version_still_succeed_in_json_mode() {
    for args in [["--json", "--help"], ["--json", "--version"]] {
        let out = run(&args);
        assert_eq!(out.status.code(), Some(0), "for {args:?}");
        assert!(!out.stdout.is_empty(), "for {args:?}");
    }
}
