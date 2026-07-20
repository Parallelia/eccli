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
/// readable by every other local user. Opening `path` directly is also unsafe:
/// in a shared output directory it may be a symlink, and we would truncate and
/// chmod its target instead. These are voter credentials, so the content goes
/// to an exclusively-created `0600` temp file alongside `path` and is then
/// renamed into place. `rename` replaces the symlink itself rather than
/// following it, and it is atomic, so readers never observe a half-written
/// token list.
#[cfg(unix)]
fn write_secret_file(path: &Path, contents: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let file_name = path.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("'{}' is not a file path", path.display()),
        )
    })?;
    let mut tmp = match path.parent().filter(|p| !p.as_os_str().is_empty()) {
        Some(dir) => dir.to_path_buf(),
        None => PathBuf::from("."),
    };
    tmp.push(format!(
        ".{}.eccli-{}.tmp",
        file_name.to_string_lossy(),
        std::process::id()
    ));

    // `create_new` fails rather than reusing an attacker-planted temp path.
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)?;

    let result = file
        .write_all(contents.as_bytes())
        .and_then(|()| file.sync_all())
        .and_then(|()| fs::rename(&tmp, path));
    if result.is_err() {
        // Never leave secrets behind in the temp file.
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Refuse to persist tokens where owner-only permissions cannot be guaranteed.
///
/// Rather than silently writing voter credentials at whatever ACL the platform
/// inherits, `--output` is rejected and the caller falls back to printing the
/// tokens, which the operator can then store deliberately.
#[cfg(not(unix))]
fn write_secret_file(path: &Path, _contents: &str) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        format!(
            "refusing to write secret tokens to '{}': eccli cannot enforce owner-only \
             file permissions on this platform",
            path.display()
        ),
    ))
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

        #[test]
        fn symlinked_output_leaves_the_target_untouched() {
            let victim = temp_path("victim");
            let link = temp_path("link");
            let _ = fs::remove_file(&victim);
            let _ = fs::remove_file(&link);
            fs::write(&victim, "important").unwrap();
            fs::set_permissions(&victim, fs::Permissions::from_mode(0o644)).unwrap();
            std::os::unix::fs::symlink(&victim, &link).unwrap();

            write_secret_file(&link, "tok\n").unwrap();

            // The rename replaced the symlink itself; the target is unchanged.
            assert_eq!(fs::read_to_string(&victim).unwrap(), "important");
            assert_eq!(mode_of(&victim), 0o644);
            assert!(!fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink());
            assert_eq!(fs::read_to_string(&link).unwrap(), "tok\n");
            assert_eq!(mode_of(&link), 0o600);

            fs::remove_file(&victim).unwrap();
            fs::remove_file(&link).unwrap();
        }

        #[test]
        fn no_temp_file_is_left_behind() {
            let path = temp_path("leftover");
            let _ = fs::remove_file(&path);

            write_secret_file(&path, "tok\n").unwrap();

            let tmp = path.with_file_name(format!(
                ".{}.eccli-{}.tmp",
                path.file_name().unwrap().to_string_lossy(),
                std::process::id()
            ));
            assert!(
                !tmp.exists(),
                "temp file must not survive a successful write"
            );
            fs::remove_file(&path).unwrap();
        }
    }
}
