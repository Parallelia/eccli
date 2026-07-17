//! Command-line interface definition and dispatch.

use anyhow::Result;
use clap::{ArgGroup, Parser, Subcommand};

use crate::client::EcClient;
use crate::commands::{candidate, election};

#[derive(Parser)]
#[command(
    name = "eccli",
    about = "Manage elections, candidates and registration tokens on an ec daemon",
    version
)]
pub struct Cli {
    /// gRPC server URL of the ec daemon.
    #[arg(
        long,
        global = true,
        default_value = "http://127.0.0.1:50051",
        env = "EC_SERVER"
    )]
    pub server: String,

    /// Admin bearer token (only needed when the ec sets EC_ADMIN_TOKEN).
    #[arg(long, global = true, env = "EC_ADMIN_TOKEN", hide_env_values = true)]
    pub token: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Verify connectivity to the ec daemon.
    Check,

    /// Create a new election.
    #[command(group(
        ArgGroup::new("window").required(true).args(["duration", "end_time"])
    ))]
    CreateElection {
        /// Human-readable election name.
        #[arg(short, long)]
        name: String,

        /// Start time: relative seconds from now (< 1_000_000_000) or absolute unix ts.
        #[arg(long)]
        start_time: i64,

        /// Duration in seconds (mutually exclusive with --end-time).
        #[arg(long)]
        duration: Option<i64>,

        /// End time: relative seconds from now or absolute unix ts (mutually exclusive with --duration).
        #[arg(long)]
        end_time: Option<i64>,

        /// Counting rules id (the ec ships `plurality` and `stv`).
        #[arg(long, default_value = "plurality")]
        rules_id: String,

        /// Path to a JSON file with candidates `[{"id":1,"name":"A"}]`.
        #[arg(long)]
        candidates_file: Option<String>,

        /// Candidates as an inline JSON string.
        #[arg(long)]
        candidates_json: Option<String>,
    },

    /// Add a candidate to an existing open election.
    AddCandidate {
        #[arg(short, long)]
        election_id: String,

        #[arg(short, long)]
        candidate_id: u32,

        #[arg(short, long)]
        name: String,
    },

    /// Show details of a single election.
    GetElection {
        #[arg(short, long)]
        election_id: String,
    },

    /// List all elections.
    ListElections,

    /// Cancel an election.
    CancelElection {
        #[arg(short, long)]
        election_id: String,
    },
}

/// Parse arguments, connect to the ec daemon, and dispatch the command.
pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    let mut client = EcClient::connect(&cli.server, cli.token.as_deref()).await?;

    match cli.command {
        Commands::Check => election::check(&mut client).await,
        Commands::CreateElection {
            name,
            start_time,
            duration,
            end_time,
            rules_id,
            candidates_file,
            candidates_json,
        } => {
            election::create(
                &mut client,
                name,
                start_time,
                duration,
                end_time,
                rules_id,
                candidates_file,
                candidates_json,
            )
            .await
        }
        Commands::AddCandidate {
            election_id,
            candidate_id,
            name,
        } => candidate::add(&mut client, election_id, candidate_id, name).await,
        Commands::GetElection { election_id } => election::get(&mut client, election_id).await,
        Commands::ListElections => election::list(&mut client).await,
        Commands::CancelElection { election_id } => {
            election::cancel(&mut client, election_id).await
        }
    }
}
