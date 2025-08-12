use std::fs;
use std::path::PathBuf;

use loctok::{count_tokens_in_path, count_tokens_in_text, get_encoder, Options};

#[test]
fn counts_tokens_and_respects_gitignore() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");

    let kept_path = root.join("kept.txt");
    let kept2_path = root.join("nested/kept2.txt");

    let kept_bytes = fs::read(&kept_path).expect("read kept");
    let kept2_bytes = fs::read(&kept2_path).expect("read kept2");
    let kept_text = String::from_utf8_lossy(&kept_bytes);
    let kept2_text = String::from_utf8_lossy(&kept2_bytes);

    let encoder = get_encoder("cl100k_base").expect("encoder");
    let expected = count_tokens_in_text(&encoder, &kept_text)
        + count_tokens_in_text(&encoder, &kept2_text);

    let opts = Options::default();
    let result = count_tokens_in_path(&root, &opts).expect("count ok");

    // Ensure ignored entries are not present
    assert!(result
        .files
        .iter()
        .all(|f| !f.path.ends_with("ignored.txt") && !f.path.to_string_lossy().contains("ignored_dir")));

    // Ensure only the two kept files are present
    assert_eq!(result.files.len(), 2);
    assert_eq!(result.total, expected);
}

#[test]
fn ext_filter_includes_only_specified_extensions() {
    use std::collections::HashSet;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");

    // Build expected from the two kept .txt files
    let kept_path = root.join("kept.txt");
    let kept2_path = root.join("nested/kept2.txt");

    let kept_bytes = fs::read(&kept_path).expect("read kept");
    let kept2_bytes = fs::read(&kept2_path).expect("read kept2");
    let kept_text = String::from_utf8_lossy(&kept_bytes);
    let kept2_text = String::from_utf8_lossy(&kept2_bytes);

    let encoder = get_encoder("cl100k_base").expect("encoder");
    let expected = count_tokens_in_text(&encoder, &kept_text)
        + count_tokens_in_text(&encoder, &kept2_text);

    let mut set = HashSet::new();
    set.insert("txt".to_string());
    let opts = Options {
        include_exts: Some(set),
        ..Options::default()
    };
    let result = count_tokens_in_path(&root, &opts).expect("count ok");

    assert_eq!(result.total, expected);
    // Only two kept files should be present
    assert_eq!(result.files.len(), 2);
    assert!(result
        .files
        .iter()
        .all(|f| f.path.ends_with("kept.txt") || f.path.ends_with("kept2.txt")));
}

#[test]
fn ext_filter_excludes_non_matching_extensions() {
    use std::collections::HashSet;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");

    let mut set = HashSet::new();
    set.insert("md".to_string());
    let opts = Options {
        include_exts: Some(set),
        ..Options::default()
    };
    let result = count_tokens_in_path(&root, &opts).expect("count ok");

    assert_eq!(result.total, 0);
    assert_eq!(result.files.len(), 0);
}
