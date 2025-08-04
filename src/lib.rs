use chrono::Local;
use pdf_extract::extract_text;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use whatlang::detect;

/// Supported export formats
#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum ExportFormat {
    Txt,
    Csv,
    Tsv,
    Json,
}

/// Struct for PMI Collocations (for test compatibility and export)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PmiEntry {
    pub word1: String,
    pub word2: String,
    pub distance: i32,
    pub pmi: f64,
    pub count: usize,
}

/// Struct with all statistics for a text
pub struct AnalysisResult {
    pub ngrams: HashMap<String, usize>,
    pub wordfreq: HashMap<String, usize>,
    pub context: HashMap<String, HashMap<String, usize>>,
    pub direct_neighbors: HashMap<String, HashMap<String, usize>>,
    pub named_entities: HashMap<String, usize>,
    pub pmi: Vec<PmiEntry>,
}

impl AnalysisResult {
    /// Generate human-readable TXT summary (for stdout, not for export)
    pub fn summary(&self) -> String {
        let mut out = String::new();

        out.push_str("=== N-gram Analysis ===\n");
        for (ngram, count) in self.ngrams.iter().take(20) {
            out.push_str(&format!("Ngram: \"{}\" — Count: {}\n", ngram, count));
        }
        out.push_str("\n=== Word Frequencies and Context ===\n");
        for (word, freq) in self.wordfreq.iter().take(20) {
            out.push_str(&format!("Word: \"{}\" — Frequency: {}\n", word, freq));
            if let Some(ctx) = self.context.get(word) {
                out.push_str("    Words near: ");
                for (w, c) in ctx.iter().take(5) {
                    out.push_str(&format!("(\"{}\", {}), ", w, c));
                }
                out.push('\n');
            }
            if let Some(neigh) = self.direct_neighbors.get(word) {
                out.push_str("    Direct neighbors: ");
                for (w, c) in neigh.iter().take(5) {
                    out.push_str(&format!("(\"{}\", {}), ", w, c));
                }
                out.push('\n');
            }
        }
        out.push_str("\n=== Named Entities ===\n");
        for (ent, count) in self.named_entities.iter().take(20) {
            out.push_str(&format!("  {:20} — Count: {}\n", ent, count));
        }
        out.push_str("\n=== PMI Collocations (min_count=5, top 20) ===\n");
        for entry in self.pmi.iter().take(20) {
            out.push_str(&format!(
                "({:>8}, {:>8}) @ d={:>2}  PMI={:.2}  count={}\n",
                entry.word1, entry.word2, entry.distance, entry.pmi, entry.count
            ));
        }

        out
    }
}

/// Analysis report for a (single or combined) analysis run
pub struct AnalysisReport {
    pub result: String,
    pub failed_files: Vec<String>,
}

/// Recursively collect all .txt and .pdf files from a given path.
pub fn collect_files(path: &Path) -> Vec<String> {
    let mut files = Vec::new();
    if path.is_file() {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if ext == "txt" || ext == "pdf" {
            files.push(path.to_string_lossy().to_string());
        }
    } else if path.is_dir() {
        for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
            let entry_path = entry.path();
            if entry_path.is_file() {
                let ext = entry_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if ext == "txt" || ext == "pdf" {
                    files.push(entry_path.to_string_lossy().to_string());
                }
            }
        }
    }
    files
}

/// Analyze all files as separate documents ("default mode")
pub fn analyze_path(
    path: &str,
    stopwords: Option<String>,
    ngram: usize,
    context: usize,
    export_format: ExportFormat,
    entities_only: bool,
) -> Result<AnalysisReport, String> {
    let input_path = Path::new(path);
    let files = collect_files(input_path);

    if files.is_empty() {
        return Err("No .txt or .pdf files found for analysis.".to_string());
    }

    let stopword_set = load_stopwords(stopwords);

    let mut failed_files = Vec::new();
    let mut report_text = String::new();

    for file in &files {
        match read_text_file(file) {
            Ok(txt) => {
                let analysis = analyze_text(&txt, &stopword_set, ngram, context);

                let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
                let stem = Path::new(file)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("file");
                let out_prefix = format!("{}_{}", stem, timestamp);

                export_results(&analysis, &out_prefix, export_format, entities_only);

                if !entities_only {
                    report_text.push_str(&format!(
                        "\n=== Result for {} ===\n{}\n",
                        file,
                        analysis.summary()
                    ));
                }
            }
            Err(e) => failed_files.push(format!("{}: {}", file, e)),
        }
    }

    Ok(AnalysisReport {
        result: report_text,
        failed_files,
    })
}

