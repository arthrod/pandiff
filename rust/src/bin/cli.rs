//! pandiff command-line interface (port of `src/cli.ts`).

use clap::Parser;
use pandiff::options::{Metadata, Options, Wrap};
use std::io::Write;

const VERSION: &str = env!("CARGO_PKG_VERSION");

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
      --metadata=STRING";

#[derive(Parser, Debug)]
#[command(disable_help_flag = true, disable_version_flag = true)]
struct Cli {
    #[arg(long)]
    bibliography: Vec<String>,
    #[arg(long)]
    columns: Option<u32>,
    #[arg(long)]
    csl: Vec<String>,
    #[arg(long = "extract-media")]
    extract_media: Option<String>,
    #[arg(short = 'F', long)]
    filter: Vec<String>,
    #[arg(short = 'f', long)]
    from: Option<String>,
    #[arg(short = 'h', long)]
    help: bool,
    #[arg(long = "highlight-style")]
    highlight_style: Option<String>,
    #[arg(long = "lua-filter")]
    lua_filter: Vec<String>,
    #[arg(long)]
    template: Option<String>,
    #[arg(long)]
    mathjax: bool,
    #[arg(long)]
    mathml: bool,
    #[arg(short = 'o', long)]
    output: Option<String>,
    #[arg(long = "pdf-engine")]
    pdf_engine: Option<String>,
    #[arg(long = "metadata-file")]
    metadata_file: Vec<String>,
    #[arg(long = "reference-doc")]
    reference_doc: Vec<String>,
    #[arg(long = "reference-links")]
    reference_links: bool,
    #[arg(long = "resource-path")]
    resource_path: Option<String>,
    #[arg(short = 's', long)]
    standalone: bool,
    #[arg(short = 't', long)]
    to: Option<String>,
    #[arg(short = 'v', long)]
    version: bool,
    #[arg(long)]
    wrap: Option<String>,
    #[arg(long)]
    metadata: Option<String>,
    // Plain positional (no `trailing_var_arg`): like the TS CLI's
    // command-line-args `defaultOption`, options stay parseable whether they
    // appear before or after the file arguments.
    files: Vec<String>,
}

fn to_options(cli: &Cli) -> Options {
    Options {
        bibliography: cli.bibliography.clone(),
        columns: cli.columns,
        csl: cli.csl.clone(),
        extract_media: cli.extract_media.clone(),
        filter: cli.filter.clone(),
        from: cli.from.clone(),
        help: cli.help,
        highlight_style: cli.highlight_style.clone(),
        lua_filter: cli.lua_filter.clone(),
        template: cli.template.clone(),
        mathjax: cli.mathjax,
        mathml: cli.mathml,
        output: cli.output.clone(),
        pdf_engine: cli.pdf_engine.clone(),
        reference_links: cli.reference_links,
        metadata_file: cli.metadata_file.clone(),
        reference_doc: cli.reference_doc.clone(),
        resource_path: cli.resource_path.clone(),
        standalone: cli.standalone,
        threshold: None,
        to: cli.to.clone(),
        version: cli.version,
        wrap: cli.wrap.as_deref().and_then(Wrap::parse),
        files: false,
        metadata: cli.metadata.as_deref().and_then(Metadata::parse),
    }
}

fn help() {
    eprintln!("{HELP}");
}

fn run(cli: &Cli) -> anyhow::Result<()> {
    let opts = to_options(cli);
    let files = &cli.files;
    let mut text: Option<String> = None;

    if opts.version {
        eprintln!("pandiff {VERSION}");
    } else if files.len() == 1 && files[0].ends_with(".docx") {
        text = pandiff::track_changes(&files[0], opts)?;
    } else if files.len() == 1 && files[0].ends_with(".md") {
        let content = std::fs::read_to_string(&files[0])?;
        text = pandiff::normalise(&content, opts)?;
    } else if files.len() == 2 {
        let mut opts = opts;
        opts.files = true;
        text = pandiff::pandiff(&files[0], &files[1], opts)?;
    } else {
        help();
    }

    if let Some(t) = text.filter(|t| !t.is_empty()) {
        let stdout = std::io::stdout();
        let mut lock = stdout.lock();
        lock.write_all(t.as_bytes())?;
    }
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    if let Err(err) = run(&cli) {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}
