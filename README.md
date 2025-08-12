# tokcount

Count LLM tokens in the current folder (or a given path) while respecting .gitignore. Useful for estimating context sizes and repository token budgets.

## Features
- Gitignore-aware directory scan
- Tiktoken-based encodings (cl100k_base, o200k_base, p50k_base, r50k_base)
- Total and per-file token counts
- Text or JSON output
- Optional hidden files and max per-file size
 - By-language summary table (Language | line of code | token count)
- Multi-threaded file processing (Rayon) for faster scans on large repos
 - Progress indicator while scanning (printed to stderr)

## Install

Requires Rust (1.70+ recommended).

```bash
# From the project root
cargo install --path .
```

If you prefer, you can build and run directly:

```bash
cargo run --release -- .
```

## Usage

```bash
# Count tokens in current directory (defaults to cl100k_base)
# Default output: by-language table with comma-formatted numbers
 tokcount

# Count tokens in a specific path
 tokcount path/to/dir

# Show per-file counts
 tokcount --per-file

# JSON output
 tokcount --format json > counts.json

# Use a different encoding
 tokcount --encoding o200k_base

# Include hidden files and limit per-file size to 1 MiB
 tokcount --hidden --max_size 1048576

# Show per-file breakdown (also with formatted numbers)
 tokcount --per-file

# Only include specific extensions (comma-separated, case-insensitive)
 tokcount --ext rs,md,ts

# Disable progress output (defaults to on; prints to stderr)
 tokcount --progress=false
```

### Output (text)
```
      1234  src/main.rs
       456  README.md
------------------------------------------------------------
Total tokens: 34567
```

### By-language table
```
----------------------------------------------
Language        line of code    token count
----------------------------------------------
Rust                    6002          123456
YAML                    3242           22222
Markdown                 212            4321
SUM:                    9456          149999
----------------------------------------------
```

### Output (json)
```json
{
  "path": ".",
  "encoding": "cl100k_base",
  "total": 34567,
  "files": [
    { "path": "src/main.rs", "tokens": 1234 },
    { "path": "README.md", "tokens": 456 }
  ]
}
```

## Notes
- The scan respects .gitignore, global git ignores, and git excludes by default.
- Non-UTF8 files are handled via lossy decoding; binary files may still be counted if they pass filters.
- Encodings are provided by tiktoken-rs. If an unsupported encoding is specified, the CLI exits with an error.
 - Progress updates print to stderr so they won’t pollute JSON or table output on stdout.

## Development

- Run tests: `cargo test`
- Lint/format: `cargo fmt` (if installed) and `cargo clippy` (optional)

## License

MIT or Apache-2.0
