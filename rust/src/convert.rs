//! `convert` (source → HTML via pandoc) and `extract_metadata`.

use crate::dom;
use crate::options::Options;
use crate::pandoc::pandoc;
use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use std::io::Read;

// Strips escaped math delimiters `\(`, `\)`, `\[`, `\]` left by pandoc.
static RE_MATH_ESCAPES: Lazy<Regex> = Lazy::new(|| Regex::new(r"\\[()\[\]]").unwrap());

fn read_stdin() -> Result<String> {
    let mut s = String::new();
    std::io::stdin()
        .read_to_string(&mut s)
        .context("failed reading stdin")?;
    Ok(s)
}

/// Convert a source (file path when `opts.files`, otherwise literal text) to
/// normalized HTML.
pub fn convert(source: &str, opts: &Options) -> Result<String> {
    let mut args = opts.build_args(&[
        "bibliography",
        "csl",
        "extract-media",
        "filter",
        "from",
        "lua-filter",
        "mathjax",
        "mathml",
        "resource-path",
    ]);
    args.push("--html-q-tags".to_string());
    args.push("--mathjax".to_string());

    let mut html = if opts.files {
        if source == "-" {
            let stdin_content = read_stdin()?;
            pandoc(&args, Some(&stdin_content))?
        } else {
            let mut with_source = args.clone();
            with_source.push(source.to_string());
            pandoc(&with_source, None)?
        }
    } else {
        pandoc(&args, Some(source))?
    };

    html = RE_MATH_ESCAPES.replace_all(&html, "").into_owned();

    if opts.extract_media.is_some() {
        let mut html_args = args.clone();
        html_args.push("--from=html".to_string());
        html = pandoc(&html_args, Some(&html))?;
    }

    // `new JSDOM(html).serialize()`
    let parsed = dom::parse(&html);
    Ok(dom::serialize_document(&parsed))
}

/// Extract the leading YAML metadata block (`---` … `---`) from a file, or "".
pub fn extract_metadata(source: &str) -> Result<String> {
    let file =
        std::fs::read_to_string(source).with_context(|| format!("failed reading {source}"))?;
    let lines: Vec<&str> = file.split('\n').collect();
    if lines.is_empty() || lines[0].trim() != "---" {
        return Ok(String::new());
    }
    let mut metadata = vec![lines[0]];
    for line in &lines[1..] {
        metadata.push(line);
        if line.trim() == "---" {
            break;
        }
    }
    Ok(metadata.join("\n") + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp_file(name: &str, content: &str) -> String {
        // Make the name unique per process and per call so concurrent test
        // workers (and repeated runs) never share or clobber a fixed path.
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir();
        let path = dir.join(format!("pandiff_test_{}_{n}_{name}", std::process::id()));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn metadata_absent_returns_empty() {
        let p = tmp_file("nometa.md", "# Title\n\nbody\n");
        assert_eq!(extract_metadata(&p).unwrap(), "");
    }

    #[test]
    fn metadata_block_extracted() {
        let p = tmp_file("meta.md", "---\ntitle: X\nauthor: Y\n---\n\nbody\n");
        assert_eq!(
            extract_metadata(&p).unwrap(),
            "---\ntitle: X\nauthor: Y\n---\n"
        );
    }

    #[test]
    fn metadata_unterminated_consumes_rest() {
        let p = tmp_file("meta2.md", "---\ntitle: X\n");
        assert_eq!(extract_metadata(&p).unwrap(), "---\ntitle: X\n\n");
    }

    // --- Additional boundary & regression tests ----------------------------

    #[test]
    fn metadata_empty_file_returns_empty() {
        let p = tmp_file("empty.md", "");
        assert_eq!(extract_metadata(&p).unwrap(), "");
    }

    #[test]
    fn metadata_only_opening_dash_fence_returns_opener() {
        // A single `---` with nothing after it: the metadata loop starts
        // (lines[0] == "---") but no subsequent lines exist, so the loop body
        // never executes. Result is ["---"].join("\n") + "\n" = "---\n".
        // This behaves like the unterminated case but with zero body lines.
        let p = tmp_file("dashonly.md", "---");
        assert_eq!(extract_metadata(&p).unwrap(), "---\n");
    }

    #[test]
    fn metadata_nonexistent_file_returns_error() {
        let result = extract_metadata("/nonexistent/path/does_not_exist.md");
        assert!(result.is_err(), "expected error for missing file, got Ok");
    }

    #[test]
    fn metadata_first_line_not_dashes_returns_empty() {
        let p = tmp_file("nodash.md", "not a yaml header\n---\nkey: val\n---\n");
        assert_eq!(extract_metadata(&p).unwrap(), "");
    }

    #[test]
    fn metadata_multi_field_block_extracted_correctly() {
        let content = "---\ntitle: My Doc\nauthor: Alice\ndate: 2024-01-01\n---\n\nBody here.\n";
        let p = tmp_file("multimeta.md", content);
        let expected = "---\ntitle: My Doc\nauthor: Alice\ndate: 2024-01-01\n---\n";
        assert_eq!(extract_metadata(&p).unwrap(), expected);
    }

    #[test]
    fn metadata_stops_at_first_closing_fence() {
        // Two closing `---` fences; extraction should stop at the first one.
        let content = "---\ntitle: A\n---\nextra: B\n---\n";
        let p = tmp_file("doublefence.md", content);
        let expected = "---\ntitle: A\n---\n";
        assert_eq!(extract_metadata(&p).unwrap(), expected);
    }
}
