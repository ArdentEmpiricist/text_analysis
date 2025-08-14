#![forbid(unsafe_code)]
#![doc = r#"
Text Analysis Library

This crate provides a fast, pragmatic toolkit for linguistic text analysis over `.txt` and `.pdf`
files. It supports:

- Tokenization (Unicode-aware, simple alphanumeric rules)
- Optional stopword filtering (user-supplied list)
- Optional stemming (auto-detected or forced language)
- N-gram counting
- Word frequency counting
- Context statistics (±N window) and direct neighbors (±1)
- PMI (Pointwise Mutual Information) collocations
- Simple Named-Entity extraction (capitalization heuristic)
- Parallel per-file analysis (compute) with serialized writes
- Combined (Map-Reduce) mode that aggregates counts across files
- **Deterministic, sorted outputs** in CSV/TSV/JSON/TXT


## Security & CSV/TSV export safety

If you open CSV/TSV in spreadsheet software (Excel/LibreOffice), cells that **start with** one of
`=`, `+`, `-`, or `@` may be interpreted as formulas (e.g., `=HYPERLINK(...)`). To prevent this, **always:**
1. Write CSV/TSV using a proper CSV library (this project uses `csv::Writer`) so commas, tabs, quotes, and newlines are escaped correctly.
2. Sanitize **text cells** by prefixing a single quote when they begin with one of the dangerous characters.

"#]

use chrono::Local;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use whatlang::{Lang, detect};

use csv::WriterBuilder;

// PDF parsing is always enabled (no feature flag)
use pdf_extract::extract_text;

// JSON writer for exports
use serde_json;

// ---------- Public API types ----------

/// Export format for analysis outputs.
#[derive(Copy, Clone, Debug)]
pub enum ExportFormat {
    Txt,
    Csv,
    Tsv,
    Json,
}

/// Stemming behavior selector.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum StemMode {
    /// No stemming.
    Off,
    /// Detect language automatically via `whatlang` and stem when supported.
    Auto,
    /// Force a specific stemming language.
    Force(StemLang),
}

/// Supported stemming languages (subset of `rust-stemmers`).
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
    /// Map a short CLI code (e.g., "en", "de") to `StemLang`.
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
    /// Map a `whatlang::Lang` to `StemLang`. Unknown mappings become `Unknown`.
    pub fn from_whatlang(lang: Lang) -> Self {
        // Use ISO-639-3 codes to be robust across whatlang versions
        match lang.code() {
            "eng" => StemLang::En,
            "deu" => StemLang::De,
            "fra" | "fre" => StemLang::Fr,
            "spa" => StemLang::Es,
            "ita" => StemLang::It,
            "por" => StemLang::Pt,
            "nld" | "dut" => StemLang::Nl,
            "rus" => StemLang::Ru,
            "swe" => StemLang::Sv,
            "fin" => StemLang::Fi,
            "nor" | "nob" | "nno" => StemLang::No,
            "ron" | "rum" => StemLang::Ro,
            "hun" => StemLang::Hu,
            "dan" => StemLang::Da,
            "tur" => StemLang::Tr,
            _ => StemLang::Unknown,
        }
    }
}

/// Parameters controlling analysis and export behavior.
#[derive(Clone, Debug)]
pub struct AnalysisOptions {
    /// N-gram size (>=1 recommended; 2 = bigrams).
    pub ngram: usize,
    /// Context window (±N) for context statistics and PMI.
    pub context: usize,
    /// Export format for files (TXT/CSV/TSV/JSON).
    pub export_format: ExportFormat,
    /// If true, export only Named Entities (skips other tables).
    pub entities_only: bool,
    /// If true, aggregate all files into one corpus (Map-Reduce). Otherwise per-file outputs.
    pub combine: bool,
    /// Stemming mode (off/auto/force).
    pub stem_mode: StemMode,
    /// If true and `stem_mode == Auto`, require detectable & supported language; otherwise fail.
    /// - Per-file: file is skipped and reported in `failed_files`, run continues (success).
    /// - Combined: the whole run aborts with an error to avoid mixed stemming.
    pub stem_require_detected: bool,
}

/// Summary of a completed run.
#[derive(Debug)]
pub struct AnalysisReport {
    /// Human-readable summary (top words per output).
    pub summary: String,
    /// (file_path, error) pairs for unreadable or skipped inputs.
    pub failed_files: Vec<(String, String)>,
}