/// Analyze all found files as a single corpus ("--combine" mode).
pub fn analyze_path_combined(
    path: &str,
    stopwords: Option<String>,
    ngram: usize,
    context: usize,
    export_format: ExportFormat,
    entities_only: bool,
) -> Result<AnalysisReport, String> {
    let files = collect_files(Path::new(path));
    if files.is_empty() {
        return Err("No .txt or .pdf files found for analysis.".to_string());
    }

    let stopword_set = load_stopwords(stopwords);

    let mut failed_files = Vec::new();
    let mut texts = Vec::new();

    for file in &files {
        match read_text_file(file) {
            Ok(txt) => texts.push(txt),
            Err(e) => failed_files.push(format!("{}: {}", file, e)),
        }
    }
    if texts.is_empty() {
        return Err("No files could be read.".to_string());
    }

    let full_text = texts.join("\n");

    let analysis = analyze_text(&full_text, &stopword_set, ngram, context);

    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let out_prefix = format!("combined_{}", timestamp);

    export_results(&analysis, &out_prefix, export_format, entities_only);

    Ok(AnalysisReport {
        result: analysis.summary(),
        failed_files,
    })
}

/// Print any files that failed to be processed.
pub fn print_failed_files(failed_files: &[String]) {
    if !failed_files.is_empty() {
        eprintln!("Warning: The following files could not be read:");
        for file in failed_files {
            eprintln!("  {}", file);
        }
    }
}

/// Read a text or PDF file (real PDF parsing).
fn read_text_file(path: &str) -> Result<String, String> {
    let p = Path::new(path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext == "txt" {
        fs::read_to_string(path).map_err(|e| format!("Failed to read txt: {}", e))
    } else if ext == "pdf" {
        extract_text(path).map_err(|e| format!("Failed to extract pdf: {}", e))
    } else {
        Err("Unsupported file extension".into())
    }
}

/// Load stopwords from file if given; else empty set.
pub fn load_stopwords(stopwords: Option<String>) -> HashSet<String> {
    let mut set = HashSet::new();
    if let Some(path) = stopwords {
        if let Ok(content) = fs::read_to_string(&path) {
            for line in content.lines() {
                let w = line.trim().to_lowercase();
                if !w.is_empty() {
                    set.insert(w);
                }
            }
        }
    }
    set
}

/// Main text analysis function.
pub fn analyze_text(
    text: &str,
    stopwords: &HashSet<String>,
    ngram: usize,
    context: usize,
) -> AnalysisResult {
    let _lang = detect(text).map(|info| info.lang().code()).unwrap_or("en");
    let words: Vec<String> = text
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .filter(|w| !stopwords.contains(w))
        .collect();

    // N-Gram stats
    let mut ngrams = HashMap::new();
    for ngram_tokens in words.windows(ngram) {
        let ngram_str = ngram_tokens.join(" ");
        *ngrams.entry(ngram_str).or_insert(0) += 1;
    }

    // Word frequency
    let mut wordfreq = HashMap::new();
    for w in &words {
        *wordfreq.entry(w.clone()).or_insert(0) += 1;
    }

    // Context statistics (±window)
    let mut context_map = HashMap::new();
    for (i, word) in words.iter().enumerate() {
        let window_start = i.saturating_sub(context);
        let window_end = std::cmp::min(words.len(), i + context + 1);
        let ctx_words = &words[window_start..window_end];
        let entry = context_map.entry(word.clone()).or_insert_with(HashMap::new);
        for ctx_word in ctx_words {
            if ctx_word != word {
                *entry.entry(ctx_word.clone()).or_insert(0) += 1;
            }
        }
    }

    // Direct neighbors (±1)
    let mut direct_neighbors = HashMap::new();
    for (i, word) in words.iter().enumerate() {
        let entry = direct_neighbors
            .entry(word.clone())
            .or_insert_with(HashMap::new);
        if i > 0 {
            *entry.entry(words[i - 1].clone()).or_insert(0) += 1;
        }
        if i + 1 < words.len() {
            *entry.entry(words[i + 1].clone()).or_insert(0) += 1;
        }
    }

    // Named Entity detection (heuristic): capitalized words not at sentence start, min length 2
    let mut named_entities = HashMap::new();
    for (i, w) in text.split_whitespace().enumerate() {
        let clean = w.trim_matches(|c: char| !c.is_alphanumeric());
        if clean
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false)
            && clean.len() > 1
        {
            if !(i == 0 && clean.chars().all(|c| c.is_uppercase())) {
                *named_entities.entry(clean.to_string()).or_insert(0) += 1;
            }
        }
    }

    // PMI Collocations
    let pmi = calc_pmi(&words, context);

    AnalysisResult {
        ngrams,
        wordfreq,
        context: context_map,
        direct_neighbors,
        named_entities,
        pmi,
    }
}

