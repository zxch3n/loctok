use anyhow::{Context, Result};
use ignore::WalkBuilder;
use rayon::prelude::*;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tiktoken_rs::CoreBPE;

#[derive(Clone, Debug)]
pub struct Options {
    pub encoding: String,
    pub include_hidden: bool,
    // Optional whitelist of file extensions to include (lowercased, no leading dot)
    pub include_exts: Option<std::collections::HashSet<String>>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            encoding: "cl100k_base".to_string(),
            include_hidden: false,
            include_exts: None,
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

/// Internal helper: enumerate files under `root` honoring ignore rules and `opts` filters.
fn enumerate_filtered_paths<P: AsRef<Path>>(root: P, opts: &Options) -> Vec<PathBuf> {
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
    let mut paths: Vec<PathBuf> = Vec::new();
    for dent in walker {
        let dent = match dent {
            Ok(d) => d,
            Err(err) => {
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
        // Filter by extension if requested
        if let Some(exts) = &opts.include_exts {
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.trim_start_matches('.').to_ascii_lowercase());
            let keep = match ext {
                Some(ref e) => exts.contains(e),
                None => exts.contains(""),
            };
            if !keep {
                continue;
            }
        }
        paths.push(path.to_path_buf());
    }
    paths
}

pub fn get_encoder(encoding: &str) -> Result<CoreBPE> {
    match encoding {
        // Common encodings
        "cl100k_base" => tiktoken_rs::cl100k_base().context("Failed to init cl100k_base"),
        "o200k_base" => tiktoken_rs::o200k_base().context("Failed to init o200k_base"),
        "p50k_base" => tiktoken_rs::p50k_base().context("Failed to init p50k_base"),
        "p50k_edit" => tiktoken_rs::p50k_edit().context("Failed to init p50k_edit"),
        "r50k_base" => tiktoken_rs::r50k_base().context("Failed to init r50k_base"),
        other => anyhow::bail!("Unsupported encoding: {other}"),
    }
}

/// Count tokens in a string with a fast path and a timeout fallback.
pub fn count_tokens_in_text(encoder: &CoreBPE, text: &str) -> usize {
    const CHUNK_SIZE: usize = 512; // bytes

    // Quick exit for trivial cases
    if text.is_empty() {
        return 0;
    }

    // For short inputs or when we couldn't split, just do a blocking encode
    if text.len() <= CHUNK_SIZE * 4 {
        return encoder.encode_with_special_tokens(text).len();
    }

    // Split into chunks to avoid some edge cases that can make the progrom super slow
    // Chunk the input and recurse in parallel (without further timeouts)
    let chunks = split_text_into_chunks(text, CHUNK_SIZE);
    if chunks.len() <= 1 {
        return encoder.encode_with_special_tokens(text).len();
    }
    chunks
        .par_iter()
        .map(|s| encoder.encode_with_special_tokens(s).len())
        .sum()
}

fn split_text_into_chunks<'a>(text: &'a str, max_chunk_bytes: usize) -> Vec<&'a str> {
    debug_assert!(max_chunk_bytes > 0);
    let mut chunks: Vec<&'a str> = Vec::new();
    let mut start = 0usize;
    let len = text.len();

    while start < len {
        let remaining = len - start;
        if remaining <= max_chunk_bytes {
            chunks.push(&text[start..]);
            break;
        }

        // 1) Ensure at least `max_chunk_bytes` bytes in this chunk
        let mut base_rel = 0usize; // end boundary relative to `start`
        let mut acc = 0usize;
        for (off, ch) in text[start..].char_indices() {
            let w = ch.len_utf8();
            acc += w;
            base_rel = off + w; // boundary after this char
            if acc >= max_chunk_bytes {
                break;
            }
        }
        let base_end = start + base_rel;

        // 2) Look ahead up to another `max_chunk_bytes` bytes for a nice split
        let mut extended_end = None;
        let mut la_bytes = 0usize;
        for (off, ch) in text[base_end..].char_indices() {
            let w = ch.len_utf8();
            if ch == ' ' || ch == '\n' {
                extended_end = Some(base_end + off + w); // split after the whitespace
                break;
            }
            la_bytes += w;
            if la_bytes >= max_chunk_bytes {
                break;
            }
        }
        let end = extended_end.unwrap_or(base_end);
        chunks.push(&text[start..end]);
        start = end;
    }

