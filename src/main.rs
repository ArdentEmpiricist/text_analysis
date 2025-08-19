#![forbid(unsafe_code)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/ArdentEmpiricist/text_analysis/main/assets/text_analysis_logo.png"
)]
//! # Text Analysis CLI
//!
//! Command-line interface for the `text_analysis` library. Runs n‑gram, context
//! statistics, named entity extraction and PMI collocations over `.txt`, `.pdf`, `.docx`, and `.odt` inputs.
//!
//! ## Highlights
//! - Analyze single files or combine a whole folder (no double scanning of files).
//! - Export to TXT/CSV/TSV/JSON.
//! - Configurable n‑gram size and ±context window.
//! - Optional: custom stopword list, stemming (auto via language detection or forced).
//! - CSV/TSV safety: When exporting CSV/TSV, always writes via `csv::Writer` and sanitize any user-derived text cell that starts with `=`, `+`, `-`, or `@` by prefixing `'` to prevent formula execution in spreadsheet apps.
//!
//! See README for details.

use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use text_analysis::{AnalysisOptions, ExportFormat, StemLang, StemMode, analyze_path};

/// Text_Analysis — fast multilingual text CLI
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// File or directory (recursively analyzed)
    path: PathBuf,

    /// Optional stopword list (one word per line)
    #[arg(long)]
    stopwords: Option<PathBuf>,

    /// N-gram size (2 = bigrams, 3 = trigrams, ...)
    #[arg(long, default_value_t = 2)]
    ngram: usize,

    /// Context window size (±N words)
    #[arg(long, default_value_t = 5)]
    context: usize,

    /// Export format
    #[arg(long, value_enum, default_value_t = CliExportFormat::Txt)]
    export_format: CliExportFormat,

    /// Export only named entities (instead of full statistics)
    #[arg(long, default_value_t = false)]
    entities_only: bool,

    /// Combine all files into one corpus (Map-Reduce)
    #[arg(long, default_value_t = false)]
    combine: bool,

    /// Enable stemming (auto-detected language)
    #[arg(long, default_value_t = false)]
    stem: bool,

    /// Force stemming language (e.g., en, de, fr, es, it, pt, nl, ru, sv, fi, no, ro, hu, da, tr). Takes effect even without --stem.
    #[arg(long)]
    stem_lang: Option<String>,

    /// Require detectable/supported language for auto stemming; otherwise fail/skip
    #[arg(long, default_value_t = false)]
    stem_strict: bool,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, ValueEnum)]
enum CliExportFormat {
    Txt,
    Csv,
    Tsv,
    Json,
}

impl From<CliExportFormat> for ExportFormat {
    fn from(v: CliExportFormat) -> Self {
        match v {
            CliExportFormat::Txt => ExportFormat::Txt,
            CliExportFormat::Csv => ExportFormat::Csv,
            CliExportFormat::Tsv => ExportFormat::Tsv,
            CliExportFormat::Json => ExportFormat::Json,
        }
    }
}

fn main() {
    let cli = Cli::parse();

    // Stemming precedence:
    // 1) --stem-lang LANG forces that language (even without --stem)
    // 2) Otherwise, --stem enables Auto detection
    // 3) Otherwise, Off
    let stem_mode = match (cli.stem, cli.stem_lang.as_deref()) {
        (_, Some(code)) => StemMode::Force(StemLang::from_code(code).unwrap_or(StemLang::Unknown)),
        (true, None) => StemMode::Auto,
        _ => StemMode::Off,
    };

    let options = AnalysisOptions {
        ngram: cli.ngram,
        context: cli.context,
        export_format: cli.export_format.into(),
        entities_only: cli.entities_only,
        combine: cli.combine,
        stem_mode,
        stem_require_detected: cli.stem_strict,
    };

    match analyze_path(&cli.path, cli.stopwords.as_ref(), &options) {
        Ok(report) => {
            // Print the tuned STDOUT summary produced by `summary_for(...)`
            println!("{}", report.summary);

            // Optional: show warnings for files that failed or were skipped
            if !report.failed_files.is_empty() {
                eprintln!("Warnings ({} files):", report.failed_files.len());
                for (file, err) in report.failed_files {
                    eprintln!("  {} -> {}", file, err);
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
