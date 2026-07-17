//! Candidate command: add a candidate to an existing open election.

use anyhow::{bail, Result};

use crate::candidates::MAX_CANDIDATE_ID;
use crate::client::EcClient;
use crate::proto::AddCandidateRequest;

pub async fn add(
    client: &mut EcClient,
    election_id: String,
    candidate_id: u32,
    name: String,
) -> Result<()> {
    if candidate_id > MAX_CANDIDATE_ID {
        bail!("candidate id {candidate_id} is out of range (must be 0-{MAX_CANDIDATE_ID})");
    }
    let req = client.request(AddCandidateRequest {
        election_id,
        id: candidate_id,
        name: name.clone(),
    });
    client.inner().add_candidate(req).await?;
    println!("✅ Added candidate {candidate_id} — {name}");
    Ok(())
}