/// Full analysis result for a single text/corpus.
#[derive(Debug, Default)]
pub struct AnalysisResult {
    pub ngrams: HashMap<String, usize>,
    pub wordfreq: HashMap<String, usize>,
    pub context_map: HashMap<String, HashMap<String, usize>>,
    pub direct_neighbors: HashMap<String, HashMap<String, usize>>,
    pub named_entities: HashMap<String, usize>,
    pub pmi: Vec<PmiEntry>,
}

/// PMI entry for a pair of words at a given distance.
#[derive(Debug)]
pub struct PmiEntry {
    pub word1: String,
    pub word2: String,
    pub distance: usize,
    pub count: usize,
    pub pmi: f64,
}

// ---------- Map-Reduce internal structures ----------

/// Partial counts emitted by the *map* stage for a single file.
#[derive(Default)]
struct PartialCounts {
    n_tokens: usize,
    ngrams: HashMap<String, usize>,
    wordfreq: HashMap<String, usize>,
    context_pairs: HashMap<(String, String), usize>,
    neighbor_pairs: HashMap<(String, String), usize>,
    cooc_by_dist: HashMap<(String, String, usize), usize>,
    named_entities: HashMap<String, usize>,
}

// ---------- High-level entry point ----------

/// Analyze a path (file or directory).  
/// - Per-file mode: compute in parallel per file; write outputs per file (serialized).  
/// - Combined mode: Map-Reduce over files; write a single combined set of outputs.
pub fn analyze_path(
    path: &Path,
    stopwords_file: Option<&PathBuf>,
    options: &AnalysisOptions,
) -> Result<AnalysisReport, String> {
    let files = collect_files(path);
    if files.is_empty() {
        return Err("No .txt or .pdf files found for analysis.".to_string());
    }

    let stopwords = load_stopwords(stopwords_file);
    let mut failed: Vec<(String, String)> = Vec::new();
    let ts = timestamp();

    // --- Combined Map-Reduce mode ---
    if options.combine {
        // Map: read + build partial counts in parallel.
        let mapped: Vec<_> = files
            .par_iter()
            .map(|f| match read_text(f) {
                Ok(t) => {
                    if matches!(options.stem_mode, StemMode::Auto) && options.stem_require_detected
                    {
                        if detect_supported_stem_lang(&t).is_none() {
                            return Err((
                                f.display().to_string(),
                                "Language detection failed or unsupported for stemming (strict)"
                                    .to_string(),
                            ));
                        }
                    }
                    Ok(partial_counts_from_text(&t, &stopwords, options))
                }
                Err(e) => Err((f.display().to_string(), e)),
            })
            .collect();

        // Reduce: merge partials, collect failures.
        let mut total = PartialCounts::default();
        let mut failed_local: Vec<(String, String)> = Vec::new();
        for item in mapped {
            match item {
                Ok(pc) => merge_counts(&mut total, pc),
                Err(fe) => failed_local.push(fe),
            }
        }
        if options.stem_require_detected && !failed_local.is_empty() {
            // Fail the combined run to avoid mixed stemming.
            let msg = format!(
                "Combined run aborted (strict stemming): {} file(s) without detectable/supported language",
                failed_local.len()
            );
            return Err(msg);
        }
        failed.extend(failed_local);

        // Finalize: build one `AnalysisResult`, export once.
        let result = analysis_from_counts(total);
        write_all_outputs("combined", &result, &ts, options)?;
        let summary = summary_for(&[("combined".to_string(), &result)], options);
        return Ok(AnalysisReport {
            summary,
            failed_files: failed,
        });
    }

    // --- Per-file mode: parallel compute, serialized writes ---
    let results: Vec<_> = files
        .par_iter()
        .map(|f| match read_text(f) {
            Ok(t) => {
                if matches!(options.stem_mode, StemMode::Auto) && options.stem_require_detected {
                    if detect_supported_stem_lang(&t).is_none() {
                        return Err((
                            f.display().to_string(),
                            "Language detection failed or unsupported for stemming (strict)"
                                .to_string(),
                        ));
                    }
                }
                let r = analyze_text_with(&t, &stopwords, options);
                let stem = stem_for(f);
                Ok((stem, r))
            }
            Err(e) => Err((f.display().to_string(), e)),
        })
        .collect();

    let mut per_file_results: Vec<(String, AnalysisResult)> = Vec::new();
    for item in results {
        match item {
            Ok(v) => per_file_results.push(v),
            Err(fe) => failed.push(fe),
        }
    }

    // Writes are serialized to reduce I/O contention.
    for (stem, r) in &per_file_results {
        write_all_outputs(stem, r, &ts, options)?;
    }

    // Human-readable summary
    let pairs: Vec<(String, &AnalysisResult)> = per_file_results
        .iter()
        .map(|(n, r)| (n.clone(), r))
        .collect();
    let summary = summary_for(&pairs, options);
    Ok(AnalysisReport {
        summary,
        failed_files: failed,
    })
}