/// Calculate PMI for all word pairs in context window.
/// Returns Vec<PmiEntry> for test and export compatibility.
fn calc_pmi(words: &[String], window: usize) -> Vec<PmiEntry> {
    let mut cooc_counts: HashMap<(String, String, i32), usize> = HashMap::new();
    let mut word_counts: HashMap<String, usize> = HashMap::new();
    let total = words.len();

    // Count word frequencies and co-occurrences
    for (i, w) in words.iter().enumerate() {
        *word_counts.entry(w.clone()).or_insert(0) += 1;
        let start = i.saturating_sub(window);
        let end = std::cmp::min(words.len(), i + window + 1);
        for j in start..end {
            if i == j {
                continue;
            }
            let dist = (j as isize - i as isize) as i32;
            let w2 = &words[j];
            let key = if w < w2 {
                (w.clone(), w2.clone(), dist)
            } else {
                (w2.clone(), w.clone(), -dist)
            };
            *cooc_counts.entry(key).or_insert(0) += 1;
        }
    }

    // Compute PMI for pairs with sufficient count
    let mut pmi_vec = Vec::new();
    for ((w1, w2, dist), count) in cooc_counts.iter() {
        if *count < 5 {
            continue;
        }
        let p_xy = *count as f64 / (total as f64);
        let p_x = *word_counts.get(w1).unwrap_or(&1) as f64 / (total as f64);
        let p_y = *word_counts.get(w2).unwrap_or(&1) as f64 / (total as f64);
        let pmi = (p_xy / (p_x * p_y)).ln();
        pmi_vec.push(PmiEntry {
            word1: w1.clone(),
            word2: w2.clone(),
            distance: *dist,
            pmi,
            count: *count,
        });
    }
    pmi_vec.sort_by(|a, b| b.pmi.partial_cmp(&a.pmi).unwrap());
    pmi_vec
}

