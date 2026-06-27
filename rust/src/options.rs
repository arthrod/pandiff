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
}