// ---------- File discovery ----------

/// Collect all supported files (.txt, .pdf) recursively from `path`.
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

// ---------- Reading & preprocessing ----------

/// Read the text from `.txt` or `.pdf`. Returns a displayable error string on failure.
fn read_text(p: &Path) -> Result<String, String> {
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "txt" => fs::read_to_string(p).map_err(|e| format!("Read .txt failed: {e}")),
        "pdf" => extract_text(p).map_err(|e| format!("PDF extract failed: {e}")),
        _ => Err("Unsupported extension".to_string()),
    }
}

/// Load stopwords from a text file (one word per line). Empty or unreadable files yield an empty set.
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

// ---------- Core analysis (per text) ----------

/// Analyze a single text buffer with the given `stopwords` and `options`.
/// This is the core pipeline used by both per-file and combined modes.
pub fn analyze_text_with(
    text: &str,
    stopwords: &HashSet<String>,
    opts: &AnalysisOptions,
) -> AnalysisResult {
    // Determine stemming language once per text (not per token).
    let stem_lang = match opts.stem_mode {
        StemMode::Off => StemLang::Unknown,
        StemMode::Force(lang) => lang,
        StemMode::Auto => detect(text)
            .map(|i| StemLang::from_whatlang(i.lang()))
            .unwrap_or(StemLang::Unknown),
    };

    // Tokenize original and normalize for stats.
    let original_tokens = tokenize(text);
    let sentences = split_sentences(text);
    let tokens_for_stats = normalize_for_stats(&original_tokens, stopwords, stem_lang);

    let mut result = AnalysisResult::default();
    ngrams_count(&tokens_for_stats, opts.ngram, &mut result.ngrams);
    wordfreq_count(&tokens_for_stats, &mut result.wordfreq);
    context_and_neighbors(
        &tokens_for_stats,
        opts.context,
        &mut result.context_map,
        &mut result.direct_neighbors,
    );
    // NER is based on original, *non-stemmed*, case-sensitive tokens.
    named_entities_heuristic(&original_tokens, &sentences, &mut result.named_entities);
    // PMI uses normalized tokens, consistent with other statistics.
    compute_pmi(
        &tokens_for_stats,
        opts.context,
        &result.wordfreq,
        &mut result.pmi,
    );

    result
}