    chunks
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
    let ans = match ext.as_str() {
        "abap" => "ABAP",
        "ac" => "m4",
        "ada" => "Ada",
        "adb" => "Ada",
        "ads" => "Ada",
        "adso" => "ADSO/IDSM",
        "ahkl" => "AutoHotkey",
        "ahk" => "AutoHotkey",
        "agda" => "Agda",
        "lagda" => "Agda",
        "aj" => "AspectJ",
        "am" => "make",
        "ample" => "AMPLE",
        "apl" => "APL",
        "apla" => "APL",
        "aplf" => "APL",
        "aplo" => "APL",
        "apln" => "APL",
        "aplc" => "APL",
        "apli" => "APL",
        "applescript" => "AppleScript",
        "dyalog" => "APL",
        "dyapp" => "APL",
        "mipage" => "APL",
        "art" => "Arturo",
        "as" => "ActionScript",
        "adoc" => "AsciiDoc",
        "asciidoc" => "AsciiDoc",
        "dofile" => "AMPLE",
        "startup" => "AMPLE",
        "aria" => "Aria",
        "axd" => "ASP",
        "ashx" => "ASP",
        "asa" => "ASP",
        "asax" => "ASP.NET",
        "ascx" => "ASP.NET",
        "asd" => "Lisp",
        "asmx" => "ASP.NET",
        "asp" => "ASP",
        "aspx" => "ASP.NET",
        "master" => "ASP.NET",
        "sitemap" => "ASP.NET",
        "nasm" => "Assembly",
        "a51" => "Assembly",
        "asm" => "Assembly",
        "astro" => "Astro",
        "asy" => "Asymptote",
        "cshtml" => "Razor",
        "razor" => "Razor",
        "nawk" => "awk",
        "mawk" => "awk",
        "gawk" => "awk",
        "auk" => "awk",
        "awk" => "awk",
        "bash" => "Bourne Again Shell",
        "bazel" => "Starlark",
        "BUILD" => "Bazel",
        "dxl" => "DOORS Extension Language",
        "bat" => "DOS Batch",
        "BAT" => "DOS Batch",
        "cmd" => "DOS Batch",
        "CMD" => "DOS Batch",
        "btm" => "DOS Batch",
        "BTM" => "DOS Batch",
        "blade" => "Blade",
        "blade.php" => "Blade",
        "build.xml" => "Ant",
        "b" => "Brainfuck",
        "bf" => "Brainfuck",
        "blp" => "Blueprint",
        "brs" => "BrightScript",
        "bzl" => "Starlark",
        "btp" => "BizTalk Pipeline",
        "odx" => "BizTalk Orchestration",
        "carbon" => "Carbon",
        "cpy" => "COBOL",
        "cobol" => "COBOL",
        "ccp" => "COBOL",
        "cbl" => "COBOL",
        "CBL" => "COBOL",
        "idc" => "C",
        "cats" => "C",
        "c" => "C",
        "c++" => "C++",
        "C" => "C++",
        "cc" => "C++",
        "ccm" => "C++",
        "c++m" => "C++",
        "cppm" => "C++",
        "cxxm" => "C++",
        "h++" => "C++",
        "inl" => "C++",
        "ipp" => "C++",
        "ixx" => "C++",
        "tcc" => "C++",
        "tpp" => "C++",
        "cdc" => "Cadence",
        "ccs" => "CCS",
        "civet" => "Civet",
        "cvt" => "Civet",
        "cvtx" => "Civet",
        "cfc" => "ColdFusion CFScript",
        "cfml" => "ColdFusion",
        "cfm" => "ColdFusion",
        "chpl" => "Chapel",
        "cl" => "Lisp/OpenCL",
        "riemann.config" => "Clojure",
        "hic" => "Clojure",
        "cljx" => "Clojure",
        "cljscm" => "Clojure",
        "cljs.hl" => "Clojure",
        "cl2" => "Clojure",
        "boot" => "Clojure",
        "cj" => "Clojure/Cangjie",
        "clj" => "Clojure",
        "cljs" => "ClojureScript",
        "cljc" => "ClojureC",
        "cls" => "Visual Basic/TeX/Apex Class",
        "cmake.in" => "CMake",
        "CMakeLists.txt" => "CMake",
        "cmake" => "CMake",
        "cob" => "COBOL",
        "COB" => "COBOL",
        "cocoa5" => "CoCoA 5",
        "c5" => "CoCoA 5",
        "cpkg5" => "CoCoA 5",
        "cocoa5server" => "CoCoA 5",
        "iced" => "CoffeeScript",
        "cjsx" => "CoffeeScript",
        "cakefile" => "CoffeeScript",
        "_coffee" => "CoffeeScript",
        "coffee" => "CoffeeScript",
        "component" => "Visualforce Component",
        "cg3" => "Constraint Grammar",
        "rlx" => "Constraint Grammar",
        "Containerfile" => "Containerfile",
        "cpp" => "C++",
        "CPP" => "C++",
        "cr" => "Crystal",
        "cs" => "C#/Smalltalk",
        "designer.cs" => "C# Designer",
        "cake" => "Cake Build Script",
        "csh" => "C Shell",
        "cson" => "CSON",
        "css" => "CSS",
        "csv" => "CSV",
        "cu" => "CUDA",
        "cuh" => "CUDA",
        "cxx" => "C++",
        "d" => "D/dtrace",
        "dfy" => "Dafny",
        "da" => "DAL",
        "dart" => "Dart",
        "dsc" => "DenizenScript",
        "derw" => "Derw",
        "def" => "Windows Module Definition",
        "dhall" => "dhall",
        "dt" => "DIET",
        "patch" => "diff",
        "diff" => "diff",
        "dmap" => "NASTRAN DMAP",
        "sthlp" => "Stata",
        "matah" => "Stata",
        "mata" => "Stata",
        "ihlp" => "Stata",
        "doh" => "Stata",
        "ado" => "Stata",
        "do" => "Stata",
        "DO" => "Stata",
        "Dockerfile" => "Dockerfile",
        "dockerfile" => "Dockerfile",
        "pascal" => "Pascal",
        "lpr" => "Pascal",
        "dfm" => "Delphi Form",
        "dpr" => "Pascal",
        "dita" => "DITA",
        "drl" => "Drools",
        "dtd" => "DTD",
        "ec" => "C",
        "ecpp" => "ECPP",
        "eex" => "EEx",
        "el" => "Lisp",
        "elm" => "Elm",
        "exs" => "Elixir Script",
        "ex" => "Elixir",
        "ecr" => "Embedded Crystal",
        "ejs" => "EJS",
        "erb" => "ERB",
        "ERB" => "ERB",
        "ets" => "ArkTs",
        "yrl" => "Erlang",
        "xrl" => "Erlang",
        "rebar.lock" => "Erlang",
        "rebar.config.lock" => "Erlang",
        "rebar.config" => "Erlang",
        "emakefile" => "Erlang",
        "app.src" => "Erlang",
        "erl" => "Erlang",
        "exp" => "Expect",
        "4th" => "Forth",
        "fish" => "Fish Shell",
        "fsl" => "Finite State Language",
        "jssm" => "Finite State Language",
        "fnl" => "Fennel",
        "forth" => "Forth",
        "fr" => "Forth",
        "frt" => "Forth",
        "fth" => "Forth",
        "f83" => "Forth",
        "fb" => "Forth",
        "fpm" => "Forth",
        "e4" => "Forth",
        "rx" => "Forth",
        "ft" => "Forth",
        "f77" => "Fortran 77",
        "F77" => "Fortran 77",
        "f90" => "Fortran 90",
        "F90" => "Fortran 90",
        "f95" => "Fortran 95",
        "F95" => "Fortran 95",
        "f" => "Fortran 77/Forth",
        "F" => "Fortran 77",
        "for" => "Fortran 77/Forth",
        "FOR" => "Fortran 77",
        "ftl" => "Freemarker Template",
        "ftn" => "Fortran 77",
        "FTN" => "Fortran 77",
        "f03" => "Fortran 2003",
        "F03" => "Fortran 2003",
        "fmt" => "Oracle Forms",
        "focexec" => "Focus",
        "fs" => "F#/Forth",
        "fsi" => "F#",
        "fsx" => "F# Script",
        "fut" => "Futhark",
        "fxml" => "FXML",
        "gnumakefile" => "make",
        "Gnumakefile" => "make",
        "gd" => "GDScript",
        "gdshader" => "Godot Shaders",
        "vshader" => "GLSL",
        "vsh" => "GLSL",
        "vrx" => "GLSL",
        "gshader" => "GLSL",
        "glslv" => "GLSL",
        "geo" => "GLSL",
        "fshader" => "GLSL",
        "fsh" => "GLSL",
        "frg" => "GLSL",
        "fp" => "GLSL",
        "fbs" => "Flatbuffers",
        "gjs" => "Glimmer JavaScript",
        "gts" => "Glimmer TypeScript",
        "glsl" => "GLSL",
        "graphqls" => "GraphQL",
        "gql" => "GraphQL",
        "graphql" => "GraphQL",
        "vert" => "GLSL",
        "tesc" => "GLSL",
        "tese" => "GLSL",
        "geom" => "GLSL",
        "feature" => "Cucumber",
        "frag" => "GLSL",
        "comp" => "GLSL",
        "g" => "ANTLR Grammar",
        "g4" => "ANTLR Grammar",
        "gleam" => "Gleam",
        "go" => "Go",
        "ʕ◔ϖ◔ʔ" => "Go",
        "gsp" => "Grails",
        "jenkinsfile" => "Groovy",
        "gvy" => "Groovy",
        "gtpl" => "Groovy",
        "grt" => "Groovy",
        "groovy" => "Groovy",
        "gant" => "Groovy",
        "gradle" => "Gradle",
        "gradle.kts" => "Gradle",
        "h" => "C/C++ Header",
        "H" => "C/C++ Header",
        "hh" => "C/C++ Header",
        "hpp" => "C/C++ Header",
        "hxx" => "C/C++ Header",
        "hb" => "Harbour",
        "hrl" => "Erlang",
        "hsc" => "Haskell",
        "hs" => "Haskell",
        "tfvars" => "HCL",
        "hcl" => "HCL",
        "tf" => "HCL",
        "nomad" => "HCL",
        "hlsli" => "HLSL",
        "fxh" => "HLSL",
        "hlsl" => "HLSL",
        "shader" => "HLSL",
        "cg" => "HLSL",
        "cginc" => "HLSL",
        "haml.deface" => "Haml",
        "haml" => "Haml",
        "handlebars" => "Handlebars",
        "hbs" => "Handlebars",
        "ha" => "Hare",
        "hxsl" => "Haxe",
        "hx" => "Haxe",
        "HC" => "HolyC",
        "hoon" => "Hoon",
        "xht" => "HTML",
        "html.hl" => "HTML",
        "htm" => "HTML",
        "html" => "HTML",
        "heex" => "HTML EEx",
        "i3" => "Modula3",
        "ice" => "Slice",
        "icl" => "Clean",
        "dcl" => "Clean",
        "dlm" => "IDL",
        "idl" => "IDL",
        "idr" => "Idris",
        "lidr" => "Literate Idris",
        "imba" => "Imba",
        "prefs" => "INI",
        "lektorproject" => "INI",
        "buildozer.spec" => "INI",
        "ini" => "INI",
        "editorconfig" => "INI",
        "ism" => "InstallShield",
        "ipl" => "IPL",
        "pro" => "IDL/Qt Project/Prolog/ProGuard",
        "ig" => "Modula3",
        "il" => "SKILL/.NET IL",
        "ils" => "SKILL++",
        "inc" => "PHP/Pascal/Fortran/Pawn",
        "ino" => "Arduino Sketch",
        "ipf" => "Igor Pro",
        "pde" => "Processing",
        "itk" => "Tcl/Tk",
        "java" => "Java",
        "jcl" => "JCL",
        "jl" => "Lisp/Julia",
        "jai" => "Jai",
        "janet" => "Janet",
        "xsjslib" => "JavaScript",
        "xsjs" => "JavaScript",
        "ssjs" => "JavaScript",
        "sjs" => "JavaScript",
        "pac" => "JavaScript",
        "njs" => "JavaScript",
        "mjs" => "JavaScript",
        "cjs" => "JavaScript",
        "jss" => "JavaScript",
        "jsm" => "JavaScript",
        "jsfl" => "JavaScript",
        "jscad" => "JavaScript",
        "jsb" => "JavaScript",
        "jakefile" => "JavaScript",
        "jake" => "JavaScript",
        "bones" => "JavaScript",
        "_js" => "JavaScript",
        "js" => "JavaScript",
        "es6" => "JavaScript",
        "jsf" => "JavaServer Faces",
        "jsx" => "JSX",
        "xhtml" => "XHTML",
        "j2" => "Jinja Template",
        "jinja" => "Jinja Template",
        "jinja2" => "Jinja Template",
        "yyp" => "JSON",
        "webmanifest" => "JSON",
        "webapp" => "JSON",
        "topojson" => "JSON",
        "tfstate.backup" => "JSON",
        "tfstate" => "JSON",
        "mcmod.info" => "JSON",
        "mcmeta" => "JSON",
        "json-tmlanguage" => "JSON",
        "jsonl" => "JSON",
        "har" => "JSON",
        "gltf" => "JSON",
        "geojson" => "JSON",
        "composer.lock" => "JSON",
        "avsc" => "JSON",
        "watchmanconfig" => "JSON",
        "tern-project" => "JSON",
        "tern-config" => "JSON",
        "htmlhintrc" => "JSON",
        "arcconfig" => "JSON",
        "json" => "JSON",
        "json5" => "JSON5",
        "jsonnet" => "Jsonnet",
        "jsp" => "JSP",
        "jspf" => "JSP",
        "junos" => "Juniper Junos",
        "just" => "Justfile",
        "vm" => "Velocity Template Language",
        "kv" => "kvlang",
        "ksc" => "Kermit",
        "ksh" => "Korn Shell",
        "ktm" => "Kotlin",
        "kt" => "Kotlin",
        "kts" => "Kotlin",
        "hlean" => "Lean",
        "lean" => "Lean",
        "lhs" => "Haskell",
        "lex" => "lex",
        "l" => "lex",
        "ld" => "Linker Script",
        "lem" => "Lem",
        "less" => "LESS",
        "lfe" => "LFE",
        "liquid" => "liquid",
        "lsp" => "Lisp",
        "lisp" => "Lisp",
        "ll" => "LLVM IR",
        "lgt" => "Logtalk",
        "logtalk" => "Logtalk",
        "lp" => "AnsProlog",
        "wlua" => "Lua",
        "rbxs" => "Lua",
        "pd_lua" => "Lua",
        "p8" => "Lua",
        "nse" => "Lua",
        "lua" => "Lua",
        "luau" => "Luau",
        "m3" => "Modula3",
        "m4" => "m4",
        "makefile" => "make",
        "Makefile" => "make",
        "mao" => "Mako",
        "mako" => "Mako",
        "workbook" => "Markdown",
        "ronn" => "Markdown",
        "mkdown" => "Markdown",
        "mkdn" => "Markdown",
        "mkd" => "Markdown",
        "mdx" => "Markdown",
        "mdwn" => "Markdown",
        "mdown" => "Markdown",
        "markdown" => "Markdown",
        "contents.lr" => "Markdown",
        "md" => "Markdown",
        "org" => "Org Mode",
        "mc" => "Windows Message File",
        "met" => "Teamcenter met",
        "mg" => "Modula3",
        "mojom" => "Mojom",
        "mojo" => "Mojo",
        "🔥" => "Mojo",
        "mbt" => "MoonBit",
        "mbti" => "MoonBit",
        "mbtx" => "MoonBit",
        "mbty" => "MoonBit",
        "meson.build" => "Meson",
        "metal" => "Metal",
        "mk" => "make",
        "ml4" => "OCaml",
        "eliomi" => "OCaml",
        "eliom" => "OCaml",
        "ml" => "OCaml",
        "mli" => "OCaml",
        "mly" => "OCaml",
        "mll" => "OCaml",
        "m" => "MATLAB/Mathematica/Objective-C/MUMPS/Mercury",
        "mm" => "Objective-C++",
        "msg" => "Gencat NLS",
        "nbp" => "Mathematica",
        "mathematica" => "Mathematica",
        "ma" => "Mathematica",
        "cdf" => "Mathematica",
        "mt" => "Mathematica",
        "wl" => "Mathematica",
        "wlt" => "Mathematica",
        "mo" => "Modelica",
        "mustache" => "Mustache",
        "wdproj" => "MSBuild script",
        "csproj" => "MSBuild script",
        "vcproj" => "MSBuild script",
        "wixproj" => "MSBuild script",
        "btproj" => "MSBuild script",
        "msbuild" => "MSBuild script",
        "sln" => "Visual Studio Solution",
        "mps" => "MUMPS",
        "mth" => "Teamcenter mth",
        "n" => "Nemerle",
        "nlogo" => "NetLogo",
        "nls" => "NetLogo",
        "nf" => "Nextflow",
        "ncl" => "Nickel",
        "nims" => "Nim",
        "nimrod" => "Nim",
        "nimble" => "Nim",
        "nim.cfg" => "Nim",
        "nim" => "Nim",
        "nix" => "Nix",
        "nu" => "Nushell",
        "nuon" => "Nushell Object Notation",
        "nut" => "Squirrel",
        "njk" => "Nunjucks",
        "odin" => "Odin",
        "oscript" => "LiveLink OScript",
        "bod" => "Oracle PL/SQL",
        "bdy" => "Oracle PL/SQL",
        "spc" => "Oracle PL/SQL",
        "fnc" => "Oracle PL/SQL",
        "prc" => "Oracle PL/SQL",
        "trg" => "Oracle PL/SQL",
        "p" => "Pascal/Pawn",
        "pad" => "Ada",
        "page" => "Visualforce Page",
        "pas" => "Pascal",
        "pcc" => "C++",
        "rexfile" => "Perl",
        "psgi" => "Perl",
        "ph" => "Perl",
        "makefile.pl" => "Perl",
        "cpanfile" => "Perl",
        "al" => "Perl",
        "ack" => "Perl",
        "perl" => "Perl",
        "pfo" => "Fortran 77",
        "pgc" => "C",
        "phpt" => "PHP",
        "phps" => "PHP",
        "phakefile" => "PHP",
        "ctp" => "PHP",
        "aw" => "PHP",
        "php_cs.dist" => "PHP",
        "php_cs" => "PHP",
        "php3" => "PHP",
        "php4" => "PHP",
        "php5" => "PHP",
        "php" => "PHP",
        "phtml" => "PHP",
        "pig" => "Pig Latin",
        "plh" => "Perl",
        "pl" => "Perl/Prolog",
        "PL" => "Perl/Prolog",
        "p6" => "Raku/Prolog",
        "P6" => "Raku/Prolog",
        "plx" => "Perl",
        "pm" => "Perl",
        "pm6" => "Raku",
        "raku" => "Raku",
        "rakumod" => "Raku",
        "pom.xml" => "Maven",
        "pom" => "Maven",
        "scad" => "OpenSCAD",
        "yap" => "Prolog",
        "prolog" => "Prolog",
        "P" => "Prolog",
        "pp" => "Pascal/Puppet",
        "viw" => "SQL",
        "udf" => "SQL",
        "tab" => "SQL",
        "mysql" => "SQL",
        "cql" => "SQL",
        "psql" => "SQL",
        "xpy" => "Python",
        "wsgi" => "Python",
        "wscript" => "Python",
        "workspace" => "Python",
        "tac" => "Python",
        "snakefile" => "Python",
        "sconstruct" => "Python",
        "sconscript" => "Python",
        "pyt" => "Python",
        "pyp" => "Python",
        "pyi" => "Python",
        "pyde" => "Python",
        "py3" => "Python",
        "lmi" => "Python",
        "gypi" => "Python",
        "gyp" => "Python",
        "build.bazel" => "Python",
        "buck" => "Python",
        "gclient" => "Python",
        "py" => "Python",
        "pyw" => "Python",
        "ipynb" => "Jupyter Notebook",
        "pyj" => "RapydScript",
        "pxi" => "Cython",
        "pxd" => "Cython",
        "pyx" => "Cython",
        "qbs" => "QML",
        "qml" => "QML",
        "watchr" => "Ruby",
        "vagrantfile" => "Ruby",
        "thorfile" => "Ruby",
        "thor" => "Ruby",
        "snapfile" => "Ruby",
        "ru" => "Ruby",
        "rbx" => "Ruby",
        "rbw" => "Ruby",
        "rbuild" => "Ruby",
        "rabl" => "Ruby",
        "puppetfile" => "Ruby",
        "podfile" => "Ruby",
        "mspec" => "Ruby",
        "mavenfile" => "Ruby",
        "jbuilder" => "Ruby",
        "jarfile" => "Ruby",
        "guardfile" => "Ruby",
        "god" => "Ruby",
        "gemspec" => "Ruby",
        "gemfile.lock" => "Ruby",
        "gemfile" => "Ruby",
        "fastfile" => "Ruby",
        "eye" => "Ruby",
        "deliverfile" => "Ruby",
        "dangerfile" => "Ruby",
        "capfile" => "Ruby",
        "buildfile" => "Ruby",
        "builder" => "Ruby",
        "brewfile" => "Ruby",
        "berksfile" => "Ruby",
        "appraisals" => "Ruby",
        "pryrc" => "Ruby",
        "irbrc" => "Ruby",
        "rb" => "Ruby",
        "podspec" => "Ruby",
        "rake" => "Ruby",

        "rex" => "Oracle Reports",
        "pprx" => "Rexx",
        "rexx" => "Rexx",
        "rhtml" => "Ruby HTML",
        "circom" => "Circom",
        "cairo" => "Cairo",
        "rs.in" => "Rust",
        "rs" => "Rust",
        "rst.txt" => "reStructuredText",
        "rest.txt" => "reStructuredText",
        "rest" => "reStructuredText",
        "rst" => "reStructuredText",
        "s" => "Assembly",
        "S" => "Assembly",
        "SCA" => "Visual Fox Pro",
        "sca" => "Visual Fox Pro",
        "sbt" => "Scala",
        "kojo" => "Scala",
        "scala" => "Scala",
        "sbl" => "Softbridge Basic",
        "SBL" => "Softbridge Basic",
        "sed" => "sed",
        "sp" => "SparForte",
        "sol" => "Solidity",
        "p4" => "P4",
        "ses" => "Patran Command Language",
        "pcl" => "Patran Command Language",
        "pwn" => "Pawn",
        "pawn" => "Pawn",
        "pek" => "Pek",
        "peg" => "PEG",
        "pegjs" => "peg.js",
        "peggy" => "peggy",
        "pest" => "Pest",
        "pkl" => "Pkl",
        "prisma" => "Prisma Schema",
        "tspeg" => "tspeg",
        "jspeg" => "tspeg",
        "pl1" => "PL/I",
        "plm" => "PL/M",
        "lit" => "PL/M",
        "iuml" => "PlantUML",
        "pu" => "PlantUML",
        "puml" => "PlantUML",
        "plantuml" => "PlantUML",
        "wsd" => "PlantUML",
        "properties" => "Properties",
        "po" => "PO File",
        "pony" => "Pony",
        "pbt" => "PowerBuilder",
        "sra" => "PowerBuilder",
        "srf" => "PowerBuilder",
        "srm" => "PowerBuilder",
        "srs" => "PowerBuilder",
        "sru" => "PowerBuilder",
        "srw" => "PowerBuilder",
        "jade" => "Pug",
        "pug" => "Pug",
        "purs" => "PureScript",
        "prefab" => "Unity-Prefab",
        "proto" => "Protocol Buffers",
        "mat" => "Unity-Prefab",
        "ps1" => "PowerShell",
        "psd1" => "PowerShell",
        "psm1" => "PowerShell",
        "prql" => "PRQL",
        "rsx" => "R",
        "rd" => "R",
        "expr-dist" => "R",
        "rprofile" => "R",
        "R" => "R",
        "r" => "R",
        "raml" => "RAML",
        "ring" => "Ring",
        "rh" => "Ring",
        "rform" => "Ring",
        "rktd" => "Racket",
        "rkt" => "Racket",
        "rktl" => "Racket",
        "Rmd" => "Rmd",
        "re" => "ReasonML",
        "rei" => "ReasonML",
        "res" => "ReScript",
        "resi" => "ReScript",
        "scrbl" => "Racket",
        "sps" => "Scheme",
        "sc" => "Scheme",
        "ss" => "Scheme",
        "scm" => "Scheme",
        "sch" => "Scheme",
        "sls" => "Scheme/SaltStack",
        "sld" => "Scheme",
        "robot" => "RobotFramework",
        "rc" => "Windows Resource File",
        "rc2" => "Windows Resource File",
        "sas" => "SAS",
        "sass" => "Sass",
        "scss" => "SCSS",
        "sh" => "Bourne Shell",
        "smarty" => "Smarty",
        "sml" => "Standard ML",
        "sig" => "Standard ML",
        "fun" => "Standard ML",
        "slim" => "Slim",
        "e" => "Specman e",
        "sql" => "SQL",
        "SQL" => "SQL",
        "sproc.sql" => "SQL Stored Procedure",
        "spoc.sql" => "SQL Stored Procedure",
        "spc.sql" => "SQL Stored Procedure",
        "udf.sql" => "SQL Stored Procedure",
        "data.sql" => "SQL Data",
        "sss" => "SugarSS",
        "slint" => "Slint",
        "st" => "Smalltalk",
        "rules" => "Snakemake",
        "smk" => "Snakemake",
        "styl" => "Stylus",
        "surql" => "SurrealQL",
        "i" => "SWIG",
        "svelte" => "Svelte",
        "sv" => "Verilog-SystemVerilog",
        "svh" => "Verilog-SystemVerilog",
        "svg" => "SVG",
        "SVG" => "SVG",
        "v" => "Verilog-SystemVerilog/Coq",
        "td" => "TableGen",
        "tcl" => "Tcl/Tk",
        "tcsh" => "C Shell",
        "tk" => "Tcl/Tk",
        "teal" => "TEAL",
        "templ" => "Templ",
        "mkvi" => "TeX",
        "mkiv" => "TeX",
        "mkii" => "TeX",
        "ltx" => "TeX",
        "lbx" => "TeX",
        "ins" => "TeX",
        "cbx" => "TeX",
        "bib" => "TeX",
        "bbx" => "TeX",
        "aux" => "TeX",
        "tex" => "TeX",
        "toml" => "TOML",
        "sty" => "TeX",

        "dtx" => "TeX",
        "bst" => "TeX",
        "txt" => "Text",
        "text" => "Text",
        "tres" => "Godot Resource",
        "tscn" => "Godot Scene",
        "thrift" => "Thrift",
        "tla" => "TLA+",
        "tpl" => "Smarty",
        "trigger" => "Apex Trigger",
        "ttcn" => "TTCN",
        "ttcn2" => "TTCN",
        "ttcn3" => "TTCN",
        "ttcnpp" => "TTCN",
        "sdl" => "TNSDL",
        "ssc" => "TNSDL",
        "sdt" => "TNSDL",
        "spd" => "TNSDL",
        "sst" => "TNSDL",
        "rou" => "TNSDL",
        "cin" => "TNSDL",
        "cii" => "TNSDL",
        "interface" => "TNSDL",
        "in1" => "TNSDL",
        "in2" => "TNSDL",
        "in3" => "TNSDL",
        "in4" => "TNSDL",
        "inf" => "TNSDL",
        "tpd" => "TITAN Project File Information",
        "ts" => "TypeScript/Qt Linguist",
        "cts" => "TypeScript",
        "mts" => "TypeScript",
        "tsx" => "TypeScript",
        "tss" => "Titanium Style Sheet",
        "twig" => "Twig",
        "typ" => "Typst",
        "um" => "Umka",
        "uss" => "USS",
        "uxml" => "UXML",
        "ui" => "XML-Qt-GTK/Glade",
        "glade" => "Glade",
        "vala" => "Vala",
        "vapi" => "Vala Header",
        "vhw" => "VHDL",
        "vht" => "VHDL",
        "vhs" => "VHDL",
        "vho" => "VHDL",
        "vhi" => "VHDL",
        "vhf" => "VHDL",
        "vhd" => "VHDL",
        "VHD" => "VHDL",
        "vhdl" => "VHDL",
        "VHDL" => "VHDL",
        "bas" => "Visual Basic",
        "BAS" => "Visual Basic",
        "ctl" => "Visual Basic",
        "dsr" => "Visual Basic",
        "frm" => "Visual Basic",
        "frx" => "Visual Basic",
        "FRX" => "Visual Basic",
        "vba" => "VB for Applications",
        "VBA" => "VB for Applications",
        "vbhtml" => "Visual Basic",
        "VBHTML" => "Visual Basic",
        "vbproj" => "Visual Basic .NET",
        "vbp" => "Visual Basic",
        "vbs" => "Visual Basic Script",
        "VBS" => "Visual Basic Script",
        "vb" => "Visual Basic .NET",
        "VB" => "Visual Basic .NET",
        "vbw" => "Visual Basic",
        "vue" => "Vuejs Component",
        "vy" => "Vyper",
        "webinfo" => "ASP.NET",
        "wsdl" => "Web Services Description",
        "x" => "Logos",
        "xm" => "Logos",
        "xpo" => "X++",
        "xmi" => "XMI",
        "XMI" => "XMI",
        "zcml" => "XML",
        "xul" => "XML",
        "xspec" => "XML",
        "xproj" => "XML",
        "xml.dist" => "XML",
        "xliff" => "XML",
        "xlf" => "XML",
        "xib" => "XML",
        "xacro" => "XML",
        "x3d" => "XML",
        "wsf" => "XML",
        "web.release.config" => "XML",
        "web.debug.config" => "XML",
        "web.config" => "XML",
        "wxml" => "WXML",
        "wxss" => "WXSS",
        "vxml" => "XML",
        "vstemplate" => "XML",
        "vssettings" => "XML",
        "vsixmanifest" => "XML",
        "vcxproj" => "XML",
        "ux" => "XML",
        "urdf" => "XML",
        "tmtheme" => "XML",
        "tmsnippet" => "XML",
        "tmpreferences" => "XML",
        "tmlanguage" => "XML",
        "tml" => "XML",
        "tmcommand" => "XML",
        "targets" => "XML",
        "sublime-snippet" => "XML",
        "sttheme" => "XML",
        "storyboard" => "XML",
        "srdf" => "XML",
        "shproj" => "XML",
        "sfproj" => "XML",
        "settings.stylecop" => "XML",
        "scxml" => "XML",
        "rss" => "XML",
        "resx" => "XML",
        "rdf" => "XML",
        "pt" => "XML",
        "psc1" => "XML",
        "ps1xml" => "XML",
        "props" => "XML",
        "proj" => "XML",
        "plist" => "XML",
        "pkgproj" => "XML",
        "packages.config" => "XML",
        "osm" => "XML",
        "odd" => "XML",
        "nuspec" => "XML",
        "nuget.config" => "XML",
        "nproj" => "XML",
        "ndproj" => "XML",
        "natvis" => "XML",
        "mjml" => "XML",
        "mdpolicy" => "XML",
        "launch" => "XML",
        "kml" => "XML",
        "jsproj" => "XML",
        "jelly" => "XML",
        "ivy" => "XML",
        "iml" => "XML",
        "grxml" => "XML",
        "gmx" => "XML",
        "fsproj" => "XML",
        "filters" => "XML",
        "dotsettings" => "XML",
        "dll.config" => "XML",
        "ditaval" => "XML",
        "ditamap" => "XML",
        "depproj" => "XML",
        "ct" => "XML",
        "csl" => "XML",
        "csdef" => "XML",
        "cscfg" => "XML",
        "cproject" => "XML",
        "clixml" => "XML",
        "ccxml" => "XML",
        "ccproj" => "XML",
        "builds" => "XML",
        "axml" => "XML",
        "app.config" => "XML",
        "ant" => "XML",
        "admx" => "XML",
        "adml" => "XML",
        "project" => "XML",
        "classpath" => "XML",
        "xml" => "XML",
        "XML" => "XML",
        "mxml" => "MXML",
        "xml.builder" => "builder",
        "build" => "NAnt script",
        "vim" => "vim script",
        "swift" => "Swift",
        "xaml" => "XAML",
        "wast" => "WebAssembly",
        "wat" => "WebAssembly",
        "wgsl" => "WGSL",
        "wxs" => "WiX source",
        "wxi" => "WiX include",
        "wxl" => "WiX string localization",
        "prw" => "xBase",
        "prg" => "xBase",
        "ch" => "xBase Header",
        "xqy" => "XQuery",
        "xqm" => "XQuery",
        "xql" => "XQuery",
        "xq" => "XQuery",
        "xquery" => "XQuery",
        "xsd" => "XSD",
        "XSD" => "XSD",
        "xslt" => "XSLT",
        "XSLT" => "XSLT",
        "xsl" => "XSLT",
        "XSL" => "XSLT",
        "xtend" => "Xtend",
        "yacc" => "yacc",
        "y" => "yacc",
        "yml.mysql" => "YAML",
        "yaml-tmlanguage" => "YAML",
        "syntax" => "YAML",
        "sublime-syntax" => "YAML",
        "rviz" => "YAML",
        "reek" => "YAML",
        "mir" => "YAML",
        "glide.lock" => "YAML",
        "gemrc" => "YAML",
        "clang-tidy" => "YAML",
        "clang-format" => "YAML",
        "yaml" => "YAML",
        "yml" => "YAML",
        "yang" => "Yang",
        "yarn" => "Yarn",
        "zig" => "Zig",
        "zsh" => "zsh",
        "rego" => "Rego",
        _ => "Others",
    };
    let ans = if ans.contains('/') {
        ans.split('/').next().unwrap_or(ans)
    } else {
        ans
    };
    ans.to_string()
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
    count_tokens_in_path_with_progress::<P, fn(usize, usize)>(root, opts, None)
}

