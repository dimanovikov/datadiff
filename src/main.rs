mod diff;
mod output;
mod parse;
mod value;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Context;
use clap::Parser;

use parse::Format;

/// Semantic diff for structured data files (JSON, YAML, CSV, TOML).
///
/// Compares data by structure instead of line by line: key order and
/// formatting are ignored, arrays of objects can be matched by a key field,
/// and changes are reported as data paths.
#[derive(Parser)]
#[command(name = "datadiff", version, about)]
struct Cli {
    /// Old (baseline) file.
    old: PathBuf,
    /// New file to compare against the baseline.
    new: PathBuf,

    /// Field used to match array elements / CSV rows instead of position
    /// (e.g. --key id). Reordering then counts as "no change".
    #[arg(long, env = "DATADIFF_KEY")]
    key: Option<String>,

    /// File format. Autodetected from the extension by default.
    #[arg(long, value_enum, env = "DATADIFF_FORMAT")]
    format: Option<Format>,

    /// Also print unchanged entries (grey `=` lines).
    #[arg(long, env = "DATADIFF_SHOW_UNCHANGED")]
    show_unchanged: bool,

    /// Disable colored output.
    #[arg(long, env = "DATADIFF_NO_COLOR")]
    no_color: bool,
}

fn run(cli: &Cli) -> anyhow::Result<diff::Summary> {
    if cli.no_color {
        colored::control::set_override(false);
    }

    let format = match cli.format {
        Some(f) => f,
        None => parse::detect_format(&cli.old)
            .context("format autodetection failed for the old file")?,
    };

    let old_text = std::fs::read_to_string(&cli.old)
        .with_context(|| format!("cannot read file '{}'", cli.old.display()))?;
    let new_text = std::fs::read_to_string(&cli.new)
        .with_context(|| format!("cannot read file '{}'", cli.new.display()))?;

    let old_value = parse::parse(&old_text, format)
        .with_context(|| format!("cannot parse '{}'", cli.old.display()))?;
    let new_value = parse::parse(&new_text, format)
        .with_context(|| format!("cannot parse '{}'", cli.new.display()))?;

    let changes = diff::diff(&old_value, &new_value, cli.key.as_deref());
    let summary = diff::summarize(&changes);

    for change in &changes {
        if let Some(line) = output::render_change(change, cli.show_unchanged, !cli.no_color) {
            println!("{line}");
        }
    }
    println!("{}", output::render_summary(&summary));

    Ok(summary)
}

fn main() -> ExitCode {
    dotenvy::dotenv().ok(); // optional .env in the working directory
    let cli = Cli::parse();
    match run(&cli) {
        Ok(summary) => {
            if summary.total() == 0 {
                ExitCode::SUCCESS // 0: no differences
            } else {
                ExitCode::from(1) // 1: differences found
            }
        }
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::from(2) // 2: usage / IO / parse error
        }
    }
}
