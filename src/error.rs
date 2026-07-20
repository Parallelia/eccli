//! Mapping gRPC `Status` codes to friendly, actionable CLI errors.

use std::fmt;

use tonic::{Code, Status};

/// Marker error for a failure whose details were *already* written to stdout in
/// the active [`OutputMode`](crate::output::OutputMode). [`crate::cli::run`]
/// exits non-zero on it without printing again, so `--json` consumers still see
/// exactly one JSON document.
#[derive(Debug)]
pub struct Reported;

impl fmt::Display for Reported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("command failed")
    }
}

impl std::error::Error for Reported {}

/// Convert a tonic [`Status`] into an [`anyhow::Error`] with an actionable hint.
pub fn friendly(status: Status) -> anyhow::Error {
    let msg = status.message().trim();
    let base = if msg.is_empty() {
        status.code().to_string()
    } else {
        msg.to_string()
    };
    let hint = match status.code() {
        Code::Unauthenticated => {
            "\n  hint: the ec requires an admin token — pass --token or set EC_ADMIN_TOKEN"
        }
        Code::Unavailable => "\n  hint: is the ec daemon reachable? check --server",
        Code::NotFound => "\n  hint: verify the election id (list-elections shows valid ids)",
        _ => "",
    };
    anyhow::anyhow!("{base}{hint}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unauthenticated_mentions_token() {
        let e = friendly(Status::unauthenticated("nope"));
        assert!(e.to_string().contains("EC_ADMIN_TOKEN"));
    }

    #[test]
    fn unavailable_mentions_server() {
        let e = friendly(Status::unavailable(""));
        assert!(e.to_string().contains("--server"));
    }

    #[test]
    fn not_found_mentions_election_id() {
        let e = friendly(Status::not_found("missing"));
        assert!(e.to_string().contains("election id"));
    }

    #[test]
    fn reported_is_recognisable_after_boxing() {
        let e: anyhow::Error = Reported.into();
        assert!(e.chain().any(|c| c.is::<Reported>()));
    }

    #[test]
    fn reported_survives_context_wrapping() {
        use anyhow::Context;
        // `downcast_ref` alone would miss this; `cli::run` scans the chain.
        let e = Err::<(), _>(Reported)
            .context("while cancelling")
            .unwrap_err();
        assert!(e.chain().any(|c| c.is::<Reported>()));
    }

    #[test]
    fn friendly_errors_are_not_reported() {
        let e = friendly(Status::unavailable("down"));
        assert!(e.downcast_ref::<Reported>().is_none());
    }
}