/// Simple tokenizer: keeps alphanumerics and `'` inside tokens, splits on everything else.
fn tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::with_capacity(text.len() / 5);
    let mut cur = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '\'' {
            cur.push(ch);
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Sentence boundary detection: record byte offsets after '.', '!' or '?'.
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

/// Normalize tokens for statistics: lowercase, optional stopword removal, optional stemming.
fn normalize_for_stats(
    tokens: &[String],
    stopwords: &HashSet<String>,
    stem_lang: StemLang,
) -> Vec<String> {
    let mut out = Vec::with_capacity(tokens.len());
    let stemmer = make_stemmer(stem_lang); // create once, reuse
    for t in tokens {
        let lower = t.to_lowercase();
        if !stopwords.is_empty() && stopwords.contains(&lower) {
            continue;
        }
        let normalized = if let Some(stem) = &stemmer {
            stem.stem(&lower).to_string()
        } else {
            lower
        };
        out.push(normalized);
    }
    out
}

/// Construct a `rust-stemmers` instance for the given language. Returns `None` if unsupported.
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

/// Count N-grams of size `n` into `out`.
fn ngrams_count(tokens: &[String], n: usize, out: &mut HashMap<String, usize>) {
    if n == 0 || tokens.len() < n {
        return;
    }
    for i in 0..=tokens.len() - n {
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

/// Count individual word frequencies.
fn wordfreq_count(tokens: &[String], out: &mut HashMap<String, usize>) {
    for t in tokens {
        *out.entry(t.clone()).or_insert(0) += 1;
    }
}

/// Build context (±window) counts and direct (±1) neighbor counts.
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

        let entry = context_map.entry(w.clone()).or_insert_with(HashMap::new);
        for j in left..right {
            if j == i {
                continue;
            }
            *entry.entry(tokens[j].clone()).or_insert(0) += 1;
        }

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

/// Naive Named-Entity heuristic:
/// - Token must start with an uppercase letter
/// - Token must not be all uppercase (filters acronyms)
/// - Filter a small set of very common determiners/articles in multiple languages
/// Counts are case-sensitive.
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
            if tok.chars().all(|c| !c.is_lowercase()) {
                continue;
            }
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

/// Compute PMI (Pointwise Mutual Information) for all pairs within ±`window`.
/// Pairs are stored canonically (`w1 <= w2`) and include the absolute distance `d`.
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

    out.clear();
    out.reserve(pair_counts.len());
    for ((w1, w2, d), c) in pair_counts {
        let c1 = *wordfreq.get(&w1).unwrap_or(&1) as f64;
        let c2 = *wordfreq.get(&w2).unwrap_or(&1) as f64;
        let p_xy = (c as f64) / total_tokens;
        let p_x = c1 / total_tokens;
        let p_y = c2 / total_tokens;
        let pmi = (p_xy / (p_x * p_y)).ln();
        out.push(PmiEntry {
            word1: w1,
            word2: w2,
            distance: d,
            count: c,
            pmi,
        });
    }

    // In-memory order: PMI desc, then count desc for stability.
    out.sort_by(|a, b| {
        b.pmi
            .partial_cmp(&a.pmi)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.count.cmp(&a.count))
    });
}

// ---------- Map-Reduce helpers ----------

/// Build partial counts for a single text buffer (map stage).
fn partial_counts_from_text(
    text: &str,
    stopwords: &HashSet<String>,
    opts: &AnalysisOptions,
) -> PartialCounts {
    let stem_lang = match opts.stem_mode {
        StemMode::Off => StemLang::Unknown,
        StemMode::Force(lang) => lang,
        StemMode::Auto => detect(text)
            .map(|i| StemLang::from_whatlang(i.lang()))
            .unwrap_or(StemLang::Unknown),
    };

    let original_tokens = tokenize(text);
    let tokens_for_stats = normalize_for_stats(&original_tokens, stopwords, stem_lang);
    let n = tokens_for_stats.len();

    let mut pc = PartialCounts::default();
    pc.n_tokens = n;

    // N-grams
    if opts.ngram > 0 && n >= opts.ngram {
        for i in 0..=n - opts.ngram {
            let mut buf = String::with_capacity(opts.ngram * 6);
            for (k, t) in tokens_for_stats[i..i + opts.ngram].iter().enumerate() {
                if k > 0 {
                    buf.push(' ');
                }
                buf.push_str(t);
            }
            *pc.ngrams.entry(buf).or_insert(0) += 1;
        }
    }

    // Word frequencies
    for t in &tokens_for_stats {
        *pc.wordfreq.entry(t.clone()).or_insert(0) += 1;
    }

    // Context, neighbors, co-occurrence-by-distance for PMI
    let window = opts.context;
    if window > 0 && n > 0 {
        for (i, w) in tokens_for_stats.iter().enumerate() {
            let left = i.saturating_sub(window);
            let right = (i + window + 1).min(n);
            for j in left..right {
                if j == i {
                    continue;
                }
                // context
                let key_ctx = (w.clone(), tokens_for_stats[j].clone());
                *pc.context_pairs.entry(key_ctx).or_insert(0) += 1;

                // PMI pair with distance
                let (a, b) = if w <= &tokens_for_stats[j] {
                    (w.clone(), tokens_for_stats[j].clone())
                } else {
                    (tokens_for_stats[j].clone(), w.clone())
                };
                let d = (i as isize - j as isize).abs() as usize;
                *pc.cooc_by_dist.entry((a, b, d)).or_insert(0) += 1;
            }

            // direct neighbors (±1)
            if i > 0 {
                let key_left = (w.clone(), tokens_for_stats[i - 1].clone());
                *pc.neighbor_pairs.entry(key_left).or_insert(0) += 1;
            }
            if i + 1 < n {
                let key_right = (w.clone(), tokens_for_stats[i + 1].clone());
                *pc.neighbor_pairs.entry(key_right).or_insert(0) += 1;
            }
        }
    }

    // NER on original tokens
    let mut ner = HashMap::new();
    let sentences = split_sentences(text);
    named_entities_heuristic(&original_tokens, &sentences, &mut ner);
    pc.named_entities = ner;

    pc
}

/// Merge `other` into `into` (reduce stage).
fn merge_counts(into: &mut PartialCounts, other: PartialCounts) {
    into.n_tokens += other.n_tokens;
    for (k, v) in other.ngrams {
        *into.ngrams.entry(k).or_insert(0) += v;
    }
    for (k, v) in other.wordfreq {
        *into.wordfreq.entry(k).or_insert(0) += v;
    }
    for (k, v) in other.context_pairs {
        *into.context_pairs.entry(k).or_insert(0) += v;
    }
    for (k, v) in other.neighbor_pairs {
        *into.neighbor_pairs.entry(k).or_insert(0) += v;
    }
    for (k, v) in other.cooc_by_dist {
        *into.cooc_by_dist.entry(k).or_insert(0) += v;
    }
    for (k, v) in other.named_entities {
        *into.named_entities.entry(k).or_insert(0) += v;
    }
}

/// Build a full `AnalysisResult` from reduced counts.
fn analysis_from_counts(total: PartialCounts) -> AnalysisResult {
    let mut result = AnalysisResult::default();
    result.ngrams = total.ngrams;
    result.wordfreq = total.wordfreq;
    result.named_entities = total.named_entities;

    for ((center, neighbor), c) in total.context_pairs {
        let entry = result
            .context_map
            .entry(center)
            .or_insert_with(HashMap::new);
        *entry.entry(neighbor).or_insert(0) += c;
    }
    for ((center, neighbor), c) in total.neighbor_pairs {
        let entry = result
            .direct_neighbors
            .entry(center)
            .or_insert_with(HashMap::new);
        *entry.entry(neighbor).or_insert(0) += c;
    }

    result.pmi = pmi_from_global_counts(&total.cooc_by_dist, total.n_tokens, &result.wordfreq);
    result
}

/// Compute PMI from global co-occurrence counts (by distance), total token count and unigram counts.
fn pmi_from_global_counts(
    cooc_by_dist: &HashMap<(String, String, usize), usize>,
    n_tokens: usize,
    wordfreq: &HashMap<String, usize>,
) -> Vec<PmiEntry> {
    if n_tokens == 0 {
        return Vec::new();
    }
    let total = n_tokens as f64;
    let mut out = Vec::with_capacity(cooc_by_dist.len());
    for ((w1, w2, d), c) in cooc_by_dist {
        let c1 = *wordfreq.get(w1).unwrap_or(&1) as f64;
        let c2 = *wordfreq.get(w2).unwrap_or(&1) as f64;
        let p_xy = (*c as f64) / total;
        let p_x = c1 / total;
        let p_y = c2 / total;
        let pmi = (p_xy / (p_x * p_y)).ln();
        out.push(PmiEntry {
            word1: w1.clone(),
            word2: w2.clone(),
            distance: *d,
            count: *c,
            pmi,
        });
    }
    // In-memory order for PMI results: PMI desc, then count desc.
    out.sort_by(|a, b| {
        b.pmi
            .partial_cmp(&a.pmi)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.count.cmp(&a.count))
    });
    out
}

