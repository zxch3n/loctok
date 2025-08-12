# tokcount

Count LLM tokens and lines of code in a folder (gitignore-aware). Handy for sizing context windows and budgeting tokens across a codebase.

## Features

- Gitignore-aware scan (respects `.gitignore`, global gitignore, and git excludes)
- Tiktoken encodings: `o200k_base` (default), `cl100k_base`, `p50k_base`, `p50k_edit`, `r50k_base`
- By-language summary table, JSON report, or file tree view
- Extension filter via `--ext rs,py,ts` (case-insensitive, no leading dots)
- Optional inclusion of hidden files via `--hidden`
- Fast parallel scanning (Rayon) with a live progress indicator (stderr)

## Install

Requires Rust (1.70+ recommended).

```bash
# From the project root
cargo install --path .

# Or build and run directly
cargo run --release -- .
```

## CLI Usage

```bash
# Count tokens in current directory
tokcount

# Count tokens in a given path
tokcount path/to/dir

# JSON output
tokcount --format json > counts.json

# File tree with per-node LOC and tokens
tokcount --format tree

# Use a specific encoding
tokcount --encoding cl100k_base

# Include only certain extensions (no dots)
tokcount --ext rs,md,ts

# Include hidden files (dotfiles)
tokcount --hidden

# Progress prints to stderr; to silence in scripts, redirect:
tokcount --format json 2>/dev/null
```

Run `tokcount --help` to see all options.

## Examples

The examples below were produced by running against `tests/fixtures` in this repo.

### By-language table (default)

```
> tokcount

595.171834ms (655.27 files/s)
╭────────────┬───────────────┬─────────────╮
│ Language   │ lines of code │ token count │
├────────────┼───────────────┼─────────────┤
│ Rust       │       109,910 │     894,106 │
│ Other      │        13,705 │     174,612 │
│ YAML       │         4,668 │      91,801 │
│ TypeScript │         6,224 │      53,639 │
│ Markdown   │         1,584 │      17,791 │
│ TOML       │         1,260 │      11,727 │
│ SVG        │           222 │      10,950 │
│ JSON       │           261 │       4,001 │
│ Vue        │           214 │       1,524 │
│ Text       │           119 │       1,296 │
│ CSS        │            69 │         420 │
│ JavaScript │            26 │         277 │
│ HTML       │            13 │         112 │
│ Shell      │             5 │          29 │
│ SUM:       │       138,280 │   1,262,285 │
╰────────────┴───────────────┴─────────────╯
```

### Tree view

```
> tokcount --format tree

379.958459ms (21.05 files/s)

Name                           LOC       TOK
--------------------------------------------
    ┌── lib.rs                 332     3,213
    ├── main.rs                573     4,707
┌── src/                       905     7,920
│           ┌── kept2.txt        1         3
│       ┌── nested/              1         3
│       ├── kept.txt             1         3
│   ┌── fixtures/                2         6
│   ├── integration.rs          69       790
├── tests/                      71       796
├── Cargo.lock                 877    10,198
├── Cargo.toml                  26       201
├── README.md                  110     1,174
./                           1,989    20,289
```

### JSON

```json
> tokcount --format json

{
  "by_language": [
    {
      "language": "Other",
      "lines": 877,
      "tokens": 10198
    },
    {
      "language": "Rust",
      "lines": 974,
      "tokens": 8710
    },
    {
      "language": "Markdown",
      "lines": 120,
      "tokens": 1316
    },
    {
      "language": "TOML",
      "lines": 26,
      "tokens": 201
    },
    {
      "language": "Text",
      "lines": 2,
      "tokens": 6
    }
  ],
  "encoding": "o200k_base",
  "files": [
    {
      "lines": 26,
      "path": "./Cargo.toml",
      "tokens": 201
    },
    {
      "lines": 1,
      "path": "./tests/fixtures/nested/kept2.txt",
      "tokens": 3
    },
    {
      "lines": 1,
      "path": "./tests/fixtures/kept.txt",
      "tokens": 3
    },
    {
      "lines": 69,
      "path": "./tests/integration.rs",
      "tokens": 790
    },
    {
      "lines": 877,
      "path": "./Cargo.lock",
      "tokens": 10198
    },
    {
      "lines": 120,
      "path": "./README.md",
      "tokens": 1316
    },
    {
      "lines": 332,
      "path": "./src/lib.rs",
      "tokens": 3213
    },
    {
      "lines": 573,
      "path": "./src/main.rs",
      "tokens": 4707
    }
  ],
  "models": [
    "GPT-4o",
    "GPT-4.1",
    "o1",
    "o3",
    "o4"
  ],
  "path": ".",
  "token_number": 200000,
  "total": 20431
}
```

## Behavior and Notes

- Respects `.gitignore`, global gitignore, and git excludes; also adds `.gitignore` as a custom ignore file in non-git contexts.
- Only UTF‑8 text files are counted; non‑UTF‑8 files are skipped silently.
- Language grouping is inferred from file extensions.
- Progress updates print to stderr. They are ephemeral on TTYs (single updating line) and line-based otherwise. Redirect or pipe stderr to silence in scripts.

## Development

- Run tests: `cargo test`
- Optional: `cargo fmt` and `cargo clippy`

## License

MIT or Apache-2.0
