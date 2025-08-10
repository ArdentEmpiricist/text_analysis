//! Integration tests for `text_analysis` with explicit stemming checks.
//
// This suite verifies:
// - Library behavior (tokenization, stopwords, stemming Auto/Force/Off, n-grams, context, PMI, NER)
// - CLI behavior including export formats and stemming flags
// - Combined mode (map-reduce) basic outputs
//
// Notes:
// - CLI tests run the binary with a per-process working directory (no global CWD change).
// - Tests that change global CWD (library-level outputs) are marked #[serial].

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::prelude::*;
use assert_fs::prelude::*;
use predicates::prelude::*;
use regex::Regex;
use serde_json::Value as Json;
use serial_test::serial;
use tempfile::tempdir;

use text_analysis::{
    AnalysisOptions, ExportFormat, StemLang, StemMode, analyze_path, analyze_text_with,
    collect_files,
};

// --------------------- helpers ---------------------

/// Create a file with content in a temp dir.
fn write_file(dir: &assert_fs::TempDir, name: &str, content: &str) -> PathBuf {
    let f = dir.child(name);
    f.write_str(content).unwrap();
    f.path().to_path_buf()
}

/// Read file to string.
fn read_to_string<P: AsRef<Path>>(p: P) -> String {
    fs::read_to_string(p).unwrap()
}

/// Default analysis options for library calls.
fn opts(fmt: ExportFormat) -> AnalysisOptions {
    AnalysisOptions {
        ngram: 2,
        context: 5,
        export_format: fmt,
        entities_only: false,
        combine: false,
        stem_mode: StemMode::Off,
        stem_require_detected: false,
    }
}

/// Run CLI successfully with a specific working directory.
fn run_cli_ok_in(dir: &std::path::Path, args: &[&str]) -> assert_cmd::assert::Assert {
    let mut cmd = assert_cmd::Command::cargo_bin("text_analysis").unwrap();
    cmd.current_dir(dir);
    cmd.args(args).assert().success()
}

/// Run CLI expecting failure with a specific working directory.
fn run_cli_fail_in(dir: &std::path::Path, args: &[&str]) -> assert_cmd::assert::Assert {
    let mut cmd = assert_cmd::Command::cargo_bin("text_analysis").unwrap();
    cmd.current_dir(dir);
    cmd.args(args).assert().failure()
}

/// Find a JSON export file that ends with a given suffix (e.g., "_wordfreq.json").
fn find_json_with_suffix(dir: &Path, suffix: &str) -> PathBuf {
    for entry in fs::read_dir(dir).unwrap().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.extension().map(|e| e == "json").unwrap_or(false) {
            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(suffix) {
                    return p;
                }
            }
        }
    }
    panic!("No JSON file found ending with {}", suffix);
}

/// Load wordfreq JSON export into a map<String, usize>.
fn load_wordfreq_map(dir: &Path) -> HashMap<String, usize> {
    let p = find_json_with_suffix(dir, "_wordfreq.json");
    let s = read_to_string(p);
    let v: Json = serde_json::from_str(&s).expect("valid json");
    let mut map = HashMap::new();
    let arr = v.as_array().expect("json array");
    for item in arr {
        let obj = item.as_object().expect("json object");
        let k = obj
            .get("item")
            .and_then(|x| x.as_str())
            .expect("item str")
            .to_string();
        let c = obj
            .get("count")
            .and_then(|x| x.as_u64())
            .expect("count u64") as usize;
        map.insert(k, c);
    }
    map
}

// --------------------- library tests ---------------------

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
    // Text strongly indicates English; Auto should map to English stemmer.
    let text = "This is an English sentence where the running runner runs and cars are common words. running runner runs cars car cars running";
    let stop = std::collections::HashSet::new();

    // Auto
    let mut o = opts(ExportFormat::Json);
    o.stem_mode = StemMode::Auto;
    let r_auto = analyze_text_with(text, &stop, &o);
    // English stemming should reduce "running"->"run", "cars"->"car"
    assert!(r_auto.wordfreq.get("run").is_some());
    assert!(r_auto.wordfreq.get("car").is_some());
    assert!(r_auto.wordfreq.get("running").is_none());
    assert!(r_auto.wordfreq.get("cars").is_none());

    // Force English
    let mut o2 = opts(ExportFormat::Json);
    o2.stem_mode = StemMode::Force(StemLang::En);
    let r_force = analyze_text_with(text, &stop, &o2);
    assert!(r_force.wordfreq.get("run").is_some());
    assert!(r_force.wordfreq.get("car").is_some());
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

