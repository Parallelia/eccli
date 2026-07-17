//! Registration-token commands — the ec's anonymous replacement for the old
//! named-voter model. Tokens are issued in bulk and redeemed anonymously by
//! voters, so no link between a voter identity and a ballot is ever stored.

use anyhow::{Context, Result};
use serde_json::json;
use std::fs;
use std::path::PathBuf;

use crate::client::EcClient;
use crate::error::friendly;
use crate::output::{self, OutputMode};
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
    mode: OutputMode,
    election_id: String,
    count: u32,
    output_path: Option<PathBuf>,
) -> Result<()> {
    let req = client.request(GenerateTokensRequest { election_id, count });
    let tokens = client
        .inner()
        .generate_registration_tokens(req)
        .await
        .map_err(friendly)?
        .into_inner()
        .tokens;

    let saved_to = if let Some(path) = &output_path {
        fs::write(path, format_tokens_file(&tokens))
            .with_context(|| format!("writing tokens to '{}'", path.display()))?;
        Some(path.display().to_string())
    } else {
        None
    };

    match mode {
        OutputMode::Json => output::emit_json(json!({
            "ok": true,
            "count": tokens.len(),
            "saved_to": saved_to,
            // Only include raw tokens inline when not written to a file.
            "tokens": if saved_to.is_some() { serde_json::Value::Null } else { json!(tokens) },
        })),
        OutputMode::Human { color } => {
            output::success(
                color,
                &format!("Generated {} registration token(s).", tokens.len()),
            );
            output::warn(
                color,
                "Tokens are secret and shown only once — distribute them securely.",
            );
            match &saved_to {
                Some(path) => println!("   Saved {} token(s) to {path}", tokens.len()),
                None => {
                    for t in &tokens {
                        println!("   {t}");
                    }
                }
            }
        }
    }
    Ok(())
}

pub async fn list(client: &mut EcClient, mode: OutputMode, election_id: String) -> Result<()> {
    let req = client.request(ElectionIdRequest { election_id });
    let tokens = client
        .inner()
        .list_registration_tokens(req)
        .await
        .map_err(friendly)?
        .into_inner()
        .tokens;

    let used = tokens.iter().filter(|t| t.used).count();
    match mode {
        OutputMode::Json => {
            let arr: Vec<serde_json::Value> = tokens
                .iter()
                .map(|t| json!({ "token_id": t.token_id, "used": t.used }))
                .collect();
            output::emit_json(json!({
                "ok": true,
                "used": used,
                "total": tokens.len(),
                "tokens": arr,
            }));
        }
        OutputMode::Human { .. } => {
            println!("🎟️  Registration tokens: {}/{} used", used, tokens.len());
            for t in &tokens {
                let mark = if t.used { "used" } else { "unused" };
                println!("   • {} [{mark}]", t.token_id);
            }
        }
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
