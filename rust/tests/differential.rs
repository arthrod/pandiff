//! End-to-end golden tests. Expected outputs in `tests/golden/` were captured
//! from the reference TypeScript pandiff running against Pandoc 3.1.3. These
//! exercise the full pipeline and require `pandoc` (and the test fixtures under
//! `../test`) to be available.

use pandiff::options::{Metadata, Options};
use pandiff::{pandiff, Wrap};
use pretty_assertions::assert_eq;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn test_path(name: &str) -> String {
    root().join("test").join(name).to_string_lossy().into_owned()
}

fn golden(name: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|_| panic!("missing golden {name}"))
}

fn pandoc_available() -> bool {
    std::process::Command::new("pandoc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Resource-path/extract-media options used by the input-format cases.
fn media_opts() -> Options {
    Options {
        extract_media: Some("/tmp".into()),
        files: true,
        resource_path: Some(test_path("")),
        ..Default::default()
    }
}

fn run_input_format(ext: &str) -> String {
    let out = pandiff(
        &test_path(&format!("old.{ext}")),
        &test_path(&format!("new.{ext}")),
        media_opts(),
    )
    .unwrap();
    out.expect("expected diff output")
}

macro_rules! input_format_test {
    ($name:ident, $ext:literal, $golden:literal) => {
        #[test]
        fn $name() {
            if !pandoc_available() {
                eprintln!("skipping: pandoc not available");
                return;
            }
            assert_eq!(run_input_format($ext), golden($golden));
        }
    };
}

input_format_test!(input_epub, "epub", "in_epub");
input_format_test!(input_latex, "tex", "in_tex");
input_format_test!(input_markdown, "md", "in_md");
input_format_test!(input_org, "org", "in_org");
input_format_test!(input_rst, "rst", "in_rst");
input_format_test!(input_textile, "textile", "in_textile");
input_format_test!(input_word, "docx", "in_docx");

#[test]
fn output_html() {
    if !pandoc_available() {
        return;
    }
    let out = pandiff(
        &test_path("old.md"),
        &test_path("new.md"),
        Options {
            files: true,
            resource_path: Some(test_path("")),
            standalone: true,
            to: Some("html".into()),
            ..Default::default()
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(out, golden("out_html"));
}

#[test]
fn output_latex() {
    if !pandoc_available() {
        return;
    }
    let out = pandiff(
        &test_path("old.md"),
        &test_path("new.md"),
        Options {
            files: true,
            standalone: true,
            to: Some("latex".into()),
            ..Default::default()
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(out, golden("out_latex"));
}

#[test]
fn tables_html() {
    if !pandoc_available() {
        return;
    }
    let out = pandiff(
        &test_path("old-table.md"),
        &test_path("new-table.md"),
        Options {
            files: true,
            to: Some("html".into()),
            ..Default::default()
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(out, golden("tables"));
}

#[test]
fn quotes() {
    if !pandoc_available() {
        return;
    }
    let out = pandiff("said “foo bar”", "said “Foo bar”", Options::default())
        .unwrap()
        .unwrap();
    assert_eq!(out, golden("quotes1"));
    let out = pandiff("", "said “foo bar”", Options::default())
        .unwrap()
        .unwrap();
    assert_eq!(out, golden("quotes2"));
}

#[test]
fn threshold_returns_none_below_cutoff() {
    if !pandoc_available() {
        return;
    }
    let out = pandiff("foo bar baz", "Foo bar baz", Options::default())
        .unwrap()
        .unwrap();
    assert_eq!(out, "{~~foo~>Foo~~} bar baz\n");
    let none = pandiff(
        "foo bar baz",
        "Foo bar baz",
        Options {
            threshold: Some(0.5),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(none.is_none());
}

#[test]
fn math() {
    if !pandoc_available() {
        return;
    }
    let out = pandiff("let $2+2=4$", "let $2+2=5$", Options::default())
        .unwrap()
        .unwrap();
    assert_eq!(out, golden("math1"));
    let out = pandiff("$$a b c$$", "$$a d c$$", Options::default())
        .unwrap()
        .unwrap();
    assert_eq!(out, golden("math2"));
}

#[test]
fn paras_html() {
    if !pandoc_available() {
        return;
    }
    let out = pandiff(
        "",
        "foo\n\nbar",
        Options {
            to: Some("html".into()),
            ..Default::default()
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(out, golden("paras"));
}

#[test]
fn metadata_old_and_new() {
    if !pandoc_available() {
        return;
    }
    let out = pandiff(
        &test_path("old-metadata.md"),
        &test_path("new-metadata.md"),
        Options {
            files: true,
            metadata: Some(Metadata::Old),
            to: Some("markdown".into()),
            ..Default::default()
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(out, golden("meta_old"));

    let out = pandiff(
        &test_path("old-metadata.md"),
        &test_path("new-metadata.md"),
        Options {
            files: true,
            metadata: Some(Metadata::New),
            to: Some("markdown".into()),
            ..Default::default()
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(out, golden("meta_new"));
}

#[test]
fn track_changes_deletion_insertion_move() {
    if !pandoc_available() {
        return;
    }
    for task in ["deletion", "insertion", "move"] {
        let out = pandiff::track_changes(
            &test_path(&format!("track_changes_{task}.docx")),
            Options::default(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(out, golden(&format!("tc_{task}")), "task={task}");
    }
}

#[test]
fn normalise_round_trip() {
    if !pandoc_available() {
        return;
    }
    let input = std::fs::read_to_string(test_path("normalise.in.md")).unwrap();
    let out = pandiff::normalise(&input, Options::default())
        .unwrap()
        .unwrap();
    assert_eq!(out, golden("normalise"));
}

#[test]
fn wrap_none_is_respected() {
    // Sanity check that the Wrap enum is wired (no pandoc dependency on parse).
    assert_eq!(Wrap::parse("none"), Some(Wrap::None));
}
