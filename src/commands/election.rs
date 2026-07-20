//! Election lifecycle commands: check, create, get, list, cancel.

use anyhow::Result;
use chrono::DateTime;
use serde_json::json;

use crate::candidates::{self, CandidateSpec};
use crate::client::EcClient;
use crate::error::{friendly, Reported};
use crate::output::{self, OutputMode};
use crate::proto::{
    AddCandidateRequest, AddElectionRequest, ElectionIdRequest, ElectionResponse, Empty,
};
use crate::time;

/// Format a unix timestamp as a human-readable UTC string.
fn fmt_ts(ts: i64) -> String {
    DateTime::from_timestamp(ts, 0)
        .map(|d| d.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| ts.to_string())
}

/// JSON representation of an election.
fn election_json(e: &ElectionResponse) -> serde_json::Value {
    json!({
        "id": e.id,
        "name": e.name,
        "status": e.status,
        "rules_id": e.rules_id,
        "start_time": e.start_time,
        "end_time": e.end_time,
        "created_at": e.created_at,
        "rsa_pub_key": e.rsa_pub_key,
    })
}

/// Print a one-line summary followed by indented election details.
fn print_election(e: &ElectionResponse) {
    println!("   • {} ({})", e.name, e.status);
    println!("     id:      {}", e.id);
    println!("     rules:   {}", e.rules_id);
    println!("     start:   {}", fmt_ts(e.start_time));
    println!("     end:     {}", fmt_ts(e.end_time));
    println!("     created: {}", fmt_ts(e.created_at));
}

/// Health check: connect and list elections to prove reachability.
pub async fn check(client: &mut EcClient, mode: OutputMode) -> Result<()> {
    let req = client.request(Empty {});
    let count = client
        .inner()
        .list_elections(req)
        .await
        .map_err(friendly)?
        .into_inner()
        .elections
        .len();
    match mode {
        OutputMode::Json => output::emit_json(json!({ "ok": true, "elections_visible": count })),
        OutputMode::Human { color } => {
            output::success(color, &format!("Connected — {count} election(s) visible"))
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn create(
    client: &mut EcClient,
    mode: OutputMode,
    name: String,
    start_time: i64,
    duration: Option<i64>,
    end_time: Option<i64>,
    rules_id: String,
    candidates_file: Option<String>,
    candidates_json: Option<String>,
) -> Result<()> {
    let now = chrono::Utc::now().timestamp();
    let (start_ts, end_ts) = time::compute_window(start_time, duration, end_time, now)?;
    let specs: Option<Vec<CandidateSpec>> =
        candidates::parse_candidates(candidates_file.as_deref(), candidates_json.as_deref())?;

    let req = client.request(AddElectionRequest {
        name,
        start_time: start_ts,
        end_time: end_ts,
        rules_id,
    });
    let election = client
        .inner()
        .add_election(req)
        .await
        .map_err(friendly)?
        .into_inner();

    // Add candidates (if any), collecting per-candidate outcomes.
    let mut results: Vec<(u32, String, bool, Option<String>)> = Vec::new();
    if let Some(specs) = specs {
        for c in specs {
            let req = client.request(AddCandidateRequest {
                election_id: election.id.clone(),
                id: c.id,
                name: c.name.clone(),
            });
            match client.inner().add_candidate(req).await {
                Ok(_) => results.push((c.id, c.name, true, None)),
                // Route through `friendly` like every other RPC so a failing
                // candidate still gets the actionable auth/connectivity hint.
                Err(status) => {
                    results.push((c.id, c.name, false, Some(friendly(status).to_string())))
                }
            }
        }
    }
    // The election itself exists at this point; report every candidate outcome
    // and only then fail, so a partial success is never silently swallowed.
    let failures = results.iter().filter(|(_, _, ok, _)| !ok).count();

    match mode {
        OutputMode::Json => {
            let candidates: Vec<serde_json::Value> = results
                .iter()
                .map(
                    |(id, name, ok, err)| json!({ "id": id, "name": name, "ok": ok, "error": err }),
                )
                .collect();
            let mut obj = election_json(&election);
            obj["ok"] = json!(failures == 0);
            obj["candidates"] = json!(candidates);
            // Keep the key set stable: `null` rather than absent on success.
            obj["error"] = match failures {
                0 => serde_json::Value::Null,
                n => json!(format!("failed to add {n} candidate(s)")),
            };
            output::emit_json(obj);
        }
        OutputMode::Human { color } => {
            output::success(
                color,
                &format!(
                    "Created election \"{}\" (id: {})",
                    election.name, election.id
                ),
            );
            println!("   rules:  {}", election.rules_id);
            println!(
                "   window: {} → {}",
                fmt_ts(election.start_time),
                fmt_ts(election.end_time)
            );
            if !results.is_empty() {
                println!("   adding {} candidate(s):", results.len());
                for (id, name, ok, err) in &results {
                    if *ok {
                        println!("     {} {id} — {name}", output::green("✅", color));
                    } else {
                        println!(
                            "     {} {id} — {name}: {}",
                            output::red("❌", color),
                            err.as_deref().unwrap_or("failed")
                        );
                    }
                }
                if failures > 0 {
                    output::failure(color, &format!("failed to add {failures} candidate(s)"));
                }
            }
        }
    }

    if failures > 0 {
        return Err(Reported.into());
    }
    Ok(())
}

pub async fn get(client: &mut EcClient, mode: OutputMode, election_id: String) -> Result<()> {
    let req = client.request(ElectionIdRequest { election_id });
    let election = client
        .inner()
        .get_election(req)
        .await
        .map_err(friendly)?
        .into_inner();
    match mode {
        OutputMode::Json => {
            let mut obj = election_json(&election);
            obj["ok"] = json!(true);
            output::emit_json(obj);
        }
        OutputMode::Human { .. } => {
            println!("📊 Election details:");
            print_election(&election);
        }
    }
    Ok(())
}

pub async fn list(client: &mut EcClient, mode: OutputMode) -> Result<()> {
    let req = client.request(Empty {});
    let elections = client
        .inner()
        .list_elections(req)
        .await
        .map_err(friendly)?
        .into_inner()
        .elections;
    match mode {
        OutputMode::Json => {
            let arr: Vec<serde_json::Value> = elections.iter().map(election_json).collect();
            output::emit_json(json!({ "ok": true, "elections": arr }));
        }
        OutputMode::Human { .. } => {
            println!("🗳️  Elections ({} total):", elections.len());
            for e in &elections {
                print_election(e);
            }
        }
    }
    Ok(())
}

pub async fn cancel(client: &mut EcClient, mode: OutputMode, election_id: String) -> Result<()> {
    let req = client.request(ElectionIdRequest {
        election_id: election_id.clone(),
    });
    let status = client
        .inner()
        .cancel_election(req)
        .await
        .map_err(friendly)?
        .into_inner();

    match mode {
        // A refusal returns `Reported`, which suppresses the generic
        // `emit_json_error` path — so this document must carry `error` itself
        // or JSON consumers get a failure with no `.error` to read.
        OutputMode::Json => output::emit_json(json!({
            "ok": status.success,
            "election_id": election_id,
            "message": status.message,
            "error": if status.success { serde_json::Value::Null } else { json!(status.message) },
        })),
        OutputMode::Human { color } => {
            if status.success {
                output::success(color, &status.message);
            } else {
                output::failure(color, &status.message);
            }
        }
    }

    // A refused cancellation must not exit 0 — the election is still live.
    if !status.success {
        return Err(Reported.into());
    }
    Ok(())
}
