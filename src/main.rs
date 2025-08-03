use clap::{Parser, ValueEnum};
use env_logger;
use log::error;
use std::process;
use text_analysis::{ExportFormat, analyze_path, print_failed_files};

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
}

fn main() {
    env_logger::init();
    let cli = Cli::parse();

    match analyze_path(
        &cli.path,
        cli.stopwords,
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
}
