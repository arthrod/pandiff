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
    let file = std::fs::read_to_string(source)
        .with_context(|| format!("failed reading {source}"))?;
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
        let dir = std::env::temp_dir();
        let path = dir.join(format!("pandiff_test_{name}"));
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
}
