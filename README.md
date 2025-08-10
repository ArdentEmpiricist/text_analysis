
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
* **Named Entity recognition (simple heuristic, see below)**
* **Collocation analysis with Pointwise Mutual Information (PMI) for all word pairs in the context window**
* **Export as TXT, CSV, TSV, or JSON for further processing**
* **Recursively scans directories**
* **Never panics: all file errors are reported, not fatal**

---

## Features

* Automatic language detection (`whatlang`)
* Custom stopword lists supported (plain .txt, one word per line)
* Optional stemming (`--stem`, auto by detected language, or `--stem-lang <code>` to override)
* Counts and outputs N-grams (size configurable via CLI, e.g. bigrams, trigrams)
* Context statistics: for every word, which words appear nearby most frequently (±N window)
* Direct neighbors (±1) are reported separately
* Collocation analysis with PMI for all word pairs in the context window (CSV/JSON export)
* Named Entity recognition via capitalization heuristic (documented below)
* All errors (unreadable files, PDF problems) are reported at the end (never panic)
* CLI built with `clap`
* Results output to timestamped files in the working directory
* Always prints a concise analysis summary to the terminal

---

## Installation

* With cargo:

  ```sh
  cargo install text_analysis
  ```
* Download binary from [Releases](https://github.com/ArdentEmpiricist/text_analysis/releases)
* Clone the repository and build from source

Add to your `Cargo.toml` if you use it as a library:

```toml
[dependencies]
text_analysis = { path = "path/to/your/text_analysis" }
```

---

## Usage

```sh
text_analysis <path> [--stopwords FILE] [--ngram N] [--context N] [--export-format FORMAT] [--entities-only] [--combine] [--stem] [--stem-lang LANG]
```

* `<path>`: file or directory (recursively scans for `.txt` and `.pdf`)
* `--stopwords <file>`: (optional) stopword list file (one word per line; if not provided, no stopword filtering is applied)
* `--ngram N`: (optional, default: 2) N-gram size (e.g. 2 = bigrams, 3 = trigrams)
* `--context N`: (optional, default: 5) context window size (N = ±N words)
* `--export-format FORMAT`: `txt` (default), `csv`, `tsv`, or `json` (exports results as separate files)
* `--entities-only`: only export named entities (names), not full statistics
* `--combine`: analyze all files together and output combined result files
* `--stem`: enable stemming (based on detected language)
* `--stem-lang LANG`: override language for stemming (e.g. `en`, `de`, `fr`, `it`, `es`, `pt`, `nl`, `ru`, `sv`, `fi`, `no`, `ro`, `hu`, `da`, `tr`); ignored if `--stem` is not set

By default, each file is analyzed and exported individually.  
With `--combine`, all files are analyzed as a single corpus and combined result files are exported.

Example:

```sh
text_analysis ./my_corpus/ --stopwords my_stoplist.txt --ngram 3 --context 4 --export-format csv --stem
```

---

## Output Example

The output (and stdout summary) prints N-gram statistics **first**, then per-word frequency and context, then named entities, then PMI collocations.

```txt
=== N-gram Analysis ===
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

Output files are named with the input filename, a timestamp, and the analysis type. For example:

- `mytext_20250803_191010_wordfreq.csv`
- `mytext_20250803_191010_ngrams.csv`
- `mytext_20250803_191010_namedentities.csv`
- `mytext_20250803_191010_pmi.csv`

When using combined analysis (`--combine`):

- `combined_20250803_191010_wordfreq.csv`
- `combined_20250803_191010_ngrams.csv`
- etc.

The file naming scheme is:  
`<filename>_<timestamp>_<analysis-type>.<ext>`

---

## Using as a Library

You get all core functions, including n-gram extraction, frequency analysis, collocation statistics (PMI), custom stopword support, and optional stemming.

### Example 1: Analyze English Bigrams (no stopwords, no stemming)

```rust
use std::collections::HashSet;
use text_analysis::*;

fn main() {
    let text = "The quick brown fox jumps over the lazy dog. The fox was very quick!";
    let stopwords: HashSet<String> = HashSet::new();
    let options = AnalysisOptions { ngram: 2, context: 2, stemming: StemMode::Off };
    let result = analyze_text_with(text, &stopwords, &options);

    println!("Top 3 bigrams:");
    for (ngram, count) in result.ngrams.iter().take(3) {
        println!("{}: {}", ngram, count);
    }
}
```

### Example 2: Frequency and Named Entities for German with Stemming

```rust
use std::collections::HashSet;
use text_analysis::*;

fn main() {
    let text = "Goethe schrieb den Faust. Faust ist ein Klassiker der deutschen Literatur.";
    let stopwords: HashSet<String> = HashSet::new();
    let options = AnalysisOptions { ngram: 1, context: 2, stemming: StemMode::Auto };
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

### Example 3: PMI Collocations with Custom Stopwords (no stemming)

```rust
use std::collections::HashSet;
use text_analysis::*;

fn main() {
    let text = "Alice loves Bob. Bob loves Alice. Alice and Bob are friends.";
    let mut stopwords = HashSet::new();
    for w in ["and", "are", "loves"] { stopwords.insert(w.to_string()); }
    let options = AnalysisOptions { ngram: 1, context: 2, stemming: StemMode::Off };
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

1. Split the original (non-stemmed) text into tokens.
2. A token is counted as a candidate entity if:
   - it starts with an uppercase letter (Unicode-aware), and
   - it is **not** fully uppercase (to avoid acronyms) and
   - it is **not** the very first token of a sentence if it is a common function word (e.g. English “The”).  
3. Candidates are aggregated case-sensitively (so “Berlin” and “BERLIN” are treated differently).

> Note: This heuristic is language-agnostic and will overgenerate in some cases (sentence-initial words). It is intentionally **fast** and **deterministic**. For higher quality, consider post-filtering with custom lists or integrating a proper NER model.

---

## Scientific Features & Best Practices

* Multi-language support: automatic language detection for English, German, French, Spanish, Italian, Arabic
* Optional stemming for many languages (e.g., en, de, fr, es, it, pt, nl, ru, sv, fi, no, ro, hu, da, tr)
* Optional custom stoplist (e.g. to filter project-specific terms)
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
* [x] Context window size (CLI flag)
* [ ] Parallel file analysis for very large corpora
* [ ] Built-in default stoplists per language (optional)
* [ ] Lemmatization for more languages (if crates become available)
* [ ] More advanced reporting (collocation metrics, word clouds)

Contributions welcome, especially for more languages, better PDF/docx parsing, or improved output!

---

## License

MIT

---

## Feedback & Issues

Feedback, bug reports, and pull requests are highly appreciated! Open an Issue or start a discussion.
