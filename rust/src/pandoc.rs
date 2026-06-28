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
        let mut stdin = child.stdin.take().unwrap();
        let write_res = stdin.write_all(inp.as_bytes());
        // Close stdin so pandoc can proceed, then, if the write failed, reap the
        // child before returning so we never leak a zombie process.
        drop(stdin);
        if let Err(e) = write_res {
            let _ = child.wait();
            return Err(e).context("failed writing to pandoc stdin");
        }
    }

    let output = child
        .wait_with_output()
        .context("failed waiting for pandoc")?;
    if !output.status.success() {
        bail!("pandoc exited with status {}", output.status);
    }
    // Mirror `nodejs-sh`'s `.toString()`, i.e. Node's `Buffer.toString('utf8')`:
    // a lossy decode (invalid bytes become U+FFFD) rather than a hard error.
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

// The success path is exercised by every golden test; the non-zero-exit `bail`
// is exercised by the CLI `errors_exit_nonzero_with_message` integration test.
// Pandoc-dependent assertions live in tests/ so their skip-guards don't dilute
// unit-coverage of the library.
