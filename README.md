# Text\_Analysis

A robust, modern CLI tool for linguistic text analysis in `.txt` and `.pdf` files, supporting:

* **Automatic language detection** (English, German, French, Spanish, Italian, Arabic)
* **Stemming** (where possible)
* **Stopword removal** (per language, and optionally custom stoplist)
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
* Built-in stopword lists for English, German, French, Spanish, Italian, Arabic
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

---

## Installation

* With cargo:

  ```sh
  cargo install text_analysis
  ```
* Download binary from [Releases](https://github.com/ArdentEmpiricist/text_analysis/releases)
* Clone the repository and build from source

**Requires Rust toolchain** ([rustup.rs](https://rustup.rs/))

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

**Exported files:**

* `20250803_191010_wordfreq.csv` (or `.json`)
* `20250803_191010_ngrams.csv`
* `20250803_191010_namedentities.csv`
* `20250803_191010_pmi.csv`

---

## Library Example

```rust
use text_analysis::{analyze_text, trim_to_words, english_stopwords};
let text = "The quick brown fox jumps over the lazy dog.";
let stopwords = english_stopwords();
let out = analyze_text(text, None, &stopwords, 2);
println!("{}", out);
```

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
