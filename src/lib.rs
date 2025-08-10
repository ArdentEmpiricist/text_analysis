#![forbid(unsafe_code)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/ArdentEmpiricist/text_analysis/main/assets/text_analysis_logo.png"
)]
//! # Text Analysis Library
//!
//! Core functions for analyzing `.txt` and `.pdf` documents.
//!
//! ## NER Heuristic (documentation)
//! The Named-Entity recognition uses a **simple capitalization heuristic**:
//!
//! 1. Tokenize the **original (non‑stemmed)** text.
//! 2. A token is counted as a candidate entity if it
//!    - starts with an uppercase letter (Unicode-aware), and
//!    - is **not** fully uppercase (to avoid acronyms), and
//!    - is **not** a common function word at a sentence start (basic list check).
//! 3. Counts are **case-sensitive** (so "Berlin" ≠ "BERLIN").
//!
//! Note: This heuristic is fast and deterministic but will overgenerate in some
//! cases (e.g., sentence-initial words). For higher quality, apply a custom
//! post-filter or integrate a proper NER model.
//!
//! ## clone() avoidance (key places)
//! - Use `HashMap::entry` to avoid double lookups and to allocate keys only on insertion.
//! - For context maps, allocate strings **only on first insertion** (`entry(key.to_owned())`).
//! - Serialization writes directly to files to avoid unnecessary intermediate allocations.
//!
//! ## No double scanning of files
//! `analyze_path` collects files once and then processes either combined or per-file.

use chrono::Local;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use std::hash::{Hash, Hasher};
use whatlang::{Lang, detect};

use pdf_extract::extract_text;

/// Export format
#[derive(Copy, Clone, Debug)]
pub enum ExportFormat {
    Txt,
    Csv,
    Tsv,
    Json,
}

/// Stemming control
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum StemMode {
    /// Disable stemming
    Off,
    /// Auto-detect language and use suitable stemmer (if available)
    Auto,
    /// Force a specific language
    Force(StemLang),
}

/// Supported stemming languages (subset of `rust-stemmers` algorithms)
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum StemLang {
    Unknown,
    En,
    De,
    Fr,
    Es,
    It,
    Pt,
    Nl,
    Ru,
    Sv,
    Fi,
    No,
    Ro,
    Hu,
    Da,
    Tr,
}

impl StemLang {
    pub fn from_code(code: &str) -> Option<Self> {
        use StemLang::*;
        let c = code.to_ascii_lowercase();
        Some(match c.as_str() {
            "en" => En,
            "de" => De,
            "fr" => Fr,
            "es" => Es,
            "it" => It,
            "pt" => Pt,
            "nl" => Nl,
            "ru" => Ru,
            "sv" => Sv,
            "fi" => Fi,
            "no" => No,
            "ro" => Ro,
            "hu" => Hu,
            "da" => Da,
            "tr" => Tr,
            _ => return None,
        })
    }

    pub fn from_whatlang(lang: Lang) -> Self {
        use StemLang::*;
        // Map via ISO-639-3 codes to be robust across whatlang versions
        // (e.g., "nor", "nob", "nno" all map to Norwegian; "fra"/"fre" -> French).
        match lang.code() {
            "eng" => En,
            "deu" => De,
            "fra" | "fre" => Fr,
            "spa" => Es,
            "ita" => It,
            "por" => Pt,
            "nld" | "dut" => Nl,
            "rus" => Ru,
            "swe" => Sv,
            "fin" => Fi,
            "nor" | "nob" | "nno" => No,
            "ron" | "rum" => Ro,
            "hun" => Hu,
            "dan" => Da,
            "tur" => Tr,
            _ => Unknown,
        }
    }
}

/// Analysis options
#[derive(Clone, Debug)]
pub struct AnalysisOptions {
    pub ngram: usize,
    pub context: usize,
    pub export_format: ExportFormat,
    pub entities_only: bool,
    pub combine: bool,
    pub stem_mode: StemMode,
}

/// Compact report (large structures are written to files)
#[derive(Debug)]
pub struct AnalysisReport {
    pub summary: String,
    pub failed_files: Vec<(String, String)>, // (file, error)
}

