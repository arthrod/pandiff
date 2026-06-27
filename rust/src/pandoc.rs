//! Thin wrapper around the external `pandoc` process, replacing the
//! `nodejs-sh` calls. `toString()` in the original returns raw stdout (no
//! trimming) with stderr inherited; we match that.

use anyhow::{bail, Context, Result};
use std::io::Write;
use std::process::{Command, Stdio};

/// Run `pandoc` with `args`, optionally piping `input` to stdin, and return raw
/// stdout as a UTF-8 string. stderr is inherited (pandoc warnings pass through).
pub fn pandoc(args: &[String], input: Option<&str>) -> Result<String> {
    let mut cmd = Command::new("pandoc");
    cmd.args(args);
    cmd.stdin(if input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::inherit());

    let mut child = cmd
        .spawn()
        .context("failed to spawn pandoc (is it installed and on PATH?)")?;

    if let Some(inp) = input {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(inp.as_bytes())
            .context("failed writing to pandoc stdin")?;
    }

    let output = child
        .wait_with_output()
        .context("failed waiting for pandoc")?;
    if !output.status.success() {
        bail!("pandoc exited with status {}", output.status);
    }
    String::from_utf8(output.stdout).context("pandoc produced invalid UTF-8")
}