// --------------------- CLI tests (general) ---------------------

#[test]
fn cli_nonexistent_path_fails() {
    let td = tempdir().unwrap(); // base dir
    let bad = td.path().join("does_not_exist_here");
    run_cli_fail_in(
        td.path(),
        &[bad.to_string_lossy().as_ref(), "--export-format", "csv"],
    );
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

    // Expect wordfreq csv
    let re = Regex::new(r".+_\d{8}_\d{6}_wordfreq\.csv$").unwrap();
    let found = fs::read_dir(td.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| re.is_match(e.file_name().to_string_lossy().as_ref()));
    assert!(found, "Expected *_wordfreq.csv in temp dir");
}

#[test]
fn cli_export_json() {
    let td = assert_fs::TempDir::new().unwrap();
    let _f = write_file(&td, "fmt.txt", "Alpha Beta. Beta Gamma. Berlin.");

    run_cli_ok_in(
        td.path(),
        &[
            td.path().to_string_lossy().as_ref(),
            "--export-format",
            "json",
        ],
    );

    // Expect at least one .json file
    let has_json = std::fs::read_dir(td.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| e.path().extension().map(|x| x == "json").unwrap_or(false));
    assert!(has_json, "Expected at least one .json export in temp dir");
}

#[test]
fn cli_export_tsv() {
    let td = assert_fs::TempDir::new().unwrap();
    let _f = write_file(&td, "fmt2.txt", "Alice Bob. Bob Alice.");

    run_cli_ok_in(
        td.path(),
        &[
            td.path().to_string_lossy().as_ref(),
            "--export-format",
            "tsv",
        ],
    );

    // Expect at least one .tsv file
    let has_tsv = std::fs::read_dir(td.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| e.path().extension().map(|x| x == "tsv").unwrap_or(false));
    assert!(has_tsv, "Expected at least one .tsv export in temp dir");
}

// --------------------- CLI tests (stemming) ---------------------

#[test]
fn cli_stem_auto_detects_language() {
    let td = assert_fs::TempDir::new().unwrap();
    let _f = write_file(
        &td,
        "stem.txt",
        "This is an English sentence where the running runner runs and cars are common words. running runner runs cars car cars running",
    );

    run_cli_ok_in(
        td.path(),
        &[
            td.path().to_string_lossy().as_ref(),
            "--export-format",
            "json",
            "--stem",
        ],
    );

    let wf = load_wordfreq_map(td.path());
    // Expect stemmed forms
    assert!(
        wf.get("run").is_some(),
        "Auto stemming should produce 'run'"
    );
    assert!(
        wf.get("car").is_some(),
        "Auto stemming should produce 'car'"
    );
    // And raw forms should be absent
    assert!(
        wf.get("running").is_none(),
        "Auto stemming should remove 'running'"
    );
    assert!(
        wf.get("cars").is_none(),
        "Auto stemming should remove 'cars'"
    );
}

#[test]
fn cli_stem_lang_without_stem_flag_forces() {
    // New semantics: --stem-lang forces stemming even without --stem.
    let td = assert_fs::TempDir::new().unwrap();
    let _f = write_file(
        &td,
        "stem2.txt",
        "running runner runs cars car cars running",
    );

    run_cli_ok_in(
        td.path(),
        &[
            td.path().to_string_lossy().as_ref(),
            "--export-format",
            "json",
            "--stem-lang",
            "en", // intentionally without --stem
        ],
    );

    let wf = load_wordfreq_map(td.path());
    // Expect raw forms present (no stemming)
    assert!(
        wf.get("running").is_none(),
        "With forced --stem-lang, 'running' should not remain"
    );
    assert!(
        wf.get("cars").is_none(),
        "With forced --stem-lang, 'cars' should not remain"
    );
    // And stemmed forms may be absent
    assert!(
        wf.get("run").is_some(),
        "With forced --stem-lang, 'run' should be produced"
    );
    assert!(
        wf.get("car").is_some(),
        "With forced --stem-lang, 'car' should be produced"
    );
}

#[test]
fn cli_stem_force_language_with_stem() {
    let td = assert_fs::TempDir::new().unwrap();
    let _f = write_file(
        &td,
        "stem3.txt",
        "running runner runs cars car cars running",
    );

    run_cli_ok_in(
        td.path(),
        &[
            td.path().to_string_lossy().as_ref(),
            "--export-format",
            "json",
            "--stem",
            "--stem-lang",
            "en",
        ],
    );

    let wf = load_wordfreq_map(td.path());
    // Expect English stemmed forms
    assert!(
        wf.get("run").is_some(),
        "Forced English should produce 'run'"
    );
    assert!(
        wf.get("car").is_some(),
        "Forced English should produce 'car'"
    );
    assert!(
        wf.get("running").is_none(),
        "Forced English should remove 'running'"
    );
    assert!(
        wf.get("cars").is_none(),
        "Forced English should remove 'cars'"
    );
}

