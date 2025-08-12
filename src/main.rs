use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{ArgAction, Parser, ValueEnum};
use num_format::{Locale, ToFormattedString};
use tabled::settings::{object::Columns, Alignment, Modify, Style};
use tabled::{Table, Tabled};
use tokcount::{aggregate_by_language, count_tokens_in_path, Options};

#[derive(Copy, Clone, Debug, ValueEnum)]
enum OutputFormat {
    /// Display a table of lines of code and tokens of code, grouped by language
    Table,
    /// Get all files and their token counts
    Json,
    /// Display the file tree and each file/folder's lines of code and tokens of code
    Tree,
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

    /// Output format (text or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    format: OutputFormat,

    /// Comma-separated list of file extensions to include (e.g., "rs,py,js"). If empty, all files are processed.
    #[arg(long, default_value = "")]
    ext: String,
}

fn main() -> Result<()> {
    let args = Cli::parse();
    // Parse ext filter: comma-separated list; case-insensitive; strip leading dots
    let include_exts = {
        let s = args.ext.trim();
        if s.is_empty() {
            None
        } else {
            let mut set = std::collections::HashSet::new();
            for part in s.split(',') {
                let p = part.trim().trim_start_matches('.').to_ascii_lowercase();
                if !p.is_empty() {
                    set.insert(p);
                }
            }
            if set.is_empty() {
                None
            } else {
                Some(set)
            }
        }
    };

    let opts = Options {
        encoding: args.encoding.clone(),
        include_hidden: args.hidden,
        include_exts,
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
        OutputFormat::Table => {
            // Default mode: always show by-language table
            print_by_language_table(&result);
            println!("Total tokens: {}", fmt_num(result.total));
        }
        OutputFormat::Tree => {
            print_tree(&args.path, &result.files);
        }
    }

    Ok(())
}

fn print_by_language_table(result: &tokcount::CountResult) {
    #[derive(Tabled)]
    struct Row {
        #[tabled(rename = "Language")]
        language: String,
        #[tabled(rename = "lines of code")]
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

// ----- Tree mode -----
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone)]
enum NodeKind {
    Dir,
    File,
}

#[derive(Debug, Clone)]
struct TreeNode {
    name: String,
    kind: NodeKind,
    lines: usize,
    tokens: usize,
    children: BTreeMap<String, TreeNode>,
}

impl TreeNode {
    fn new_dir(name: String) -> Self {
        Self {
            name,
            kind: NodeKind::Dir,
            lines: 0,
            tokens: 0,
            children: BTreeMap::new(),
        }
    }

    fn new_file(name: String, lines: usize, tokens: usize) -> Self {
        Self {
            name,
            kind: NodeKind::File,
            lines,
            tokens,
            children: BTreeMap::new(),
        }
    }
}

fn rel_to_root(path: &Path, root_abs: &Path, root_arg: &Path) -> PathBuf {
    // Prefer absolute root prefix; fall back to provided arg prefix; else filename
    if let Ok(p) = path.strip_prefix(root_abs) {
        return p.to_path_buf();
    }
    if let Ok(p) = path.strip_prefix(root_arg) {
        return p.to_path_buf();
    }
    path.file_name()
        .map(|s| PathBuf::from(s))
        .unwrap_or_else(|| path.to_path_buf())
}

fn build_tree(root: &Path, files: &[tokcount::FileCount]) -> TreeNode {
    let root_abs = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let root_name = root
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| root.display().to_string());

    let mut root_node = TreeNode::new_dir(root_name);

    for f in files {
        let rel = rel_to_root(&f.path, &root_abs, root);
        let mut cur = &mut root_node;
        let mut comps = rel.components().peekable();
        while let Some(comp) = comps.next() {
            let name = comp.as_os_str().to_string_lossy().to_string();
            let is_last = comps.peek().is_none();
            if is_last {
                // file
                cur.children
                    .insert(name.clone(), TreeNode::new_file(name, f.lines, f.tokens));
            } else {
                // dir
                cur = cur
                    .children
                    .entry(name.clone())
                    .or_insert_with(|| TreeNode::new_dir(name));
            }
        }
    }

    // Accumulate directory totals
    fn accumulate(node: &mut TreeNode) {
        if matches!(node.kind, NodeKind::Dir) {
            node.lines = 0;
            node.tokens = 0;
            // We want dirs printed before files; BTreeMap groups by key, so we just sum all
            for child in node.children.values_mut() {
                accumulate(child);
                node.lines += child.lines;
                node.tokens += child.tokens;
            }
        }
    }
    accumulate(&mut root_node);
    root_node
}