// ---------- Output helpers (ALL SORTED) ----------

/// Write all outputs for a single result using the configured format.
fn write_all_outputs(
    stem: &str,
    r: &AnalysisResult,
    ts: &str,
    opts: &AnalysisOptions,
) -> Result<(), String> {
    if opts.entities_only {
        // Entities-only export path (sorted)
        match opts.export_format {
            ExportFormat::Txt => {
                let mut out = String::new();
                out.push_str("=== Named Entities ===\n");
                let mut items: Vec<(&String, &usize)> = r.named_entities.iter().collect();
                items.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
                for (e, c) in items.into_iter().take(2000) {
                    out.push_str(&format!("{e}\t{c}\n"));
                }
                let fname = format!("{stem}_{ts}_entities.txt");
                fs::write(&fname, out).map_err(|e| format!("Write txt failed: {e}"))?;
            }
            ExportFormat::Csv | ExportFormat::Tsv | ExportFormat::Json => {
                write_table("entities", stem, ts, &r.named_entities, opts)?;
            }
        }
        return Ok(());
    }

    match opts.export_format {
        ExportFormat::Txt => {
            // Human-readable TXT (sorted sections; top-50 only)
            let mut out = String::new();

            // N-grams
            out.push_str(&format!("=== N-grams (N={}) ===\n", opts.ngram));
            let mut ngram_items: Vec<(&String, &usize)> = r.ngrams.iter().collect();
            ngram_items.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
            for (ng, c) in ngram_items.into_iter().take(50) {
                out.push_str(&format!("{ng}\t{c}\n"));
            }

            // Word frequencies
            out.push_str("\n=== Word Frequencies ===\n");
            let mut wf_items: Vec<(&String, &usize)> = r.wordfreq.iter().collect();
            wf_items.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
            for (w, c) in wf_items.into_iter().take(50) {
                out.push_str(&format!("{w}\t{c}\n"));
            }

            // Named Entities
            out.push_str("\n=== Named Entities ===\n");
            let mut ne_items: Vec<(&String, &usize)> = r.named_entities.iter().collect();
            ne_items.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
            for (e, c) in ne_items.into_iter().take(50) {
                out.push_str(&format!("{e}\t{c}\n"));
            }

            // PMI
            out.push_str("\n=== PMI (top 50, by count) ===\n");
            let mut pmi_rows: Vec<&PmiEntry> = r.pmi.iter().collect();
            pmi_rows.sort_by(|a, b| {
                b.count
                    .cmp(&a.count)
                    .then_with(|| {
                        b.pmi
                            .partial_cmp(&a.pmi)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .then_with(|| a.word1.cmp(&b.word1))
                    .then_with(|| a.word2.cmp(&b.word2))
            });
            for p in pmi_rows.into_iter().take(50) {
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

/// Write a simple map table as CSV/TSV/JSON. Content is **sorted by count desc, key asc**.
/// Write a flat table `<item -> count>` as CSV/TSV/JSON.
/// CSV/TSV are emitted via `csv::Writer` (proper quoting & newlines),
/// and **text cells** are sanitized with `csv_safe_cell()` to neutralize
/// leading `= + - @` (spreadsheet formula injection).
fn write_table(
    name: &str,
    stem: &str,
    ts: &str,
    map: &std::collections::HashMap<String, usize>,
    opts: &AnalysisOptions,
) -> Result<(), String> {
    let fname = format!("{stem}_{ts}_{name}.{}", ext(opts.export_format));

    // Deterministic order: count desc, then key asc
    let mut items: Vec<(&String, &usize)> = map.iter().collect();
    items.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));

    match opts.export_format {
        ExportFormat::Csv | ExportFormat::Tsv => {
            let delim: u8 = if matches!(opts.export_format, ExportFormat::Csv) {
                b','
            } else {
                b'\t'
            };
            let file = std::fs::File::create(&fname).map_err(|e| format!("create {fname}: {e}"))?;
            let mut wtr = csv::WriterBuilder::new().delimiter(delim).from_writer(file);

            // header
            wtr.write_record(["item", "count"])
                .map_err(|e| e.to_string())?;

            for (k, v) in items {
                wtr.write_record([csv_safe_cell(k.to_string()), v.to_string()])
                    .map_err(|e| e.to_string())?;
            }
            wtr.flush().map_err(|e| e.to_string())?;
        }
        ExportFormat::Json => {
            let v: Vec<_> = items
                .iter()
                .map(|(k, v)| serde_json::json!({ "item": k, "count": v }))
                .collect();
            std::fs::write(&fname, serde_json::to_string_pretty(&v).unwrap())
                .map_err(|e| format!("write {fname}: {e}"))?;
        }
        ExportFormat::Txt => unreachable!(),
    }
    Ok(())
}

/// Write a nested map `<center -> neighbor -> count>` as a flat table (sorted by count desc).
/// Write a nested map `<center -> neighbor -> count>` as a flat table
/// with columns: `item1, item2, count`.
/// Uses `csv::Writer` for CSV/TSV and sanitizes text cells with `csv_safe_cell()`.
fn write_nested(
    name: &str,
    stem: &str,
    ts: &str,
    map: &std::collections::HashMap<String, std::collections::HashMap<String, usize>>,
    opts: &AnalysisOptions,
) -> Result<(), String> {
    let fname = format!("{stem}_{ts}_{name}.{}", ext(opts.export_format));

    // Flatten + deterministic order: count desc, then keys
    let mut rows: Vec<(&String, &String, &usize)> = Vec::new();
    for (k, inner) in map {
        for (k2, v) in inner {
            rows.push((k, k2, v));
        }
    }
    rows.sort_by(|a, b| {
        b.2.cmp(a.2)
            .then_with(|| a.0.cmp(b.0))
            .then_with(|| a.1.cmp(b.1))
    });

    match opts.export_format {
        ExportFormat::Csv | ExportFormat::Tsv => {
            let delim: u8 = if matches!(opts.export_format, ExportFormat::Csv) {
                b','
            } else {
                b'\t'
            };
            let file = std::fs::File::create(&fname).map_err(|e| format!("create {fname}: {e}"))?;
            let mut wtr = csv::WriterBuilder::new().delimiter(delim).from_writer(file);

            // header
            wtr.write_record(["item1", "item2", "count"])
                .map_err(|e| e.to_string())?;

            for (k, k2, v) in rows {
                wtr.write_record([
                    csv_safe_cell(k.to_string()),
                    csv_safe_cell(k2.to_string()),
                    v.to_string(),
                ])
                .map_err(|e| e.to_string())?;
            }
            wtr.flush().map_err(|e| e.to_string())?;
        }
        ExportFormat::Json => {
            let v: Vec<_> = rows
                .iter()
                .map(|(k, k2, v)| serde_json::json!({ "item1": k, "item2": k2, "count": v }))
                .collect();
            std::fs::write(&fname, serde_json::to_string_pretty(&v).unwrap())
                .map_err(|e| format!("write {fname}: {e}"))?;
        }
        ExportFormat::Txt => unreachable!(),
    }
    Ok(())
}

/// Write PMI entries **sorted by count desc, then PMI desc, then words lex**.
/// Write PMI rows with columns: `word1, word2, distance, count, pmi`.
/// Sorted by `count desc, PMI desc, then words`. CSV/TSV via `csv::Writer`,
/// **text cells** sanitized via `csv_safe_cell()`.
fn write_pmi(
    name: &str,
    stem: &str,
    ts: &str,
    pmi: &[PmiEntry], // assumes fields: word1, word2, distance, count, pmi
    opts: &AnalysisOptions,
) -> Result<(), String> {
    let fname = format!("{stem}_{ts}_{name}.{}", ext(opts.export_format));

    // Deterministic order
    let mut rows: Vec<&PmiEntry> = pmi.iter().collect();
    rows.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| {
                b.pmi
                    .partial_cmp(&a.pmi)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.word1.cmp(&b.word1))
            .then_with(|| a.word2.cmp(&b.word2))
    });

    match opts.export_format {
        ExportFormat::Csv | ExportFormat::Tsv => {
            let delim: u8 = if matches!(opts.export_format, ExportFormat::Csv) {
                b','
            } else {
                b'\t'
            };
            let file = std::fs::File::create(&fname).map_err(|e| format!("create {fname}: {e}"))?;
            let mut wtr = csv::WriterBuilder::new().delimiter(delim).from_writer(file);

            // header
            wtr.write_record(["word1", "word2", "distance", "count", "pmi"])
                .map_err(|e| e.to_string())?;

            for r in rows {
                wtr.write_record([
                    csv_safe_cell(r.word1.clone()),
                    csv_safe_cell(r.word2.clone()),
                    r.distance.to_string(),
                    r.count.to_string(),
                    format!("{:.6}", r.pmi),
                ])
                .map_err(|e| e.to_string())?;
            }
            wtr.flush().map_err(|e| e.to_string())?;
        }
        ExportFormat::Json => {
            let v: Vec<_> = rows
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "word1": r.word1,
                        "word2": r.word2,
                        "distance": r.distance,
                        "count": r.count,
                        "pmi": r.pmi
                    })
                })
                .collect();
            std::fs::write(&fname, serde_json::to_string_pretty(&v).unwrap())
                .map_err(|e| format!("write {fname}: {e}"))?;
        }
        ExportFormat::Txt => unreachable!(),
    }
    Ok(())
}

