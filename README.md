
[![rust-clippy analyze](https://github.com/ArdentEmpiricist/text_analysis/actions/workflows/rust-clippy.yml/badge.svg)](https://github.com/ArdentEmpiricist/text_analysis/actions/workflows/rust-clippy.yml)
[![Crates.io](https://img.shields.io/crates/v/text_analysis)](https://crates.io/crates/text_analysis)
[![Documentation](https://docs.rs/text_analysis/badge.svg)](https://docs.rs/text_analysis/)
[![Crates.io](https://img.shields.io/crates/l/text_analysis)](https://github.com/LazyEmpiricist/text_analysis/blob/main/LICENSE)
[![Deploy](https://github.com/ArdentEmpiricist/text_analysis/actions/workflows/deploy.yml/badge.svg)](https://github.com/ArdentEmpiricist/text_analysis/actions/workflows/deploy.yml)
[![Crates.io](https://img.shields.io/crates/d/text_analysis?color=darkblue)](https://crates.io/crates/text_analysis)

# Text\_Analysis

<p align="center">
  <img src="https://raw.githubusercontent.com/ArdentEmpiricist/text_analysis/main/assets/text_analysis_logo.png" alt="Text Analysis logo" width="200"/>
</p>

A robust, fast, modern CLI tool for linguistic text analysis in `.txt` and `.pdf` files, supporting:

* **Automatic language detection** (English, German, French, Spanish, Italian, Arabic)
* **Stemming** (where possible)
* **Optional stopword removal** (via custom stoplist)
* **N-gram analysis** (user-defined N)
* **Word frequency and context statistics**
* **Sliding-window co-occurrence and direct neighbors**
* **Named Entity recognition (simple heuristic)**
* **Collocation analysis with Pointwise Mutual Information (PMI) for all word pairs in the context window**
* **Export as TXT, CSV, TSV, or JSON for further processing**
* **Recursively scans directories**
* **Live progress bar for file reading**
* **Never panics: all file errors are reported, not fatal**

---

## Features

* Automatic language detection (`whatlang`)
* Per-language stemming (via `rust-stemmers`), or none (e.g. for Arabic)
* Custom stopword lists supported (plain .txt, one word per line)
* Counts and outputs N-grams (size configurable via CLI, e.g. bigrams, trigrams)
* Context statistics: for every word, which words appear nearby most frequently (±N window)
* Direct neighbors (±1) are reported separately
* Collocation analysis with PMI, for all word pairs in the context window (CSV/JSON export)
* Named Entity recognition via capitalization heuristic
* Progress bar and current file output during analysis (`indicatif`)
* All errors (unreadable files, PDF problems) are reported at the end, never panic
* CLI built with `clap`
* Results output to timestamped files in the working directory
* Failsafe: Always outputs a `.txt` file containing the whole analysis

---

## Installation

* With cargo:

  ```sh
  cargo install text_analysis
  ```
* Download binary from [Releases](https://github.com/ArdentEmpiricist/text_analysis/releases)
* Clone the repository and build from source

---

## Usage

```sh
text_analysis <path> [--stopwords stoplist.txt] [--ngram N] [--context N] [--export-format FORMAT] [--entities-only]
```

* `<path>`: file or directory (recursively scans for `.txt` and `.pdf`)
* `--stopwords <file>`: (optional) additional stopword list (one word per line)
* `--ngram N`: (optional, default: 2) N-gram size (e.g. 2 = bigrams, 3 = trigrams)
* `--context N`: (optional, default: 5) context window size (N = ±N words)
* `--export-format FORMAT`: `txt` (default), `csv`, `tsv`, or `json` (exports results as separate files)
* `--entities-only`: only export named entities (names), not full statistics
* `--combine`: Analyze all files together and output combined result files

By default, each file is analyzed and exported individually.
With --combine, all files are analyzed as a single corpus and combined result files are exported.

**During analysis, a progress bar and the current file being read are shown in the terminal.**

Example:

```sh
text_analysis ./my_corpus/ --stopwords my_stoplist.txt --ngram 3 --context 4 --export-format csv
```

---

## Output Example

The output file and stdout print N-gram statistics **first**, then per-word frequency and context, then named entities, then PMI collocations.

```txt
=== N-gram Analysis (N=3) ===
Ngram: "the quick brown" — Count: 18
Ngram: "quick brown fox" — Count: 18
Ngram: "brown fox jumps" — Count: 17
...

=== Word Frequencies and Context (window ±5) ===
Word: "fox" — Frequency: 25
    Words near: [("the", 22), ("quick", 18), ("brown", 15), ...]
    Direct neighbors: [("quick", 10), ("jumps", 9), ...]

Word: "dog" — Frequency: 19
    Words near: [("lazy", 14), ("brown", 9), ...]
    Direct neighbors: [("lazy", 7), ...]

=== Named Entities ===
  Fox                    — Count: 8
  Dog                    — Count: 5

=== PMI Collocations (min_count=5, top 20) ===
(      fox,      quick) @ d= 1  PMI= 4.13  count=19
(     lazy,        dog) @ d= 1  PMI= 4.02  count=18
...

# At the end of run (stderr):
Warning: The following files could not be read:
  ./broken.pdf: PDF error: ...
  ./unreadable.txt: ...
```

**Exported Files:**

The output files now start with the analyzed filename, followed by the analysis type and a timestamp. For example:

- `mytext_wordfreq_20250803_191010.csv`
- `mytext_ngrams_20250803_191010.csv`
- `mytext_namedentities_20250803_191010.csv`
- `mytext_pmi_20250803_191010.csv`

When using combined analysis (`--combine`):

- `combined_wordfreq_20250803_191010.csv`
- `combined_ngrams_20250803_191010.csv`
- etc.

The exact file naming scheme is:  
`<filename>_<analysis-type>_<timestamp>.<ext>`

---


## Using as a Library

This crate can be used directly in your own Rust projects for fast multi-language text analysis.  
You get all core functions, including n-gram extraction, frequency analysis, collocation statistics (PMI), and automatic or custom stopword support.

Add to your `Cargo.toml`:
```toml
[dependencies]
text_analysis = { path = "path/to/your/text_analysis" }
```

### Example 1: Analyze a Text for English Bigrams

```rust
use text_analysis::*;

fn main() {
    let text = "The quick brown fox jumps over the lazy dog. The fox was very quick!";
    let stopwords = default_stopwords_for_language("en");
    let result = analyze_text(text, &stopwords, 2, 2); // bigrams, window = 2

    println!("Top 3 bigrams:");
    for (ngram, count) in result.ngrams.iter().take(3) {
        println!("{}: {}", ngram, count);
    }
}
```

### Example 2: Frequency and Named Entity Extraction for German

```rust
use text_analysis::*;

fn main() {
    let text = "Goethe schrieb den Faust. Faust ist ein Klassiker der deutschen Literatur.";
    let stopwords = default_stopwords_for_language("de");
    let result = analyze_text(text, &stopwords, 1, 2); // unigrams, window = 2

    println!("Most frequent words:");
    for (word, count) in result.wordfreq.iter().take(5) {
        println!("{}: {}", word, count);
    }
    println!("
Named entities:");
    for (entity, count) in result.named_entities.iter() {
        println!("{}: {}", entity, count);
    }
}
```

### Example 3: PMI Collocation Extraction with Custom Stopwords

```rust
use std::collections::HashSet;
use text_analysis::*;

fn main() {
    let text = "Alice loves Bob. Bob loves Alice. Alice and Bob are friends.";
    let mut stopwords = HashSet::new();
    for w in ["and", "are", "loves"] { stopwords.insert(w.to_string()); }
    let result = analyze_text(text, &stopwords, 1, 2); // unigrams, window = 2

    println!("PMI pairs (min_count=5):");
    for entry in result.pmi.iter().take(5) {
        println!("({}, {})  PMI: {:.2}", entry.word1, entry.word2, entry.pmi);
    }
}
```

---

**Tip:**  
All functions work with any Unicode text.

---

## Scientific Features & Best Practices

* Language-aware stemming and stopwords for English, German, French, Spanish, Italian, Arabic
* Optional additional stoplist (e.g. for project-specific terms)
* N-gram and co-occurrence analysis for computational linguistics or stylometry
* Collocation statistics with mutual information (PMI)
* Configurable context window size (±N words)
* All outputs can be processed as CSV/TSV/JSON, e.g. in R, Python, Excel, pandas, etc.
* Named Entities exported for further annotation or statistics
* Errors and files skipped are always listed at the end

---

## To Do / Ideas

* [x] Multi-language support
* [x] Custom stopword list from file
* [x] N-gram statistics
* [x] Direct neighbor analysis
* [x] Named Entity detection (heuristic)
* [x] Collocation/PMI output
* [x] CSV/JSON export
* [x] Robust error handling & test coverage
* [ ] Lemmatization for more languages (if crates become available)
* [ ] Option for context window size (CLI flag)
* [ ] Parallel file analysis for very large corpora
* [ ] More advanced reporting (collocation metrics, word clouds)

Contributions welcome, especially for more languages, better PDF/docx parsing, or improved output!

---

## License

MIT

---

## Feedback & Issues

Feedback, bug reports, and pull requests are highly appreciated! Open an [Issue](https://github.com/ArdentEmpiricist/text_analysis/issues) or [start a discussion](https://github.com/ArdentEmpiricist/text_analysis/discussions).