// --------------------- PDF smoke test ---------------------

#[test]
fn lib_pdf_best_effort_read() {
    // This test runs unconditionally; if no test PDF is present, we just ensure nothing crashes
    // by creating a simple .txt instead (since PDF parsing is built-in).
    let td = assert_fs::TempDir::new().unwrap();
    let _f = write_file(&td, "doc.txt", "Simple text file to ensure analyzer runs.");
    std::env::set_current_dir(td.path()).unwrap();

    let mut o = opts(ExportFormat::Json);
    let _ = analyze_path(td.path(), None, &o).expect("analysis runs");
}

// --------------------- Stemming strict-mode tests ---------------------

#[test]
#[serial]
fn lib_stem_strict_per_file_skips_undetected() {
    let td = assert_fs::TempDir::new().unwrap();
    // Undetectable / unsupported text (digits/punct only)
    let _gib = write_file(&td, "gib.txt", "12345 67890 !!! ??? 00000 ---");
    let _eng = write_file(
        &td,
        "eng.txt",
        "This is clearly English so detection should work and stemming should run.",
    );

    // Library call with strict auto-stemming
    let mut o = opts(ExportFormat::Json);
    o.combine = false;
    o.stem_mode = StemMode::Auto;
    o.stem_require_detected = true;

    std::env::set_current_dir(td.path()).unwrap();
    let rep = analyze_path(td.path(), None, &o)
        .expect("per-file strict should succeed (skips undetected)");

    // Expect exactly one wordfreq.json (only the English file)
    let wordfreq_jsons: Vec<_> = std::fs::read_dir(td.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.ends_with("_wordfreq.json"))
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(
        wordfreq_jsons.len(),
        1,
        "Expected exactly one wordfreq.json"
    );

    // And the report should list exactly one failed file (gib.txt)
    assert_eq!(
        rep.failed_files.len(),
        1,
        "Expected one failed file in strict mode"
    );
}

#[test]
fn lib_stem_strict_combined_aborts_on_undetected() {
    let td = assert_fs::TempDir::new().unwrap();
    let _gib = write_file(&td, "gib.txt", "12345 67890 !!! ??? 00000 ---");
    let _eng = write_file(
        &td,
        "eng.txt",
        "This is clearly English so detection should work and stemming should run.",
    );

    let mut o = opts(ExportFormat::Json);
    o.combine = true;
    o.stem_mode = StemMode::Auto;
    o.stem_require_detected = true;

    let res = analyze_path(td.path(), None, &o);
    assert!(
        res.is_err(),
        "Combined strict should abort when a file's language is undetected"
    );
}

#[test]
fn cli_stem_strict_per_file_skips_undetected() {
    let td = assert_fs::TempDir::new().unwrap();
    let _gib = write_file(&td, "gib.txt", "12345 67890 !!! ??? 00000 ---");
    let _eng = write_file(
        &td,
        "eng.txt",
        "This is clearly English so detection should work and stemming should run.",
    );

    run_cli_ok_in(
        td.path(),
        &[
            td.path().to_string_lossy().as_ref(),
            "--export-format",
            "json",
            "--stem",
            "--stem-strict",
        ],
    );

    // Expect exactly one wordfreq.json (only the English file)
    let wordfreq_jsons: Vec<_> = std::fs::read_dir(td.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.ends_with("_wordfreq.json"))
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(
        wordfreq_jsons.len(),
        1,
        "Expected exactly one wordfreq.json"
    );
}

#[test]
fn cli_stem_strict_combined_aborts_on_undetected() {
    let td = assert_fs::TempDir::new().unwrap();
    let _gib = write_file(&td, "gib.txt", "12345 67890 !!! ??? 00000 ---");
    let _eng = write_file(
        &td,
        "eng.txt",
        "This is clearly English so detection should work and stemming should run.",
    );

    // Expect failure and error message
    run_cli_fail_in(
        td.path(),
        &[
            td.path().to_string_lossy().as_ref(),
            "--combine",
            "--export-format",
            "json",
            "--stem",
            "--stem-strict",
        ],
    )
    .stderr(
        predicate::str::contains("Combined run aborted")
            .or(predicate::str::contains("strict stemming")),
    );
}
