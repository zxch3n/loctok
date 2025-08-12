# PRD: tokcount – Gitignore-aware Token Counting CLI

## Overview
tokcount is a Rust CLI that scans a directory, honoring .gitignore, and counts LLM tokens per file and in total using tiktoken encodings. It helps estimate context sizes, auditing code/doc footprints, and planning token budgets.

## Goals
- Count tokens for text/code files under a directory.
- Respect .gitignore, git global ignores, and excludes.
- Support common tiktoken encodings (default: cl100k_base).
- Provide total and optional per-file results in text or JSON.

## Non-Goals
- Exact model-specific prompt formatting nuances.
- Network access or remote API calls.
- Content classification or language detection.

## Target Users
- Developers and writers estimating token usage of repos or documents.

## Functional Requirements
- FR1: Scan a given path (default ".") recursively.
- FR2: Skip files/directories ignored by gitignore rules.
- FR3: Read file content and compute token count via selected encoding.
- FR4: Output total token count.
- FR5: Optional per-file counts.
- FR6: Optional JSON output.
- FR7: Optionally include hidden files.
- FR8: Optionally skip files above a size threshold (bytes).

## CLI
- Command: `tokcount [path] [--encoding <name>] [--per-file] [--format text|json] [--hidden] [--max_size <bytes>]`
- Defaults: `path=.`, `encoding=cl100k_base`, `format=text`, `hidden=false`.
- Errors: Unknown encoding returns a non-zero exit with a descriptive message.

## Acceptance Criteria
- AC1: Running `tokcount` in a repo with a .gitignore excludes ignored files from totals.
- AC2: `--per-file` prints one line per included file and a final total line.
- AC3: `--format json` prints stable JSON with `path`, `encoding`, `total`, `files`.
- AC4: Changing `--encoding` changes totals deterministically.
- AC5: Hidden files are excluded by default and included with `--hidden`.

## Constraints & Risks
- Tokenization differences across encodings may surprise users; README documents supported encodings.
- Non-UTF8 files are decoded lossily; this is acceptable for counting and is documented.

## Metrics
- Time to scan a medium repo (< 5s for ~10k files).
- Deterministic counts for a given encoding and tree state.

