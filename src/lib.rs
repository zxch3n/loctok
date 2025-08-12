use anyhow::{Context, Result};
use ignore::WalkBuilder;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tiktoken_rs::CoreBPE;

#[derive(Clone, Debug)]
pub struct Options {
    pub encoding: String,
    pub include_hidden: bool,
    pub max_file_size: Option<u64>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            encoding: "cl100k_base".to_string(),
            include_hidden: false,
            max_file_size: None,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct FileCount {
    pub path: PathBuf,
    pub tokens: usize,
}

#[derive(Debug, Serialize, Clone)]
pub struct CountResult {
    pub total: usize,
    pub files: Vec<FileCount>,
}

pub fn get_encoder(encoding: &str) -> Result<CoreBPE> {
    match encoding {
        // Common encodings
        "cl100k_base" => tiktoken_rs::cl100k_base().context("Failed to init cl100k_base"),
        "o200k_base" => tiktoken_rs::o200k_base().context("Failed to init o200k_base"),
        "p50k_base" => tiktoken_rs::p50k_base().context("Failed to init p50k_base"),
        "r50k_base" => tiktoken_rs::r50k_base().context("Failed to init r50k_base"),
        other => anyhow::bail!("Unsupported encoding: {other}"),
    }
}

pub fn count_tokens_in_text(encoder: &CoreBPE, text: &str) -> usize {
    encoder.encode_ordinary(text).len()
}

pub fn count_tokens_in_path<P: AsRef<Path>>(root: P, opts: &Options) -> Result<CountResult> {
    let encoder = get_encoder(&opts.encoding)?;

    let mut total: usize = 0;
    let mut files: Vec<FileCount> = Vec::new();

    let mut builder = WalkBuilder::new(root);
    // Honor .gitignore and related git rules explicitly; control hidden files via option
    builder.hidden(!opts.include_hidden);
    builder.follow_links(false);
    builder.ignore(true); // respect .ignore
    builder.git_ignore(true); // respect .gitignore
    builder.git_global(true); // respect global gitignore
    builder.git_exclude(true); // respect .git/info/exclude
    // In environments without a .git directory, also treat .gitignore as a custom ignore file
    builder.add_custom_ignore_filename(".gitignore");

    let walker = builder.build();
    for dent in walker {
        let dent = match dent {
            Ok(d) => d,
            Err(err) => {
                // Skip entries we cannot read, but surface context
                eprintln!("warn: skipping entry: {err}");
                continue;
            }
        };

        let ft = match dent.file_type() {
            Some(t) if t.is_file() => t,
            _ => continue,
        };
        let _ = ft; // silence unused in some toolchains

        // Size filter
        if let Some(limit) = opts.max_file_size {
            if let Ok(md) = dent.metadata() {
                if md.len() > limit {
                    continue;
                }
            }
        }

        let path = dent.path();
        let bytes = match fs::read(path) {
            Ok(b) => b,
            Err(err) => {
                eprintln!("warn: failed to read {}: {err}", path.display());
                continue;
            }
        };
        let text = String::from_utf8_lossy(&bytes);
        let tokens = count_tokens_in_text(&encoder, &text);
        total += tokens;
        files.push(FileCount {
            path: path.to_path_buf(),
            tokens,
        });
    }

    Ok(CountResult { total, files })
}