// ---------- Utilities ----------

/// Build a human-readable summary for debug/logging.
fn summary_for<'a>(pairs: &[(String, &'a AnalysisResult)], _opts: &AnalysisOptions) -> String {
    // STDOUT summary is tuned for usefulness:
    // 1) Top 20 N-grams (sorted by count desc, then key lex asc)
    // 2) Top 20 PMI pairs (sorted by count desc, then PMI desc, then words lex)
    // 3) Top 20 words (sorted by count desc, then key lex asc)
    //
    // This order surfaces more informative signals before common stopwords.
    let mut s = String::new();
    s.push_str("=== Analysis Summary ===\n");

    for (name, r) in pairs {
        s.push_str(&format!("\n# {}\n", name));

        // ---- Top 20 N-grams ----
        s.push_str("Top 20 n-grams:\n");
        let mut ngram_items: Vec<(&String, &usize)> = r.ngrams.iter().collect();
        ngram_items.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        for (ng, c) in ngram_items.into_iter().take(20) {
            s.push_str(&format!("  {}\t{}\n", ng, c));
        }

        // ---- Top 20 PMI ----
        s.push_str("Top 20 PMI (by count, then PMI):\n");
        let mut pmi_rows: Vec<&PmiEntry> = r.pmi.iter().collect();
        pmi_rows.sort_by(|a, b| {
            b.count
                .cmp(&a.count)
                .then_with(|| {
                    b.pmi
                        .partial_cmp(&a.pmi)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| a.word1.cmp(&b.word1))
                .then_with(|| a.word2.cmp(&b.word2))
        });
        for p in pmi_rows.into_iter().take(20) {
            s.push_str(&format!(
                "  ({}, {}) @d={}  count={}  PMI={:.3}\n",
                p.word1, p.word2, p.distance, p.count, p.pmi
            ));
        }

        // ---- Top 20 words ----
        s.push_str("Top 20 words:\n");
        let mut wf_items: Vec<(&String, &usize)> = r.wordfreq.iter().collect();
        wf_items.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        for (w, c) in wf_items.into_iter().take(20) {
            s.push_str(&format!("  {}\t{}\n", w, c));
        }
    }

    s
}

/// A short timestamp used in output filenames.
fn timestamp() -> String {
    Local::now().format("%Y%m%d_%H%M%S").to_string()
}

/// File extension for an export format.
fn ext(fmt: ExportFormat) -> &'static str {
    match fmt {
        ExportFormat::Txt => "txt",
        ExportFormat::Csv => "csv",
        ExportFormat::Tsv => "tsv",
        ExportFormat::Json => "json",
    }
}

/// Collision-safe stem used in output filenames: "<stem[.ext]>_<hash8>".
/// The hash is a stable hash of the full path to avoid collisions across parallel runs.
pub fn stem_for(p: &Path) -> String {
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
    let h = short_hash(p);
    if ext.is_empty() {
        format!("{stem}_{h}")
    } else {
        format!("{stem}.{ext}_{h}")
    }
}

fn short_hash<P: AsRef<Path>>(p: P) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    p.as_ref().to_string_lossy().hash(&mut hasher);
    let v = hasher.finish();
    format!("{:08x}", v)
}

/// Detect a supported stemming language. Returns `None` if undetected or unsupported.
fn detect_supported_stem_lang(text: &str) -> Option<StemLang> {
    let info = whatlang::detect(text)?;
    let sl = StemLang::from_whatlang(info.lang());
    if make_stemmer(sl).is_some() {
        Some(sl)
    } else {
        None
    }
}

pub fn csv_safe_cell(mut s: String) -> String {
    if matches!(s.chars().next(), Some('=' | '+' | '-' | '@')) {
        s.insert(0, '\'');
    }
    s
}