/// Detailed analysis result
#[derive(Debug, Default)]
pub struct AnalysisResult {
    pub ngrams: HashMap<String, usize>,
    pub wordfreq: HashMap<String, usize>,
    pub context_map: HashMap<String, HashMap<String, usize>>,
    pub direct_neighbors: HashMap<String, HashMap<String, usize>>,
    pub named_entities: HashMap<String, usize>,
    pub pmi: Vec<PmiEntry>,
}

#[derive(Debug)]
pub struct PmiEntry {
    pub word1: String,
    pub word2: String,
    pub distance: usize,
    pub count: usize,
    pub pmi: f64,
}

/// Entry point: analyze a path (file or directory).
/// Files are collected **once**; then either combined or per-file processing happens.
pub fn analyze_path(
    path: &Path,
    stopwords_file: Option<&PathBuf>, // may be None
    options: &AnalysisOptions,
) -> Result<AnalysisReport, String> {
    let files = collect_files(path);
    if files.is_empty() {
        return Err("No .txt or .pdf files found for analysis.".to_string());
    }

    let stopwords = load_stopwords(stopwords_file);

    let mut failed: Vec<(String, String)> = Vec::new();
    let ts = timestamp();

    if options.combine {
        // Combined mode: read all texts, analyze once.
        // Parallel read
        let read_results: Vec<_> = files.par_iter().map(|f| (f, read_text(f))).collect();

        let mut failed: Vec<(String, String)> = Vec::new();
        let mut combined_text = String::new();
        for (f, res) in read_results {
            match res {
                Ok(t) => {
                    combined_text.push_str(&t);
                    combined_text.push('\n');
                }
                Err(e) => failed.push((f.display().to_string(), e)),
            }
        }

        let result = analyze_text_with(&combined_text, &stopwords, options);
        write_all_outputs("combined", &result, &ts, options)?;
        let summary = summary_for(&[("combined".to_string(), &result)], options);
        return Ok(AnalysisReport {
            summary,
            failed_files: failed,
        });
    } else {
        // --- Per-file mode (parallel compute, serial write) ---
        // Compute in parallel:
        let results: Vec<_> = files
            .par_iter()
            .map(|f| match read_text(f) {
                Ok(t) => {
                    let r = analyze_text_with(&t, &stopwords, options);
                    let stem = stem_for(f);
                    Ok((stem, r))
                }
                Err(e) => Err((f.display().to_string(), e)),
            })
            .collect();

        // Partition successes and failures:
        let mut per_file_results: Vec<(String, AnalysisResult)> = Vec::new();
        let mut failed: Vec<(String, String)> = Vec::new();
        for item in results {
            match item {
                Ok((stem, r)) => per_file_results.push((stem, r)),
                Err(fe) => failed.push(fe),
            }
        }

        // Serialize writes (avoid IO contention / name races):
        for (stem, r) in &per_file_results {
            write_all_outputs(stem, r, &ts, options)?;
        }

        // Build summary
        let pairs: Vec<(String, &AnalysisResult)> = per_file_results
            .iter()
            .map(|(n, r)| (n.clone(), r))
            .collect();
        let summary = summary_for(&pairs, options);
        return Ok(AnalysisReport {
            summary,
            failed_files: failed,
        });
    }
}

/// Recursively collect .txt/.pdf files
pub fn collect_files(path: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if path.is_file() {
        if is_supported(path) {
            out.push(path.to_path_buf());
        }
    } else if path.is_dir() {
        let walker = walkdir::WalkDir::new(path).follow_links(true);
        for entry in walker.into_iter().filter_map(Result::ok) {
            let p = entry.path();
            if p.is_file() && is_supported(p) {
                out.push(p.to_path_buf());
            }
        }
    }
    out
}

fn is_supported(p: &Path) -> bool {
    match p
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
    {
        Some(ref e) if e == "txt" || e == "pdf" => true,
        _ => false,
    }
}

/// Read text from a file (.txt directly, .pdf via optional feature)
fn read_text(p: &Path) -> Result<String, String> {
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "txt" => std::fs::read_to_string(p).map_err(|e| format!("Read .txt failed: {e}")),
        "pdf" => extract_text(p).map_err(|e| format!("PDF extract failed: {e}")),
        _ => Err("Unsupported extension".to_string()),
    }
}