/// Export results to TXT/CSV/TSV/JSON with correct file naming.
pub fn export_results(
    result: &AnalysisResult,
    out_prefix: &str,
    format: ExportFormat,
    entities_only: bool,
) {
    // Ngrams
    if !entities_only {
        let ext = match format {
            ExportFormat::Txt => "txt",
            ExportFormat::Csv => "csv",
            ExportFormat::Tsv => "tsv",
            ExportFormat::Json => "json",
        };
        let ngram_file = format!("{}_ngrams.{}", out_prefix, ext);
        let wordfreq_file = format!("{}_wordfreq.{}", out_prefix, ext);
        let context_file = format!("{}_context.{}", out_prefix, ext);
        let neighbors_file = format!("{}_neighbors.{}", out_prefix, ext);
        let pmi_file = format!("{}_pmi.{}", out_prefix, ext);

        let _ = write_stat(&ngram_file, &result.ngrams, format);
        let _ = write_stat(&wordfreq_file, &result.wordfreq, format);
        let _ = write_nested_stat(&context_file, &result.context, format);
        let _ = write_nested_stat(&neighbors_file, &result.direct_neighbors, format);
        let _ = write_pmi(&pmi_file, &result.pmi, format);
    }
    // Named Entities
    let ext = match format {
        ExportFormat::Txt => "txt",
        ExportFormat::Csv => "csv",
        ExportFormat::Tsv => "tsv",
        ExportFormat::Json => "json",
    };
    let entities_file = format!("{}_namedentities.{}", out_prefix, ext);
    let _ = write_stat(&entities_file, &result.named_entities, format);
}

/// Write a simple HashMap<String, usize> as TXT/CSV/TSV/JSON.
fn write_stat(
    filename: &str,
    map: &HashMap<String, usize>,
    format: ExportFormat,
) -> std::io::Result<()> {
    let mut file = fs::File::create(filename)?;
    match format {
        ExportFormat::Txt => {
            for (k, v) in map {
                writeln!(file, "{}\t{}", k, v)?;
            }
        }
        ExportFormat::Csv => {
            writeln!(file, "item,count")?;
            for (k, v) in map {
                writeln!(file, "\"{}\",{}", k, v)?;
            }
        }
        ExportFormat::Tsv => {
            writeln!(file, "item\tcount")?;
            for (k, v) in map {
                writeln!(file, "{}\t{}", k, v)?;
            }
        }
        ExportFormat::Json => {
            let json = serde_json::to_string_pretty(&map).unwrap_or_default();
            write!(file, "{}", json)?;
        }
    }
    Ok(())
}

/// Write nested HashMap<String, HashMap<String, usize>> for context etc.
fn write_nested_stat(
    filename: &str,
    map: &HashMap<String, HashMap<String, usize>>,
    format: ExportFormat,
) -> std::io::Result<()> {
    let mut file = fs::File::create(filename)?;
    match format {
        ExportFormat::Txt | ExportFormat::Tsv => {
            for (k, submap) in map {
                for (sk, v) in submap {
                    writeln!(file, "{}\t{}\t{}", k, sk, v)?;
                }
            }
        }
        ExportFormat::Csv => {
            writeln!(file, "item,neighbor,count")?;
            for (k, submap) in map {
                for (sk, v) in submap {
                    writeln!(file, "\"{}\",\"{}\",{}", k, sk, v)?;
                }
            }
        }
        ExportFormat::Json => {
            let json = serde_json::to_string_pretty(&map).unwrap_or_default();
            write!(file, "{}", json)?;
        }
    }
    Ok(())
}

/// Write PMI results as table or JSON.
fn write_pmi(filename: &str, data: &[PmiEntry], format: ExportFormat) -> std::io::Result<()> {
    let mut file = fs::File::create(filename)?;
    match format {
        ExportFormat::Txt | ExportFormat::Tsv => {
            for entry in data {
                writeln!(
                    file,
                    "{}\t{}\t{}\t{:.4}\t{}",
                    entry.word1, entry.word2, entry.distance, entry.pmi, entry.count
                )?;
            }
        }
        ExportFormat::Csv => {
            writeln!(file, "word1,word2,distance,pmi,count")?;
            for entry in data {
                writeln!(
                    file,
                    "\"{}\",\"{}\",{},{:.4},{}",
                    entry.word1, entry.word2, entry.distance, entry.pmi, entry.count
                )?;
            }
        }
        ExportFormat::Json => {
            let json = serde_json::to_string_pretty(data).unwrap_or_default();
            write!(file, "{}", json)?;
        }
    }
    Ok(())
}