/// Like `count_tokens_in_path`, but reports progress via the provided callback.
/// The callback receives `(processed_files, total_files)`.
pub fn count_tokens_in_path_with_progress<P, F>(
    root: P,
    opts: &Options,
    progress: Option<&F>,
) -> Result<CountResult>
where
    P: AsRef<Path>,
    F: Fn(usize, usize) + Send + Sync,
{
    // Validate encoder early; per-thread encoders will be created below
    let _ = get_encoder(&opts.encoding)?;

    // Collect file paths first (sequential, cheap), then process in parallel
    let paths: Vec<PathBuf> = enumerate_filtered_paths(&root, opts);
    let total_files = paths.len();
    if let Some(cb) = progress {
        cb(0, total_files);
    }

    let processed = AtomicUsize::new(0);
    let encoding = opts.encoding.clone();

    // Simple encoder pool backed by a mutex-protected stack.
    struct EncoderPool {
        encoding: String,
        inners: Mutex<Vec<CoreBPE>>,
    }
    impl EncoderPool {
        fn new(encoding: String) -> Self {
            Self {
                encoding,
                inners: Mutex::new(Vec::new()),
            }
        }
        fn take(&self) -> CoreBPE {
            if let Some(enc) = self.inners.lock().unwrap().pop() {
                return enc;
            }
            // Create a new one if pool empty
            get_encoder(&self.encoding).expect("Failed to init encoder")
        }
        fn give(&self, enc: CoreBPE) {
            self.inners.lock().unwrap().push(enc);
        }
    }

    let pool = Arc::new(EncoderPool::new(encoding.clone()));

    let files: Vec<FileCount> = paths
        .par_iter()
        .filter_map(|path| {
            // Skip files larger than 100MB
            let metadata = match fs::metadata(path) {
                Ok(m) => m,
                Err(err) => {
                    eprintln!("warn: failed to get metadata for {}: {err}", path.display());
                    return None;
                }
            };
            if metadata.len() > 64 * 1024 * 1024 {
                eprintln!(
                    "warn: skipping large file ({}MB): {}",
                    metadata.len() / 1024 / 1024,
                    path.display()
                );
                return None;
            }
            let bytes = match fs::read(path) {
                Ok(b) => b,
                Err(err) => {
                    eprintln!("warn: failed to read {}: {err}", path.display());
                    return None;
                }
            };
            let byte_len = bytes.len();
            let Ok(text) = String::from_utf8(bytes) else {
                return None;
            };

            let enc = pool.take();
            let tokens = count_tokens_in_text(&enc, &text);
            pool.give(enc);
            let lines = count_non_empty_lines(&text);

            let res = Some(FileCount {
                path: path.clone(),
                tokens,
                lines,
            });
            let done = processed.fetch_add(1, Ordering::Relaxed) + 1;
            if let Some(cb) = progress {
                cb(done, total_files);
            }
            res
        })
        .collect();

    let total: usize = files.iter().map(|f| f.tokens).sum();

    Ok(CountResult { total, files })
}

