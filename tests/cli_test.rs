//! Integration tests driving the compiled `eccli` binary against an in-process
//! fake `ec` Admin gRPC server. Verifies wire compatibility, output formatting,
//! error mapping, and auth.

mod common;

use serde_json::Value;

fn json(out: &std::process::Output) -> Value {
    serde_json::from_slice(&out.stdout).expect("stdout is valid JSON")
}

#[tokio::test]
async fn check_reports_connectivity() {
    let url = common::start_fake().await;
    let out = common::run_eccli(&["--server", &url, "--json", "check"]).await;
    assert!(out.status.success());
    let v = json(&out);
    assert_eq!(v["ok"], true);
    assert_eq!(v["elections_visible"], 2);
}

#[tokio::test]
async fn list_elections_returns_all() {
    let url = common::start_fake().await;
    let out = common::run_eccli(&["--server", &url, "--json", "list-elections"]).await;
    assert!(out.status.success());
    let v = json(&out);
    assert_eq!(v["elections"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn get_election_shows_metadata() {
    let url = common::start_fake().await;
    let out = common::run_eccli(&["--server", &url, "--json", "get-election", "-e", "el-1"]).await;
    assert!(out.status.success());
    let v = json(&out);
    assert_eq!(v["id"], "el-1");
    assert_eq!(v["rules_id"], "plurality");
    assert_eq!(v["status"], "open");
}

#[tokio::test]
async fn get_missing_election_maps_not_found() {
    let url = common::start_fake().await;
    let out = common::run_eccli(&["--server", &url, "get-election", "-e", "missing"]).await;
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("election id"), "stderr was: {stderr}");
}

#[tokio::test]
async fn create_election_with_candidates() {
    let url = common::start_fake().await;
    let out = common::run_eccli(&[
        "--server",
        &url,
        "--json",
        "create-election",
        "-n",
        "My Vote",
        "--start-time",
        "60",
        "--duration",
        "3600",
        "--candidates-json",
        r#"[{"id":1,"name":"A"},{"id":2,"name":"B"}]"#,
    ])
    .await;
    assert!(out.status.success());
    let v = json(&out);
    assert_eq!(v["ok"], true);
    assert_eq!(v["id"], "el-test");
    let cands = v["candidates"].as_array().unwrap();
    assert_eq!(cands.len(), 2);
    assert_eq!(cands[0]["ok"], true);
}

#[tokio::test]
async fn add_candidate_succeeds() {
    let url = common::start_fake().await;
    let out = common::run_eccli(&[
        "--server",
        &url,
        "--json",
        "add-candidate",
        "-e",
        "el-1",
        "-c",
        "3",
        "-n",
        "Carol",
    ])
    .await;
    assert!(out.status.success());
    let v = json(&out);
    assert_eq!(v["candidate_id"], 3);
    assert_eq!(v["name"], "Carol");
}

#[tokio::test]
async fn cancel_election_with_yes() {
    let url = common::start_fake().await;
    let out = common::run_eccli(&[
        "--server",
        &url,
        "--json",
        "--yes",
        "cancel-election",
        "-e",
        "el-1",
    ])
    .await;
    assert!(out.status.success());
    let v = json(&out);
    assert_eq!(v["ok"], true);
}

#[tokio::test]
async fn cancel_without_yes_refuses_in_json() {
    let url = common::start_fake().await;
    let out =
        common::run_eccli(&["--server", &url, "--json", "cancel-election", "-e", "el-1"]).await;
    assert!(!out.status.success());
    let v = json(&out);
    assert_eq!(v["ok"], false);
}

#[tokio::test]
async fn generate_tokens_prints_count() {
    let url = common::start_fake().await;
    let out = common::run_eccli(&[
        "--server",
        &url,
        "--json",
        "generate-tokens",
        "-e",
        "el-1",
        "-c",
        "3",
    ])
    .await;
    assert!(out.status.success());
    let v = json(&out);
    assert_eq!(v["count"], 3);
    assert_eq!(v["tokens"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn generate_tokens_writes_output_file() {
    let url = common::start_fake().await;
    let path = std::env::temp_dir().join(format!("eccli-toks-{}.txt", std::process::id()));
    let path_str = path.to_str().unwrap();
    let out = common::run_eccli(&[
        "--server",
        &url,
        "generate-tokens",
        "-e",
        "el-1",
        "-c",
        "4",
        "-o",
        path_str,
    ])
    .await;
    assert!(out.status.success());
    let contents = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0], "tok-0");
    std::fs::remove_file(&path).ok();
}

#[tokio::test]
async fn list_tokens_summarizes_usage() {
    let url = common::start_fake().await;
    let out = common::run_eccli(&["--server", &url, "--json", "list-tokens", "-e", "el-1"]).await;
    assert!(out.status.success());
    let v = json(&out);
    assert_eq!(v["used"], 1);
    assert_eq!(v["total"], 2);
}

#[tokio::test]
async fn auth_required_without_token_fails() {
    let url = common::start_fake_with_auth("Bearer testtoken").await;
    let out = common::run_eccli(&["--server", &url, "check"]).await;
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("EC_ADMIN_TOKEN"), "stderr was: {stderr}");
}

#[tokio::test]
async fn auth_required_with_token_succeeds() {
    let url = common::start_fake_with_auth("Bearer testtoken").await;
    let out =
        common::run_eccli(&["--server", &url, "--token", "testtoken", "--json", "check"]).await;
    assert!(out.status.success());
    let v = json(&out);
    assert_eq!(v["ok"], true);
}

// --- Human-mode (non-JSON) output paths ---

#[tokio::test]
async fn human_list_elections_renders() {
    let url = common::start_fake().await;
    let out = common::run_eccli(&["--server", &url, "list-elections"]).await;
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Elections (2 total)"), "was: {stdout}");
    assert!(stdout.contains("el-1"));
}

#[tokio::test]
async fn human_get_election_renders_details() {
    let url = common::start_fake().await;
    let out = common::run_eccli(&["--server", &url, "get-election", "-e", "el-9"]).await;
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Election details"));
    assert!(stdout.contains("rules:"));
}

#[tokio::test]
async fn human_generate_tokens_prints_tokens_and_warning() {
    let url = common::start_fake().await;
    let out =
        common::run_eccli(&["--server", &url, "generate-tokens", "-e", "el-1", "-c", "2"]).await;
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("tok-0"));
    assert!(stdout.contains("shown only once"));
}

#[tokio::test]
async fn human_create_election_reports_candidates() {
    let url = common::start_fake().await;
    let out = common::run_eccli(&[
        "--server",
        &url,
        "create-election",
        "-n",
        "Vote",
        "--start-time",
        "60",
        "--end-time",
        "2000000000",
        "--candidates-json",
        r#"[{"id":1,"name":"A"}]"#,
    ])
    .await;
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Created election"));
    assert!(stdout.contains("1 — A"));
}

#[tokio::test]
async fn human_list_tokens_renders() {
    let url = common::start_fake().await;
    let out = common::run_eccli(&["--server", &url, "list-tokens", "-e", "el-1"]).await;
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("1/2 used"));
    assert!(stdout.contains("[used]"));
}
