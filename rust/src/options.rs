//! Port of the `pandiff.Options` interface and the `buildArgs` helper.

/// Word-wrap mode (`--wrap`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Wrap {
    Auto,
    None,
    Preserve,
}

impl Wrap {
    pub fn parse(s: &str) -> Option<Wrap> {
        match s {
            "auto" => Some(Wrap::Auto),
            "none" => Some(Wrap::None),
            "preserve" => Some(Wrap::Preserve),
            _ => None,
        }
    }
}

/// Which document's metadata to keep (`--metadata`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Metadata {
    Old,
    New,
    None,
}

impl Metadata {
    pub fn parse(s: &str) -> Option<Metadata> {
        match s {
            "old" => Some(Metadata::Old),
            "new" => Some(Metadata::New),
            "none" => Some(Metadata::None),
            _ => None,
        }
    }
}

/// Options accepted by `pandiff`. Mirrors the TypeScript interface; absent
/// values are modelled with `Option`/empty `Vec`/`false`.
#[derive(Clone, Debug, Default)]
pub struct Options {
    pub bibliography: Vec<String>,
    pub columns: Option<u32>,
    pub csl: Vec<String>,
    pub extract_media: Option<String>,
    pub filter: Vec<String>,
    pub from: Option<String>,
    pub help: bool,
    pub highlight_style: Option<String>,
    pub lua_filter: Vec<String>,
    pub template: Option<String>,
    pub mathjax: bool,
    pub mathml: bool,
    pub output: Option<String>,
    pub pdf_engine: Option<String>,
    pub reference_links: bool,
    pub metadata_file: Vec<String>,
    pub reference_doc: Vec<String>,
    pub resource_path: Option<String>,
    pub standalone: bool,
    pub threshold: Option<f64>,
    pub to: Option<String>,
    pub version: bool,
    pub wrap: Option<Wrap>,
    /// When true, sources are file paths rather than literal document text.
    pub files: bool,
    pub metadata: Option<Metadata>,
}

impl Options {
    /// Equivalent of `buildArgs(opts, ...params)`: expand the named options into
    /// pandoc CLI arguments. Boolean options emit `--name` only when true,
    /// array options emit one `--name=value` per element, and scalar options
    /// emit `--name=value` when present.
    pub fn build_args(&self, params: &[&str]) -> Vec<String> {
        let mut args = Vec::new();
        for param in params {
            match *param {
                // boolean flags
                "mathjax" => {
                    if self.mathjax {
                        args.push("--mathjax".to_string());
                    }
                }
                "mathml" => {
                    if self.mathml {
                        args.push("--mathml".to_string());
                    }
                }
                "reference-links" => {
                    if self.reference_links {
                        args.push("--reference-links".to_string());
                    }
                }
                "standalone" => {
                    if self.standalone {
                        args.push("--standalone".to_string());
                    }
                }
                // array flags
                "bibliography" => push_each(&mut args, "bibliography", &self.bibliography),
                "csl" => push_each(&mut args, "csl", &self.csl),
                "filter" => push_each(&mut args, "filter", &self.filter),
                "lua-filter" => push_each(&mut args, "lua-filter", &self.lua_filter),
                "metadata-file" => push_each(&mut args, "metadata-file", &self.metadata_file),
                "reference-doc" => push_each(&mut args, "reference-doc", &self.reference_doc),
                // scalar flags
                "extract-media" => push_opt(&mut args, "extract-media", &self.extract_media),
                "from" => push_opt(&mut args, "from", &self.from),
                "highlight-style" => push_opt(&mut args, "highlight-style", &self.highlight_style),
                "output" => push_opt(&mut args, "output", &self.output),
                "template" => push_opt(&mut args, "template", &self.template),
                "pdf-engine" => push_opt(&mut args, "pdf-engine", &self.pdf_engine),
                "resource-path" => push_opt(&mut args, "resource-path", &self.resource_path),
                "to" => push_opt(&mut args, "to", &self.to),
                other => panic!("build_args: unknown param {other:?}"),
            }
        }
        args
    }
}

fn push_each(args: &mut Vec<String>, name: &str, values: &[String]) {
    for v in values {
        args.push(format!("--{name}={v}"));
    }
}

