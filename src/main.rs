#![forbid(unsafe_code)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/ArdentEmpiricist/text_analysis/main/assets/text_analysis_logo.png"
)]
//! # Text Analysis CLI
//!
//! Command-line interface for the `text_analysis` library. Runs n‑gram, context
//! statistics, named entity extraction and PMI collocations over `.txt` and `.pdf` inputs.
//!
//! ## Highlights
//! - Analyze single files or combine a whole folder (no double scanning of files).
//! - Export to TXT/CSV/TSV/JSON.
//! - Configurable n‑gram size and ±context window.
//! - Optional: custom stopword list, stemming (auto via language detection or forced).
//! - Entities-only export mode.
//!
//! See README for details.

use clap::{Parser, ValueEnum};
use std::path::PathBuf;
use std::process;

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

    /// Combine all files into one corpus
    #[arg(long, default_value_t = false)]
    combine: bool,

    /// Enable stemming (language auto-detected)
    #[arg(long, default_value_t = false)]
    stem: bool,

    /// Force stemming language (e.g., en, de, fr, es, it, pt, nl, ru, sv, fi, no, ro, hu, da, tr)
    #[arg(long)]
    stem_lang: Option<String>,
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

    // Determine stemming mode from CLI flags.
    let stem_mode = if let Some(code) = cli.stem_lang.as_deref() {
        // Explicit language wins, but only if --stem is set.
        if cli.stem {
            StemMode::Force(StemLang::from_code(code).unwrap_or(StemLang::Unknown))
        } else {
            StemMode::Off
        }
    } else if cli.stem {
        StemMode::Auto
    } else {
        StemMode::Off
    };

    let options = AnalysisOptions {
        ngram: cli.ngram,
        context: cli.context,
        export_format: cli.export_format.into(),
        entities_only: cli.entities_only,
        combine: cli.combine,
        stem_mode,
    };

    match analyze_path(&cli.path, cli.stopwords.as_ref(), &options) {
        Ok(report) => {
            // Human-readable summary. Detailed data is written to files or stdout depending on format.
            eprintln!("{}", report.summary);
        }
        Err(e) => {
            eprintln!("Error: {e}");
            process::exit(1);
        }
    }
}