/// Load stopwords (optional). It is recommended to store them in lowercase.
fn load_stopwords(p: Option<&PathBuf>) -> HashSet<String> {
    let mut set = HashSet::new();
    if let Some(file) = p {
        if let Ok(txt) = fs::read_to_string(file) {
            for line in txt.lines() {
                let w = line.trim();
                if !w.is_empty() {
                    set.insert(w.to_string());
                }
            }
        }
    }
    set
}

/// Analyze a text with options
pub fn analyze_text_with(
    text: &str,
    stopwords: &HashSet<String>,
    opts: &AnalysisOptions,
) -> AnalysisResult {
    // 1) Detect language (for stemming and heuristic)
    let lang_guess = whatlang::detect(text);
    let stem_lang = match opts.stem_mode {
        StemMode::Off => StemLang::Unknown,
        StemMode::Auto => lang_guess
            .as_ref()
            .map(|i| StemLang::from_whatlang(i.lang()))
            .unwrap_or(StemLang::Unknown),
        StemMode::Force(lang) => lang,
    };

    // 2) Tokenize (keep original tokens for NER)
    let original_tokens = tokenize(text);
    let sentences = split_sentences(text); // used only by the heuristic (lightweight)

    // 3) Optional stemming + stopword filter for statistics path
    //    (NER uses original_tokens, not stemmed)
    let tokens_for_stats = normalize_for_stats(&original_tokens, stopwords, stem_lang);

    // 4) Statistics
    let mut result = AnalysisResult::default();
    ngrams_count(&tokens_for_stats, opts.ngram, &mut result.ngrams);
    wordfreq_count(&tokens_for_stats, &mut result.wordfreq);
    context_and_neighbors(
        &tokens_for_stats,
        opts.context,
        &mut result.context_map,
        &mut result.direct_neighbors,
    );
    named_entities_heuristic(&original_tokens, &sentences, &mut result.named_entities);
    compute_pmi(
        &tokens_for_stats,
        opts.context,
        &result.wordfreq,
        &mut result.pmi,
    );

    result
}

