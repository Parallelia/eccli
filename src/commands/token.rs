//! Registration-token commands — the ec's anonymous replacement for the old
//! named-voter model. Tokens are issued in bulk and redeemed anonymously by
//! voters, so no link between a voter identity and a ballot is ever stored.

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

use crate::client::EcClient;
use crate::proto::{ElectionIdRequest, GenerateTokensRequest};

/// Serialize tokens for the `--output` file: one per line, trailing newline.
pub fn format_tokens_file(tokens: &[String]) -> String {
    if tokens.is_empty() {
        return String::new();
    }
    let mut out = tokens.join("\n");
    out.push('\n');
    out
}

pub async fn generate(
    client: &mut EcClient,
    election_id: String,
    count: u32,
    output: Option<PathBuf>,
) -> Result<()> {
    let req = client.request(GenerateTokensRequest { election_id, count });
    let tokens = client
        .inner()
        .generate_registration_tokens(req)
        .await?
        .into_inner()
        .tokens;

    println!("✅ Generated {} registration token(s).", tokens.len());
    println!("⚠️  Tokens are secret and shown only once — distribute them securely.");

    match output {
        Some(path) => {
            fs::write(&path, format_tokens_file(&tokens))
                .with_context(|| format!("writing tokens to '{}'", path.display()))?;
            println!("   Saved {} token(s) to {}", tokens.len(), path.display());
        }
        None => {
            for t in &tokens {
                println!("   {t}");
            }
        }
    }
    Ok(())
}

pub async fn list(client: &mut EcClient, election_id: String) -> Result<()> {
    let req = client.request(ElectionIdRequest { election_id });
    let tokens = client
        .inner()
        .list_registration_tokens(req)
        .await?
        .into_inner()
        .tokens;

    let used = tokens.iter().filter(|t| t.used).count();
    println!("🎟️  Registration tokens: {}/{} used", used, tokens.len());
    for t in &tokens {
        let mark = if t.used { "used" } else { "unused" };
        println!("   • {} [{mark}]", t.token_id);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_tokens_file_is_empty() {
        assert_eq!(format_tokens_file(&[]), "");
    }

    #[test]
    fn tokens_file_has_one_per_line_and_trailing_newline() {
        let toks = vec!["aaa".to_string(), "bbb".to_string()];
        assert_eq!(format_tokens_file(&toks), "aaa\nbbb\n");
    }
}
