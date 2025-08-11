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

#[test]
#[serial]
fn lib_combine_wordfreq_sums_across_files() {
    // Prepare two files with known counts:
    // file1: apple x2, banana x1, orange x1
    // file2: banana x2, apple x1
    // combined expected: apple=3, banana=3, orange=1
    let td = assert_fs::TempDir::new().unwrap();
    let _f1 = write_file(&td, "a1.txt", "apple apple banana orange");
    let _f2 = write_file(&td, "a2.txt", "banana banana apple");

    // Combined mode, JSON export
    let mut o = opts(ExportFormat::Json);
    o.combine = true;
    std::env::set_current_dir(td.path()).unwrap();
    let _ = analyze_path(td.path(), None, &o).expect("combined analysis runs");

    // Load the combined wordfreq JSON (ends with _wordfreq.json)
    let wf = load_wordfreq_map(td.path());

    assert_eq!(
        wf.get("apple").copied().unwrap_or(0),
        3,
        "apple count should be 3 in combined"
    );
    assert_eq!(
        wf.get("banana").copied().unwrap_or(0),
        3,
        "banana count should be 3 in combined"
    );
    assert_eq!(
        wf.get("orange").copied().unwrap_or(0),
        1,
        "orange count should be 1 in combined"
    );

    // Ensure no per-file wordfreq.json outputs exist (only combined_*)
    let non_combined_exists = std::fs::read_dir(td.path())
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
        .any(|p| {
            !p.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("combined_")
        });
    assert!(
        !non_combined_exists,
        "Expected only combined_*_wordfreq.json outputs in combined mode"
    );
}

