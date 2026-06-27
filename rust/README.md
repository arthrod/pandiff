# pandiff (Rust port)

A faithful Rust port of [pandiff](https://github.com/davidar/pandiff) — prose
diffs for any document format supported by Pandoc. This crate reproduces the
behaviour of the original TypeScript implementation, including its public API
(`pandiff`, `track_changes`, `normalise`) and CLI.

See [`../RUST_PORT_PLAN.md`](../RUST_PORT_PLAN.md) for the design, element
inventory, and TDD roadmap that drove this port.

## Requirements

- A recent stable Rust toolchain.
- [`pandoc`](https://pandoc.org/) on `PATH` (the library shells out to it, just
  like the original). The golden test fixtures were captured against
  **Pandoc 3.1.3**; a different Pandoc version may legitimately produce slightly
  different output (Pandoc's own writers evolve between releases), in which case
  regenerate the goldens — see below.
- `lualatex` is only needed if you produce PDF output.

## Build & run

```sh
cargo build --release
./target/release/pandiff old.md new.md
```

Usage mirrors the original CLI:

```sh
pandiff FILE1 FILE2            # diff two documents → CriticMarkup on stdout
pandiff doc.docx              # render Word tracked-changes as CriticMarkup
pandiff doc.md               # normalise an existing CriticMarkup document
pandiff old.md new.md -s -o diff.html
pandiff old.md new.md -o diff.pdf
```

## Architecture

| Module | Responsibility |
|---|---|
| `htmldiff` | Word-level HTML diff (port of `node-htmldiff`) |
| `dom` | Mutable DOM helpers over `markup5ever_rcdom` |
| `postprocess` | The 13 DOM transforms that clean up htmldiff output |
| `critic` | CriticMarkup regexes + conversions (`fancy-regex` for the sub lookahead) |
| `wrap` | `diffu` (code-block line diff) and the `wordwrap` port |
| `options` | `Options` struct + `build_args` flag expansion |
| `pandoc` | External `pandoc` process wrapper |
| `convert` | Source → HTML, metadata extraction |
| `render` | postprocess → pandoc round-trips → CriticMarkup → wrap → target |
| `bin/cli` | Command-line interface |

The Pandoc process boundary is preserved verbatim, so anything Pandoc itself
produces is identical between this port and the original.

## Tests

```sh
cargo test          # unit + end-to-end golden + CLI tests
cargo clippy --all-targets
cargo fmt --check
```

- **Unit tests** (in each module) are Pandoc-free and cover the algorithms
  directly. The `htmldiff` and `postprocess` suites assert against vectors
  captured from the reference JavaScript implementation.
- **`tests/differential.rs`** runs the full pipeline and compares to golden
  files in `tests/golden/`, which were captured from the reference TypeScript
  pandiff on Pandoc 3.1.3. These tests no-op gracefully if `pandoc` is absent.
- **`tests/cli.rs`** drives the compiled binary.

### Regenerating golden fixtures

The goldens are byte-for-byte outputs of the upstream Node implementation. To
refresh them for a new Pandoc version, run the upstream `pandiff` over the same
inputs/options used in `tests/differential.rs` and overwrite the corresponding
files in `tests/golden/`.

## Assets

`assets/github-markdown.css` is vendored from the `github-markdown-css` npm
package (used for standalone HTML output); `assets/pandiff.css` is copied from
the upstream repository. They are resolved at runtime relative to the executable
(or via the `PANDIFF_ASSETS` environment variable), falling back to the crate
directory during development.
