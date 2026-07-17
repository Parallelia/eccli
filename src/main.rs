//! `eccli` — friendly command-line client for the Parallelia Electoral
//! Commission (`ec`) daemon.

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    eccli::cli::run().await
}
