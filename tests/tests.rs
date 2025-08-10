//! Integration tests for `text_analysis`.
//
// Coverage:
// - Library: tokenization, stopwords, stemming (Auto/Force/Off), n-grams,
//   wordfreq, context & neighbors, PMI, NER heuristic, analyze_path (combined/per-file), exporters.
// - CLI: flags (--stopwords, --ngram, --context, --export-format, --entities-only,
//   --combine, --stem, --stem-lang), file creation, exit codes.
//
// Notes:
// - Tests run in isolated temp directories (no pollution).
// - Tests that change the global CWD are marked #[serial] to avoid races.
// - PDF tests are optional; see cfg(feature = "pdf") at bottom.

use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::prelude::*;
use assert_fs::prelude::*;
use predicates::prelude::*;
use serial_test::serial;
use tempfile::tempdir;

use regex::Regex;
use serde_json::Value as Json;

use text_analysis::{
    AnalysisOptions, ExportFormat, StemLang, StemMode, analyze_path, analyze_text_with,
    collect_files,
};

/// Helper: create a file with content.
fn write_file(dir: &assert_fs::TempDir, name: &str, content: &str) -> PathBuf {
    let f = dir.child(name);
    f.write_str(content).unwrap();
    f.path().to_path_buf()
}

/// Helper: read file to string.
fn read_to_string<P: AsRef<Path>>(p: P) -> String {
    fs::read_to_string(p).unwrap()
}

/// Build default options (library)
fn opts(fmt: ExportFormat) -> AnalysisOptions {
    AnalysisOptions {
        ngram: 2,
        context: 5,
        export_format: fmt,
        entities_only: false,
        combine: false,
        stem_mode: StemMode::Off,
    }
}

#[test]
fn lib_tokenize_and_basic_counts() {
    let mut o = opts(ExportFormat::Json);
    o.stem_mode = StemMode::Off;
    let text = "The quick brown fox jumps over the lazy dog. The fox was very quick!";
    let stop = std::collections::HashSet::new();
    let r = analyze_text_with(text, &stop, &o);

    // n-grams present (bigrams)
    assert!(r.ngrams.get("the quick").is_some());
    assert!(r.ngrams.get("quick brown").is_some());

    // wordfreq should include tokens lowercased (stemming off)
    assert!(r.wordfreq.get("the").unwrap() >= &2);
    assert!(r.wordfreq.get("quick").unwrap() >= &2);

    // neighbors/context present for typical word
    assert!(r.context_map.get("fox").is_some());
    assert!(r.direct_neighbors.get("fox").is_some());

    // PMI computed
    assert!(!r.pmi.is_empty());
}

#[test]
fn lib_stopwords_filtering() {
    let mut o = opts(ExportFormat::Json);
    o.stem_mode = StemMode::Off;

    let text = "Cats and dogs and cats and dogs.";
    let mut stop = std::collections::HashSet::new();
    stop.insert("and".to_string());

    let r = analyze_text_with(text, &stop, &o);

    // "and" must be filtered out from statistics
    assert!(r.wordfreq.get("and").is_none());
    assert!(r.ngrams.keys().all(|ng| !ng.contains("and")));
}

#[test]
fn lib_stemming_auto_and_force() {
    // The text strongly indicates English; Auto should map to English stemmer.
    let text = "running runner runs cars car cars running";
    let stop = std::collections::HashSet::new();

    // Auto
    let mut o = opts(ExportFormat::Json);
    o.stem_mode = StemMode::Auto;
    let r_auto = analyze_text_with(text, &stop, &o);
    // English stemming should reduce "running"->"run", "cars"->"car"
    assert!(r_auto.wordfreq.get("run").is_some());
    assert!(r_auto.wordfreq.get("car").is_some());

    // Force German
    let mut o2 = opts(ExportFormat::Json);
    o2.stem_mode = StemMode::Force(StemLang::De);
    let r_force = analyze_text_with(text, &stop, &o2);
    assert!(!r_force.wordfreq.is_empty());

    // Off: no stemming -> raw lowercased
    let mut o3 = opts(ExportFormat::Json);
    o3.stem_mode = StemMode::Off;
    let r_off = analyze_text_with(text, &stop, &o3);
    assert!(r_off.wordfreq.get("running").is_some());
    assert!(r_off.wordfreq.get("cars").is_some());
}

