//! CLI behaviour tests (port-equivalent of the cli.ts dispatch). These drive
//! the compiled binary with assert_cmd.

use assert_cmd::Command;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn test_path(name: &str) -> String {
    root()
        .join("test")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn pandoc_available() -> bool {
    std::process::Command::new("pandoc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

const HELP: &str = "\
Usage: pandiff [OPTIONS] FILE1 FILE2
      --bibliography=FILE
      --columns=NUMBER
      --csl=FILE
      --extract-media=PATH
  -F, --filter=STRING
  -f, --from=FORMAT
  -h, --help
      --highlight-style=STRING
      --lua-filter=FILE
      --template=STRING
      --mathjax
      --mathml
  -o, --output=FILE
      --pdf-engine=STRING
      --metadata-file=FILE
      --reference-doc=FILE
      --reference-links
      --resource-path=PATH
  -s, --standalone
  -t, --to=FORMAT
  -v, --version
      --wrap=STRING
      --metadata=STRING
";

#[test]
fn version_prints_to_stderr() {
    Command::cargo_bin("pandiff")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stderr(format!("pandiff {}\n", env!("CARGO_PKG_VERSION")))
        .stdout("");
}

#[test]
fn no_args_prints_help_to_stderr() {
    Command::cargo_bin("pandiff")
        .unwrap()
        .assert()
        .success()
        .stderr(HELP)
        .stdout("");
}

#[test]
fn two_files_produce_diff() {
    if !pandoc_available() {
        return;
    }
    Command::cargo_bin("pandiff")
        .unwrap()
        .args([
            "--resource-path",
            &test_path(""),
            &test_path("old.md"),
            &test_path("new.md"),
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("{~~Old~>New~~} Title"));
}

#[test]
fn single_docx_runs_track_changes() {
    if !pandoc_available() {
        return;
    }
    Command::cargo_bin("pandiff")
        .unwrap()
        .arg(test_path("track_changes_move.docx"))
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "{++Here is the text to be moved.++}",
        ));
}

#[test]
fn single_md_runs_normalise() {
    if !pandoc_available() {
        return;
    }
    Command::cargo_bin("pandiff")
        .unwrap()
        .arg(test_path("normalise.in.md"))
        .assert()
        .success()
        .stdout(predicates::str::contains("{~~fonts~>font-styles~~}"));
}