#[test]
#[serial]
fn lib_combine_wordfreq_with_pdf() {
    // TXT totals (lowercased by analyzer):
    //  - a1.txt: "Apple and banana. Apple, orange; banana! Apple? Grape grape apple."
    //      => apple=4, banana=2, orange=1, grape=2, and=1
    //  - a2.txt: "Banana and apple; banana and pear. Apple banana banana, apple!"
    //      => banana=4, apple=3, and=2, pear=1
    //
    // Base expected (TXT only): apple=7, banana=6, orange=1, grape=2, and=3, pear=1
    //
    // PDF adds (valid, parseable): "Banana apple banana grape apple banana orange"
    //      => banana=3, apple=2, grape=1, orange=1
    //
    // Final expected (TXT + PDF): apple=9, banana=9, grape=3, orange=2, and=3, pear=1

    use std::io::Write as _;

    let td = assert_fs::TempDir::new().unwrap();
    let _f1 = write_file(
        &td,
        "a1.txt",
        "Apple and banana. Apple, orange; banana! Apple? Grape grape apple.",
    );
    let _f2 = write_file(
        &td,
        "a2.txt",
        "Banana and apple; banana and pear. Apple banana banana, apple!",
    );

    // Build a minimal, *valid* PDF (with correct xref offsets) containing the text.
    fn build_pdf_bytes(text: &str) -> Vec<u8> {
        fn esc_parens(s: &str) -> String {
            s.replace('(', r"\(").replace(')', r"\)")
        }
        let content = format!("BT\n/F1 12 Tf\n10 100 Td\n({}) Tj\nET\n", esc_parens(text));

        let mut pdf: Vec<u8> = Vec::new();
        let mut offsets: [usize; 6] = [0; 6];

        // Header
        pdf.extend_from_slice(b"%PDF-1.4\n");

        // 1 0 obj  (Catalog)
        offsets[1] = pdf.len();
        pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

        // 2 0 obj  (Pages)
        offsets[2] = pdf.len();
        pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

        // 3 0 obj  (Page)
        offsets[3] = pdf.len();
        pdf.extend_from_slice(b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n");

        // 4 0 obj  (Contents stream)
        let stream_len = content.as_bytes().len();
        offsets[4] = pdf.len();
        pdf.extend_from_slice(
            format!("4 0 obj\n<< /Length {} >>\nstream\n", stream_len).as_bytes(),
        );
        pdf.extend_from_slice(content.as_bytes());
        pdf.extend_from_slice(b"endstream\nendobj\n");

        // 5 0 obj  (Font)
        offsets[5] = pdf.len();
        pdf.extend_from_slice(
            b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n",
        );

        // xref
        let xref_pos = pdf.len();
        let mut xref = String::new();
        xref.push_str("xref\n0 6\n");
        xref.push_str("0000000000 65535 f \n");
        for i in 1..=5 {
            xref.push_str(&format!("{:010} 00000 n \n", offsets[i]));
        }
        pdf.extend_from_slice(xref.as_bytes());

        // trailer + startxref
        let trailer = format!(
            "trailer << /Size 6 /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            xref_pos
        );
        pdf.extend_from_slice(trailer.as_bytes());

        pdf
    }

    // Write robust PDF with complex text
    let pdf_path = td.child("doc.pdf");
    {
        let bytes = build_pdf_bytes("Banana apple banana grape apple banana orange");
        let mut f = std::fs::File::create(pdf_path.path()).unwrap();
        f.write_all(&bytes).unwrap();
    }

    // Combined mode, JSON export
    let mut o = opts(ExportFormat::Json);
    o.combine = true;
    std::env::set_current_dir(td.path()).unwrap();
    let rep = analyze_path(td.path(), None, &o).expect("combined analysis runs");

    // Ensure PDF parsed successfully (since we generated a valid one)
    assert!(
        !rep.failed_files
            .iter()
            .any(|(file, _)| file.ends_with("doc.pdf")),
        "PDF should be parsed successfully"
    );

    // Load combined wordfreq and assert sums including PDF
    let wf = load_wordfreq_map(td.path());
    assert_eq!(
        wf.get("apple").copied().unwrap_or(0),
        9,
        "apple count should be 9 (7 TXT + 2 PDF)"
    );
    assert_eq!(
        wf.get("banana").copied().unwrap_or(0),
        9,
        "banana count should be 9 (6 TXT + 3 PDF)"
    );
    assert_eq!(
        wf.get("grape").copied().unwrap_or(0),
        3,
        "grape count should be 3 (2 TXT + 1 PDF)"
    );
    assert_eq!(
        wf.get("orange").copied().unwrap_or(0),
        2,
        "orange count should be 2 (1 TXT + 1 PDF)"
    );
    assert_eq!(
        wf.get("and").copied().unwrap_or(0),
        3,
        "and count should be 3 (TXT only)"
    );
    assert_eq!(
        wf.get("pear").copied().unwrap_or(0),
        1,
        "pear count should be 1 (TXT only)"
    );

    // Ensure no per-file wordfreq.json exists (only combined_* outputs)
    let non_combined_exists = std::fs::read_dir(td.path())
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
        .any(|p| {
            !p.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("combined_")
        });
    assert!(
        !non_combined_exists,
        "Expected only combined_*_wordfreq.json outputs in combined mode"
    );
}

#[test]
#[serial]
fn lib_combine_wordfreq_with_multipage_pdf_and_noise() {
    // TXT totals (lowercased by analyzer):
    //  a1.txt: "Apple and banana. Apple, orange; banana! Apple? Grape grape apple."
    //    => apple=4, banana=2, orange=1, grape=2, and=1
    //  a2.txt: "Banana and apple; banana and pear. Apple banana banana, apple!"
    //    => banana=4, apple=3, and=2, pear=1
    //
    // Base expected (TXT only):
    //    apple=7, banana=6, orange=1, grape=2, and=3, pear=1
    //
    // Multi-page PDF (3 pages) adds (note: 'orange' is NOT last token now):
    //  p1: "Banana apple banana grape apple banana orange kiwi"
    //      => banana=3, apple=2, grape=1, orange=1  (kiwi ignored in asserts)
    //  p2: "Noise NOISE n123 tokens; apple banana banana pear."
    //      => apple=1, banana=2, pear=1
    //  p3: "banana grape grape banana apple."
    //      => banana=2, grape=2, apple=1
    //
    // PDF contribution:
    //    apple=4, banana=7, grape=3, orange=1, pear=1
    //
    // Final expected (TXT + PDF):
    //    apple=11, banana=13, grape=5, orange=2, and=3, pear=2

    use std::io::Write as _;

    let td = assert_fs::TempDir::new().unwrap();
    let _f1 = write_file(
        &td,
        "a1.txt",
        "Apple and banana. Apple, orange; banana! Apple? Grape grape apple.",
    );
    let _f2 = write_file(
        &td,
        "a2.txt",
        "Banana and apple; banana and pear. Apple banana banana, apple!",
    );

    fn build_multipage_pdf_bytes(pages: &[&str]) -> Vec<u8> {
        fn esc_parens(s: &str) -> String {
            s.replace('(', r"\(").replace(')', r"\)")
        }
        let n = pages.len();
        let font_id = 3 + 2 * n;

        let mut pdf: Vec<u8> = Vec::new();
        let mut offsets: Vec<usize> = vec![0; font_id as usize + 1]; // 0..=font_id

        pdf.extend_from_slice(b"%PDF-1.4\n");
        offsets[1] = pdf.len();
        pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

        offsets[2] = pdf.len();
        {
            let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 3 + 2 * i)).collect();
            let kids_arr = kids.join(" ");
            let pages_obj = format!(
                "2 0 obj\n<< /Type /Pages /Kids [ {} ] /Count {} >>\nendobj\n",
                kids_arr, n
            );
            pdf.extend_from_slice(pages_obj.as_bytes());
        }

        for (i, text) in pages.iter().enumerate() {
            let page_id = 3 + 2 * i;
            let cont_id = 4 + 2 * i;

            offsets[page_id as usize] = pdf.len();
            let page_obj = format!(
                "{id} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Contents {cid} 0 R /Resources << /Font << /F1 {fid} 0 R >> >> >>\nendobj\n",
                id = page_id,
                cid = cont_id,
                fid = font_id
            );
            pdf.extend_from_slice(page_obj.as_bytes());

            let content = format!("BT\n/F1 12 Tf\n10 200 Td\n({}) Tj\nET\n", esc_parens(text));
            offsets[cont_id as usize] = pdf.len();
            pdf.extend_from_slice(
                format!(
                    "{cid} 0 obj\n<< /Length {len} >>\nstream\n",
                    cid = cont_id,
                    len = content.len()
                )
                .as_bytes(),
            );
            pdf.extend_from_slice(content.as_bytes());
            pdf.extend_from_slice(b"endstream\nendobj\n");
        }

        offsets[font_id as usize] = pdf.len();
        pdf.extend_from_slice(
            format!(
                "{fid} 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n",
                fid = font_id
            )
            .as_bytes(),
        );

        let xref_pos = pdf.len();
        let mut xref = String::new();
        xref.push_str(&format!("xref\n0 {}\n", font_id + 1));
        xref.push_str("0000000000 65535 f \n");
        for obj in 1..=font_id {
            xref.push_str(&format!("{:010} 00000 n \n", offsets[obj as usize]));
        }
        pdf.extend_from_slice(xref.as_bytes());

        let trailer = format!(
            "trailer << /Size {size} /Root 1 0 R >>\nstartxref\n{pos}\n%%EOF\n",
            size = font_id + 1,
            pos = xref_pos
        );
        pdf.extend_from_slice(trailer.as_bytes());

        pdf
    }

    // Page 1 ends with "kiwi" so "orange" is not the last token in the Tj block
    let pdf_bytes = build_multipage_pdf_bytes(&[
        "Banana apple banana grape apple banana orange kiwi",
        "Noise NOISE n123 tokens; apple banana banana pear.",
        "banana grape grape banana apple.",
    ]);

    let pdf_path = td.child("doc_multi.pdf");
    {
        let mut f = std::fs::File::create(pdf_path.path()).unwrap();
        f.write_all(&pdf_bytes).unwrap();
    }

    let mut o = opts(ExportFormat::Json);
    o.combine = true;
    std::env::set_current_dir(td.path()).unwrap();
    let rep = analyze_path(td.path(), None, &o).expect("combined analysis runs");

    assert!(
        !rep.failed_files
            .iter()
            .any(|(file, _)| file.ends_with("doc_multi.pdf")),
        "Multi-page PDF should be parsed successfully"
    );

    let wf = load_wordfreq_map(td.path());
    assert_eq!(
        wf.get("apple").copied().unwrap_or(0),
        11,
        "apple total mismatch"
    );
    assert_eq!(
        wf.get("banana").copied().unwrap_or(0),
        13,
        "banana total mismatch"
    );
    assert_eq!(
        wf.get("grape").copied().unwrap_or(0),
        5,
        "grape total mismatch"
    );
    assert_eq!(
        wf.get("orange").copied().unwrap_or(0),
        2,
        "orange total mismatch"
    );
    assert_eq!(wf.get("and").copied().unwrap_or(0), 3, "and total mismatch");
    assert_eq!(
        wf.get("pear").copied().unwrap_or(0),
        2,
        "pear total mismatch"
    );

    let non_combined_exists = std::fs::read_dir(td.path())
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
        .any(|p| {
            !p.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("combined_")
        });
    assert!(
        !non_combined_exists,
        "Expected only combined_*_wordfreq.json outputs in combined mode"
    );
}

