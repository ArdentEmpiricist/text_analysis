use chrono::Local;
use clap::ValueEnum;
use indicatif::{ProgressBar, ProgressStyle};
use pdf_extract::extract_text;
use rust_stemmers::{Algorithm, Stemmer};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use walkdir::WalkDir;
use whatlang::{Lang, detect};

#[derive(ValueEnum, Clone, Debug)]
pub enum ExportFormat {
    Txt,
    Csv,
    Tsv,
    Json,
}

impl std::fmt::Display for ExportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportFormat::Txt => write!(f, "txt"),
            ExportFormat::Csv => write!(f, "csv"),
            ExportFormat::Tsv => write!(f, "tsv"),
            ExportFormat::Json => write!(f, "json"),
        }
    }
}

#[derive(Debug)]
pub struct AnalysisReport {
    pub result: String,
    pub failed_files: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct NamedEntity {
    pub entity: String,
    pub count: u32,
}

#[derive(Debug, Serialize)]
pub struct PmiEntry {
    pub word1: String,
    pub word2: String,
    pub pmi: f64,
    pub count: u32,
    pub distance: i32,
}

pub fn analyze_path(
    path: &str,
    stopwords_path: Option<String>,
    ngram_size: usize,
    context_window: usize,
    export_format: ExportFormat,
    entities_only: bool,
) -> Result<AnalysisReport, String> {
    let mut failed_files = Vec::new();
    let mut texts = Vec::new();

    let files = get_files_recursively(path);
    let num_files = files.len();

    if files.is_empty() {
        return Err("No .txt or .pdf files found.".to_string());
    }

    // Progress bar for reading files
    let pb = ProgressBar::new(num_files as u64);
    pb.set_style(
        ProgressStyle::with_template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("##-"),
    );

    for file in &files {
        pb.set_message(format!("Reading: {}", file));
        match read_text_from_file(file) {
            Ok(text) => texts.push(text),
            Err(e) => failed_files.push(format!("{}: {}", file, e)),
        }
        pb.inc(1);
    }
    pb.finish_with_message("Done!");

    // Join all file texts into one big string
    let all_text = texts.join(" ");

    // Detect language
    let lang = detect(&all_text)
        .map(|info| info.lang())
        .unwrap_or(Lang::Eng);

    // Use stemming and stopwords according to detected language
    let (stemmer, default_stopwords) = match lang {
        Lang::Eng => (
            Some(Stemmer::create(Algorithm::English)),
            english_stopwords(),
        ),
        Lang::Deu => (Some(Stemmer::create(Algorithm::German)), german_stopwords()),
        Lang::Fra => (Some(Stemmer::create(Algorithm::French)), french_stopwords()),
        Lang::Spa => (
            Some(Stemmer::create(Algorithm::Spanish)),
            spanish_stopwords(),
        ),
        Lang::Ita => (
            Some(Stemmer::create(Algorithm::Italian)),
            italian_stopwords(),
        ),
        Lang::Ara => (None, arabic_stopwords()),
        _ => (None, english_stopwords()),
    };

    // Load optional custom stopwords
    let mut stopwords = default_stopwords;
    if let Some(path) = stopwords_path {
        if let Ok(custom) = load_stopword_list(&path) {
            stopwords.extend(custom);
        }
    }

    let words = trim_to_words(&all_text, stemmer.as_ref(), &stopwords);

    // === Named Entities ===
    let named_entities = detect_named_entities(&words);

    // === Collocation and Direct Neighbors ===
    let (freq, context_stat, direct_neighbors, pos_matrix) =
        collocation_stats(&words, context_window);

    // === N-gram Analysis ===
    let ngram_counts = ngram_analysis(&words, ngram_size);

    // === PMI Calculation ===
    let total_tokens: u32 = freq.values().sum();
    let min_pmi_count = 5;
    let pmi_entries = compute_pmi(&freq, &pos_matrix, total_tokens, min_pmi_count);

    // === Export Statistics ===
    export_statistics(
        &freq,
        &ngram_counts,
        &context_stat,
        &direct_neighbors,
        &pos_matrix,
        &named_entities,
        &pmi_entries,
        ngram_size,
        context_window,
        &export_format,
        entities_only,
    )?;

    // Formatted TXT output for terminal
    let mut out = String::new();

    if !entities_only {
        out.push_str(&format!("=== N-gram Analysis (N={}) ===\n", ngram_size));
        for (ngram, count) in sort_map_to_vec(&ngram_counts) {
            out.push_str(&format!(
                "  {:<30} — Count: {}\n",
                format!("\"{}\"", ngram),
                count
            ));
        }

        out.push_str(&format!(
            "\n=== Word Frequencies and Context (window ±{}) ===\n",
            context_window
        ));
        let empty_vec: &Vec<(String, u32)> = &Vec::new();
        for (word, count) in sort_map_to_vec(&freq) {
            let ctx = context_stat.get(&word).unwrap_or(empty_vec);
            let direct = direct_neighbors.get(&word).unwrap_or(empty_vec);
            out.push_str(&format!("Word: \"{}\" — Frequency: {}\n", word, count));
            if !ctx.is_empty() {
                out.push_str("    Words near: [");
                let preview: Vec<String> = ctx
                    .iter()
                    .take(12)
                    .map(|(w, c)| format!("(\"{}\", {})", w, c))
                    .collect();
                out.push_str(&preview.join(", "));
                if ctx.len() > 12 {
                    out.push_str(", ...");
                }
                out.push_str("]\n");
            }
            if !direct.is_empty() {
                out.push_str("    Direct neighbors: [");
                let preview: Vec<String> = direct
                    .iter()
                    .take(8)
                    .map(|(w, c)| format!("(\"{}\", {})", w, c))
                    .collect();
                out.push_str(&preview.join(", "));
                if direct.len() > 8 {
                    out.push_str(", ...");
                }
                out.push_str("]\n");
            }
            out.push('\n');
        }

        out.push_str("\n=== Named Entities ===\n");
        for ne in &named_entities {
            out.push_str(&format!("  {:<25} — Count: {}\n", ne.entity, ne.count));
        }

        out.push_str("\n=== PMI Collocations (min_count=5, top 20) ===\n");
        for entry in pmi_entries.iter().take(20) {
            out.push_str(&format!(
                "({:>10}, {:>10}) @ d={:<2}  PMI={:>6.2}  count={:<4}\n",
                entry.word1, entry.word2, entry.distance, entry.pmi, entry.count
            ));
        }
    }

    // Failsafe: Save the main output to a TXT file every time (if not entities_only)
    if !entities_only {
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
        let path = format!("{}_analysis.txt", timestamp);
        if let Err(e) = std::fs::write(&path, &out) {
            eprintln!("Warning: Could not write TXT output file '{}': {}", path, e);
        }
    }

    Ok(AnalysisReport {
        result: out,
        failed_files,
    })
}

fn get_files_recursively(path: &str) -> Vec<String> {
    let mut files = Vec::new();
    let walker = WalkDir::new(path).into_iter();
    for entry in walker.filter_map(Result::ok) {
        if entry.file_type().is_file() {
            let f = entry.path().display().to_string();
            if f.ends_with(".txt") || f.ends_with(".pdf") {
                files.push(f);
            }
        }
    }
    files
}

fn read_text_from_file(path: &str) -> Result<String, String> {
    if path.ends_with(".txt") {
        fs::read_to_string(path).map_err(|e| e.to_string())
    } else if path.ends_with(".pdf") {
        extract_text(path).map_err(|e| format!("PDF error: {}", e))
    } else {
        Err("Unknown file type".to_string())
    }
}

/// Clean tokens, lowercase, stopword-filter, stemming, arabic prefix-removal
pub fn trim_to_words(
    text: &str,
    stemmer: Option<&Stemmer>,
    stopwords: &HashSet<String>,
) -> Vec<String> {
    text.split_whitespace()
        .map(|s| s.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .filter(|w| !stopwords.contains(w.as_str()))
        .map(|w| match stemmer {
            Some(stemmer) => stemmer.stem(&w).to_string(),
            None => w,
        })
        // For Arabic: Remove definite article 'ال' at word start if present and word is longer than 2 chars
        .map(|w| {
            if is_arabic(&w) && w.starts_with("ال") && w.len() > 2 {
                w.trim_start_matches("ال").to_string()
            } else {
                w
            }
        })
        .collect()
}

fn is_arabic(word: &str) -> bool {
    word.chars()
        .next()
        .map_or(false, |c| ('\u{0600}'..='\u{06FF}').contains(&c))
}

// Sliding-Window, direct neighbors, and collocation position matrix
fn collocation_stats(
    words: &[String],
    window: usize,
) -> (
    HashMap<String, u32>,                                // frequency
    HashMap<String, Vec<(String, u32)>>,                 // context stats (sliding window)
    HashMap<String, Vec<(String, u32)>>,                 // direct neighbors
    HashMap<String, HashMap<i32, HashMap<String, u32>>>, // word: position(-window..window):word:count
) {
    let mut freq = HashMap::new();
    let mut context_stat: HashMap<String, HashMap<String, u32>> = HashMap::new();
    let mut direct_neighbors: HashMap<String, HashMap<String, u32>> = HashMap::new();
    let mut pos_matrix: HashMap<String, HashMap<i32, HashMap<String, u32>>> = HashMap::new();

    for (i, word) in words.iter().enumerate() {
        *freq.entry(word.clone()).or_insert(0) += 1;

        let start = i.saturating_sub(window);
        let end = usize::min(i + window + 1, words.len());

        for j in start..end {
            if i == j {
                continue;
            }
            let rel_pos = j as i32 - i as i32; // -window..window
            let ctx_word = &words[j];

            *context_stat
                .entry(word.clone())
                .or_insert_with(HashMap::new)
                .entry(ctx_word.clone())
                .or_insert(0) += 1;

            *pos_matrix
                .entry(word.clone())
                .or_insert_with(HashMap::new)
                .entry(rel_pos)
                .or_insert_with(HashMap::new)
                .entry(ctx_word.clone())
                .or_insert(0) += 1;

            if rel_pos == -1 || rel_pos == 1 {
                *direct_neighbors
                    .entry(word.clone())
                    .or_insert_with(HashMap::new)
                    .entry(ctx_word.clone())
                    .or_insert(0) += 1;
            }
        }
    }

    (
        freq,
        context_stat
            .into_iter()
            .map(|(k, v)| (k, sort_map_to_vec(&v)))
            .collect(),
        direct_neighbors
            .into_iter()
            .map(|(k, v)| (k, sort_map_to_vec(&v)))
            .collect(),
        pos_matrix,
    )
}

// Named Entity Extraction by simple capitalization rule
fn detect_named_entities(words: &[String]) -> Vec<NamedEntity> {
    let mut count = HashMap::new();
    let mut prev_end = true; // True if previous token ended a sentence

    for word in words {
        let is_cap = word
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false);
        if is_cap && !prev_end {
            *count.entry(word.clone()).or_insert(0) += 1;
        }
        prev_end = word.ends_with('.') || word.ends_with('!') || word.ends_with('?');
    }
    sort_map_to_vec(&count)
        .into_iter()
        .map(|(entity, count)| NamedEntity { entity, count })
        .collect()
}

// Calculate PMI for all word pairs (by context distance)
pub fn compute_pmi(
    freq: &HashMap<String, u32>,
    pos_matrix: &HashMap<String, HashMap<i32, HashMap<String, u32>>>,
    total: u32,
    min_count: u32,
) -> Vec<PmiEntry> {
    let mut result = Vec::new();

    // Pre-calculate probabilities
    let total_f = total as f64;
    let freq_f: HashMap<&String, f64> =
        freq.iter().map(|(w, c)| (w, *c as f64 / total_f)).collect();

    for (word, positions) in pos_matrix {
        for (distance, partners) in positions {
            for (ctx_word, count) in partners {
                if *count < min_count || word == ctx_word {
                    continue;
                }
                let p_w1 = *freq_f.get(word).unwrap_or(&0.0);
                let p_w2 = *freq_f.get(ctx_word).unwrap_or(&0.0);
                let p_w1w2 = *count as f64 / total_f;
                if p_w1 > 0.0 && p_w2 > 0.0 && p_w1w2 > 0.0 {
                    let pmi = (p_w1w2 / (p_w1 * p_w2)).log2();
                    result.push(PmiEntry {
                        word1: word.clone(),
                        word2: ctx_word.clone(),
                        pmi,
                        count: *count,
                        distance: *distance,
                    });
                }
            }
        }
    }
    // Sort by highest PMI, then by count
    result.sort_by(|a, b| {
        b.pmi
            .partial_cmp(&a.pmi)
            .unwrap()
            .then(b.count.cmp(&a.count))
    });
    result
}

// Write results for further processing
fn export_statistics(
    freq: &HashMap<String, u32>,
    ngrams: &HashMap<String, u32>,
    context_stat: &HashMap<String, Vec<(String, u32)>>,
    direct_neighbors: &HashMap<String, Vec<(String, u32)>>,
    pos_matrix: &HashMap<String, HashMap<i32, HashMap<String, u32>>>,
    named_entities: &[NamedEntity],
    pmi_entries: &[PmiEntry],
    ngram_size: usize,
    context_window: usize,
    export_format: &ExportFormat,
    entities_only: bool,
) -> Result<(), String> {
    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();

    match export_format {
        ExportFormat::Txt => {
            // 1. Word Frequencies
            let path = format!("{}_wordfreq.txt", timestamp);
            let mut wf = String::from("word\tcount\n");
            for (word, count) in sort_map_to_vec(freq) {
                wf.push_str(&format!("{}\t{}\n", word, count));
            }
            fs::write(&path, wf).map_err(|e| e.to_string())?;

            // 2. N-grams
            let path = format!("{}_ngrams.txt", timestamp);
            let mut ng = String::from("ngram\tcount\n");
            for (ngram, count) in sort_map_to_vec(ngrams) {
                ng.push_str(&format!("{}\t{}\n", ngram, count));
            }
            fs::write(&path, ng).map_err(|e| e.to_string())?;

            // 3. Named Entities
            let path = format!("{}_namedentities.txt", timestamp);
            let mut ne = String::from("entity\tcount\n");
            for n in named_entities {
                ne.push_str(&format!("{}\t{}\n", n.entity, n.count));
            }
            fs::write(&path, ne).map_err(|e| e.to_string())?;

            // 4. PMI
            let path = format!("{}_pmi.txt", timestamp);
            let mut pmi_s = String::from("word1\tword2\tpmi\tcount\tdistance\n");
            for entry in pmi_entries {
                pmi_s.push_str(&format!(
                    "{}\t{}\t{:.3}\t{}\t{}\n",
                    entry.word1, entry.word2, entry.pmi, entry.count, entry.distance
                ));
            }
            fs::write(&path, pmi_s).map_err(|e| e.to_string())?;
        }

        ExportFormat::Csv | ExportFormat::Tsv => {
            let sep = if matches!(export_format, ExportFormat::Tsv) {
                b'\t'
            } else {
                b','
            };
            // 1. Word Frequencies
            let path = format!("{}_wordfreq.{}", timestamp, export_format.to_string());
            let mut wtr = csv::WriterBuilder::new()
                .delimiter(sep)
                .from_path(&path)
                .map_err(|e| e.to_string())?;
            wtr.write_record(&["word", "count"])
                .map_err(|e| e.to_string())?;
            for (word, count) in sort_map_to_vec(freq) {
                wtr.write_record(&[word, count.to_string()])
                    .map_err(|e| e.to_string())?;
            }
            wtr.flush().map_err(|e| e.to_string())?;

            // 2. N-grams
            let path = format!("{}_ngrams.{}", timestamp, export_format.to_string());
            let mut wtr = csv::WriterBuilder::new()
                .delimiter(sep)
                .from_path(&path)
                .map_err(|e| e.to_string())?;
            wtr.write_record(&["ngram", "count"])
                .map_err(|e| e.to_string())?;
            for (ngram, count) in sort_map_to_vec(ngrams) {
                wtr.write_record(&[ngram, count.to_string()])
                    .map_err(|e| e.to_string())?;
            }
            wtr.flush().map_err(|e| e.to_string())?;

            // 3. Named Entities
            let path = format!("{}_namedentities.{}", timestamp, export_format.to_string());
            let mut wtr = csv::WriterBuilder::new()
                .delimiter(sep)
                .from_path(&path)
                .map_err(|e| e.to_string())?;
            wtr.write_record(&["entity", "count"])
                .map_err(|e| e.to_string())?;
            for ne in named_entities {
                wtr.write_record(&[&ne.entity, &ne.count.to_string()])
                    .map_err(|e| e.to_string())?;
            }
            wtr.flush().map_err(|e| e.to_string())?;

            // 4. PMI
            let path = format!("{}_pmi.{}", timestamp, export_format.to_string());
            let mut wtr = csv::WriterBuilder::new()
                .delimiter(sep)
                .from_path(&path)
                .map_err(|e| e.to_string())?;
            wtr.write_record(&["word1", "word2", "pmi", "count", "distance"])
                .map_err(|e| e.to_string())?;
            for entry in pmi_entries {
                wtr.write_record(&[
                    &entry.word1,
                    &entry.word2,
                    &format!("{:.3}", entry.pmi),
                    &entry.count.to_string(),
                    &entry.distance.to_string(),
                ])
                .map_err(|e| e.to_string())?;
            }
            wtr.flush().map_err(|e| e.to_string())?;
        }
        ExportFormat::Json => {
            // Frequencies
            let path = format!("{}_wordfreq.json", timestamp);
            fs::write(&path, serde_json::to_string_pretty(freq).unwrap())
                .map_err(|e| e.to_string())?;
            // Ngrams
            let path = format!("{}_ngrams.json", timestamp);
            fs::write(&path, serde_json::to_string_pretty(ngrams).unwrap())
                .map_err(|e| e.to_string())?;
            // Named Entities
            let path = format!("{}_namedentities.json", timestamp);
            fs::write(&path, serde_json::to_string_pretty(named_entities).unwrap())
                .map_err(|e| e.to_string())?;
            // PMI
            let path = format!("{}_pmi.json", timestamp);
            fs::write(&path, serde_json::to_string_pretty(&pmi_entries).unwrap())
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

// Helper: Sort HashMap<String, u32> by count descending
fn sort_map_to_vec<T: Clone + Eq + std::hash::Hash>(map: &HashMap<T, u32>) -> Vec<(T, u32)> {
    let mut vec: Vec<_> = map.iter().map(|(k, v)| (k.clone(), *v)).collect();
    vec.sort_by(|a, b| b.1.cmp(&a.1));
    vec
}

fn ngram_analysis(words: &[String], n: usize) -> HashMap<String, u32> {
    let mut ngram_counts = HashMap::new();
    if n == 0 || words.len() < n {
        return ngram_counts;
    }
    for i in 0..=(words.len() - n) {
        let ngram = words[i..i + n].join(" ");
        *ngram_counts.entry(ngram).or_insert(0) += 1;
    }
    ngram_counts
}

// Load stopword list from file, one word per line.
fn load_stopword_list(path: &str) -> Result<HashSet<String>, String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    Ok(content
        .lines()
        .map(|l| l.trim().to_lowercase())
        .filter(|l| !l.is_empty())
        .collect())
}

// Return errors at the end
pub fn print_failed_files(failed: &[String]) {
    if !failed.is_empty() {
        eprintln!("Warning: The following files could not be read:");
        for entry in failed {
            eprintln!("{}", entry);
        }
    }
}

// Minimal stopword lists; you can expand as you like
fn english_stopwords() -> HashSet<String> {
    [
        "the", "and", "is", "in", "an", "to", "of", "a", "for", "on", "with", "at", "by", "from",
        "up", "about", "into", "over", "after", "than", "out", "against", "during", "without",
        "before", "under", "around", "among", "as", "it", "that", "this", "these", "those", "are",
        "was", "be", "been", "being", "have", "has", "had", "do", "does", "did",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn german_stopwords() -> HashSet<String> {
    [
        "der", "die", "das", "und", "ist", "in", "zu", "den", "von", "mit", "auf", "für", "im",
        "an", "aus", "bei", "als", "durch", "nach", "über", "auch", "es", "sie", "sich", "dem",
        "er", "wir", "ich", "nicht", "ein", "eine", "des", "am", "so", "wie", "oder", "aber",
        "wenn", "man", "noch", "nur", "vor", "zur", "mehr", "um", "bis", "dann", "da", "zum",
        "haben", "hat", "war", "werden", "wird", "sein",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn french_stopwords() -> HashSet<String> {
    [
        "le", "la", "les", "et", "est", "en", "du", "un", "une", "des", "dans", "pour", "par",
        "au", "aux", "avec", "de", "ce", "cette", "ces", "que", "qui", "sur", "pas", "plus", "se",
        "son", "sa", "ses", "ne", "nous", "vous", "ils", "elles", "il", "elle", "je", "tu", "me",
        "te", "on", "y", "mais", "ou", "où", "donc", "or", "ni", "car", "leur", "leurs", "a",
        "été", "être", "avoir",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn spanish_stopwords() -> HashSet<String> {
    [
        "el", "la", "los", "las", "y", "es", "en", "un", "una", "de", "del", "al", "con", "por",
        "para", "que", "quien", "quienes", "se", "su", "sus", "no", "sí", "yo", "tú", "él", "ella",
        "nosotros", "vosotros", "ellos", "ellas", "mi", "mis", "tu", "tus", "nuestro", "nuestra",
        "nuestros", "nuestras", "vuestro", "vuestra", "vuestros", "vuestras", "o", "u", "pero",
        "porque", "como", "cuándo", "cuál", "dónde", "mientras", "donde", "cuando", "algunos",
        "algunas", "ser", "haber", "estar",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn italian_stopwords() -> HashSet<String> {
    [
        "il", "lo", "la", "i", "gli", "le", "e", "è", "in", "un", "una", "di", "da", "con", "per",
        "su", "che", "chi", "cui", "non", "si", "suo", "sua", "suoi", "lui", "lei", "io", "tu",
        "noi", "voi", "loro", "mio", "mia", "tuo", "tua", "nostro", "nostra", "vostro", "vostra",
        "questo", "questa", "questi", "queste", "quello", "quella", "quelli", "quelle", "dove",
        "come", "quando", "perché", "ma", "anche", "se", "al", "agli", "all", "dall", "del",
        "della", "degli", "delle", "dei", "dagli", "dalle", "dai", "essere", "avere",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn arabic_stopwords() -> HashSet<String> {
    [
        "و",
        "في",
        "من",
        "على",
        "إلى",
        "عن",
        "أن",
        "إن",
        "لا",
        "ما",
        "كل",
        "كان",
        "هذه",
        "هذا",
        "هناك",
        "بعد",
        "مع",
        "التي",
        "الذي",
        "ذلك",
        "بين",
        "حتى",
        "لكن",
        "أو",
        "أي",
        "أيضا",
        "أكثر",
        "بعض",
        "إذا",
        "قد",
        "أمام",
        "أحد",
        "أثناء",
        "إذ",
        "أصبح",
        "أصبح",
        "أصبح",
        "أصبحت",
        "أصبحوا",
        "إلى",
        "إلا",
        "أم",
        "أما",
        "إن",
        "أنت",
        "أنا",
        "أو",
        "أي",
        "أين",
        "إيا",
        "إياه",
        "إياها",
        "إياهم",
        "إياهن",
        "إياكما",
        "إياكم",
        "إياكن",
        "إياي",
        "إياه",
        "إياها",
        "إياهم",
        "إياهن",
        "إياكما",
        "إياكم",
        "إياكن",
        "ب",
        "بأن",
        "بها",
        "به",
        "بهم",
        "بهن",
        "بي",
        "بين",
        "بيد",
        "تحت",
        "حيث",
        "حين",
        "خلال",
        "ذلك",
        "ذات",
        "رغم",
        "سوف",
        "شبه",
        "شخص",
        "ضمن",
        "عدم",
        "على",
        "عند",
        "قبل",
        "قد",
        "كل",
        "كلما",
        "لم",
        "لن",
        "له",
        "لها",
        "لهم",
        "لهن",
        "لي",
        "ما",
        "ماذا",
        "متى",
        "من",
        "منذ",
        "مهما",
        "نحو",
        "نفس",
        "هؤلاء",
        "هذه",
        "هذا",
        "هل",
        "هم",
        "هما",
        "هن",
        "هو",
        "هي",
        "و",
        "يا",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_stemmers::{Algorithm, Stemmer};

    #[test]
    fn test_trim_to_words_basic() {
        let en_stemmer = Stemmer::create(Algorithm::English);
        let stopwords = english_stopwords();
        let input = "Running and running, is running fast.";
        let res = trim_to_words(input, Some(&en_stemmer), &stopwords);
        assert_eq!(res, vec!["run", "run", "run", "fast"]);
    }

    #[test]
    fn test_french_stopwords() {
        let fr_stemmer = Stemmer::create(Algorithm::French);
        let stopwords = french_stopwords();
        let input = "Le chat et le chien sont dans la maison.";
        let res = trim_to_words(input, Some(&fr_stemmer), &stopwords);
        // "le", "et", "la", "dans" should be removed
        assert_eq!(res, vec!["chat", "chien", "sont", "maison"]);
    }

    #[test]
    fn test_arabic_stopwords() {
        let stopwords = arabic_stopwords();
        let input = "هذا الكتاب في المكتب و هو جميل";
        let res = trim_to_words(input, None, &stopwords);
        assert_eq!(res, vec!["كتاب", "مكتب", "جميل"]);
    }
}
