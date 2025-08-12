use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{ArgAction, Parser, ValueEnum};
use num_format::{Locale, ToFormattedString};
use tabled::settings::{object::Columns, Alignment, Modify, Style};
use tabled::{Table, Tabled};
use tokcount::{aggregate_by_language, count_tokens_in_path, Options};

#[derive(Copy, Clone, Debug, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Parser, Debug)]
#[command(
    name = "tokcount",
    version,
    about = "Count LLM tokens in a folder (gitignore-aware)"
)]
struct Cli {
    /// Root path to scan (defaults to current directory)
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Encoding to use (cl100k_base, o200k_base, p50k_base, r50k_base)
    #[arg(long, default_value = "o200k_base")]
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

    /// Show a by-language table (Language | line of code | token count) [default in text mode]
    #[arg(long, action = ArgAction::SetTrue, hide = true)]
    by_lang: bool,

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
                    .map(|f| serde_json::json!({
                        "path": f.path,
                        "tokens": f.tokens,
                        "lines": f.lines
                    }))
                    .collect::<Vec<_>>(),
                "by_language": aggregate_by_language(&result.files)
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        OutputFormat::Text => {
            // Default mode: always show by-language table
            print_by_language_table(&result);

            if args.per_file {
                for f in &result.files {
                    let tok = fmt_num(f.tokens);
                    println!("{:>12}  {}", tok, f.path.display());
                }
                println!("{}", "-".repeat(60));
            }
            println!("Total tokens: {}", fmt_num(result.total));
        }
    }

    Ok(())
}

fn print_by_language_table(result: &tokcount::CountResult) {
    #[derive(Tabled)]
    struct Row {
        #[tabled(rename = "Language")]
        language: String,
        #[tabled(rename = "line of code")]
        loc: String,
        #[tabled(rename = "token count")]
        tokens: String,
    }

    let rows = aggregate_by_language(&result.files);
    let mut table_rows: Vec<Row> = rows
        .iter()
        .map(|r| Row {
            language: r.language.clone(),
            loc: fmt_num(r.lines),
            tokens: fmt_num(r.tokens),
        })
        .collect();

    let sum_lines: usize = rows.iter().map(|r| r.lines).sum();
    let sum_tokens: usize = rows.iter().map(|r| r.tokens).sum();
    table_rows.push(Row {
        language: "SUM:".to_string(),
        loc: fmt_num(sum_lines),
        tokens: fmt_num(sum_tokens),
    });

    let mut table = Table::new(table_rows);
    table.with(Style::rounded());
    table.with(Modify::new(Columns::single(1)).with(Alignment::right())); // loc
    table.with(Modify::new(Columns::single(2)).with(Alignment::right())); // tokens
    println!("{}", table);
}

fn fmt_num(n: usize) -> String {
    (n as u64).to_formatted_string(&Locale::en)
}