#[test]
fn lib_ngrams_window_and_neighbors() {
    let mut o = opts(ExportFormat::Json);
    o.ngram = 3;
    o.context = 2;
    let text = "alpha beta gamma delta epsilon";
    let stop = std::collections::HashSet::new();

    let r = analyze_text_with(text, &stop, &o);
    // Trigrams count
    assert!(r.ngrams.get("alpha beta gamma").is_some());
    assert!(r.ngrams.get("beta gamma delta").is_some());

    // context window ±2: neighbors for "gamma" must include beta and delta
    let neigh = r.direct_neighbors.get("gamma").unwrap();
    assert!(neigh.get("beta").is_some());
    assert!(neigh.get("delta").is_some());
}

#[test]
fn lib_ner_heuristic() {
    let mut o = opts(ExportFormat::Json);
    let text = "Berlin is in Germany. NASA launched a rocket. The dog sleeps.";
    let stop = std::collections::HashSet::new();
    let r = analyze_text_with(text, &stop, &o);

    // Should count Berlin and Germany (capitalized), but filter all-upper "NASA"
    assert!(r.named_entities.get("Berlin").is_some());
    assert!(r.named_entities.get("Germany").is_some());
    assert!(r.named_entities.get("NASA").is_none());
    // "The" as function word should not be counted
    assert!(r.named_entities.get("The").is_none());
}

#[test]
fn lib_pmi_sanity() {
    let mut o = opts(ExportFormat::Json);
    o.context = 1; // tight window yields strong pairs
    let text = "alice bob alice bob alice bob";
    let stop = std::collections::HashSet::new();
    let r = analyze_text_with(text, &stop, &o);

    // There should be PMI entries for the pair (alice,bob)
    let has_pair = r.pmi.iter().any(|p| {
        (p.word1 == "alice" && p.word2 == "bob") || (p.word1 == "bob" && p.word2 == "alice")
    });
    assert!(has_pair);
}

#[test]
#[serial]
fn lib_analyze_path_per_file_and_combined_csv() {
    // Prepare temp dir with two text files
    let td = assert_fs::TempDir::new().unwrap();
    let _f1 = write_file(&td, "a.txt", "Hello world. Berlin Berlin.");
    let _f2 = write_file(&td, "b.txt", "Hello Alice. Alice meets Bob.");

    // Per-file mode (default), CSV export
    let mut o = opts(ExportFormat::Csv);
    o.combine = false;
    // Change CWD so relative outputs are written into td
    std::env::set_current_dir(td.path()).unwrap();
    let _rep = analyze_path(td.path(), None, &o).expect("analyze_path");

    // Expect output files for at least one stem + one table (wordfreq)
    let re = Regex::new(r".+_\d{8}_\d{6}_wordfreq\.csv$").unwrap();
    let found = fs::read_dir(td.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| re.is_match(e.file_name().to_string_lossy().as_ref()));
    assert!(found, "Expected <stem>_*_wordfreq.csv in temp dir");

    // Combined mode, CSV export
    let mut o2 = opts(ExportFormat::Csv);
    o2.combine = true;
    std::env::set_current_dir(td.path()).unwrap();
    let _rep2 = analyze_path(td.path(), None, &o2).expect("analyze_path combined");

    // combined_* files should exist
    let has_combined = fs::read_dir(td.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| e.file_name().to_string_lossy().starts_with("combined_"));
    assert!(has_combined, "Expected combined_* outputs");
}

#[test]
#[serial]
fn lib_export_json() {
    let td = assert_fs::TempDir::new().unwrap();
    let _f = write_file(&td, "c.txt", "Alice loves Bob. Bob loves Alice.");
    std::env::set_current_dir(td.path()).unwrap();

    let mut o = opts(ExportFormat::Json);
    o.combine = false;
    let _ = analyze_path(td.path(), None, &o).expect("export json");

    // ensure at least one .json exists
    let any_json = fs::read_dir(td.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().map(|e| e == "json").unwrap_or(false))
        .expect("expected at least one json export");
    // parse JSON to ensure validity
    let js = read_to_string(&any_json);
    let _: Json = serde_json::from_str(&js).expect("valid json");
}

