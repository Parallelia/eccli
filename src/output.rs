//! Output formatting: human vs JSON, optional ANSI color, and confirmations.

use std::io::{stdin, stdout, IsTerminal, Write};

use anyhow::{bail, Result};

/// How command results are rendered.
#[derive(Clone, Copy, Debug)]
pub enum OutputMode {
    /// Human-readable output. `color` enables ANSI styling.
    Human { color: bool },
    /// Machine-readable JSON on stdout.
    Json,
}

impl OutputMode {
    /// Resolve the mode from the `--json` flag; color is auto-enabled on a TTY.
    pub fn resolve(json: bool) -> Self {
        if json {
            OutputMode::Json
        } else {
            OutputMode::Human {
                color: stdout().is_terminal(),
            }
        }
    }

    pub fn is_json(&self) -> bool {
        matches!(self, OutputMode::Json)
    }
}

fn paint(s: &str, code: &str, color: bool) -> String {
    if color {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

pub fn green(s: &str, color: bool) -> String {
    paint(s, "32", color)
}
pub fn red(s: &str, color: bool) -> String {
    paint(s, "31", color)
}
pub fn yellow(s: &str, color: bool) -> String {
    paint(s, "33", color)
}

/// Print a success line (`✅ msg`), optionally colored.
pub fn success(color: bool, msg: &str) {
    println!("{} {msg}", green("✅", color));
}

/// Print a failure line (`❌ msg`), optionally colored.
pub fn failure(color: bool, msg: &str) {
    // Failures go to stderr per the documented stdout/stderr contract: callers
    // returning `Reported` suppress the later error rendering, so this is the
    // only place the message is emitted.
    eprintln!("{} {msg}", red("❌", color));
}

/// Print a warning line (`⚠️ msg`), optionally colored.
pub fn warn(color: bool, msg: &str) {
    println!("{} {msg}", yellow("⚠️", color));
}

/// Pretty-print a JSON value to stdout.
pub fn emit_json(value: serde_json::Value) {
    match serde_json::to_string_pretty(&value) {
        Ok(s) => println!("{s}"),
        Err(_) => println!("{value}"),
    }
}

/// Emit a JSON error object (used on the failure path in `--json` mode).
pub fn emit_json_error(message: &str) {
    emit_json(serde_json::json!({ "ok": false, "error": message }));
}

/// Ask the user to confirm a destructive action. Fails (rather than assuming
/// "no") in a non-interactive session so scripts must pass `--yes` explicitly.
pub fn confirm(prompt: &str) -> Result<bool> {
    if !stdin().is_terminal() {
        bail!("refusing to proceed without confirmation in a non-interactive session; pass --yes");
    }
    print!("{prompt} [y/N] ");
    stdout().flush().ok();
    let mut line = String::new();
    stdin().read_line(&mut line)?;
    Ok(matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paint_disabled_is_plain() {
        assert_eq!(paint("x", "31", false), "x");
    }

    #[test]
    fn paint_enabled_wraps_ansi() {
        assert_eq!(paint("x", "31", true), "\x1b[31mx\x1b[0m");
    }

    #[test]
    fn resolve_json_is_json() {
        assert!(OutputMode::resolve(true).is_json());
    }
}
