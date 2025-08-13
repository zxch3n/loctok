# loctok

Count LOC (lines of code) & TOK (LLM tokens), fast.

<img width="914" height="842" alt="CleanShot 2025-08-13 at 11 16 23@2x" src="https://github.com/user-attachments/assets/f600f9b2-7355-4760-bff4-49db25e10e05" />

## Features

- Gitignore-aware scan (respects `.gitignore`, global gitignore, and git excludes)
- Tiktoken encodings: `o200k_base` (default), `cl100k_base`, `p50k_base`, `p50k_edit`, `r50k_base`
- By-language summary table, JSON report, or file tree view
- Copy mode to concatenate filtered files into a clipboard-ready payload
- Extension filter via `--ext rs,py,ts` (case-insensitive, no leading dots)
- Optional inclusion of hidden files via `--hidden`
- Fast parallel scanning (Rayon) with a live progress indicator (stderr)

## Install

Option 1: via npm (prebuilt binary)

```bash
npx loctok          # run directly
# or
npm i -g loctok     # install globally
```

Option 2: build from source (Rust 1.70+ recommended)

```bash
# From crates.io (recommended)
cargo install loctok

# From Git (without publishing)
cargo install --git https://github.com/yourname/loctok --bin loctok

# From the project root (for local testing)
cargo install --path .
```

## CLI Usage

```bash
# Count tokens in current directory
loctok

# Count tokens in a given path
loctok path/to/dir

# JSON output
loctok --format json > counts.json

# File tree with per-node LOC and tokens
loctok --format tree

# Use a specific encoding
loctok --encoding cl100k_base

# Include only certain extensions (no dots)
loctok --ext rs,md,ts

# Include hidden files (dotfiles)
loctok --hidden

# Progress prints to stderr; to silence in scripts, redirect:
loctok --format json 2>/dev/null

# Concatenate filtered files and copy to clipboard
loctok copy                  # from current directory
loctok copy path/to/dir      # from a given path

# Copy with filters and also print the content
loctok --ext rs,md --hidden copy --show
```

Run `loctok --help` to see all options.

## Examples

The examples below were produced by running against `tests/fixtures` in this repo.

### By-language table (default)

```
> loctok

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
> loctok --format tree

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

```
> loctok --format json

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
    ...
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
    ...
  ],
  "total": 20431
}
```

## Copy Mode

Use copy to bundle filtered files into a single, structured payload that is copied to your clipboard. Optionally print it with `--show`.

```
loctok copy [PATH] [--show] [--ext rs,md] [--hidden]
```

What it does:

- Renders a tree of the included files
- Appends each file as a section with a header and numbered lines
- Copies the entire payload to your system clipboard
- Prints a summary like: `Copied 123 lines (22,333 tokens)`

Snippet of the format:

```
├── src
│   ├── lib.rs
│   └── main.rs
└── README.md

/src/lib.rs:
--------------------------------------------------------------------------------
1 | use anyhow::{Context, Result};
2 | // ...


/README.md:
--------------------------------------------------------------------------------
1 | # loctok
2 | Count LOC and tokens
```

## Behavior and Notes

- Respects `.gitignore`, global gitignore, and git excludes; also adds `.gitignore` as a custom ignore file in non-git contexts.
- Only UTF‑8 text files are counted; non‑UTF‑8 files are skipped silently.
- Language grouping is inferred from file extensions.
- Copy mode requires a platform clipboard tool:
  - macOS: `pbcopy`
  - Windows: `clip`
  - Linux: `xclip` or `xsel`

## License

MIT or Apache-2.0
