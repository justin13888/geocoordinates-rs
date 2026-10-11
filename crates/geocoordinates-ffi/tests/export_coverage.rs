//! Guards that every `#[uniffi::export]` function is called by at least one
//! Rust test in this directory, so a new export cannot land untested.
//!
//! The scan is textual: it reads every `.rs` file under `src/`, takes the
//! function that follows each `#[uniffi::export]` attribute, and requires a
//! call `name(` in some other test file here.

use std::fs;
use std::path::{Path, PathBuf};

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("readable directory") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Names of the functions annotated `#[uniffi::export]` in `source`.
fn exported_functions(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut pending = false;
    for line in source.lines() {
        let line = line.trim();
        if line == "#[uniffi::export]" {
            pending = true;
        } else if pending && let Some(rest) = line.strip_prefix("pub fn ") {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            names.push(name);
            pending = false;
        }
    }
    names
}

#[test]
fn every_export_is_called_by_a_rust_test() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut sources = Vec::new();
    rust_files(&root.join("src"), &mut sources);
    let exports: Vec<String> = sources
        .iter()
        .flat_map(|path| exported_functions(&fs::read_to_string(path).unwrap()))
        .collect();
    // A sanity floor, so a broken scan cannot pass vacuously.
    assert!(exports.len() >= 100, "found only {} exports", exports.len());

    let this_file = Path::new(file!()).file_name().unwrap();
    let mut tests = Vec::new();
    rust_files(&root.join("tests"), &mut tests);
    let test_text: String = tests
        .iter()
        .filter(|path| path.file_name() != Some(this_file))
        .map(|path| fs::read_to_string(path).unwrap())
        .collect();

    let missing: Vec<&String> = exports
        .iter()
        .filter(|name| {
            let call = format!("{name}(");
            // Reject a match that is only the tail of a longer identifier.
            !test_text.match_indices(&call).any(|(i, _)| {
                test_text[..i]
                    .chars()
                    .next_back()
                    .is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
            })
        })
        .collect();
    assert!(missing.is_empty(), "exports with no Rust test: {missing:?}");
}
