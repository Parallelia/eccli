//! Command-line interface definition and dispatch.

use anyhow::Result;
use clap::{ArgGroup, Parser, Subcommand};

use crate::client::EcClient;
use crate::commands::{candidate, election, token};
use crate::output::{self, OutputMode};

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

    /// Emit machine-readable JSON instead of human-readable output.
    #[arg(long, global = true)]
    pub json: bool,

    /// Skip confirmation prompts (required for destructive ops in scripts).
    #[arg(short = 'y', long, global = true)]
    pub yes: bool,

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

    /// Generate anonymous registration tokens for an election.
    GenerateTokens {
        #[arg(short, long)]
        election_id: String,

        /// Number of tokens to generate (the ec allows 1..=10000).
        #[arg(short, long, value_parser = clap::value_parser!(u32).range(1..=10_000))]
        count: u32,

        /// Optional path to write the raw tokens (one per line). They are shown only once.
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
    },

    /// List registration tokens for an election (ids + used status).
    ListTokens {
        #[arg(short, long)]
        election_id: String,
    },
}

/// Parse arguments and run, formatting any error per the selected output mode.
pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    let mode = OutputMode::resolve(cli.json);

    if let Err(e) = execute(cli, mode).await {
        match mode {
            OutputMode::Json => output::emit_json_error(&e.to_string()),
            OutputMode::Human { .. } => eprintln!("Error: {e:#}"),
        }
        std::process::exit(1);
    }
    Ok(())
}

/// Connect to the ec daemon and dispatch the requested command.
async fn execute(cli: Cli, mode: OutputMode) -> Result<()> {
    // Pre-flight confirmation for destructive commands, before we connect.
    if let Commands::CancelElection { election_id } = &cli.command {
        if !cli.yes {
            if mode.is_json() {
                anyhow::bail!("refusing to cancel without --yes in --json mode");
            }
            if !output::confirm(&format!("Cancel election {election_id}?"))? {
                println!("Aborted; no changes made.");
                return Ok(());
            }
        }
    }

    let mut client = EcClient::connect(&cli.server, cli.token.as_deref()).await?;

    match cli.command {
        Commands::Check => election::check(&mut client, mode).await,
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
                mode,
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
        } => candidate::add(&mut client, mode, election_id, candidate_id, name).await,
        Commands::GetElection { election_id } => {
            election::get(&mut client, mode, election_id).await
        }
        Commands::ListElections => election::list(&mut client, mode).await,
        Commands::CancelElection { election_id } => {
            election::cancel(&mut client, mode, election_id).await
        }
        Commands::GenerateTokens {
            election_id,
            count,
            output,
        } => token::generate(&mut client, mode, election_id, count, output).await,
        Commands::ListTokens { election_id } => token::list(&mut client, mode, election_id).await,
    }
}