#[test]
#[serial]
fn lib_exports_are_sorted_by_frequency() {
    use std::fs;
    use std::io::Read;
    use std::path::Path;

    // Small corpus with predictable frequencies
    let td = assert_fs::TempDir::new().unwrap();
    let _f = write_file(
        &td,
        "sorted.txt",
        // duplicated sequence -> z=10, a=6, b=4, c=2
        "z z z z z a a a b b c  |  z z z z z a a a b b c",
    );

    // Run per-file CSV export
    let mut o = opts(ExportFormat::Csv);
    o.combine = false;
    o.ngram = 2;
    o.context = 2;
    std::env::set_current_dir(td.path()).unwrap();
    analyze_path(td.path(), None, &o).expect("analysis runs");

    // Helpers
    fn find_csv<P: AsRef<Path>>(dir: P, suffix: &str) -> std::path::PathBuf {
        let mut matches: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map(|x| x == "csv").unwrap_or(false))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.ends_with(suffix))
                    .unwrap_or(false)
            })
            .collect();
        matches.sort(); // deterministic pick
        matches
            .pop()
            .expect(&format!("no CSV with suffix {}", suffix))
    }
    fn read_csv_lines(p: &Path) -> Vec<String> {
        let mut s = String::new();
        std::fs::File::open(p)
            .unwrap()
            .read_to_string(&mut s)
            .unwrap();
        s.lines().map(|ln| ln.to_string()).collect()
    }

    // ---------- WORD FREQ ----------
    let wf_csv = find_csv(td.path(), "_wordfreq.csv");
    let wf_lines = read_csv_lines(&wf_csv);
    assert!(wf_lines.len() >= 5, "needs header + at least 4 rows");
    let parse = |row: &str| {
        let mut it = row.splitn(2, ',');
        let item = it.next().unwrap().to_string();
        let cnt: usize = it.next().unwrap().parse().unwrap();
        (item, cnt)
    };
    let wf_rows: Vec<(String, usize)> = wf_lines.iter().skip(1).map(|r| parse(r)).collect();

    // expected: sort by count desc, then item asc
    let mut wf_sorted = wf_rows.clone();
    wf_sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    assert_eq!(wf_rows, wf_sorted, "wordfreq CSV is not sorted as expected");

    // sanity: max should be ("z",10)
    let max = wf_rows.iter().max_by_key(|(_, c)| *c).unwrap();
    assert_eq!(max, &("z".to_string(), 10));

    // ---------- N-GRAMS (bigrams) ----------
    let ng_csv = find_csv(td.path(), "_ngrams.csv");
    let ng_lines = read_csv_lines(&ng_csv);
    let ng_rows: Vec<(String, usize)> = ng_lines.iter().skip(1).map(|r| parse(r)).collect();
    let mut ng_sorted = ng_rows.clone();
    ng_sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    assert_eq!(ng_rows, ng_sorted, "ngrams CSV is not sorted as expected");

    // ---------- PMI ----------
    let pmi_csv = find_csv(td.path(), "_pmi.csv");
    let pmi_lines = read_csv_lines(&pmi_csv);
    // header: word1,word2,distance,count,pmi
    #[derive(Clone, Debug, PartialEq)]
    struct Row {
        w1: String,
        w2: String,
        d: usize,
        c: usize,
        p: f64,
    }
    let parse_pmi = |row: &str| {
        let cols: Vec<&str> = row.split(',').collect();
        Row {
            w1: cols[0].to_string(),
            w2: cols[1].to_string(),
            d: cols[2].parse().unwrap(),
            c: cols[3].parse().unwrap(),
            p: cols[4].parse().unwrap(),
        }
    };
    let pmi_rows: Vec<Row> = pmi_lines.iter().skip(1).map(|r| parse_pmi(r)).collect();
    let mut pmi_sorted = pmi_rows.clone();
    pmi_sorted.sort_by(|a, b| {
        b.c.cmp(&a.c)
            .then_with(|| b.p.partial_cmp(&a.p).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| a.w1.cmp(&b.w1))
            .then_with(|| a.w2.cmp(&b.w2))
    });
    assert_eq!(pmi_rows, pmi_sorted, "PMI CSV is not sorted as expected");
}
