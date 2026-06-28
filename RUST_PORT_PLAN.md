# pandiff → Rust Port Plan

> **Goal:** Port the TypeScript implementation of `pandiff` (a Pandoc-based prose-diff
> tool) to Rust, preserving byte-for-byte output compatibility, using strict
> **Test-Driven Development (red → green → refactor)** with meaningful, behavior-level
> test coverage.
>
> **Scope source:** `src/index.ts` (536 LOC), `src/cli.ts` (80 LOC), `src/index.spec.ts`
> (242 LOC). Total production surface ≈ **616 LOC**.
>
> **North star:** The Rust binary `pandiff` must produce the **exact same stdout/files**
> as the Node binary for every existing fixture in `test/`. The existing fixtures
> (`test/diff.md`, `test/diff.html`, `test/diff.tex`, …) are the oracle.

---

## 1. Architecture & Strategy

### 1.1 What pandiff actually does (data-flow)

```
              ┌────────────────────────────────────────────────────────────┐
              │                        pandiff(a, b, opts)                   │
              └────────────────────────────────────────────────────────────┘
  source1 ──► convert() ──► html1 ┐
                                  ├─► node-htmldiff(html1, html2) ──► html(ins/del)
  source2 ──► convert() ──► html2 ┘                                      │
                                                                         ▼
                       threshold check  ◄───────────────  similarity ratio
                                                                         │
                                                                         ▼
                          render(html, opts, metadata)
                            │  postprocess(html)   (jsdom DOM surgery)
                            │  pandoc html → markdown (x2)
                            │  span/div → CriticMarkup regex
                            │  word-wrap
                            ▼
                          postrender(text, opts, metadata)
                            │  critic{HTML,LaTeX,TrackChanges} per target
                            │  pandoc markdown → target (html/docx/latex/pdf)
                            ▼
                          stdout | file
```

Three sub-commands branch off the same machinery (see `cli.ts`):
- **two files** → `pandiff(a, b)`
- **one `.docx`** → `trackChanges(file)` (pandoc `--track-changes=all` → `render`)
- **one `.md`** → `normalise(text)` = `pandiff(criticReject(text), criticAccept(text))`

### 1.2 The hard dependency boundary

`pandiff` is fundamentally a **Pandoc orchestrator**. Pandoc is invoked as an external
process. **We keep that boundary identical in Rust** — we shell out to the same `pandoc`
binary with the same arguments. This guarantees output parity for everything Pandoc
itself produces, and shrinks the port to the glue logic around it.

The three *non-Pandoc* JS libraries are the real porting work:

| JS dependency | Role | Rust strategy |
|---|---|---|
| `node-htmldiff` (`htmldiff`) | word-level HTML diff → `<ins>`/`<del>` | **Port the algorithm** to a `htmldiff` module (tokenizer + LCS + wrap). Highest-risk component; must match output exactly. |
| `jsdom` (`JSDOM`) | DOM surgery in `postprocess()` | Use `html5ever` + `markup5ever_rcdom` (or `kuchikiki`/`scraper`). Need mutable DOM + `outerHTML`/`innerHTML` semantics + `serialize()`. |
| `diff` (`diffLines`) | line diff inside code blocks (`diffu`) | `similar` crate (`TextDiff::from_lines`) or `dissimilar`. Small surface. |
| `wordwrap` | greedy word wrap at N columns | `textwrap` crate **or** a hand-rolled port (wordwrap's algorithm is trivial & must match exactly — see §6). |
| `command-line-args` | CLI parsing | `clap` (derive) — must replicate flag names/aliases/`multiple`/`defaultOption`. |
| `nodejs-sh` (`sh.pandoc`) | spawn pandoc, pipe stdin, capture stdout | `std::process::Command` wrapper (`pandoc.rs`). |
| `fs`, `path` | file IO, extension parsing | `std::fs`, `std::path`. |

> ⚠️ **Output-parity risk ranking:** `htmldiff` > `jsdom`/`postprocess` > `wordwrap` >
> everything else. The test plan front-loads these.

### 1.3 Proposed Rust crate layout

```
pandiff-rs/
├── Cargo.toml
├── assets/
│   └── pandiff.css                 # copied verbatim from assets/
├── src/
│   ├── lib.rs                      # pub api: pandiff(), track_changes(), normalise()
│   ├── options.rs                  # Options struct (was: interface Options)
│   ├── pandoc.rs                   # sh.pandoc wrapper (spawn/pipe)
│   ├── convert.rs                  # convert(), extract_metadata(), build_args()
│   ├── htmldiff/
│   │   ├── mod.rs                  # public htmldiff(before, after) -> String
│   │   ├── tokenizer.rs            # html → tokens
│   │   └── diff.rs                 # LCS / matching + ins/del wrapping
│   ├── dom.rs                      # thin DOM helpers over html5ever (forEachR etc.)
│   ├── postprocess.rs              # postprocess() — DOM surgery
│   ├── render.rs                   # render(), postrender(), markdown profile, regexes
│   ├── critic.rs                   # critic{HTML,LaTeX,TrackChanges,Reject,Accept}
│   ├── wrap.rs                     # wordwrap port (diffu lives near render)
│   └── bin/
│       └── cli.rs                  # main(), optionDefinitions, help()
└── tests/
    ├── fixtures.rs                 # golden-file tests mirroring index.spec.ts
    ├── htmldiff.rs                 # unit tests for the htmldiff port
    ├── postprocess.rs              # unit tests for each DOM transform
    ├── critic.rs                   # unit tests for critic markup conversions
    ├── wrap.rs                     # unit tests for word wrap
    └── cli.rs                      # CLI arg-parsing + end-to-end binary tests
```

Crate dependencies (proposed `Cargo.toml`):

```toml
[dependencies]
clap        = { version = "4", features = ["derive"] }
html5ever   = "0.27"
markup5ever_rcdom = "0.3"   # rcdom for mutable tree (or kuchikiki = "0.8")
similar     = "2"           # line diff for diffu()
textwrap    = "0.16"        # OR hand-rolled wrap.rs (decide in Phase 6)
regex       = "1"
once_cell   = "1"           # lazy static regexes
anyhow      = "1"           # error plumbing
thiserror   = "1"

[dev-dependencies]
pretty_assertions = "1"
assert_cmd  = "2"           # CLI binary tests
predicates  = "3"
```

> **Pandoc requirement for CI:** tests that hit fixtures require `pandoc` (and
> `lualatex` for the PDF test) on `PATH`, exactly like the Node test-suite. The
> SessionStart hook / CI image must install Pandoc. Unit tests for `htmldiff`,
> `postprocess`, `critic`, `wrap` are **Pandoc-free** and form the fast inner TDD loop.

---

## 2. Complete Element Inventory (TypeScript → Rust)

> The table catalogues **every** declaration in `src/index.ts` and `src/cli.ts`:
> name, kind, source lines, line count, parameters, types, and the **Rust equivalent
> to be developed** (signature target + target module). "LOC" counts the declaration
> span inclusive of braces.

### 2.1 `src/index.ts`

| # | Element | Kind | Lines | LOC | TS Parameters | TS Return / Type | Rust Equivalent (to develop) | Module |
|---|---|---|---|---|---|---|---|---|
| 1 | `ArrayLike<T>` | interface | 9–12 | 4 | — | `{ length; [i]: T }` | **Eliminated.** Native slices/iterators replace it. | — |
| 2 | `forEachR<T>` | fn (generic) | 14–16 | 3 | `a: ArrayLike<T>, f: (e:T)=>void` | `void` | `fn for_each_r<T>(items, f)` — or just `.iter().rev()`. Reverse iteration matters (mutation safety). | `dom.rs` |
| 3 | `removeNode` | fn | 17–19 | 3 | `node: Node` | `void` | `fn remove_node(handle: &Handle)` detach from parent. | `dom.rs` |
| 4 | `removeNodes<T>` | fn (generic) | 20–22 | 3 | `nodes: ArrayLike<T>` | `void` | `fn remove_nodes(nodes: &[Handle])` reverse-detach. | `dom.rs` |
| 5 | `diffu` | fn | 24–35 | 12 | `text1: string, text2: string` | `string` (unified `-/+/ ` diff) | `fn diffu(a: &str, b: &str) -> String` via `similar::TextDiff::from_lines`. | `render.rs`/`wrap.rs` |
| 6 | `postprocess` | fn | 37–203 | 167 | `html: string` | `string` | `fn postprocess(html: &str) -> String` — DOM surgery (12 distinct transforms, see §4). **Largest unit.** | `postprocess.rs` |
| 7 | `buildArgs` | fn (variadic) | 206–223 | 18 | `opts: Record<string,any>, ...params: string[]` | `string[]` | `fn build_args(opts: &Options, params: &[&str]) -> Vec<String>` — bool/array/scalar flag expansion. | `convert.rs` |
| 8 | `convert` | async fn | 225–264 | 40 | `source: string, opts: Options` | `Promise<string>` (html) | `fn convert(source: &str, opts: &Options) -> Result<String>` — pandoc → html, math cleanup, optional extract-media reflow. | `convert.rs` |
| 9 | `extractMetadata` | async fn | 266–278 | 13 | `source: string` | `Promise<string>` | `fn extract_metadata(source: &Path) -> Result<String>` — pull leading `---…---` YAML block. | `convert.rs` |
| 10 | `pandiff` | async fn (default export) | 280–314 | 35 | `source1, source2: string, opts: Options` | `Promise<string \| null>` | `pub fn pandiff(a: &str, b: &str, opts: &Options) -> Result<Option<String>>` — orchestrator + threshold. | `lib.rs` |
| 11 | `markdown` | const (string) | 316–330 | 15 | — | `string` (pandoc md profile) | `const MARKDOWN: &str` (or `fn markdown_profile()`). Exact extension list. | `render.rs` |
| 12 | `regex` | const (object) | 332–347 | 16 | — | `{ critic{del,ins,sub}, span{…}, div{…} }` | `static` `Regex` set via `once_cell::Lazy`. **Translate JS regex → Rust `regex` syntax carefully** (see §5). | `render.rs` |
| 13 | `render` | async fn | 349–388 | 40 | `html: string, opts, metadata` | `Promise<string\|null>` | `fn render(html: &str, opts: &Options, metadata: &str) -> Result<Option<String>>` — postprocess → pandoc×2 → span→critic → wrap. | `render.rs` |
| 14 | `criticHTML` | const arrow fn | 390–394 | 5 | `text: string` | `string` | `fn critic_html(text: &str) -> String` | `critic.rs` |
| 15 | `criticLaTeX` | const arrow fn | 396–406 | 11 | `text: string` | `string` | `fn critic_latex(text: &str) -> String` | `critic.rs` |
| 16 | `criticTrackChanges` | const arrow fn | 408–415 | 8 | `text: string` | `string` | `fn critic_track_changes(text: &str) -> String` | `critic.rs` |
| 17 | `criticReject` | const arrow fn | 417–421 | 5 | `text: string` | `string` | `fn critic_reject(text: &str) -> String` | `critic.rs` |
| 18 | `criticAccept` | const arrow fn | 422–426 | 5 | `text: string` | `string` | `fn critic_accept(text: &str) -> String` | `critic.rs` |
| 19 | `pandocOptionsHTML` | const (string[]) | 428–439 | 12 | — | `string[]` | `fn pandoc_options_html() -> Vec<String>` — resolves `github-markdown-css` + bundled `pandiff.css` paths. **Asset-path resolution differs** (see §7). | `render.rs` |
| 20 | `postrender` | async fn | 441–491 | 51 | `text: string, opts, metadata` | `Promise<string\|null>` | `fn postrender(text:&str, opts:&Options, metadata:&str) -> Result<Option<String>>` — target dispatch (latex/docx/html/none) + final pandoc. | `render.rs` |
| 21 | `File` / `Format` / `Path` | type alias | 495–497 | 3 | — | `string` | Type aliases or just `String`. Used for `Options` field clarity. | `options.rs` |
| 22 | `Options` | interface | 498–521 | 24 | — | 23 optional fields | `#[derive(Default, Clone)] struct Options { … }` — see §3 for full field map. | `options.rs` |
| 23 | `trackChanges` | async fn (ns export) | 523–529 | 7 | `file: string, opts: Options` | `Promise<string\|null>` | `pub fn track_changes(file: &str, opts: &Options) -> Result<Option<String>>` — pandoc `--track-changes=all` → render. | `lib.rs` |
| 24 | `normalise` | fn (ns export) | 530–535 | 6 | `text: string, opts: Options` | `Promise<string\|null>` | `pub fn normalise(text: &str, opts: &Options) -> Result<Option<String>>` = pandiff(reject, accept). | `lib.rs` |

### 2.2 `src/cli.ts`

| # | Element | Kind | Lines | LOC | TS Parameters | TS Return / Type | Rust Equivalent (to develop) | Module |
|---|---|---|---|---|---|---|---|---|
| 25 | `main` | async fn | 11–27 | 17 | `{files, ...opts}: CommandLineOptions` | `Promise<void>` | `fn run(args: Cli) -> Result<()>` — dispatch (version / docx / md / two-file / help), write stdout. | `bin/cli.rs` |
| 26 | `File`/`Format`/`Path` | const fn (identity) | 29–31 | 3 | `s: string` | `string` | clap value parsers (`String`). | `bin/cli.rs` |
| 27 | `optionDefinitions` | const (array) | 33–58 | 26 | — | `OptionDefinition[]` (23 defs) | `#[derive(Parser)] struct Cli { … }` with `#[arg(...)]` — replicate names, aliases (`-F -f -h -o -t -v -s`), `multiple`, `defaultOption`. | `bin/cli.rs` |
| 28 | `help` | fn | 60–76 | 17 | — | `void` (prints usage) | clap auto-help **must be overridden** to match the exact custom format (see §8). | `bin/cli.rs` |
| 29 | entry call | top-level | 78–80 | 3 | — | — | `fn main()` → parse → `run()` → eprintln error. | `bin/cli.rs` |

**Inventory totals:** 29 catalogued elements. Production LOC ≈ 616. The single dominant
unit is `postprocess` (167 LOC, 27% of the codebase) — it gets its own phase and its own
test file.

---

## 3. `Options` field map (interface → struct)

Every field is optional in TS. In Rust use `Option<T>` (or `bool` for flags where the
struct default `false` == "absent"). Field names contain hyphens in TS/CLI; the struct
uses snake_case with `#[arg(long = "extract-media")]` / serde rename as needed.

| TS field | TS type | Rust field | Rust type | Notes |
|---|---|---|---|---|
| `bibliography` | `File[]` | `bibliography` | `Vec<String>` | multiple |
| `columns` | `number` | `columns` | `Option<u32>` | default wrap 72 |
| `csl` | (cli only) | `csl` | `Vec<String>` | passed to build_args |
| `extract-media` | `Path` | `extract_media` | `Option<String>` | presence-sensitive (`'extract-media' in opts`) |
| `filter` | `string[]` | `filter` | `Vec<String>` | alias `-F` |
| `from` | `Format` | `from` | `Option<String>` | alias `-f` |
| `help` | `boolean` | `help` | `bool` | alias `-h` |
| `highlight-style` | `string` | `highlight_style` | `Option<String>` | defaulted to `kate` in postrender |
| `lua-filter` | `File[]` | `lua_filter` | `Vec<String>` | multiple |
| `template` | `string` | `template` | `Option<String>` | |
| `output` | `File` | `output` | `Option<String>` | alias `-o`; ext drives target |
| `pdf-engine` | `string` | `pdf_engine` | `Option<String>` | |
| `reference-links` | `boolean` | `reference_links` | `bool` | |
| `metadata-file` | `File` | `metadata_file` | `Vec<String>` | cli `multiple:true` |
| `reference-doc` | `File` | `reference_doc` | `Vec<String>` | cli `multiple:true` |
| `resource-path` | `Path` | `resource_path` | `Option<String>` | |
| `standalone` | `boolean` | `standalone` | `bool` | alias `-s`; mutated internally |
| `threshold` | `number` | `threshold` | `Option<f64>` | similarity gate |
| `to` | `Format` | `to` | `Option<String>` | alias `-t` |
| `version` | `boolean` | `version` | `bool` | alias `-v` |
| `wrap` | `'auto'\|'none'\|'preserve'` | `wrap` | `Option<Wrap>` enum | controls word-wrap |
| `files` | `boolean` | `files` | `bool` | "source is a path, not literal text" |
| `metadata` | `'new'\|'old'\|'none'` | `metadata` | `Option<Metadata>` enum | |
| `mathjax` | `boolean` (cli) | `mathjax` | `bool` | build_args |
| `mathml` | `boolean` (cli) | `mathml` | `bool` | build_args |

> **Critical semantic to preserve:** several code paths test **presence** of a key
> (`'extract-media' in opts`, `'highlight-style' in opts`) and several **mutate** opts
> (`opts.standalone = true`). In Rust, model presence with `Option` and pass `&mut Options`
> (or return a derived copy) so the mutation semantics in `pandiff`/`postrender` are
> reproduced exactly. Document each mutation site in the port.

---

## 4. `postprocess()` decomposition (the 12 transforms)

`postprocess` is ported transform-by-transform; **each transform is one red→green cycle**
with its own focused unit test. Order is load-bearing (later transforms depend on earlier
output), so the integration test chains them in sequence.

| T# | Lines | Transform | DOM operation | Test focus |
|---|---|---|---|---|
| T1 | 42–47 | Inline/display math wrapping | wrap `span.math.inline` in `\( \)`, `span.math.display` in `\[ \]` | math span in/out |
| T2 | 48–56 | Math diff de-dup | compare ins-stripped vs del-stripped textContent; emit `<del>…</del><ins>…</ins>` only if changed | changed vs unchanged math |
| T3 | 59–63 | Strip `style` from `<img>` | `removeAttribute('style')` | img style removal |
| T4 | 66–74 | Strip pre-existing span/div/section | unwrap, but keep `insertion`/`deletion` classed ones as `<ins>/<del>` | class-preserving unwrap |
| T5 | 77–125 | Fix figures w/ modified images | rebuild `<figure>` with >1 img into del/ins img blocks + caption alt | multi-image figure |
| T6 | 128–130 | Compact lists | `li>p:only-child` → unwrap `<p>` | single-para list item |
| T7 | 133–136 | Redundant `title` attr | drop `title` when `title===alt` | title==alt |
| T8 | 139–146 | Code-block line diff | for each `<pre>`, if ins/del differ → class `diff` + `diffu()` content | changed code block |
| T9 | 149–154 | `<del>/<ins>` → `span.del/.ins` | `outerHTML` swap | tag→span |
| T10 | 157–169 | Pull diff spans outside inline tags | when span is sole child of `a/code/em/q/strong/sub/sup`, hoist span outside | inline hoist |
| T11 | 172–178 | Merge adjacent same-class spans | concat siblings | adjacency merge |
| T12 | 181–191 | Split rewritten paragraphs | `<p>` with exactly `[del, ins]` → two `<p>` | full rewrite split |
| T13 | 194–201 | Identify substitutions | adjacent `del`+`ins` → `span.sub` wrapper | sub detection |

> Helper trio `forEachR` / `removeNode` / `removeNodes` underpins all of these. They are
> ported **first** (Phase 2) so every transform can reuse them. Note the **reverse**
> iteration is deliberate: mutating a live `NodeList` while iterating forward would skip
> nodes. The Rust DOM port must snapshot the node set (collect to `Vec<Handle>`) then
> iterate `.rev()` — replicate this to keep ordering/serialization identical.

---

## 5. Regex translation table (JS → Rust `regex`)

JS regexes use features the Rust `regex` crate lacks (notably **lookahead** in `critic.sub`).
These must be re-expressed. This is a tracked risk; each gets a dedicated unit test with
adversarial inputs (nested `~`, `>` inside content, multi-line `[\s\S]`).

| Key | JS pattern | Rust approach |
|---|---|---|
| `critic.del` | `/\{--([\s\S]*?)--\}/g` | `(?s)\{--(.*?)--\}` — `(?s)` = dotall for `[\s\S]`. Direct. |
| `critic.ins` | `/\{\+\+([\s\S]*?)\+\+\}/g` | `(?s)\{\+\+(.*?)\+\+\}` |
| `critic.sub` | `/\{~~((?:[^~]\|(?:~(?!>)))+)~>((?:[^~]\|(?:~(?!~\})))+)~~\}/g` | **Uses negative lookahead `(?!…)` — unsupported by `regex`.** Options: (a) `fancy-regex` crate (supports lookaround) for this one pattern; (b) hand-written scanner. **Recommend `fancy-regex` for `critic.sub` only**, keep `regex` for the rest. Add to Cargo.toml. |
| `span.del/.ins` | `/<span class="del">([\s\S]*?)<\/span>/g` | `(?s)<span class="del">(.*?)</span>` |
| `span.sub` | nested span pattern | `(?s)<span class="sub"><span class="del">(.*?)</span><span class="ins">(.*?)</span></span>` |
| `div.del/.ins` | `/<div class="del">\s*([\s\S]*?)\s*<\/div>/g` | `(?s)<div class="del">\s*(.*?)\s*</div>` |
| math cleanup | `html.replace(/\\[()[\]]/g, '')` (index.ts:256) | `Regex::new(r"\\[()\[\]]")` then `.replace_all(_, "")` |

> **Replacement-string semantics:** JS `$1`/`$2` map to Rust `${1}`/`${2}`. Use
> `replace_all` with a closure where ordering/overlap matters. Verify global (`/g`)
> behavior == `replace_all` (it does) and that non-greedy `*?` is preserved.

---

## 6. `wordwrap` port (exact-match requirement)

`wordwrap(columns)(line)` is invoked in `render` (index.ts:379) with default 72. Output
parity depends on its **exact** wrapping algorithm (greedy, splits on `/(\S+\s+)/`,
preserves… ). Two-track decision:

1. **Spike both** `textwrap` and a faithful hand-port against a fixture of tricky lines
   (long URLs, trailing spaces, multi-space runs, CJK, tabs).
2. If `textwrap` matches the fixtures → use it (less code). If any divergence → ship a
   `wrap.rs` that mirrors `wordwrap`'s `\n`-joining + whitespace handling precisely.

The decision is **made by tests**, not assumed. `test/diff.md` already exercises wrapping
at 72 columns (the prose paragraph), so it is a built-in oracle.

---

## 7. Asset / path resolution differences (must handle explicitly)

| Concern | TS mechanism | Rust mechanism |
|---|---|---|
| `pandiff.css` | `require.resolve('../assets/pandiff.css')` | Resolve relative to exe / `CARGO_MANIFEST_DIR`; or embed via `include_str!` and write to a temp file passed to `--css`. **Recommend** locating the installed `assets/pandiff.css` next to the binary, falling back to an embedded copy. |
| `github-markdown-css` | `require.resolve('github-markdown-css')` (npm pkg path) | The npm package ships a `.css` file. **Vendor it** into `assets/github-markdown.css` (record version + license) and reference the local path. Removes the Node dependency. |
| stdin read | `fs.readFileSync(0, 'utf8')` (fd 0) | `std::io::stdin().read_to_string` |
| temp/extract-media `/tmp` | passed through to pandoc | identical pass-through |

---

## 8. CLI parity details (`cli.ts`)

- **Dispatch order matters** (cli.ts:11–25): `version` → single `.docx` → single `.md`
  → two files → help. Replicate exactly; the `.docx`/`.md` single-file branches map to
  `track_changes` / `normalise`.
- **Aliases:** `-F filter`, `-f from`, `-h help`, `-o output`, `-t to`, `-v version`,
  `-s standalone`. `files` is the positional/`defaultOption` (variadic).
- **Custom `--help`:** the TS `help()` prints a bespoke layout (alias column, `=TYPE`
  suffix from `type.name.toUpperCase()`). clap's auto-help differs — **disable it**
  (`disable_help_flag = true`, manual `-h`) and hand-write the formatter to match
  `help()` line-for-line. Snapshot-test the help output.
- **Version string** comes from `package.json` (`require('../package.json').version`).
  In Rust use `env!("CARGO_PKG_VERSION")`, keep `Cargo.toml` version in sync with
  `package.json` (`0.8.0`). Output line: `pandiff 0.8.0` to **stderr** (note: `console.error`).
- Errors print `Error: <e>` to stderr (cli.ts:78–80).

---

## 9. TDD Roadmap — Red → Green → Refactor, phase by phase

> **Method per unit:** (1) write a failing test capturing the TS behavior (RED),
> (2) implement minimal Rust to pass (GREEN), (3) refactor with tests green. Golden
> fixtures from `test/` are reused verbatim as oracles. Each phase ends only when its
> tests are green **and** `cargo clippy -- -D warnings` is clean.

### Phase 0 — Scaffolding & oracle harness
- `cargo new`, add deps, copy `assets/pandiff.css`, vendor `github-markdown.css`.
- **RED:** write `tests/fixtures.rs` golden tests that call the (stub) library for every
  case in `index.spec.ts` (Input formats ×7, Output formats ×4, Track Changes ×3, Misc
  ×5, Metadata ×7). They fail (not implemented).
- Build a `pandoc` availability guard (`#[ignore]` + helper) so unit phases run without it.
- **Exit:** harness compiles; all fixture tests RED for the right reason.

### Phase 1 — `pandoc.rs` (process wrapper) + `build_args`
- **RED:** unit tests: `build_args` bool→`--flag`, array→repeated `--k=v`, scalar→`--k=v`,
  absent→nothing; presence semantics. `pandoc` wrapper: feed stdin, capture stdout,
  arg passing (test against real `pandoc -t plain` with a known input).
- **GREEN:** implement `Pandoc` command builder + `build_args`.
- **Exit:** `build_args` table-tests green; a round-trip `echo | pandoc` test green.

### Phase 2 — DOM helpers (`dom.rs`)
- **RED:** tests for `for_each_r` reverse order, `remove_node`, `remove_nodes`, and
  `inner_html`/`outer_html`/`serialize` round-trips on small fragments (parity vs jsdom
  serialization quirks — self-closing tags, attribute order, entity escaping).
- **GREEN:** wrap `html5ever`/`rcdom`. Pin serialization to match jsdom where fixtures
  demand it (this is the subtle part — capture jsdom output in fixtures first).
- **Exit:** DOM helper tests green; a "parse→serialize is stable" property test green.

### Phase 3 — `htmldiff` port (highest risk) ⚠️
- Reference: `node-htmldiff` source (tokenizer → `o`/`n` token arrays → matching blocks →
  `<ins>/<del>` wrapping, with `data-operation-index`/atomic-tag handling).
- **RED:** port the library's own test vectors + targeted cases:
  word insert, word delete, substitution, tag boundaries, atomic tags (`<img>`, `<math>`),
  entities, whitespace runs. Each as a unit assertion `htmldiff(a,b) == expected`.
- **GREEN:** implement `tokenizer.rs` then `diff.rs` (LCS/longest-matching-block), then
  wrapper. Iterate until vectors pass.
- **Exit:** all htmldiff unit vectors green; fuzz test (random small HTML pairs) doesn't
  panic and produces balanced `<ins>/<del>`.

### Phase 4 — `postprocess.rs` (13 transforms)
- One red→green cycle **per transform T1–T13** (§4), each with a focused fragment test
  derived by feeding minimal HTML through the TS version and capturing output.
- `diffu()` (T8) gets its own unit tests (insert/delete/context lines, trailing newline).
- **GREEN:** implement transforms in original order; integration test chains all 13.
- **Exit:** every transform test green; full `postprocess` integration test green against
  a captured jsdom oracle.

### Phase 5 — `critic.rs` + regex set
- **RED:** unit tests for `critic_html/latex/track_changes/reject/accept` and the span/div
  → critic conversions, including the **`critic.sub` lookahead** adversarial cases
  (`{~~a~>b~~}`, content containing `~`, `>`, `~}`, multi-line).
- **GREEN:** implement with `regex` + `fancy-regex` (sub only). Validate replacement
  ordering (sub before del before ins — order is load-bearing in `render`).
- **Exit:** critic unit tests green incl. round-trip span↔critic.

### Phase 6 — `wrap.rs` decision
- **RED:** wrapping fixture (tricky lines) + the wrap-sensitive slice of `test/diff.md`.
- Spike `textwrap` vs hand-port; pick by test result (§6).
- **Exit:** wrap tests green; `render`'s wrap path matches `diff.md` prose.

### Phase 7 — `convert.rs` (`convert`, `extract_metadata`)
- **RED:** `extract_metadata` unit tests (no header → `''`; `---…---` block extracted;
  header without closing). `convert` integration tests (md→html via pandoc, math
  backslash cleanup, extract-media reflow path).
- **GREEN:** implement.
- **Exit:** convert/metadata tests green.

### Phase 8 — `render.rs` (`render`, `postrender`, `markdown` profile, `pandocOptionsHTML`)
- **RED:** integration tests for each target branch in `postrender` (none/latex/docx/html),
  standalone handling, `highlight-style` defaulting, `.pdf`→standalone, output-to-file vs
  stdout (returns `None` when writing a file).
- **GREEN:** wire postprocess → pandoc×2 → critic regex → wrap → postrender.
- **Exit:** render/postrender branch tests green.

### Phase 9 — Public API (`lib.rs`: `pandiff`, `track_changes`, `normalise`)
- **GREEN against golden fixtures** (Phase 0 tests start passing):
  - Input formats: `epub, tex, md, org, rst, textile, docx` vs `test/diff.md`.
  - Output formats: HTML/`diff.html`, LaTeX/`diff.tex`, Word/`diff.docx.md`, PDF exists.
  - Track Changes: deletion/insertion/move.
  - Misc: normalise, quotes, threshold (incl. `None` return), math, paras, tables.
  - Metadata: old/new/none × with/without metadata (7 cases).
- Reproduce the **threshold** "`N% of the content has changed`" stderr + `null` return.
- Reproduce opts **mutation** semantics (`standalone` toggles).
- **Exit:** all `tests/fixtures.rs` green.

### Phase 10 — CLI (`bin/cli.rs`)
- **RED:** `assert_cmd` tests: two-file invocation == library output; single `.docx`
  → track changes; single `.md` → normalise; `-v` prints version to stderr; `--help`
  snapshot matches custom format; unknown/no args → help; `-o file` writes file, no stdout.
- **GREEN:** clap struct + custom help + dispatch.
- **Exit:** CLI e2e tests green; binary diffs identical to Node CLI on sample inputs.

### Phase 11 — Parity sweep & hardening
- **Differential test:** a script runs Node `pandiff` and Rust `pandiff` over every
  `test/old.* / new.*` pair and asserts byte-identical stdout (where Node is available),
  catching anything fixtures miss.
- `cargo clippy -D warnings`, `cargo fmt --check`, doc comments on public API.
- Coverage gate via `cargo llvm-cov`: target **≥ 90% line / ≥ 85% branch** on
  non-pandoc modules (`htmldiff`, `postprocess`, `critic`, `wrap`, `convert`, options).
  Pandoc-orchestration lines covered by fixture/integration tests.

---

## 10. Test coverage matrix (meaningful, not vanity)

> "Meaningful" = each test pins an **observable behavior** that, if broken, changes output.
> No tests that merely re-assert the implementation.

| Area | Unit tests | Integration / golden | Oracle source |
|---|---|---|---|
| `build_args` | bool/array/scalar/absent/presence | — | TS lines 206–223 |
| `htmldiff` | ins/del/sub/atomic/entity/ws + fuzz | via full pipeline | node-htmldiff vectors + `diff.md` |
| `postprocess` T1–T13 | 13 focused + `diffu` | full chain | captured jsdom output |
| `critic.*` + regex | per-fn incl. sub-lookahead edge | via render | TS 332–426 + Misc tests |
| `wrap` | tricky-line fixture | `diff.md` prose | `wordwrap` behavior |
| `convert`/metadata | extract_metadata 3 cases | md→html | TS 266–278 + Metadata tests |
| `render`/`postrender` | target dispatch branches | all output formats | `diff.{html,tex,docx.md}` |
| `pandiff`/`normalise`/`track_changes` | threshold null, mutation | 7 input + 7 metadata + misc | `index.spec.ts` (full) |
| CLI | dispatch, version, help snapshot, `-o` | binary e2e vs Node | `cli.ts` |

**Every `it(...)` in `index.spec.ts` has a 1:1 Rust counterpart** (28 cases incl. the
commented-out `citeproc`/`captions` which we port as `#[ignore]` to track parity later).

---

## 11. Risks & mitigations

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| `htmldiff` output drift from node-htmldiff | High | High | Port from source + its own test vectors; differential test vs Node; lock before building dependents. |
| jsdom vs html5ever serialization differences (attr order, entities, void tags) | High | High | Capture jsdom oracle fragments in Phase 2; pin serializer; adjust where fixtures demand. |
| `critic.sub` lookahead not expressible in `regex` | Certain | Med | Use `fancy-regex` for that single pattern; unit-test adversarial inputs. |
| `wordwrap` micro-differences | Med | Med | Test-driven choice between `textwrap` and hand-port (§6). |
| Pandoc version variance changes fixtures | Med | High | Pin pandoc version in CI/devcontainer; document required version; treat fixtures as version-locked. |
| Asset path resolution (`github-markdown-css`, css) | Med | Low | Vendor CSS into `assets/`; embed + temp-file fallback. |
| opts mutation / presence semantics lost in translation | Med | Med | Model presence with `Option`, pass `&mut`, document each mutation site, test threshold/standalone toggles. |
| Custom `--help` format mismatch | Low | Low | Disable clap auto-help; snapshot-test exact output. |

---

## 12. Definition of Done

1. `cargo test` green: all unit + integration + golden-fixture + CLI tests.
2. Differential parity: Rust `pandiff` stdout byte-identical to Node `pandiff` across all
   `test/` fixtures (where pandoc available).
3. `cargo clippy -- -D warnings` and `cargo fmt --check` clean.
4. Coverage ≥ 90% line on non-orchestration modules (`cargo llvm-cov`).
5. `pandiff --help` and `pandiff -v` match the TS CLI output.
6. README/port notes document pandoc version pin and asset vendoring.
7. Every `it()` in `index.spec.ts` has a passing (or explicitly `#[ignore]`-tracked) Rust
   equivalent.

---

### Appendix A — Phase dependency order

```
P0 scaffold ─► P1 pandoc/build_args ─► P7 convert ─┐
            └► P2 dom ─► P3 htmldiff ─► P4 postprocess ─┤
            └► P5 critic ─► P6 wrap ───────────────────┤
                                                       ▼
                                          P8 render/postrender
                                                       ▼
                                          P9 public API (fixtures green)
                                                       ▼
                                          P10 CLI ─► P11 parity sweep
```

P2/P3/P4 (DOM→htmldiff→postprocess) and P1/P7 (pandoc→convert) and P5/P6 (critic→wrap)
are three independent tracks that converge at P8 — they can be developed in parallel.
