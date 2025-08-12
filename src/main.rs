use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{ArgAction, Parser, ValueEnum};
use tokcount::{count_tokens_in_path, Options};

#[derive(Copy, Clone, Debug, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Parser, Debug)]
#[command(name = "tokcount", version, about = "Count LLM tokens in a folder (gitignore-aware)")]
struct Cli {
    /// Root path to scan (defaults to current directory)
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Encoding to use (cl100k_base, o200k_base, p50k_base, r50k_base)
    #[arg(long, default_value = "cl100k_base")]
    encoding: String,

    /// Include hidden files (dotfiles)
    #[arg(long, action = ArgAction::SetTrue)]
    hidden: bool,

    /// Maximum size per file (bytes); larger files are skipped
    #[arg(long)]
    max_size: Option<u64>,

    /// Show per-file token counts
    #[arg(long, action = ArgAction::SetTrue)]
    per_file: bool,

    /// Output format (text or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    format: OutputFormat,
}

fn main() -> Result<()> {
    let args = Cli::parse();
    let opts = Options {
        encoding: args.encoding.clone(),
        include_hidden: args.hidden,
        max_file_size: args.max_size,
    };

    let result = count_tokens_in_path(&args.path, &opts)
        .with_context(|| format!("failed to scan {}", args.path.display()))?;

    match args.format {
        OutputFormat::Json => {
            let json = serde_json::json!({
                "path": args.path,
                "encoding": args.encoding,
                "total": result.total,
                "files": result
                    .files
                    .iter()
                    .map(|f| serde_json::json!({"path": f.path, "tokens": f.tokens}))
                    .collect::<Vec<_>>()
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        OutputFormat::Text => {
            if args.per_file {
                for f in &result.files {
                    println!("{:>10}  {}", f.tokens, f.path.display());
                }
                println!("{}", "-".repeat(60));
            }
            println!("Total tokens: {}", result.total);
        }
    }

    Ok(())
}

