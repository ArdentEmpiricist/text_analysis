
[![rust-clippy analyze](https://github.com/ArdentEmpiricist/text_analysis/actions/workflows/rust-clippy.yml/badge.svg)](https://github.com/ArdentEmpiricist/text_analysis/actions/workflows/rust-clippy.yml)
[![Crates.io](https://img.shields.io/crates/v/text_analysis)](https://crates.io/crates/text_analysis)
[![Documentation](https://docs.rs/text_analysis/badge.svg)](https://docs.rs/text_analysis/)
[![Crates.io](https://img.shields.io/crates/l/text_analysis)](https://github.com/LazyEmpiricist/text_analysis/blob/main/LICENSE)
[![Deploy](https://github.com/ArdentEmpiricist/text_analysis/actions/workflows/deploy.yml/badge.svg)](https://github.com/ArdentEmpiricist/text_analysis/actions/workflows/deploy.yml)
[![Crates.io](https://img.shields.io/crates/d/text_analysis?color=darkblue)](https://crates.io/crates/text_analysis)

# Text_Analysis

<p align="center">
  <img src="https://raw.githubusercontent.com/ArdentEmpiricist/text_analysis/main/assets/text_analysis_logo.png" alt="Text Analysis logo" width="200"/>
</p>

A robust, fast, modern CLI tool for linguistic text analysis in `.txt` and `.pdf` files, supporting:

* **Automatic language detection** (English, German, French, Spanish, Italian, Arabic)
* **Optional stopword filtering** (user-provided custom list; no automatic removal)
* **Optional stemming** (via `rust-stemmers` for supported languages)
* **N-gram analysis** (user-defined N)
* **Word frequency and context statistics**
* **Sliding-window co-occurrence and direct neighbors**
* **Named Entity recognition (simple capitalization heuristic)**
* **Collocation analysis with Pointwise Mutual Information (PMI)**
* **Export as TXT, CSV, TSV, or JSON**
* **Recursively scans directories**
* **Per-file analysis is parallelized** (Rayon); output writing is serialized
* **Combined Mode uses Map‑Reduce** (see below)
* **Never panics: all file errors are reported, not fatal**

> **Note:** PDF parsing is built-in via `pdf-extract` — no feature flag required.

---

## Features

* Automatic language detection (`whatlang`)
* Custom stopword lists (plain `.txt`, one word per line)
* Stemming (optional): auto by detected language or forced via CLI
* N-grams (size configurable), word frequencies
* Context statistics (±N window) & direct neighbors (±1)
* PMI collocations for word pairs within the window
* Named Entities via capitalization heuristic (see below)
* All errors (unreadable files, PDF issues) are reported at the end
* CLI built with `clap`
* Results written to timestamped files; a concise run summary is printed to the terminal

---

## Installation

* With cargo:

  ```sh
  cargo install text_analysis
  ```
* Download binary from [Releases](https://github.com/ArdentEmpiricist/text_analysis/releases)
* Clone the repository and build from source

Use in your own Rust project:

```toml
[dependencies]
text_analysis = { path = "path/to/text_analysis" }
```

---

## Usage

```sh
text_analysis <path> [--stopwords FILE] [--ngram N] [--context N] [--export-format FORMAT] [--entities-only] [--combine] [--stem] [--stem-lang LANG]
```

* `<path>`: file or directory (recursively scans for `.txt` and `.pdf`)
* `--stopwords <file>`: (optional) stopword list (one word per line; if not provided, no filtering)
* `--ngram N`: (optional, default: 2) N-gram size (2=bigrams, 3=trigrams, …)
* `--context N`: (optional, default: 5) context window size (±N words)
* `--export-format FORMAT`: `txt` (default), `csv`, `tsv`, or `json`
* `--entities-only`: only export named entities (not all statistics)
* `--combine`: analyze all files together as **one corpus** (Map‑Reduce, see below)
* `--stem`: enable stemming (based on detected language)
* `--stem-lang LANG`: force stemming language (`en`, `de`, `fr`, `es`, `it`, `pt`, `nl`, `ru`, `sv`, `fi`, `no`, `ro`, `hu`, `da`, `tr`); only effective with `--stem`

By default, each file is analyzed and exported individually.  
With `--combine`, files are analyzed as a single corpus using **Map‑Reduce**.

Example:

```sh
text_analysis ./my_corpus/ --stopwords my_stoplist.txt --ngram 3 --context 4 --export-format csv --stem --combine
```

---

## Output files & naming

Output files use a collision-safe stem, an 8-char path hash, a timestamp, and the analysis type.

```
<stem[.ext]>_<hash8>_<timestamp>_<analysis-type>.<ext>
```

Examples:

- `cli.txt_f3a9c2b1_20250810_155411_wordfreq.csv`
- `cli.txt_f3a9c2b1_20250810_155411_ngrams.csv`
- `combined_20250810_155411_wordfreq.csv` (combined mode has no hash)

> The short hash prevents filename collisions (e.g., same stem across different files), especially with parallel runs.

---

## Combined Mode (Map‑Reduce)

When `--combine` is set, the corpus is processed via **Map‑Reduce** for scalability and consistency:

**Map (parallel):** for each file, build **partial counts** from normalized tokens  
(lowercased, optional stopwords removed, optional stemming):
- `ngrams: HashMap<String, usize>`
- `wordfreq: HashMap<String, usize>`
- `context_pairs: HashMap<(String, String), usize>` (center, neighbor in ±window)
- `neighbor_pairs: HashMap<(String, String), usize>` (direct neighbors ±1)
- `cooc_by_dist: HashMap<(String, String, usize), usize>` (canonical pair, distance)
- `named_entities: HashMap<String, usize>` from the **original** (non‑stemmed) tokens
- `n_tokens: usize`

**Reduce (serial):** merge all partial counts into global totals.

**Finalize (serial):**
- Construct the final `AnalysisResult` from totals
- Compute **PMI** from global pair counts & unigram counts (single global pass)
- Write **one** set of combined outputs, e.g. `combined_<timestamp>_wordfreq.csv`

> Benefits: avoids holding a giant concatenated string in memory, maximizes parallelism, and ensures PMI/frequencies are consistent across the whole corpus.

---

## Using as a Library

You get n-gram extraction, frequency analysis, PMI collocations, optional stemming, and custom stopword support.

### Example 1: English bigrams (no stopwords, no stemming)

```rust
use std::collections::HashSet;
use text_analysis::*;

fn main() {
    let text = "The quick brown fox jumps over the lazy dog. The fox was very quick!";
    let stopwords: HashSet<String> = HashSet::new();
    let options = AnalysisOptions { ngram: 2, context: 2, export_format: ExportFormat::Json, entities_only: false, combine: false, stem_mode: StemMode::Off };
    let result = analyze_text_with(text, &stopwords, &options);

    println!("Top 3 bigrams:");
    for (ngram, count) in result.ngrams.iter().take(3) {
        println!("{}: {}", ngram, count);
    }
}
```

### Example 2: German unigrams with auto stemming

```rust
use std::collections::HashSet;
use text_analysis::*;

fn main() {
    let text = "Goethe schrieb den Faust. Faust ist ein Klassiker der deutschen Literatur.";
    let stopwords: HashSet<String> = HashSet::new();
    let options = AnalysisOptions { ngram: 1, context: 2, export_format: ExportFormat::Json, entities_only: false, combine: false, stem_mode: StemMode::Auto };
    let result = analyze_text_with(text, &stopwords, &options);

    println!("Most frequent words:");
    for (word, count) in result.wordfreq.iter().take(5) {
        println!("{}: {}", word, count);
    }
    println!("\nNamed entities:");
    for (entity, count) in result.named_entities.iter() {
        println!("{}: {}", entity, count);
    }
}
```

### Example 3: PMI with custom stopwords (no stemming)

```rust
use std::collections::HashSet;
use text_analysis::*;

fn main() {
    let text = "Alice loves Bob. Bob loves Alice. Alice and Bob are friends.";
    let mut stopwords = HashSet::new();
    for w in ["and", "are", "loves"] { stopwords.insert(w.to_string()); }
    let options = AnalysisOptions { ngram: 1, context: 2, export_format: ExportFormat::Json, entities_only: false, combine: false, stem_mode: StemMode::Off };
    let result = analyze_text_with(text, &stopwords, &options);

    println!("PMI pairs (min_count=5):");
    for entry in result.pmi.iter().take(5) {
        println!("({}, {})  PMI: {:.2}", entry.word1, entry.word2, entry.pmi);
    }
}
```

---

## Named-Entity Heuristic (how it works)

The current NER is a **simple capitalization heuristic**:

1. Tokenize the **original (non-stemmed)** text.
2. Count a token as an entity candidate if it:
   - starts with an uppercase letter (Unicode-aware),
   - is **not** fully uppercase (filters acronyms like “NASA”),
   - is **not** a common function word at sentence start (basic list).
3. Counts are **case-sensitive** (so “Berlin” ≠ “BERLIN”).

> This heuristic is fast and deterministic and will overgenerate in some cases (e.g., sentence-initial words). For higher quality, post-filter with custom lists or integrate a proper NER model. NER uses original tokens; stemming affects only statistics.

---

## Performance Notes

* Per-file analysis runs in parallel using Rayon (compute); output writing is serialized to avoid I/O contention.
* Combined mode uses Map‑Reduce: files are mapped in parallel to partial counts, then reduced. PMI is computed once globally from aggregated counts.
* The short hash in filenames avoids collisions across files with the same stem when running in parallel.

---

## To Do / Ideas

* [x] Multi-language support
* [x] Custom stopword list from file
* [x] N-gram statistics
* [x] Direct neighbor analysis
* [x] Named Entity detection (heuristic)
* [x] Collocation/PMI output
* [x] CSV/JSON/TSV export
* [x] Context window size (CLI flag)
* [x] Parallel per-file analysis (Rayon)
* [x] **Map‑Reduce combined mode**
* [ ] Lemmatization for more languages
* [ ] Richer reporting (collocation metrics, word clouds)

Contributions welcome — especially for more languages, better PDF parsing, or improved output!

---

## License

MIT

---

## Feedback & Issues

Feedback, bug reports, and pull requests are highly appreciated! Open an Issue or start a Discussion.