/// Step 1: Extract filtered relative file paths and their UTF-8 content.
/// Returns `(relative_path, content)` for each file, sorted by path.
pub fn collect_filtered_texts<P: AsRef<Path>>(
    root: P,
    opts: &Options,
) -> Result<Vec<(PathBuf, String)>> {
    let root_ref = root.as_ref();
    let mut rel_and_text: Vec<(PathBuf, String)> = Vec::new();
    let mut paths = enumerate_filtered_paths(root_ref, opts);
    // Sort by relative path for deterministic output
    paths.sort();
    for abs in paths {
        let rel = abs.strip_prefix(root_ref).unwrap_or(&abs).to_path_buf();
        let Ok(bytes) = fs::read(&abs) else { continue };
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        rel_and_text.push((rel, text));
    }
    Ok(rel_and_text)
}

/// Step 2: Build final output from relative paths and content collected in step 1.
/// Format:
///  - file tree header using ├──/└── and │/    guides
///  - blank line
///  - sections per file: `/<path>:` + dashed line + numbered content lines
pub fn build_copy_output(_root: &Path, rel_and_texts: &[(PathBuf, String)]) -> String {
    use std::collections::BTreeMap;
    use std::fmt::Write as _;

    // Normalize path to unix-style with '/'
    fn path_to_unix_string(p: &Path) -> String {
        p.components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("/")
    }

    #[derive(Default)]
    struct DirNode {
        dirs: BTreeMap<String, DirNode>,
        files: Vec<String>,
    }

    let mut root_node = DirNode::default();
    let mut rel_paths: Vec<PathBuf> = rel_and_texts.iter().map(|(p, _)| p.clone()).collect();
    rel_paths.sort();
    for rel in &rel_paths {
        let mut cur = &mut root_node;
        let mut comps = rel.components().peekable();
        while let Some(comp) = comps.next() {
            let name = comp.as_os_str().to_string_lossy().to_string();
            let is_last = comps.peek().is_none();
            if is_last {
                cur.files.push(name);
            } else {
                cur = cur.dirs.entry(name).or_default();
            }
        }
    }

    fn render_dir(node: &DirNode, prefix: &str, out: &mut String) {
        // Order: directories first, then files; both lexicographically
        let mut dir_names: Vec<_> = node.dirs.keys().cloned().collect();
        dir_names.sort();
        let mut file_names = node.files.clone();
        file_names.sort();

        enum Entry {
            Dir(String),
            File(String),
        }
        let mut entries: Vec<Entry> = Vec::new();
        for d in &dir_names {
            entries.push(Entry::Dir(d.clone()));
        }
        for f in &file_names {
            entries.push(Entry::File(f.clone()));
        }
        let len = entries.len();
        for (idx, e) in entries.into_iter().enumerate() {
            let last = idx + 1 == len;
            let (branch, next_prefix) = if last {
                ("└── ", format!("{}    ", prefix))
            } else {
                ("├── ", format!("{}│   ", prefix))
            };
            match e {
                Entry::Dir(name) => {
                    let _ = writeln!(out, "{}{}{}", prefix, branch, name);
                    if let Some(child) = node.dirs.get(&name) {
                        render_dir(child, &next_prefix, out);
                    }
                }
                Entry::File(name) => {
                    let _ = writeln!(out, "{}{}{}", prefix, branch, name);
                }
            }
        }
    }

    let mut s = String::new();
    render_dir(&root_node, "", &mut s);
    if !s.is_empty() {
        s.push_str("\n");
    }

    for (rel, text) in rel_and_texts {
        let path_unix = path_to_unix_string(rel);
        s.push_str(
            "--------------------------------------------------------------------------------\n",
        );
        let _ = writeln!(s, "/{}:", path_unix);
        s.push_str(
            "--------------------------------------------------------------------------------\n",
        );
        for (i, line) in text.lines().enumerate() {
            if line.is_empty() {
                let _ = writeln!(s, "{} |", i + 1);
            } else {
                let _ = writeln!(s, "{} | {}", i + 1, line);
            }
        }
        s.push_str("\n\n");
    }

    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_kind() {
        let filename = "hello.ts";
        let lang = language_from_path(Path::new(filename));
        assert_eq!(lang, "TypeScript");
        let filename = "hello.rs";
        let lang = language_from_path(Path::new(filename));
        assert_eq!(lang, "Rust");
    }

    #[test]
    fn test_build_copy_output() {
        // Given relative paths and content
        let inputs = vec![
            (PathBuf::from("a.txt"), "line1\n\nline2".to_string()),
            (PathBuf::from("dir/b.txt"), "x\ny".to_string()),
            (PathBuf::from("dir/sub/c.rs"), "fn main() {}\n".to_string()),
        ];
        let out = build_copy_output(Path::new("."), &inputs);

        let expected = "\
├── dir
│   ├── sub
│   │   └── c.rs
│   └── b.txt
└── a.txt

--------------------------------------------------------------------------------
/a.txt:
--------------------------------------------------------------------------------
1 | line1
2 |
3 | line2


--------------------------------------------------------------------------------
/dir/b.txt:
--------------------------------------------------------------------------------
1 | x
2 | y


--------------------------------------------------------------------------------
/dir/sub/c.rs:
--------------------------------------------------------------------------------
1 | fn main() {}


";

        assert_eq!(out, expected);
    }
}