#[test]
#[serial]
fn lib_export_tsv() {
    let td = assert_fs::TempDir::new().unwrap();
    let _f = write_file(&td, "d.txt", "Alice loves Bob. Bob loves Alice.");
    std::env::set_current_dir(td.path()).unwrap();

    let mut o = opts(ExportFormat::Tsv);
    o.combine = false;
    let _ = analyze_path(td.path(), None, &o).expect("export tsv");

    let any_tsv = fs::read_dir(td.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().map(|e| e == "tsv").unwrap_or(false))
        .expect("expected at least one tsv export");
    let content = read_to_string(&any_tsv);
    assert!(!content.is_empty());
}

// ---------------- CLI tests ----------------

fn run_cli_ok_in(dir: &std::path::Path, args: &[&str]) -> assert_cmd::assert::Assert {
    let mut cmd = assert_cmd::Command::cargo_bin("text_analysis").unwrap();
    cmd.current_dir(dir);
    cmd.args(args).assert().success()
}

fn run_cli_fail_in(dir: &std::path::Path, args: &[&str]) -> assert_cmd::assert::Assert {
    let mut cmd = assert_cmd::Command::cargo_bin("text_analysis").unwrap();
    cmd.current_dir(dir);
    cmd.args(args).assert().failure()
}

#[test]
fn cli_basic_run_csv() {
    let td = assert_fs::TempDir::new().unwrap();
    let _f = write_file(
        &td,
        "cli.txt",
        "Berlin meets Alice. Alice meets Bob. NASA FAILS.",
    );

    // Also provide a stopword list
    let stop = write_file(&td, "stop.txt", "meets\n");

    run_cli_ok_in(
        td.path(),
        &[
            td.path().to_string_lossy().as_ref(),
            "--export-format",
            "csv",
            "--stopwords",
            stop.to_str().unwrap(),
            "--ngram",
            "2",
            "--context",
            "3",
        ],
    );

    // Expect wordfreq csv for cli.txt
    let re = Regex::new(r".+_\d{8}_\d{6}_wordfreq\.csv$").unwrap();
    let found = fs::read_dir(td.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| re.is_match(e.file_name().to_string_lossy().as_ref()));
    assert!(found, "Expected cli_*_wordfreq.csv in temp dir");
}

#[test]
fn cli_entities_only_and_stemming() {
    let td = assert_fs::TempDir::new().unwrap();
    let _f = write_file(
        &td,
        "entities.txt",
        "The City of Berlin is in Germany. Cars running.",
    );

    // entities-only with stemming enabled (should not affect NER, but test flag combo)
    run_cli_ok_in(
        td.path(),
        &[
            td.path().to_string_lossy().as_ref(),
            "--entities-only",
            "--stem",
            "--export-format",
            "json",
        ],
    );

    // Force language (en) with stemming
    run_cli_ok_in(
        td.path(),
        &[
            td.path().to_string_lossy().as_ref(),
            "--stem",
            "--stem-lang",
            "en",
            "--export-format",
            "tsv",
        ],
    );

    // Combined mode
    run_cli_ok_in(
        td.path(),
        &[
            td.path().to_string_lossy().as_ref(),
            "--combine",
            "--export-format",
            "csv",
        ],
    );
}

#[test]
fn cli_nonexistent_path_fails() {
    let td = tempdir().unwrap(); // base dir
    let bad = td.path().join("does_not_exist_here");
    run_cli_fail_in(
        td.path(),
        &[bad.to_string_lossy().as_ref(), "--export-format", "csv"],
    );
}

// --------- Optional: PDF (requires feature = "pdf") ----------

#[test]
fn lib_pdf_best_effort_read() {
    // If you ship a tiny test.pdf under tests/assets/test.pdf, you can read & analyze here.
    // This test is a placeholder; ensure pdf_extract works at runtime.
    let td = assert_fs::TempDir::new().unwrap();
    // Copy tests/assets/test.pdf -> td
    let src = PathBuf::from("tests/assets/test.pdf");
    if src.exists() {
        let dst = td.child("doc.pdf");
        fs::copy(&src, dst.path()).unwrap();
        let mut o = opts(ExportFormat::Json);
        let _r = analyze_path(td.path(), None, &o).expect("analyze pdf");
        // Just assert that at least one output file exists
        let has_any = fs::read_dir(td.path()).unwrap().next().is_some();
        assert!(has_any);
    } else {
        eprintln!("(skipped) tests/assets/test.pdf not found");
    }
}
