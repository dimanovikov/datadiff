mod diff;
mod output;
mod parse;
mod patch;
mod policy;
mod value;
mod write;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Context;
use clap::Parser;

use output::OutputFormat;
use parse::Format;

/// Semantic diff for structured data files (JSON, YAML, CSV, TOML, XML).
///
/// Compares data by structure instead of line by line: key order and
/// formatting are ignored, arrays of objects can be matched by a key field,
/// and changes are reported as data paths.
#[derive(Parser)]
#[command(name = "datadiff", version, about)]
struct Cli {
    /// Old (baseline) file.
    old: Option<PathBuf>,
    /// New file to compare against the baseline.
    new: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Sub>,

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

    /// Output format: human-readable text or machine-readable JSON.
    #[arg(long, value_enum, env = "DATADIFF_OUTPUT")]
    output: Option<OutputFormat>,

    /// Always exit with code 0 when the comparison succeeds, even when
    /// differences are found (for use as a git external diff driver).
    /// Errors still exit with code 2.
    #[arg(long, env = "DATADIFF_EXIT_ZERO")]
    exit_zero: bool,

    /// Risk policy for CI: exit 1 only when a change path matches one of
    /// these patterns (exact, dot-segment prefix, or `*` glob). Other
    /// changes still print but exit 0. Repeatable, comma-separated.
    #[arg(long, env = "DATADIFF_FAIL_ON", value_delimiter = ',')]
    fail_on: Vec<String>,
}

#[derive(clap::Subcommand)]
enum Sub {
    /// Apply a JSON patch (as produced by `datadiff --output json`) to a
    /// file and print the patched document as JSON to stdout.
    Patch {
        /// File to patch.
        file: PathBuf,
        /// Patch document in the `--output json` format.
        patch: PathBuf,
    },
    /// Convert a file between supported formats (e.g. YAML to JSON). The
    /// output format is taken from the output file's extension.
    Convert {
        /// Input file.
        input: PathBuf,
        /// Output file (created or overwritten).
        output: PathBuf,
    },
}

fn run_diff(cli: &Cli) -> anyhow::Result<DiffOutcome> {
    let old_path = cli
        .old
        .as_deref()
        .context("missing OLD and NEW file arguments (or use a subcommand like `patch`)")?;
    let new_path = cli.new.as_deref().context("missing NEW file argument")?;

    let format = match cli.format {
        Some(f) => f,
        None => parse::detect_format(old_path)
            .or_else(|_| parse::detect_format(new_path))
            .context("format autodetection failed for both files")?,
    };

    let old_value = read_and_parse(old_path, format)?;
    let new_value = read_and_parse(new_path, format)?;

    // A missing side (git diff driver passes /dev/null for added/deleted
    // files) diffs as an empty container of the other side's type, so an
    // added file reports every entry as `+` instead of one root change.
    let old_value =
        old_value.unwrap_or_else(|| empty_like(new_value.as_ref().unwrap_or(&value::Value::Null)));
    let new_value = new_value.unwrap_or_else(|| empty_like(&old_value));

    let changes = diff::diff(&old_value, &new_value, cli.key.as_deref());
    let summary = diff::summarize(&changes);

    match cli.output.unwrap_or_default() {
        OutputFormat::Text => {
            for change in &changes {
                if let Some(line) = output::render_change(change, cli.show_unchanged, !cli.no_color)
                {
                    println!("{line}");
                }
            }
            println!("{}", output::render_summary(&summary));
        }
        OutputFormat::Json => {
            println!(
                "{}",
                output::render_json(&changes, &summary, cli.show_unchanged)
            );
        }
    }

    let fail_on_hit = !cli.fail_on.is_empty()
        && changes.iter().any(|c| {
            c.kind != diff::ChangeKind::Unchanged
                && cli.fail_on.iter().any(|p| policy::matches(p, &c.path))
        });

    Ok(DiffOutcome {
        summary,
        fail_on_hit,
    })
}

/// Result of a diff run: the change counts plus whether any change path
/// matched a `--fail-on` pattern.
struct DiffOutcome {
    summary: diff::Summary,
    fail_on_hit: bool,
}

fn run_patch(cli: &Cli, file: &Path, patch_path: &Path) -> anyhow::Result<()> {
    let format = match cli.format {
        Some(f) => f,
        None => parse::detect_format(file).context("format autodetection failed for the file")?,
    };

    let mut tree = read_and_parse(file, format)?.unwrap_or(value::Value::Null);
    let patch_text = std::fs::read_to_string(patch_path)
        .with_context(|| format!("cannot read file '{}'", patch_path.display()))?;
    let patch_doc: serde_json::Value = serde_json::from_str(&patch_text)
        .with_context(|| format!("invalid patch JSON in '{}'", patch_path.display()))?;

    patch::apply_patch(&mut tree, &patch_doc)?;

    println!("{}", serde_json::to_string_pretty(&tree.to_serde_json())?);
    Ok(())
}

fn run_convert(cli: &Cli, input: &Path, output: &Path) -> anyhow::Result<()> {
    let in_format = match cli.format {
        Some(f) => f,
        None => {
            parse::detect_format(input).context("format autodetection failed for the input file")?
        }
    };
    let out_format =
        parse::detect_format(output).context("format autodetection failed for the output file")?;

    let tree = read_and_parse(input, in_format)?.unwrap_or(value::Value::Null);
    let text = write::write(&tree, out_format)
        .with_context(|| format!("cannot convert to '{}'", output.display()))?;
    std::fs::write(output, format!("{text}\n"))
        .with_context(|| format!("cannot write file '{}'", output.display()))?;
    Ok(())
}

/// Read and parse a file; `Ok(None)` means "no document here": the path is
/// `/dev/null` (git's placeholder for the missing side of an added/deleted
/// file) or the file is empty.
fn read_and_parse(path: &Path, format: Format) -> anyhow::Result<Option<value::Value>> {
    if path == Path::new("/dev/null") {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read file '{}'", path.display()))?;
    if text.trim().is_empty() {
        return Ok(None);
    }
    parse::parse(&text, format)
        .map(Some)
        .with_context(|| format!("cannot parse '{}'", path.display()))
}

/// An empty container of the same kind as `other` (or Null for scalars).
fn empty_like(other: &value::Value) -> value::Value {
    match other {
        value::Value::Object(_) => value::Value::Object(Default::default()),
        value::Value::Array(_) => value::Value::Array(Vec::new()),
        _ => value::Value::Null,
    }
}

fn main() -> ExitCode {
    dotenvy::dotenv().ok(); // optional .env in the working directory
    let cli = Cli::parse();
    if cli.no_color {
        colored::control::set_override(false);
    }

    let result = match &cli.command {
        Some(Sub::Patch { file, patch }) => run_patch(&cli, file, patch).map(|_| 0),
        Some(Sub::Convert { input, output }) => run_convert(&cli, input, output).map(|_| 0),
        None => run_diff(&cli).map(|outcome| {
            let policy_passed = !cli.fail_on.is_empty() && !outcome.fail_on_hit;
            if outcome.summary.total() == 0 || cli.exit_zero || policy_passed {
                0 // no differences, overridden, or no --fail-on pattern matched
            } else {
                1 // differences found (and matching --fail-on, if given)
            }
        }),
    };

    match result {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::from(2) // usage / IO / parse / patch error
        }
    }
}
