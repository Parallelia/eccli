//! Candidate command: add a candidate to an existing open election.

use anyhow::{bail, Result};
use serde_json::json;

use crate::candidates::MAX_CANDIDATE_ID;
use crate::client::EcClient;
use crate::error::friendly;
use crate::output::{self, OutputMode};
use crate::proto::AddCandidateRequest;

pub async fn add(
    client: &mut EcClient,
    mode: OutputMode,
    election_id: String,
    candidate_id: u32,
    name: String,
) -> Result<()> {
    if candidate_id > MAX_CANDIDATE_ID {
        bail!("candidate id {candidate_id} is out of range (must be 0-{MAX_CANDIDATE_ID})");
    }
    let req = client.request(AddCandidateRequest {
        election_id: election_id.clone(),
        id: candidate_id,
        name: name.clone(),
    });
    client.inner().add_candidate(req).await.map_err(friendly)?;

    match mode {
        OutputMode::Json => output::emit_json(json!({
            "ok": true,
            "election_id": election_id,
            "candidate_id": candidate_id,
            "name": name,
        })),
        OutputMode::Human { color } => {
            output::success(color, &format!("Added candidate {candidate_id} — {name}"))
        }
    }
    Ok(())
}