/// Simple Unicode-friendly tokenizer
fn tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::with_capacity(text.len() / 5);
    let mut cur = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '\'' {
            cur.push(ch);
        } else {
            if !cur.is_empty() {
                out.push(cur);
                cur = String::new();
            }
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Very rough sentence boundary detection (., !, ?) — used for heuristic only
fn split_sentences(text: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    let mut idx = 0usize;
    for ch in text.chars() {
        idx += ch.len_utf8();
        if ch == '.' || ch == '!' || ch == '?' {
            starts.push(idx);
        }
    }
    starts.sort_unstable();
    starts
}

/// Normalization for statistics: lowercasing, stopword filtering, optional stemming
fn normalize_for_stats(
    tokens: &[String],
    stopwords: &HashSet<String>,
    stem_lang: StemLang,
) -> Vec<String> {
    let mut out = Vec::with_capacity(tokens.len());
    for t in tokens {
        let lower = t.to_lowercase();
        if !stopwords.is_empty() && stopwords.contains(&lower) {
            continue;
        }
        let normalized = if let Some(stemmer) = make_stemmer(stem_lang) {
            stemmer.stem(&lower).to_string()
        } else {
            lower
        };
        out.push(normalized);
    }
    out
}

/// Stemmer factory (`rust-stemmers`). Returns None if unknown/unsupported.
fn make_stemmer(lang: StemLang) -> Option<rust_stemmers::Stemmer> {
    use StemLang::*;
    use rust_stemmers::{Algorithm, Stemmer};
    let algo = match lang {
        En => Algorithm::English,
        De => Algorithm::German,
        Fr => Algorithm::French,
        Es => Algorithm::Spanish,
        It => Algorithm::Italian,
        Pt => Algorithm::Portuguese,
        Nl => Algorithm::Dutch,
        Ru => Algorithm::Russian,
        Sv => Algorithm::Swedish,
        Fi => Algorithm::Finnish,
        No => Algorithm::Norwegian,
        Ro => Algorithm::Romanian,
        Hu => Algorithm::Hungarian,
        Da => Algorithm::Danish,
        Tr => Algorithm::Turkish,
        Unknown => return None,
    };
    Some(Stemmer::create(algo))
}

/// Count n-grams with minimal allocations
fn ngrams_count(tokens: &[String], n: usize, out: &mut HashMap<String, usize>) {
    if n == 0 || tokens.len() < n {
        return;
    }
    for i in 0..=tokens.len() - n {
        // Build the n-gram into a single buffer to avoid repeated concatenations.
        let mut buf = String::with_capacity(n * 6);
        for (k, t) in tokens[i..i + n].iter().enumerate() {
            if k > 0 {
                buf.push(' ');
            }
            buf.push_str(t);
        }
        *out.entry(buf).or_insert(0) += 1;
    }
}

/// Count word frequencies
fn wordfreq_count(tokens: &[String], out: &mut HashMap<String, usize>) {
    for t in tokens {
        // `entry(t.clone())` only allocates if the key is inserted for the first time.
        *out.entry(t.clone()).or_insert(0) += 1;
    }
}

/// Build context co-occurrences and direct neighbors
fn context_and_neighbors(
    tokens: &[String],
    window: usize,
    context_map: &mut HashMap<String, HashMap<String, usize>>,
    direct_neighbors: &mut HashMap<String, HashMap<String, usize>>,
) {
    if window == 0 {
        return;
    }
    let len = tokens.len();

    for (i, w) in tokens.iter().enumerate() {
        let left = i.saturating_sub(window);
        let right = (i + window + 1).min(len);

        // Context map
        let entry = context_map.entry(w.clone()).or_insert_with(HashMap::new);
        for j in left..right {
            if j == i {
                continue;
            }
            *entry.entry(tokens[j].clone()).or_insert(0) += 1;
        }
        // Direct neighbors
        let neigh = direct_neighbors
            .entry(w.clone())
            .or_insert_with(HashMap::new);
        if i > 0 {
            *neigh.entry(tokens[i - 1].clone()).or_insert(0) += 1;
        }
        if i + 1 < len {
            *neigh.entry(tokens[i + 1].clone()).or_insert(0) += 1;
        }
    }
}

/// Apply the capitalization heuristic (see module docs)
fn named_entities_heuristic(
    original_tokens: &[String],
    _sentence_starts: &[usize],
    out: &mut HashMap<String, usize>,
) {
    for tok in original_tokens {
        if tok
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false)
        {
            // Filter acronyms (fully uppercase)
            if tok.chars().all(|c| !c.is_lowercase()) {
                continue;
            }
            // Crude sentence-start function-word filter (language-agnostic)
            let lower = tok.to_lowercase();
            if [
                "the", "a", "an", "der", "die", "das", "ein", "eine", "le", "la", "les", "un",
                "una", "el", "los", "las", "il", "lo", "gli", "i",
            ]
            .contains(&lower.as_str())
            {
                continue;
            }
            *out.entry(tok.clone()).or_insert(0) += 1;
        }
    }
}

/// Compute PMI (Pointwise Mutual Information) over word pairs within ±window
fn compute_pmi(
    tokens: &[String],
    window: usize,
    wordfreq: &HashMap<String, usize>,
    out: &mut Vec<PmiEntry>,
) {
    if window == 0 || tokens.len() < 2 {
        return;
    }

    let total_tokens = tokens.len() as f64;

    // Pair counts by distance
    let mut pair_counts: HashMap<(String, String, usize), usize> = HashMap::new();
    for i in 0..tokens.len() {
        let w1 = &tokens[i];
        let left = i.saturating_sub(window);
        let right = (i + window + 1).min(tokens.len());
        for j in left..right {
            if j == i {
                continue;
            }
            let w2 = &tokens[j];
            let d = (i as isize - j as isize).abs() as usize;
            let key = if w1 <= w2 {
                (w1.clone(), w2.clone(), d)
            } else {
                (w2.clone(), w1.clone(), d)
            };
            *pair_counts.entry(key).or_insert(0) += 1;
        }
    }

    // PMI values
    out.clear();
    out.reserve(pair_counts.len());
    for ((w1, w2, d), c) in pair_counts {
        let c1 = *wordfreq.get(&w1).unwrap_or(&1) as f64;
        let c2 = *wordfreq.get(&w2).unwrap_or(&1) as f64;
        let p_xy = (c as f64) / total_tokens;
        let p_x = c1 / total_tokens;
        let p_y = c2 / total_tokens;
        let pmi = (p_xy / (p_x * p_y)).ln(); // natural log
        out.push(PmiEntry {
            word1: w1,
            word2: w2,
            distance: d,
            count: c,
            pmi,
        });
    }

    // Sort: higher PMI first, then by count
    out.sort_by(|a, b| {
        b.pmi
            .partial_cmp(&a.pmi)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.count.cmp(&a.count))
    });
}