fn push_opt(args: &mut Vec<String>, name: &str, value: &Option<String>) {
    if let Some(v) = value {
        args.push(format!("--{name}={v}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boolean_flag_emitted_only_when_true() {
        let mut o = Options::default();
        assert!(o.build_args(&["standalone"]).is_empty());
        o.standalone = true;
        assert_eq!(o.build_args(&["standalone"]), vec!["--standalone"]);
    }

    #[test]
    fn array_flag_emits_one_arg_per_value() {
        let o = Options {
            bibliography: vec!["a.bib".into(), "b.bib".into()],
            ..Default::default()
        };
        assert_eq!(
            o.build_args(&["bibliography"]),
            vec!["--bibliography=a.bib", "--bibliography=b.bib"]
        );
    }

    #[test]
    fn scalar_flag_emitted_when_present() {
        let mut o = Options::default();
        assert!(o.build_args(&["from"]).is_empty());
        o.from = Some("latex".into());
        assert_eq!(o.build_args(&["from"]), vec!["--from=latex"]);
    }

    #[test]
    fn preserves_param_order() {
        let o = Options {
            from: Some("html".into()),
            to: Some("markdown".into()),
            ..Default::default()
        };
        assert_eq!(
            o.build_args(&["from", "to"]),
            vec!["--from=html", "--to=markdown"]
        );
    }

    #[test]
    fn wrap_parse_all_variants() {
        assert_eq!(Wrap::parse("auto"), Some(Wrap::Auto));
        assert_eq!(Wrap::parse("none"), Some(Wrap::None));
        assert_eq!(Wrap::parse("preserve"), Some(Wrap::Preserve));
        assert_eq!(Wrap::parse("bogus"), None);
    }

    #[test]
    fn metadata_parse_all_variants() {
        assert_eq!(Metadata::parse("old"), Some(Metadata::Old));
        assert_eq!(Metadata::parse("new"), Some(Metadata::New));
        assert_eq!(Metadata::parse("none"), Some(Metadata::None));
        assert_eq!(Metadata::parse("bogus"), None);
    }

    #[test]
    fn build_args_covers_every_known_param() {
        let o = Options {
            bibliography: vec!["a.bib".into()],
            csl: vec!["c.csl".into()],
            extract_media: Some("/tmp".into()),
            filter: vec!["f".into()],
            from: Some("latex".into()),
            highlight_style: Some("kate".into()),
            lua_filter: vec!["l.lua".into()],
            template: Some("t".into()),
            mathjax: true,
            mathml: true,
            output: Some("o.html".into()),
            pdf_engine: Some("lualatex".into()),
            reference_links: true,
            metadata_file: vec!["m.yaml".into()],
            reference_doc: vec!["r.docx".into()],
            resource_path: Some("res".into()),
            standalone: true,
            to: Some("html".into()),
            ..Default::default()
        };
        let args = o.build_args(&[
            "bibliography",
            "csl",
            "extract-media",
            "filter",
            "from",
            "lua-filter",
            "mathjax",
            "mathml",
            "resource-path",
            "reference-links",
            "highlight-style",
            "output",
            "template",
            "pdf-engine",
            "metadata-file",
            "reference-doc",
            "standalone",
            "to",
        ]);
        assert_eq!(
            args,
            vec![
                "--bibliography=a.bib",
                "--csl=c.csl",
                "--extract-media=/tmp",
                "--filter=f",
                "--from=latex",
                "--lua-filter=l.lua",
                "--mathjax",
                "--mathml",
                "--resource-path=res",
                "--reference-links",
                "--highlight-style=kate",
                "--output=o.html",
                "--template=t",
                "--pdf-engine=lualatex",
                "--metadata-file=m.yaml",
                "--reference-doc=r.docx",
                "--standalone",
                "--to=html",
            ]
        );
    }

    #[test]
    #[should_panic(expected = "unknown param")]
    fn build_args_panics_on_unknown_param() {
        Options::default().build_args(&["not-a-real-param"]);
    }

    // --- Additional boundary & regression tests ----------------------------

    #[test]
    fn default_options_all_absent() {
        let o = Options::default();
        // All boolean flags default false
        assert!(!o.mathjax);
        assert!(!o.mathml);
        assert!(!o.reference_links);
        assert!(!o.standalone);
        assert!(!o.help);
        assert!(!o.version);
        assert!(!o.files);
        // All Option fields default None
        assert!(o.columns.is_none());
        assert!(o.extract_media.is_none());
        assert!(o.from.is_none());
        assert!(o.highlight_style.is_none());
        assert!(o.template.is_none());
        assert!(o.output.is_none());
        assert!(o.pdf_engine.is_none());
        assert!(o.resource_path.is_none());
        assert!(o.threshold.is_none());
        assert!(o.to.is_none());
        assert!(o.wrap.is_none());
        assert!(o.metadata.is_none());
        // All Vec fields default empty
        assert!(o.bibliography.is_empty());
        assert!(o.csl.is_empty());
        assert!(o.filter.is_empty());
        assert!(o.lua_filter.is_empty());
        assert!(o.metadata_file.is_empty());
        assert!(o.reference_doc.is_empty());
    }

    #[test]
    fn empty_array_emits_no_args() {
        let o = Options::default(); // bibliography is empty
        assert!(o.build_args(&["bibliography"]).is_empty());
        assert!(o.build_args(&["csl"]).is_empty());
        assert!(o.build_args(&["filter"]).is_empty());
        assert!(o.build_args(&["lua-filter"]).is_empty());
        assert!(o.build_args(&["metadata-file"]).is_empty());
        assert!(o.build_args(&["reference-doc"]).is_empty());
    }

    #[test]
    fn all_boolean_flags_emitted_when_set() {
        let o = Options {
            mathjax: true,
            mathml: true,
            reference_links: true,
            standalone: true,
            ..Default::default()
        };
        assert_eq!(o.build_args(&["mathjax"]), vec!["--mathjax"]);
        assert_eq!(o.build_args(&["mathml"]), vec!["--mathml"]);
        assert_eq!(o.build_args(&["reference-links"]), vec!["--reference-links"]);
        assert_eq!(o.build_args(&["standalone"]), vec!["--standalone"]);
    }

    #[test]
    fn boolean_flags_not_emitted_when_false() {
        let o = Options::default();
        assert!(o.build_args(&["mathjax"]).is_empty());
        assert!(o.build_args(&["mathml"]).is_empty());
        assert!(o.build_args(&["reference-links"]).is_empty());
    }

    #[test]
    fn multiple_values_in_same_array_field() {
        let o = Options {
            filter: vec!["f1".into(), "f2".into(), "f3".into()],
            ..Default::default()
        };
        assert_eq!(
            o.build_args(&["filter"]),
            vec!["--filter=f1", "--filter=f2", "--filter=f3"]
        );
    }

    #[test]
    fn all_scalar_fields_emit_correct_format() {
        let o = Options {
            extract_media: Some("/tmp/media".into()),
            from: Some("org".into()),
            highlight_style: Some("pygments".into()),
            output: Some("out.pdf".into()),
            template: Some("my.tex".into()),
            pdf_engine: Some("xelatex".into()),
            resource_path: Some("/resources".into()),
            to: Some("latex".into()),
            ..Default::default()
        };
        assert_eq!(
            o.build_args(&["extract-media"]),
            vec!["--extract-media=/tmp/media"]
        );
        assert_eq!(o.build_args(&["from"]), vec!["--from=org"]);
        assert_eq!(
            o.build_args(&["highlight-style"]),
            vec!["--highlight-style=pygments"]
        );
        assert_eq!(o.build_args(&["output"]), vec!["--output=out.pdf"]);
        assert_eq!(o.build_args(&["template"]), vec!["--template=my.tex"]);
        assert_eq!(
            o.build_args(&["pdf-engine"]),
            vec!["--pdf-engine=xelatex"]
        );
        assert_eq!(
            o.build_args(&["resource-path"]),
            vec!["--resource-path=/resources"]
        );
        assert_eq!(o.build_args(&["to"]), vec!["--to=latex"]);
    }

    #[test]
    fn wrap_enum_equality_and_debug() {
        assert_eq!(Wrap::Auto, Wrap::Auto);
        assert_ne!(Wrap::Auto, Wrap::None);
        assert_ne!(Wrap::None, Wrap::Preserve);
        // Debug formatting should not panic
        let _ = format!("{:?}", Wrap::Auto);
        let _ = format!("{:?}", Wrap::None);
        let _ = format!("{:?}", Wrap::Preserve);
    }

    #[test]
    fn metadata_enum_equality_and_debug() {
        assert_eq!(Metadata::Old, Metadata::Old);
        assert_ne!(Metadata::Old, Metadata::New);
        assert_ne!(Metadata::New, Metadata::None);
        let _ = format!("{:?}", Metadata::Old);
        let _ = format!("{:?}", Metadata::New);
        let _ = format!("{:?}", Metadata::None);
    }

    #[test]
    fn wrap_parse_empty_and_invalid_strings() {
        assert_eq!(Wrap::parse(""), None);
        assert_eq!(Wrap::parse("Auto"), None); // case-sensitive
        assert_eq!(Wrap::parse("NONE"), None);
    }

    #[test]
    fn metadata_parse_empty_and_invalid_strings() {
        assert_eq!(Metadata::parse(""), None);
        assert_eq!(Metadata::parse("Old"), None); // case-sensitive
        assert_eq!(Metadata::parse("NEW"), None);
    }

    #[test]
    fn build_args_empty_params_slice_returns_empty_vec() {
        let o = Options {
            from: Some("markdown".into()),
            ..Default::default()
        };
        assert!(o.build_args(&[]).is_empty());
    }

    #[test]
    fn options_clone_is_independent() {
        let o = Options {
            from: Some("html".into()),
            bibliography: vec!["a.bib".into()],
            ..Default::default()
        };
        let mut cloned = o.clone();
        cloned.from = Some("latex".into());
        cloned.bibliography.push("b.bib".into());
        // Original unchanged
        assert_eq!(o.from.as_deref(), Some("html"));
        assert_eq!(o.bibliography.len(), 1);
    }
}
