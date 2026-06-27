//! pandiff — prose diffs for any document format supported by Pandoc.
//!
//! Rust port of the TypeScript implementation. The public entry points mirror
//! the original: [`pandiff`], [`track_changes`], and [`normalise`].

pub mod convert;
pub mod critic;
pub mod dom;
pub mod htmldiff;
pub mod options;
pub mod pandoc;
pub mod postprocess;
pub mod render;
pub mod wrap;

use anyhow::Result;
use convert::{convert, extract_metadata};
use htmldiff::htmldiff;
use once_cell::sync::Lazy;
use options::{Metadata, Options};
use regex::Regex;
use render::render;

pub use options::{Metadata as MetadataMode, Wrap};

// `.` does not match newline, matching the JavaScript regexes.
static RE_DEL: Lazy<Regex> = Lazy::new(|| Regex::new(r"<del.*?del>").unwrap());
static RE_INS: Lazy<Regex> = Lazy::new(|| Regex::new(r"<ins.*?ins>").unwrap());

/// Compare two sources and produce a CriticMarkup (or target-format) diff.
/// Returns `None` if the similarity falls below `opts.threshold`, or when output
/// was written to a file.
pub fn pandiff(source1: &str, source2: &str, opts: Options) -> Result<Option<String>> {
    let mut opts = opts;
    let mut metadata = String::new();
    match opts.metadata {
        Some(Metadata::Old) => {
            opts.standalone = true;
            metadata = extract_metadata(source1)?;
        }
        Some(Metadata::New) => {
            opts.standalone = true;
            metadata = extract_metadata(source2)?;
        }
        _ => {}
    }

    let html1 = convert(source1, &opts)?;
    let html2 = convert(source2, &opts)?;
    let html = htmldiff(&html1, &html2);

    let unmodified = RE_INS
        .replace_all(&RE_DEL.replace_all(&html, ""), "")
        .into_owned();
    let similarity = unmodified.chars().count() as f64 / html.chars().count().max(1) as f64;
    if let Some(threshold) = opts.threshold {
        if threshold != 0.0 && similarity < threshold {
            eprintln!(
                "{}% of the content has changed",
                (100.0 - 100.0 * similarity).round() as i64
            );
            return Ok(None);
        }
    }

    render(&html, &mut opts, &metadata)
}

/// Render a Word document's tracked changes as CriticMarkup.
pub fn track_changes(file: &str, opts: Options) -> Result<Option<String>> {
    let mut opts = opts;
    let html = pandoc::pandoc(&[file.to_string(), "--track-changes=all".to_string()], None)?;
    render(&html, &mut opts, "")
}

/// Normalize a CriticMarkup document by diffing its rejected and accepted forms.
pub fn normalise(text: &str, opts: Options) -> Result<Option<String>> {
    let reject = critic::critic_reject(text);
    let accept = critic::critic_accept(text);
    pandiff(&reject, &accept, opts)
}