/// Write all outputs for a given result
fn write_all_outputs(
    stem: &str,
    r: &AnalysisResult,
    ts: &str,
    opts: &AnalysisOptions,
) -> Result<(), String> {
    match opts.export_format {
        ExportFormat::Txt => {
            // Human-readable compact summary
            let mut out = String::new();
            out.push_str(&format!("=== N-grams (N={}) ===\n", opts.ngram));
            for (ng, c) in r.ngrams.iter().take(50) {
                out.push_str(&format!("{ng}\t{c}\n"));
            }
            out.push_str("\n=== Word Frequencies ===\n");
            for (w, c) in r.wordfreq.iter().take(50) {
                out.push_str(&format!("{w}\t{c}\n"));
            }
            out.push_str("\n=== Named Entities ===\n");
            for (e, c) in r.named_entities.iter().take(50) {
                out.push_str(&format!("{e}\t{c}\n"));
            }
            out.push_str("\n=== PMI (top 50) ===\n");
            for p in r.pmi.iter().take(50) {
                out.push_str(&format!(
                    "({}, {}) @d={}  PMI={:.3}  count={}\n",
                    p.word1, p.word2, p.distance, p.pmi, p.count
                ));
            }
            let fname = format!("{stem}_{ts}_summary.txt");
            fs::write(&fname, out).map_err(|e| format!("Write txt failed: {e}"))?;
        }
        ExportFormat::Csv | ExportFormat::Tsv | ExportFormat::Json => {
            write_table("ngrams", stem, ts, &r.ngrams, opts)?;
            write_table("wordfreq", stem, ts, &r.wordfreq, opts)?;
            write_nested("context", stem, ts, &r.context_map, opts)?;
            write_nested("neighbors", stem, ts, &r.direct_neighbors, opts)?;
            write_pmi("pmi", stem, ts, &r.pmi, opts)?;
            write_table("namedentities", stem, ts, &r.named_entities, opts)?;
        }
    }
    Ok(())
}

/// Build a short summary for multiple files
fn summary_for<'a>(pairs: &[(String, &'a AnalysisResult)], _opts: &AnalysisOptions) -> String {
    let mut s = String::new();
    s.push_str("=== Analysis Summary ===\n");
    for (name, r) in pairs {
        s.push_str(&format!("\n# {name}\n"));
        s.push_str("Top 10 words:\n");
        let mut wf: Vec<_> = r.wordfreq.iter().collect();
        wf.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
        for (w, c) in wf.into_iter().take(10) {
            s.push_str(&format!("  {w}\t{c}\n"));
        }
        s.push('\n');
    }
    s
}

fn timestamp() -> String {
    Local::now().format("%Y%m%d_%H%M%S").to_string()
}

// ---------- Export helpers ----------

fn write_table(
    name: &str,
    stem: &str,
    ts: &str,
    map: &HashMap<String, usize>,
    opts: &AnalysisOptions,
) -> Result<(), String> {
    let fname = format!("{stem}_{ts}_{name}.{}", ext(opts.export_format));
    match opts.export_format {
        ExportFormat::Csv | ExportFormat::Tsv => {
            let sep = if matches!(opts.export_format, ExportFormat::Csv) {
                ','
            } else {
                '\t'
            };
            let mut f = fs::File::create(&fname).map_err(|e| format!("create {fname}: {e}"))?;
            writeln!(f, "item{}count", sep).map_err(|e| e.to_string())?;
            for (k, v) in map {
                writeln!(f, "{}{sep}{}", k, v).map_err(|e| e.to_string())?;
            }
        }
        ExportFormat::Json => {
            let v: Vec<_> = map
                .iter()
                .map(|(k, v)| serde_json::json!({"item":k,"count":v}))
                .collect();
            fs::write(&fname, serde_json::to_string_pretty(&v).unwrap())
                .map_err(|e| format!("write {fname}: {e}"))?;
        }
        ExportFormat::Txt => unreachable!(),
    }
    Ok(())
}

