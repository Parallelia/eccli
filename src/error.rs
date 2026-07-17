//! Mapping gRPC `Status` codes to friendly, actionable CLI errors.

use tonic::{Code, Status};

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
}
