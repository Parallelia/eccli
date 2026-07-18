//! Registration-token commands — the ec's anonymous replacement for the old
//! named-voter model. Tokens are issued in bulk and redeemed anonymously by
//! voters, so no link between a voter identity and a ballot is ever stored.

use anyhow::Result;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

use crate::client::EcClient;
use crate::error::{friendly, Reported};
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

/// Write secret content to `path` readable only by the owner.
///
/// `fs::write` would create the file at the process umask — commonly `0644`,
/// and `0664` under a `002` umask — leaving one-time registration tokens
/// readable by every other local user. These are voter credentials, so the file
/// is created `0600` and an existing file is tightened before the write.
#[cfg(unix)]
fn write_secret_file(path: &Path, contents: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    // `.mode()` applies only when the file is newly created.
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    file.write_all(contents.as_bytes())
}

#[cfg(not(unix))]
fn write_secret_file(path: &Path, contents: &str) -> std::io::Result<()> {
    fs::write(path, contents)
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

    // The daemon has already minted these tokens and will never reveal them
    // again. If persisting them fails we must still surface them, or the
    // operator silently loses credentials that cannot be recovered.
    let mut write_error: Option<String> = None;
    let saved_to = match &output_path {
        Some(path) => match write_secret_file(path, &format_tokens_file(&tokens)) {
            Ok(()) => Some(path.display().to_string()),
            Err(e) => {
                write_error = Some(format!("writing tokens to '{}': {e}", path.display()));
                None
            }
        },
        None => None,
    };

    match mode {
        OutputMode::Json => output::emit_json(json!({
            "ok": write_error.is_none(),
            "error": write_error,
            "count": tokens.len(),
            "saved_to": saved_to,
            // Inline the raw tokens whenever they did not reach a file, so a
            // failed write never destroys them.
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
                    if let Some(err) = &write_error {
                        output::failure(color, err);
                        output::warn(
                            color,
                            "Printing the tokens below instead — they cannot be retrieved again.",
                        );
                    }
                    for t in &tokens {
                        println!("   {t}");
                    }
                }
            }
        }
    }

    if write_error.is_some() {
        return Err(Reported.into());
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

    #[cfg(unix)]
    mod unix {
        use super::*;
        use std::os::unix::fs::PermissionsExt;

        fn mode_of(path: &Path) -> u32 {
            fs::metadata(path).unwrap().permissions().mode() & 0o777
        }

        fn temp_path(name: &str) -> PathBuf {
            let mut p = std::env::temp_dir();
            p.push(format!("eccli-test-{}-{name}", std::process::id()));
            p
        }

        #[test]
        fn new_secret_file_is_owner_only() {
            let path = temp_path("new");
            let _ = fs::remove_file(&path);

            write_secret_file(&path, "tok\n").unwrap();

            assert_eq!(
                mode_of(&path),
                0o600,
                "tokens must not be readable by others"
            );
            assert_eq!(fs::read_to_string(&path).unwrap(), "tok\n");
            fs::remove_file(&path).unwrap();
        }

        #[test]
        fn existing_loose_file_is_tightened() {
            let path = temp_path("loose");
            fs::write(&path, "stale").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();

            write_secret_file(&path, "tok\n").unwrap();

            assert_eq!(mode_of(&path), 0o600);
            assert_eq!(fs::read_to_string(&path).unwrap(), "tok\n");
            fs::remove_file(&path).unwrap();
        }
    }
}
