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
    let exe_hit = std::env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.parent()
                .map(|d| [d.join("assets").join(name), d.join("../assets").join(name)])
        })
        .into_iter()
        .flatten()
        .find(|c| c.exists());
    if let Some(c) = exe_hit {
        return c.to_string_lossy().into_owned();
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

    let first_args = [
        "-f".to_string(),
        "html+tex_math_single_backslash".to_string(),
        "-t".to_string(),
        MARKDOWN.to_string(),
    ];
    let mut output = pandoc(&first_args, Some(&html))?;
    let mut second = args.clone();
    second.push("-t".to_string());
    second.push(MARKDOWN.to_string());
    output = pandoc(&second, Some(&output))?;

    output = critic::spans_to_critic(&output);

    let text = rewrap(&output, opts.wrap == Some(Wrap::None), wrap_columns(opts));
    postrender(&text, opts, metadata)
}

/// Re-wrap the markdown output line by line: leave code blocks and reference
/// links untouched, truncate Setext underlines to the heading length, and soft
/// word-wrap everything else (unless wrapping is disabled).
fn rewrap(output: &str, no_wrap: bool, columns: usize) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut pre = false;
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
    lines.join("\n")
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

    let (body, args) = prepare_postrender(text, opts);
    let input = format!("{metadata}{body}");
    if opts.output.is_some() {
        pandoc(&args, Some(&input))?;
        Ok(None)
    } else {
        Ok(Some(pandoc(&args, Some(&input))?))
    }
}