fn write_nested(
    name: &str,
    stem: &str,
    ts: &str,
    map: &HashMap<String, HashMap<String, usize>>,
    opts: &AnalysisOptions,
) -> Result<(), String> {
    let fname = format!("{stem}_{ts}_{name}.{}", ext(opts.export_format));
    match opts.export_format {
        ExportFormat::Csv | ExportFormat::Tsv => {
            let sep = if matches!(opts.export_format, ExportFormat::Csv) {
                ','
            } else {
                '\t'
            };
            let mut f = fs::File::create(&fname).map_err(|e| format!("create {fname}: {e}"))?;
            writeln!(f, "item1{}item2{}count", sep, sep).map_err(|e| e.to_string())?;
            for (k, inner) in map {
                for (k2, v) in inner {
                    writeln!(f, "{}{sep}{}{sep}{}", k, k2, v).map_err(|e| e.to_string())?;
                }
            }
        }
        ExportFormat::Json => {
            let mut rows = Vec::new();
            for (k, inner) in map {
                for (k2, v) in inner {
                    rows.push(serde_json::json!({"item1":k,"item2":k2,"count":v}));
                }
            }
            fs::write(&fname, serde_json::to_string_pretty(&rows).unwrap())
                .map_err(|e| format!("write {fname}: {e}"))?;
        }
        ExportFormat::Txt => unreachable!(),
    }
    Ok(())
}

fn write_pmi(
    name: &str,
    stem: &str,
    ts: &str,
    pmi: &[PmiEntry],
    opts: &AnalysisOptions,
) -> Result<(), String> {
    let fname = format!("{stem}_{ts}_{name}.{}", ext(opts.export_format));
    match opts.export_format {
        ExportFormat::Csv | ExportFormat::Tsv => {
            let sep = if matches!(opts.export_format, ExportFormat::Csv) {
                ','
            } else {
                '\t'
            };
            let mut f = fs::File::create(&fname).map_err(|e| format!("create {fname}: {e}"))?;
            writeln!(f, "word1{}word2{}distance{}count{}pmi", sep, sep, sep, sep)
                .map_err(|e| e.to_string())?;
            for row in pmi {
                writeln!(
                    f,
                    "{}{sep}{}{sep}{}{sep}{}{sep}{:.6}",
                    row.word1, row.word2, row.distance, row.count, row.pmi
                )
                .map_err(|e| e.to_string())?;
            }
        }
        ExportFormat::Json => {
            let v: Vec<_> = pmi.iter().map(|r|
                serde_json::json!({
                    "word1":r.word1,"word2":r.word2,"distance":r.distance,"count":r.count,"pmi":r.pmi
                })
            ).collect();
            fs::write(&fname, serde_json::to_string_pretty(&v).unwrap())
                .map_err(|e| format!("write {fname}: {e}"))?;
        }
        ExportFormat::Txt => unreachable!(),
    }
    Ok(())
}

fn ext(fmt: ExportFormat) -> &'static str {
    match fmt {
        ExportFormat::Txt => "txt",
        ExportFormat::Csv => "csv",
        ExportFormat::Tsv => "tsv",
        ExportFormat::Json => "json",
    }
}

fn stem_for(p: &std::path::Path) -> String {
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
    let h = short_hash(p); // stable short hash over the full path
    if ext.is_empty() {
        format!("{stem}_{h}")
    } else {
        format!("{stem}.{ext}_{h}")
    }
}

fn short_hash<P: AsRef<std::path::Path>>(p: P) -> String {
    // Simple, fast non-cryptographic short hash
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    p.as_ref().to_string_lossy().hash(&mut hasher);
    let v = hasher.finish();
    // 8 hex chars are enough to disambiguate in practice
    format!("{:08x}", v)
}
