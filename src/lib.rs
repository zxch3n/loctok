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
}

impl Default for Options {
    fn default() -> Self {
        Self {
            encoding: "cl100k_base".to_string(),
            include_hidden: false,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct FileCount {
    pub path: PathBuf,
    pub tokens: usize,
    pub lines: usize,
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

pub fn count_non_empty_lines(text: &str) -> usize {
    text.lines().filter(|l| !l.trim().is_empty()).count()
}

pub fn language_from_path(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "rs" => "Rust",
        "ts" | "tsx" => "TypeScript",
        "js" | "jsx" => "JavaScript",
        "svelte" => "Svelte",
        "py" => "Python",
        "go" => "Go",
        "java" => "Java",
        "kt" | "kts" => "Kotlin",
        "c" => "C",
        "h" => "C",
        "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => "C++",
        "cs" => "C#",
        "rb" => "Ruby",
        "php" => "PHP",
        "swift" => "Swift",
        "mm" => "Objective-C",
        "scala" => "Scala",
        "rs.in" => "Rust",
        "md" | "markdown" => "Markdown",
        "toml" => "TOML",
        "yaml" | "yml" => "YAML",
        "json" => "JSON",
        "html" | "htm" => "HTML",
        "css" => "CSS",
        "svg" => "SVG",
        "sh" | "bash" | "zsh" => "Shell",
        "bat" | "cmd" => "Batch",
        "ps1" => "PowerShell",
        "txt" => "Text",
        "mbt" | "mbti" => "Moonbit",
        "dart" => "Dart",
        "ex" | "exs" => "Elixir",
        "erl" | "hrl" => "Erlang",
        "fs" | "fsi" | "fsx" => "F#",
        "clj" | "cljs" | "cljc" => "Clojure",
        "hs" | "lhs" => "Haskell",
        "lua" => "Lua",
        "pl" | "pm" => "Perl",
        "r" | "R" => "R",
        "jl" => "Julia",
        "nim" => "Nim",
        "cr" => "Crystal",
        "elm" => "Elm",
        "ml" | "mli" => "OCaml",
        "pas" | "pp" => "Pascal",
        "d" => "D",
        "zig" => "Zig",
        "vb" => "Visual Basic",
        "sql" => "SQL",
        "dockerfile" => "Dockerfile",
        "makefile" | "mk" => "Makefile",
        "cmake" => "CMake",
        "gradle" => "Gradle",
        "xml" => "XML",
        "proto" => "Protocol Buffers",
        "graphql" | "gql" => "GraphQL",
        "vue" => "Vue",
        "sol" => "Solidity",
        "tf" => "Terraform",
        "nix" => "Nix",
        "asm" | "s" => "Assembly",
        "cobol" | "cob" => "COBOL",
        "fortran" | "f90" | "f95" => "Fortran",
        "ada" | "adb" | "ads" => "Ada",
        "lisp" | "lsp" => "Lisp",
        "scheme" | "scm" => "Scheme",
        "prolog" | "pro" => "Prolog",
        "matlab" | "m" => "MATLAB",
        "octave" => "Octave",
        "mathematica" | "nb" => "Mathematica",
        "sage" => "SageMath",
        "sas" => "SAS",
        "spss" => "SPSS",
        "stata" | "do" => "Stata",
        "verilog" | "v" => "Verilog",
        "vhdl" | "vhd" => "VHDL",
        "tcl" => "Tcl",
        "expect" => "Expect",
        "awk" => "AWK",
        "sed" => "Sed",
        "grep" => "Grep",
        "powershell" | "psm1" => "PowerShell",
        "vim" => "Vim Script",
        "emacs" | "el" => "Emacs Lisp",
        "latex" | "tex" => "LaTeX",
        "bibtex" | "bib" => "BibTeX",
        "rmd" | "rnw" => "R Markdown",
        "ipynb" => "Jupyter Notebook",
        "org" => "Org Mode",
        "rst" => "reStructuredText",
        "asciidoc" | "adoc" => "AsciiDoc",
        "wiki" => "Wiki Markup",
        "dot" => "Graphviz DOT",
        "plantuml" | "puml" => "PlantUML",
        "mermaid" => "Mermaid",
        "webassembly" | "wat" | "wasm" => "WebAssembly",
        "glsl" | "vert" | "frag" => "GLSL",
        "hlsl" => "HLSL",
        "cg" => "Cg",
        "maxscript" | "ms" => "MAXScript",
        "mel" => "MEL",
        "gdscript" | "gd" => "GDScript",
        "actionscript" | "as" => "ActionScript",
        "applescript" | "scpt" => "AppleScript",
        "vbs" | "vbscript" => "VBScript",
        "jscript" => "JScript",
        "coffeescript" | "coffee" => "CoffeeScript",
        "livescript" | "ls" => "LiveScript",
        "typescript" | "d.ts" => "TypeScript",
        "purescript" | "purs" => "PureScript",
        "reasonml" | "re" | "rei" => "ReasonML",
        "rescript" | "res" | "resi" => "ReScript",
        "chapel" | "chpl" => "Chapel",
        "pony" => "Pony",
        "red" | "reds" => "Red",
        "rebol" => "REBOL",
        "factor" => "Factor",
        "forth" | "4th" => "Forth",
        "smalltalk" | "st" => "Smalltalk",
        "self" => "Self",
        "io" => "Io",
        "groovy" => "Groovy",
        "gradle.kts" => "Kotlin",
        _ => "Other",
    }
    .to_string()
}

#[derive(Debug, Serialize, Clone)]
pub struct LangSummary {
    pub language: String,
    pub lines: usize,
    pub tokens: usize,
}

pub fn aggregate_by_language(files: &[FileCount]) -> Vec<LangSummary> {
    use std::collections::BTreeMap;
    let mut map: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for f in files {
        let lang = language_from_path(&f.path);
        let entry = map.entry(lang).or_insert((0, 0));
        entry.0 += f.lines;
        entry.1 += f.tokens;
    }
    let mut v: Vec<LangSummary> = map
        .into_iter()
        .map(|(language, (lines, tokens))| LangSummary {
            language,
            lines,
            tokens,
        })
        .collect();
    // Sort by token count desc
    v.sort_by(|a, b| b.tokens.cmp(&a.tokens));
    v
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

        let path = dent.path();
        let bytes = match fs::read(path) {
            Ok(b) => b,
            Err(err) => {
                eprintln!("warn: failed to read {}: {err}", path.display());
                continue;
            }
        };
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        let tokens = count_tokens_in_text(&encoder, &text);
        let lines = count_non_empty_lines(&text);
        total += tokens;
        files.push(FileCount {
            path: path.to_path_buf(),
            tokens,
            lines,
        });
    }

    Ok(CountResult { total, files })
}