fn print_tree(root: &Path, files: &[tokcount::FileCount]) {
    let tree = build_tree(root, files);

    // Compute widths for formatted numbers for nicer alignment
    fn compute_widths(node: &TreeNode, max_loc: &mut usize, max_tok: &mut usize) {
        let loc_s = fmt_num(node.lines);
        let tok_s = fmt_num(node.tokens);
        *max_loc = (*max_loc).max(loc_s.len());
        *max_tok = (*max_tok).max(tok_s.len());
        for child in node.children.values() {
            compute_widths(child, max_loc, max_tok);
        }
    }
    let mut max_loc = 0usize;
    let mut max_tok = 0usize;
    compute_widths(&tree, &mut max_loc, &mut max_tok);

    // Determine the maximum label width (prefix + name + optional slash for dirs)
    // Using character count (approx. display width) to avoid byte-length issues
    fn vis_len(s: &str) -> usize {
        s.chars().count()
    }
    fn compute_label_widths(
        node: &TreeNode,
        line_prefix: &str,
        child_prefix: &str,
        max_label: &mut usize,
    ) {
        let name_plain = match node.kind {
            NodeKind::Dir => format!("{}/", node.name),
            NodeKind::File => node.name.clone(),
        };
        let this_len = vis_len(line_prefix) + vis_len(&name_plain);
        *max_label = (*max_label).max(this_len);

        // Order children like printing: dirs first, then files
        let mut dirs: Vec<&TreeNode> = node
            .children
            .values()
            .filter(|n| matches!(n.kind, NodeKind::Dir))
            .collect();
        let mut files: Vec<&TreeNode> = node
            .children
            .values()
            .filter(|n| matches!(n.kind, NodeKind::File))
            .collect();
        dirs.sort_by(|a, b| a.name.cmp(&b.name));
        files.sort_by(|a, b| a.name.cmp(&b.name));
        let ordered = dirs
            .into_iter()
            .chain(files.into_iter())
            .collect::<Vec<_>>();

        for (idx, child) in ordered.into_iter().enumerate() {
            let is_first = idx == 0;
            let branch = if is_first { "┌── " } else { "├── " };
            let child_line_prefix = format!("{}{}", child_prefix, branch);
            let next_prefix = format!("{}{}", child_prefix, if is_first { "    " } else { "│   " });
            compute_label_widths(child, &child_line_prefix, &next_prefix, max_label);
        }
    }
    let mut max_label = 0usize;
    compute_label_widths(&tree, "", "", &mut max_label);

    // Simple ANSI colors; no external deps
    fn color_bold(s: &str) -> String {
        format!("\x1b[1m{}\x1b[0m", s)
    }
    fn color_dir(s: &str) -> String {
        // bright blue
        format!("\x1b[1;34m{}\x1b[0m", s)
    }

    // Print header
    let header_name = color_bold("Name");
    let header_loc_plain = "LOC";
    let header_tok_plain = "TOK";
    let header_loc = color_bold(header_loc_plain);
    let header_tok = color_bold(header_tok_plain);
    let gap = "    "; // spacing between columns
    let pad_label = if max_label > 4 { max_label - 4 } else { 0 }; // 4 == len("Name")
    let pad_loc = if max_loc > header_loc_plain.len() {
        max_loc - header_loc_plain.len()
    } else {
        0
    };
    let pad_tok = if max_tok > header_tok_plain.len() {
        max_tok - header_tok_plain.len()
    } else {
        0
    };
    println!(
        "{}{}{}{}{}{}{}{}",
        header_name,
        " ".repeat(pad_label),
        gap,
        " ".repeat(pad_loc),
        header_loc,
        gap,
        " ".repeat(pad_tok),
        header_tok
    );
    let total_width = max_label + gap.len() + max_loc + gap.len() + max_tok;
    println!("{}", "-".repeat(total_width));

    // Helper to print one line (with colors, dir slash, and vertical alignment)
    fn line_with_counts(
        prefix: &str,
        name: &str,
        is_dir: bool,
        lines: usize,
        tokens: usize,
        gap: &str,
        max_label: usize,
        max_loc: usize,
        max_tok: usize,
    ) {
        let display_name = if is_dir {
            format!("{}/", name)
        } else {
            name.to_string()
        };
        let colored_name = if is_dir {
            color_dir(&display_name)
        } else {
            display_name.clone()
        };
        let label_len = vis_len(prefix) + vis_len(&display_name);
        let pad_label = if max_label > label_len {
            max_label - label_len
        } else {
            0
        };
        let loc_s = fmt_num(lines);
        let tok_s = fmt_num(tokens);
        let pad_loc = if max_loc > loc_s.len() {
            max_loc - loc_s.len()
        } else {
            0
        };
        let pad_tok = if max_tok > tok_s.len() {
            max_tok - tok_s.len()
        } else {
            0
        };
        println!(
            "{}{}{}{}{}{}{}{}{}",
            prefix,
            colored_name,
            " ".repeat(pad_label),
            gap,
            " ".repeat(pad_loc),
            loc_s,
            gap,
            " ".repeat(pad_tok),
            tok_s
        );
    }

    // Post-order print: children first, then the node itself.
    fn print_node_post(
        node: &TreeNode,
        line_prefix: String,
        child_prefix: String,
        gap: &str,
        max_label: usize,
        max_loc: usize,
        max_tok: usize,
    ) {
        // dirs first, then files
        let mut dirs: Vec<&TreeNode> = node
            .children
            .values()
            .filter(|n| matches!(n.kind, NodeKind::Dir))
            .collect();
        let mut files: Vec<&TreeNode> = node
            .children
            .values()
            .filter(|n| matches!(n.kind, NodeKind::File))
            .collect();
        dirs.sort_by(|a, b| a.name.cmp(&b.name));
        files.sort_by(|a, b| a.name.cmp(&b.name));
        let ordered = dirs
            .into_iter()
            .chain(files.into_iter())
            .collect::<Vec<_>>();

        let len = ordered.len();
        for (idx, child) in ordered.into_iter().enumerate() {
            let is_first = idx == 0;
            let branch = if is_first { "┌── " } else { "├── " };
            let child_line_prefix = format!("{}{}", child_prefix, branch);
            let next_prefix = format!("{}{}", child_prefix, if is_first { "    " } else { "│   " });
            print_node_post(
                child,
                child_line_prefix,
                next_prefix,
                gap,
                max_label,
                max_loc,
                max_tok,
            );
        }

        // Print the node itself last
        line_with_counts(
            &line_prefix,
            &node.name,
            matches!(node.kind, NodeKind::Dir),
            node.lines,
            node.tokens,
            gap,
            max_label,
            max_loc,
            max_tok,
        );
    }

    // Kick off from root with empty prefixes so root appears last
    print_node_post(
        &tree,
        String::new(),
        String::new(),
        gap,
        max_label,
        max_loc,
        max_tok,
    );
}