/// Apply the target-format-specific CriticMarkup transform and assemble the
/// pandoc arguments (everything except the final pandoc invocation). Mutates
/// `opts` (highlight-style default, `.pdf` ⇒ standalone) exactly like the
/// original. Returned as a pure step so it can be tested without pandoc.
fn prepare_postrender(text: &str, opts: &mut Options) -> (String, Vec<String>) {
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

    (text, args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ext_of_extracts_extension() {
        assert_eq!(ext_of("diff.pdf"), ".pdf");
        assert_eq!(ext_of("diff.docx"), ".docx");
        assert_eq!(ext_of("noext"), "");
    }

    #[test]
    fn wrap_columns_defaults_and_overrides() {
        assert_eq!(wrap_columns(&Options::default()), 72);
        assert_eq!(
            wrap_columns(&Options {
                columns: Some(0),
                ..Default::default()
            }),
            72
        );
        assert_eq!(
            wrap_columns(&Options {
                columns: Some(40),
                ..Default::default()
            }),
            40
        );
    }

    #[test]
    fn asset_path_resolves_env_exe_and_manifest() {
        // 1. Executable-relative branch: create assets/ next to the test binary.
        let exe = std::env::current_exe().unwrap();
        let exe_dir = exe.parent().unwrap().to_path_buf();
        let assets_dir = exe_dir.join("assets");
        std::fs::create_dir_all(&assets_dir).unwrap();
        let marker = assets_dir.join("cov_marker.css");
        std::fs::write(&marker, "x").unwrap();
        std::env::remove_var("PANDIFF_ASSETS");
        assert_eq!(asset_path("cov_marker.css"), marker.to_string_lossy());
        std::fs::remove_file(&marker).unwrap();

        // 2. Environment-override branch.
        let tmp = std::env::temp_dir().join("pandiff_assets_cov");
        std::fs::create_dir_all(&tmp).unwrap();
        let env_css = tmp.join("env.css");
        std::fs::write(&env_css, "y").unwrap();
        std::env::set_var("PANDIFF_ASSETS", &tmp);
        assert_eq!(asset_path("env.css"), env_css.to_string_lossy());
        // Env set but the requested file is absent → falls through past the env
        // branch to the later fallbacks.
        let missing = asset_path("not_in_env_dir.css");
        assert!(!missing.starts_with(&*tmp.to_string_lossy()));
        std::env::remove_var("PANDIFF_ASSETS");

        // 3. Manifest fallback for a name that doesn't exist next to the exe.
        let p = asset_path("definitely_missing_asset.css");
        assert!(p.contains("assets"));
        assert!(p.ends_with("definitely_missing_asset.css"));
    }

    #[test]
    fn pandoc_options_html_lists_both_stylesheets() {
        let opts = pandoc_options_html();
        assert!(opts.iter().any(|a| a.ends_with("github-markdown.css")));
        assert!(opts.iter().any(|a| a.ends_with("pandiff.css")));
        assert!(opts.contains(&"--embed-resources".to_string()));
    }

    #[test]
    fn rewrap_truncates_setext_underline_to_heading_length() {
        // "Title" is 5 chars; the underline of 20 `=` should be truncated to 5.
        let out = rewrap("Title\n====================\n\nbody", false, 72);
        assert_eq!(out, "Title\n=====\n\nbody");
    }

    #[test]
    fn rewrap_leaves_code_blocks_and_reference_links_untouched() {
        let input = "```\na very long line that would otherwise be wrapped aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n```\n  [1]: http://example.com/very/long/url/that/exceeds/the/column/limit/aaaaaaaaaaaaaaaa";
        // No wrapping inside the fenced block or on the reference-link line.
        assert_eq!(rewrap(input, false, 72), input);
    }

    #[test]
    fn rewrap_wraps_prose_unless_disabled() {
        let long = "word ".repeat(30);
        let wrapped = rewrap(&long, false, 72);
        assert!(wrapped.contains('\n'));
        let unwrapped = rewrap(&long, true, 72);
        assert!(!unwrapped.contains('\n'));
    }

    #[test]
    fn prepare_postrender_latex_via_to() {
        let mut o = Options {
            to: Some("latex".into()),
            ..Default::default()
        };
        let (body, args) = prepare_postrender("{++x++}", &mut o);
        assert!(body.contains("\\color{OliveGreen}"));
        assert!(args.iter().any(|a| a == "colorlinks=true"));
        assert_eq!(o.highlight_style.as_deref(), Some("kate"));
    }

    #[test]
    fn prepare_postrender_pdf_output_forces_standalone() {
        let mut o = Options {
            output: Some("diff.pdf".into()),
            ..Default::default()
        };
        let (body, args) = prepare_postrender("{--x--}", &mut o);
        assert!(o.standalone, "pdf output should force standalone");
        assert!(body.contains("\\color{Maroon}"));
        assert!(args.contains(&"-s".to_string()));
        assert!(args.contains(&"--output=diff.pdf".to_string()));
    }

    #[test]
    fn prepare_postrender_docx_uses_track_changes() {
        let mut o = Options {
            output: Some("diff.docx".into()),
            ..Default::default()
        };
        let (body, _args) = prepare_postrender("{~~a~>b~~}", &mut o);
        assert!(body.contains("class=\"deletion\""));
        assert!(body.contains("class=\"insertion\""));
    }

    #[test]
    fn prepare_postrender_html_standalone_embeds_css_and_wraps_paragraphs() {
        let mut o = Options {
            to: Some("html".into()),
            standalone: true,
            ..Default::default()
        };
        let (body, args) = prepare_postrender("{++foo++}", &mut o);
        assert!(body.starts_with("<p><ins>foo</ins></p>"));
        assert!(args.iter().any(|a| a.ends_with("github-markdown.css")));
        assert!(args.contains(&"-s".to_string()));
    }

    #[test]
    fn prepare_postrender_html_non_standalone_omits_css() {
        let mut o = Options {
            output: Some("diff.html".into()),
            ..Default::default()
        };
        let (_body, args) = prepare_postrender("{++foo++}", &mut o);
        assert!(!args.iter().any(|a| a.ends_with("github-markdown.css")));
        assert!(!args.contains(&"-s".to_string()));
    }

    // --- Additional boundary & regression tests ----------------------------

    #[test]
    fn ext_of_handles_nested_paths() {
        assert_eq!(ext_of("/some/path/to/diff.tex"), ".tex");
        assert_eq!(ext_of("relative/path/out.docx"), ".docx");
        assert_eq!(ext_of("file"), "");
        assert_eq!(ext_of(".hidden"), ""); // dotfile without extension
    }

    #[test]
    fn ext_of_html_extension() {
        assert_eq!(ext_of("output.html"), ".html");
    }

    #[test]
    fn prepare_postrender_tex_extension_triggers_latex() {
        let mut o = Options {
            output: Some("diff.tex".into()),
            ..Default::default()
        };
        let (body, args) = prepare_postrender("{--removed--}", &mut o);
        assert!(body.contains("\\color{Maroon}"), "expected LaTeX coloring");
        assert!(args.iter().any(|a| a == "colorlinks=true"));
    }

    #[test]
    fn prepare_postrender_to_docx_uses_track_changes() {
        let mut o = Options {
            to: Some("docx".into()),
            ..Default::default()
        };
        let (body, _args) = prepare_postrender("{~~a~>b~~}", &mut o);
        assert!(body.contains("class=\"deletion\""));
        assert!(body.contains("class=\"insertion\""));
    }

    #[test]
    fn prepare_postrender_to_html_non_standalone_no_css() {
        let mut o = Options {
            to: Some("html".into()),
            standalone: false,
            ..Default::default()
        };
        let (_body, args) = prepare_postrender("{++x++}", &mut o);
        assert!(!args.iter().any(|a| a.ends_with("github-markdown.css")));
        // No `-s` flag when not standalone
        assert!(!args.contains(&"-s".to_string()));
    }

    #[test]
    fn prepare_postrender_sets_highlight_style_default() {
        let mut o = Options::default();
        // Explicitly absent before call
        assert!(o.highlight_style.is_none());
        let _ = prepare_postrender("", &mut o);
        assert_eq!(o.highlight_style.as_deref(), Some("kate"));
    }

    #[test]
    fn prepare_postrender_preserves_explicit_highlight_style() {
        let mut o = Options {
            highlight_style: Some("pygments".into()),
            ..Default::default()
        };
        let _ = prepare_postrender("", &mut o);
        assert_eq!(o.highlight_style.as_deref(), Some("pygments"));
    }

    #[test]
    fn prepare_postrender_pdf_output_sets_standalone_true() {
        let mut o = Options {
            output: Some("diff.pdf".into()),
            standalone: false,
            ..Default::default()
        };
        let _ = prepare_postrender("", &mut o);
        assert!(o.standalone, "PDF output must force standalone=true");
    }

    #[test]
    fn prepare_postrender_non_pdf_does_not_set_standalone() {
        let mut o = Options {
            output: Some("diff.html".into()),
            standalone: false,
            ..Default::default()
        };
        let _ = prepare_postrender("", &mut o);
        assert!(!o.standalone, "non-PDF should not force standalone");
    }

    #[test]
    fn rewrap_empty_input_returns_empty() {
        assert_eq!(rewrap("", false, 72), "");
    }

    #[test]
    fn rewrap_code_block_toggle_is_symmetric() {
        // Opening ``` toggles pre=true, closing ``` toggles back.
        // Lines inside the block should be preserved verbatim even if long.
        let input = "```\nlong line aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n```";
        assert_eq!(rewrap(input, false, 72), input);
    }

    #[test]
    fn rewrap_no_wrap_preserves_long_lines() {
        let long = "word ".repeat(40);
        let result = rewrap(long.trim_end(), true, 72);
        // With no_wrap=true, line should not be split
        assert_eq!(result, long.trim_end());
    }

    #[test]
    fn rewrap_setext_underline_longer_than_heading_is_truncated() {
        let input = "Hi\n=========================\n";
        let out = rewrap(input.trim_end(), false, 72);
        assert_eq!(out, "Hi\n==");
    }

    #[test]
    fn rewrap_setext_underline_with_empty_preceding_line_is_kept() {
        // If preceding line is empty (len 0), the underline candidate is left as-is.
        let input = "\n=========";
        let out = rewrap(input, false, 72);
        // No truncation since last_line_len == 0
        assert_eq!(out, "\n=========");
    }

    #[test]
    fn rewrap_reference_link_line_preserved_verbatim() {
        let input = "  [ref]: http://example.com/very/long/url/that/would/be/wrapped/otherwise/yes/it/would";
        assert_eq!(rewrap(input, false, 30), input);
    }

    #[test]
    fn wrap_columns_zero_gives_72() {
        let o = Options {
            columns: Some(0),
            ..Default::default()
        };
        assert_eq!(wrap_columns(&o), 72);
    }

    #[test]
    fn pandoc_options_html_has_embed_resources_and_standalone() {
        let opts = pandoc_options_html();
        assert!(opts.contains(&"--embed-resources".to_string()));
        assert!(opts.contains(&"--standalone".to_string()));
    }

    #[test]
    fn pandoc_options_html_has_include_before_after_article() {
        let opts = pandoc_options_html();
        assert!(opts.iter().any(|a| a.contains("markdown-body")));
        assert!(opts.iter().any(|a| a.contains("</article>")));
    }
}
