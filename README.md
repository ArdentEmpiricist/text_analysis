[![rust-clippy analyze](https://github.com/ArdentEmpiricist/text_analysis/actions/workflows/rust-clippy.yml/badge.svg)](https://github.com/ArdentEmpiricist/text_analysis/actions/workflows/rust-clippy.yml)
[![Crates.io](https://img.shields.io/crates/v/text_analysis)](https://crates.io/crates/text_analysis)
[![Documentation](https://docs.rs/text_analysis/badge.svg)](https://docs.rs/text_analysis/)
[![Crates.io](https://img.shields.io/crates/l/text_analysis)](https://github.com/LazyEmpiricist/text_analysis/blob/main/LICENSE)
[![Deploy](https://github.com/ArdentEmpiricist/text_analysis/actions/workflows/deploy.yml/badge.svg)](https://github.com/ArdentEmpiricist/text_analysis/actions/workflows/deploy.yml)
[![Crates.io](https://img.shields.io/crates/d/text_analysis?color=darkblue)](https://crates.io/crates/text_analysis)

# text_analysis

<p align="center">
  <img src="https://raw.githubusercontent.com/ArdentEmpiricist/text_analysis/main/assets/text_analysis_logo.png" alt="Text Analysis logo" width="200"/>
</p>

A fast, pragmatic CLI & library for multi-language **text analysis** across `.txt` and `.pdf` files.

**Highlights**
- Unicode-aware tokenization
- Optional stopword filtering (custom list)
- Optional stemming (auto-detected or forced language)
- N‑gram counts
- Word frequencies
- Context stats (±N) & direct neighbors (±1)
- Collocation analysis with **Pointwise Mutual Information (PMI)** for all word pairs in the context window
- Named‑Entity extraction (simple capitalization heuristic)
- **Parallel** per‑file compute (safe, serialized writes)
- **Combined (Map‑Reduce)** mode to aggregate multiple files
- **Deterministic, sorted exports** (CSV/TSV/JSON/TXT)
- Robust I/O: errors are **reported, never panic**

---

## Installation

* With cargo:

  ```sh
  cargo install text_analysis
  ```
* Download binary from [Releases](https://github.com/ArdentEmpiricist/text_analysis/releases)
* Clone the repository and build from source

---

## Quick start

```bash
# Default TXT summary (one file)
text_analysis <path>

# CSV exports (multiple files: ngrams, wordfreq, context, neighbors, pmi, namedentities)
text_analysis <path> --export-format csv

# Combine all files into one corpus (Map-Reduce) and export as JSON
text_analysis <path> --combine --export-format json
```

**Path** can be a file or a directory (recursively scanned). Supported: `.txt`, `.pdf`.

---

## CLI

```
text_analysis <path> [--stopwords <FILE>] [--ngram N] [--context N]
                  [--export-format {txt|csv|tsv|json}] [--entities-only]
                  [--combine]
                  [--stem] [--stem-lang <CODE>] [--stem-strict]
```

- `--stopwords <FILE>` – optional stopword list (one token per line).
- `--ngram N` – n‑gram size (default: **2**).
- `--context N` – context window size for context & PMI (default: **5**).
- `--export-format` – `txt` (default), `csv`, `tsv`, `json`.
- `--entities-only` – only export Named Entities (skips other tables).
- `--combine` – analyze all files as one corpus (Map‑Reduce) and write a single set of outputs.
- `--stem` – enable stemming with **auto language detection**.
- `--stem-lang <CODE>` – force stemming language (e.g., `en`, `de`, `fr`, `es`, `it`, `pt`, `nl`, `ru`, `sv`, `fi`, `no`, `ro`, `hu`, `da`, `tr`).
- `--stem-strict` – in auto mode, require detectable & supported language:  
  - **Per‑file mode:** files without detectable/supported language are **skipped** (reported).  
  - **Combined mode:** the **whole run aborts** (prevents mixed stemming).

### STDOUT summary (human-readable)

When the CLI finishes, it prints a concise summary to **stdout**. The order is tuned for usefulness:

1. **Top 20 N‑grams** (count ↓, lexicographic tie‑break)
2. **Top 20 PMI pairs** (count ↓, then PMI ↓, then words)
3. **Top 20 words** (count ↓, lexicographic tie‑break)

This surfaces phrases and salient collocations before common function words.

---

## Outputs

### TXT (default)

- **Exactly one file per run**:  
  `<stem>_<timestamp>_summary.txt`  
  Contains the three sorted blocks (Top 20 N‑grams → Top 20 PMI → Top 20 words).

### CSV / TSV / JSON

- **Multiple files per run** (one per analysis):
  - `<stem>_<timestamp>_ngrams.<ext>`
  - `<stem>_<timestamp>_wordfreq.<ext>`
  - `<stem>_<timestamp>_context.<ext>`
  - `<stem>_<timestamp>_neighbors.<ext>`
  - `<stem>_<timestamp>_pmi.<ext>`
  - `<stem>_<timestamp>_namedentities.<ext>`

  ### Output file overview

| File suffix              | Contents                                                                                     | Notes                                                                                  |
|--------------------------|----------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------|
| `_ngrams.<ext>`          | List of all observed n-grams and their counts                                                | Sorted by count ↓, then lexicographically ↑                                           |
| `_wordfreq.<ext>`        | Word frequency table (unigrams only)                                                         | Sorted by count ↓, then lexicographically ↑                                           |
| `_context.<ext>`         | Directed co-occurrence counts for all tokens in a ±N window around each center token         | Window size set by `--context` (default 5); includes all words except the center word |
| `_neighbors.<ext>`       | Directed co-occurrence counts for immediate left/right neighbors (±1 distance)               | Always exactly one left and one right position per center token          |
| `_pmi.<ext>`             | Word pairs within the context window with their counts, distances, and Pointwise Mutual Information | Pairs are unordered in storage, sorted by count ↓, PMI ↓ in export                     |
| `_namedentities.<ext>`   | Named entities detected via capitalization heuristic and their counts                        | Case-sensitive; ignores acronyms and common articles/determiners                      |


Sorting rules applied to **all** tabular exports:

- **N‑grams & Wordfreq**: by **count desc**, then **key asc**.
- **Context & Neighbors** (flattened): by **count desc**, then keys.
- **PMI**: by **count desc**, then **PMI desc**, then words.

### Combined mode

With `--combine`, all inputs are processed as one corpus and exported **once** with stem `"combined"`:
- `combined_<timestamp>_wordfreq.<ext>`, `combined_<timestamp>_ngrams.<ext>`, …

### File naming

`<stem>` is collision‑safe: derived from the file name plus a short path hash. In per‑file mode each input gets its own stem; in combined mode the stem is literally `combined`.

---

## Library usage

Add to `Cargo.toml`:

```toml
[dependencies]
text_analysis = "0.4.7"
```

Basic example:

```rust
use std::collections::HashSet;
use text_analysis::*;

fn main() -> Result<(), String> {
    let text = "The quick brown fox jumps over the lazy dog.";
    let opts = AnalysisOptions {
        ngram: 2,
        context: 5,
        export_format: ExportFormat::Json,
        entities_only: false,
        combine: false,
        stem_mode: StemMode::Off,
        stem_require_detected: false,
    };
    let stop = HashSet::new();
    let result = analyze_text_with(text, &stop, &opts);
    println!("Top words: {:?}", result.wordfreq);
    Ok(())
}
```

### Named‑Entity heuristic

- Token starts with an **uppercase** letter
- Token is **not all uppercase** (filters acronyms)
- Filters very common determiners/articles across DE/EN/FR/ES/IT

Counts are **case‑sensitive** and computed on **original tokens** (not stemmed).

### Stemming

- `StemMode::Off` – no stemming
- `StemMode::Auto` – language via `whatlang`; stem if supported
- `StemMode::Force(lang)` – use a specific stemmer

`stem_require_detected` controls strictness in auto mode (see CLI).

---

## PDF support

Uses **pdf-extract**. Files that fail to parse are listed in the warnings and don’t abort the run.

---

## Best practices

- Use `--export-format csv` (or `tsv`/`json`) for downstream analysis in pandas/R/Excel.
- In noisy corpora, prefer `--ngram 2` or `--ngram 3` and check PMI first.
- For mixed‑language corpora, consider `--stem-strict` to avoid inconsistent stemming.

---

## License

MIT


## Security: CSV/TSV safety

If you open exports in Excel/LibreOffice, cells that begin with `=`, `+`, `-`, or `@` can be interpreted
as formulas. The recommended approach is:

- Use a proper CSV library (this project uses `csv::Writer`) for escaping.
- Prefix a `'` for any **text cell** that starts with one of those characters.

This prevents spreadsheet software from executing user-provided content.
