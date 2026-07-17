//! `eccli` — friendly command-line client for the Parallelia Electoral
//! Commission (`ec`) daemon. Talks to the `ec` gRPC Admin API.

mod client;

use anyhow::Result;
use clap::{Parser, Subcommand};

use client::EcClient;

/// Generated protobuf types for the `proto.admin` package.
pub mod proto {
    tonic::include_proto!("proto.admin");
}

#[derive(Parser)]
#[command(
    name = "eccli",
    about = "Manage elections, candidates and registration tokens on an ec daemon",
    version
)]
struct Cli {
    /// gRPC server URL of the ec daemon.
    #[arg(
        long,
        global = true,
        default_value = "http://127.0.0.1:50051",
        env = "EC_SERVER"
    )]
    server: String,

    /// Admin bearer token (only needed when the ec sets EC_ADMIN_TOKEN).
    #[arg(long, global = true, env = "EC_ADMIN_TOKEN", hide_env_values = true)]
    token: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Verify connectivity to the ec daemon (lists elections as a health check).
    Check,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check => {
            let mut client = EcClient::connect(&cli.server, cli.token.as_deref()).await?;
            let req = client.request(proto::Empty {});
            let resp = client.inner().list_elections(req).await?;
            let count = resp.into_inner().elections.len();
            println!(
                "✅ Connected to ec at {} — {} election(s) visible",
                cli.server, count
            );
        }
    }

    Ok(())
}
