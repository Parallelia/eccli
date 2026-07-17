//! `eccli` library crate — command implementations for the Parallelia
//! Electoral Commission (`ec`) gRPC Admin API. The binary in `main.rs` is a
//! thin wrapper around [`cli::run`].

pub mod candidates;
pub mod cli;
pub mod client;
pub mod commands;
pub mod error;
pub mod output;
pub mod time;

/// Generated protobuf types for the `proto.admin` package.
pub mod proto {
    tonic::include_proto!("proto.admin");
}
