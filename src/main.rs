#![forbid(unsafe_code)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/ArdentEmpiricist/text_analysis/main/assets/text_analysis_logo.png"
)]
//! # Text Analysis CLI
//!
//! This is the command-line interface for the `text_analysis` crate.
//! It provides a convenient way to run N-gram, context, and entity analysis
//! on `.txt` and `.pdf` documents without writing Rust code.
//!
//! ## Features
//! - Analyze each file individually or all files combined.
//! - Export results in multiple formats.
//! - Configure N-gram size, context window, and optional stopword list.
//! - Named entity extraction mode.
//!
//! ## Example
//! ```bash
//! cargo run --release -- path/to/data --ngram 3 --context 5 --export-format csv
//! ```
//!
//! See `--help` for all available options.

use clap::{Parser, ValueEnum};
use env_logger;
use log::error;
use std::path::Path;
use std::process;
use text_analysis::{
    ExportFormat, analyze_path, analyze_path_combined, collect_files, print_failed_files,
};

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    /// File or directory to analyze
    path: String,

    /// Optional path to additional stopword file (.txt, one word per line)
    #[arg(long)]
    stopwords: Option<String>,

    /// Size of N for N-gram analysis (e.g. 2 for bigrams, 3 for trigrams)
    #[arg(long, default_value_t = 2)]
    ngram: usize,

    /// Context window size for collocation (e.g. 5 = ±5)
    #[arg(long, default_value_t = 5)]
    context: usize,

    /// Output format for export (txt, csv, tsv, json)
    #[arg(long, default_value = "txt")]
    export_format: ExportFormat,

    /// Export only named entities (names) (default: false)
    #[arg(long, default_value_t = false)]
    entities_only: bool,

    /// If set, analyze all files together and output combined results
    #[arg(long, default_value_t = false)]
    combine: bool,
}

fn main() {
    env_logger::init();
    let cli = Cli::parse();

    if cli.combine {
        // Combine mode: Analyze all files jointly and export one combined result set
        match analyze_path_combined(
            &cli.path,
            cli.stopwords.clone(),
            cli.ngram,
            cli.context,
            cli.export_format,
            cli.entities_only,
        ) {
            Ok(report) => {
                if !cli.entities_only {
                    println!("{}", report.result);
                }
                if !report.failed_files.is_empty() {
                    print_failed_files(&report.failed_files);
                }
            }
            Err(e) => {
                error!("Error: {}", e);
                process::exit(1);
            }
        }
    } else {
        // Default mode: Analyze each file separately and output results per file
        let files = collect_files(Path::new(&cli.path));
        let mut any_errors = false;
        for file in files {
            match analyze_path(
                &file,
                cli.stopwords.clone(),
                cli.ngram,
                cli.context,
                cli.export_format,
                cli.entities_only,
            ) {
                Ok(report) => {
                    if !cli.entities_only {
                        println!("{}", report.result);
                    }
                    if !report.failed_files.is_empty() {
                        print_failed_files(&report.failed_files);
                        any_errors = true;
                    }
                }
                Err(e) => {
                    error!("Error analyzing {}: {}", file, e);
                    any_errors = true;
                }
            }
        }
        if any_errors {
            process::exit(1);
        }
    }
}
