# loctok (npm)

Count LOC (lines of code) and LLM tokens — fast. This npm package is a thin wrapper around the Rust CLI that downloads a prebuilt binary and exposes the `loctok` command on your system.

## Quick Start

```bash
npx loctok            # run without installing
# or
npm i -g loctok       # install globally
```

- Requires Node.js >= 14.
- On install, a small script downloads the correct prebuilt binary for your OS/CPU.

If you prefer to build from source (Rust required):

```bash
cargo install loctok
```

## What It Does

loctok scans a directory, respecting `.gitignore`, and reports:

- lines of code per language (non-empty lines)
- token counts using tiktoken encodings (default: `o200k_base`)
- output as a summary table, a file tree view, or JSON

It only reads UTF‑8 text files and skips non‑UTF‑8 files silently.

## Usage

```bash
# Count tokens in the current directory
loctok

# Count tokens in a specific path
loctok path/to/dir

# Choose output format: table (default), tree, json
loctok --format tree
loctok --format json > counts.json

# Choose encoding
loctok --encoding cl100k_base

# Filter by file extensions (comma-separated, no dots)
loctok --ext rs,ts,md

# Include hidden files (dotfiles)
loctok --hidden

# Silence progress output in scripts
loctok --format json 2>/dev/null

# See all options
loctok --help
```

Supported encodings: `o200k_base` (default), `cl100k_base`, `p50k_base`, `p50k_edit`, `r50k_base`.

## Output Examples

Table (default):

```
> loctok

595.17ms (655.27 files/s)
╭────────────┬───────────────┬─────────────╮
│ Language   │ lines of code │ token count │
├────────────┼───────────────┼─────────────┤
│ Rust       │       109,910 │     894,106 │
│ Other      │        13,705 │     174,612 │
│ ...        │            ...│         ... │
│ SUM:       │       138,280 │   1,262,285 │
╰────────────┴───────────────┴─────────────╯
```

Tree view:

```
> loctok --format tree

379.96ms (21.05 files/s)

Name                           LOC       TOK
--------------------------------------------
    ┌── lib.rs                 332     3,213
    ├── main.rs                573     4,707
┌── src/                       905     7,920
...
./                           1,989    20,289
```

JSON:

```json
{
  "path": ".",
  "encoding": "o200k_base",
  "token_number": 200000,
  "models": ["GPT-4o", "GPT-4.1", "o1", "o3", "o4"],
  "total": 20431,
  "files": [
    { "path": "./Cargo.toml", "lines": 26, "tokens": 201 },
    { "path": "./src/main.rs", "lines": 573, "tokens": 4707 }
  ],
  "by_language": [
    { "language": "Rust", "lines": 974, "tokens": 8710 },
    { "language": "Markdown", "lines": 120, "tokens": 1316 }
  ]
}
```

Notes:

- Progress updates print to stderr; they are in-place on TTYs and line-based otherwise.
- Token counts use the chosen encoding; `token_number` and `models` are informative mapping hints.

## Supported Platforms

Prebuilt binaries are provided for:

- macOS: `x64`, `arm64`
- Linux (gnu): `x64`, `arm64`
- Windows (MSVC): `x64`

If your platform isn’t covered, build from source with `cargo install loctok`.

## Install Details & Troubleshooting

- This package downloads a prebuilt binary during `npm install` into an internal `vendor/` folder and exposes a small JS shim at `bin/loctok.js`.
- To use a custom mirror, set `LOCTOK_DOWNLOAD_BASE` to a base URL that mirrors the GitHub Releases layout.
- If the download fails (e.g., no network), try again or install from source: `cargo install loctok`.

## License

MIT or Apache-2.0
