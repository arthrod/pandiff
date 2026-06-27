//! `render` and `postrender`: postprocess → pandoc round-trips → CriticMarkup →
//! word-wrap → target-format output.

use crate::critic;
use crate::options::{Options, Wrap};
use crate::pandoc::pandoc;
use crate::postprocess::postprocess;
use crate::wrap::wordwrap;
use anyhow::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use std::path::Path;

/// The restricted `markdown` writer profile used for the intermediate diff.
pub const MARKDOWN: &str = "markdown-bracketed_spans-fenced_code_attributes-fenced_divs-grid_tables-header_attributes-inline_code_attributes-link_attributes-multiline_tables-pipe_tables-raw_attribute-simple_tables-smart";

static RE_SETEXT: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[=-]+$").unwrap());

/// Resolve a bundled asset (CSS) file path: env override, then next to the
/// executable, then the compile-time manifest directory (dev/test).
fn asset_path(name: &str) -> String {
    if let Ok(dir) = std::env::var("PANDIFF_ASSETS") {
        let p = Path::new(&dir).join(name);
        if p.exists() {
            return p.to_string_lossy().into_owned();
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            for cand in [d.join("assets").join(name), d.join("../assets").join(name)] {
                if cand.exists() {
                    return cand.to_string_lossy().into_owned();
                }
            }
        }
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn pandoc_options_html() -> Vec<String> {
    vec![
        "--css".into(),
        asset_path("github-markdown.css"),
        "--css".into(),
        asset_path("pandiff.css"),
        "--variable".into(),
        "include-before=<article class=\"markdown-body\">".into(),
        "--variable".into(),
        "include-after=</article>".into(),
        "--embed-resources".into(),
        "--standalone".into(),
    ]
}

fn wrap_columns(opts: &Options) -> usize {
    match opts.columns {
        Some(0) | None => 72,
        Some(n) => n as usize,
    }
}

/// Render the htmldiff HTML into a CriticMarkup document (and optionally a final
/// target format via `postrender`).
pub fn render(html: &str, opts: &mut Options, metadata: &str) -> Result<Option<String>> {
    let html = postprocess(html);
    let mut args = opts.build_args(&["reference-links"]);
    args.push("--wrap=none".to_string());

    let mut output = pandoc(
        &[
            "-f".to_string(),
            "html+tex_math_single_backslash".to_string(),
            "-t".to_string(),
            MARKDOWN.to_string(),
        ],
        Some(&html),
    )?;
    let mut second = args.clone();
    second.push("-t".to_string());
    second.push(MARKDOWN.to_string());
    output = pandoc(&second, Some(&output))?;

    output = critic::spans_to_critic(&output);

    let mut lines: Vec<String> = Vec::new();
    let mut pre = false;
    let no_wrap = opts.wrap == Some(Wrap::None);
    let columns = wrap_columns(opts);
    for line in output.split('\n') {
        let last_line_len = lines.last().map(|l| l.chars().count()).unwrap_or(0);
        if line.starts_with("```") {
            pre = !pre;
        }
        if pre || line.starts_with("  [") {
            lines.push(line.to_string());
        } else if RE_SETEXT.is_match(line) && last_line_len > 0 {
            let truncated: String = line.chars().take(last_line_len).collect();
            lines.push(truncated);
        } else if !no_wrap {
            for wrapped in wordwrap(columns, line).split('\n') {
                lines.push(wrapped.to_string());
            }
        } else {
            lines.push(line.to_string());
        }
    }
    let text = lines.join("\n");
    postrender(&text, opts, metadata)
}

fn ext_of(output: &str) -> String {
    match Path::new(output).extension() {
        Some(e) => format!(".{}", e.to_string_lossy()),
        None => String::new(),
    }
}

/// Produce the final output in the requested target format (or raw CriticMarkup
/// when neither `output` nor `to` is set). Returns `None` when writing a file.
pub fn postrender(text: &str, opts: &mut Options, metadata: &str) -> Result<Option<String>> {
    if opts.output.is_none() && opts.to.is_none() {
        return Ok(Some(format!("{metadata}{text}")));
    }

    if opts.highlight_style.is_none() {
        opts.highlight_style = Some("kate".to_string());
    }
    let mut args = opts.build_args(&[
        "highlight-style",
        "output",
        "template",
        "pdf-engine",
        "metadata-file",
        "reference-doc",
        "resource-path",
        "standalone",
        "to",
    ]);
    let output_ext = opts.output.as_deref().map(ext_of);
    if output_ext.as_deref() == Some(".pdf") {
        opts.standalone = true;
    }

    let mut text = text.to_string();
    let to = opts.to.as_deref();
    let ext = output_ext.as_deref();
    if to == Some("latex") || ext == Some(".tex") || ext == Some(".pdf") {
        text = critic::critic_latex(&text);
        args.push("--variable".to_string());
        args.push("colorlinks=true".to_string());
    } else if to == Some("docx") || ext == Some(".docx") {
        text = critic::critic_track_changes(&text);
    } else if to == Some("html") || ext == Some(".html") {
        text = critic::critic_html(&text);
        let paras: Vec<String> = text
            .split("\n\n")
            .map(|p| {
                if p.starts_with("<ins>") || p.starts_with("<del>") {
                    format!("<p>{p}</p>")
                } else {
                    p.to_string()
                }
            })
            .collect();
        text = paras.join("\n\n");
        if opts.standalone {
            args.extend(pandoc_options_html());
        }
    }

    if opts.standalone {
        args.push("-s".to_string());
    }

    let input = format!("{metadata}{text}");
    if opts.output.is_some() {
        pandoc(&args, Some(&input))?;
        Ok(None)
    } else {
        Ok(Some(pandoc(&args, Some(&input))?))
    }
}
