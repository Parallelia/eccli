//! Election lifecycle commands: check, create, get, list, cancel.

use anyhow::Result;
use chrono::DateTime;

use crate::candidates::{self, CandidateSpec};
use crate::client::EcClient;
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
pub async fn check(client: &mut EcClient) -> Result<()> {
    let req = client.request(Empty {});
    let count = client
        .inner()
        .list_elections(req)
        .await?
        .into_inner()
        .elections
        .len();
    println!("✅ Connected — {count} election(s) visible");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn create(
    client: &mut EcClient,
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
    let election = client.inner().add_election(req).await?.into_inner();

    println!(
        "✅ Created election \"{}\" (id: {})",
        election.name, election.id
    );
    println!("   rules:  {}", election.rules_id);
    println!(
        "   window: {} → {}",
        fmt_ts(election.start_time),
        fmt_ts(election.end_time)
    );

    if let Some(specs) = specs {
        println!("   adding {} candidate(s):", specs.len());
        for c in specs {
            let req = client.request(AddCandidateRequest {
                election_id: election.id.clone(),
                id: c.id,
                name: c.name.clone(),
            });
            match client.inner().add_candidate(req).await {
                Ok(_) => println!("     ✅ {} — {}", c.id, c.name),
                Err(status) => {
                    println!("     ❌ {} — {}: {}", c.id, c.name, status.message())
                }
            }
        }
    }
    Ok(())
}

pub async fn get(client: &mut EcClient, election_id: String) -> Result<()> {
    let req = client.request(ElectionIdRequest { election_id });
    let election = client.inner().get_election(req).await?.into_inner();
    println!("📊 Election details:");
    print_election(&election);
    Ok(())
}

pub async fn list(client: &mut EcClient) -> Result<()> {
    let req = client.request(Empty {});
    let elections = client
        .inner()
        .list_elections(req)
        .await?
        .into_inner()
        .elections;
    println!("🗳️  Elections ({} total):", elections.len());
    for e in &elections {
        print_election(e);
    }
    Ok(())
}

pub async fn cancel(client: &mut EcClient, election_id: String) -> Result<()> {
    let req = client.request(ElectionIdRequest { election_id });
    let status = client.inner().cancel_election(req).await?.into_inner();
    if status.success {
        println!("✅ {}", status.message);
    } else {
        println!("❌ {}", status.message);
    }
    Ok(())
}
