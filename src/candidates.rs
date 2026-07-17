//! Parsing of candidate specifications supplied to `create-election`.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::fs;

/// Maximum candidate id accepted by the `ec` daemon (stored as a u8).
pub const MAX_CANDIDATE_ID: u32 = 255;

/// A candidate entry as provided in JSON: `{"id": 1, "name": "Alice"}`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CandidateSpec {
    pub id: u32,
    pub name: String,
}

/// Parse candidates from an optional file path or inline JSON string.
///
/// Returns `Ok(None)` when neither source is provided (candidates are optional
/// at creation time and can be added later with `add-candidate`).
pub fn parse_candidates(
    file: Option<&str>,
    json: Option<&str>,
) -> Result<Option<Vec<CandidateSpec>>> {
    let raw = match (file, json) {
        (Some(_), Some(_)) => {
            bail!("--candidates-file and --candidates-json are mutually exclusive")
        }
        (Some(path), None) => {
            fs::read_to_string(path).with_context(|| format!("reading candidates file '{path}'"))?
        }
        (None, Some(s)) => s.to_string(),
        (None, None) => return Ok(None),
    };

    let specs: Vec<CandidateSpec> =
        serde_json::from_str(&raw).context("parsing candidates JSON")?;

    for c in &specs {
        if c.id > MAX_CANDIDATE_ID {
            bail!(
                "candidate id {} is out of range (must be 0-{MAX_CANDIDATE_ID})",
                c.id
            );
        }
    }
    Ok(Some(specs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_when_no_source() {
        assert_eq!(parse_candidates(None, None).unwrap(), None);
    }

    #[test]
    fn parses_inline_json() {
        let got = parse_candidates(None, Some(r#"[{"id":1,"name":"A"},{"id":2,"name":"B"}]"#))
            .unwrap()
            .unwrap();
        assert_eq!(
            got,
            vec![
                CandidateSpec {
                    id: 1,
                    name: "A".into()
                },
                CandidateSpec {
                    id: 2,
                    name: "B".into()
                },
            ]
        );
    }

    #[test]
    fn both_sources_is_error() {
        assert!(parse_candidates(Some("f.json"), Some("[]")).is_err());
    }

    #[test]
    fn out_of_range_id_is_error() {
        assert!(parse_candidates(None, Some(r#"[{"id":256,"name":"X"}]"#)).is_err());
    }

    #[test]
    fn invalid_json_is_error() {
        assert!(parse_candidates(None, Some("not json")).is_err());
    }
}
