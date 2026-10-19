//! `kubectl datadiff`: `kubectl diff` with datadiff as its diff program.
//!
//! kubectl runs any executable named `kubectl-datadiff` on PATH as
//! `kubectl datadiff` (krew installs one as a symlink to this binary), so the
//! binary checks the name it was started under. It then runs `kubectl diff`
//! with `KUBECTL_EXTERNAL_DIFF` pointing back at itself. kubectl silently
//! drops every argument in that variable that is not made of letters, digits,
//! `-` and `=`, which rules out paths such as `metadata.generation`, so
//! datadiff's own options travel as `DATADIFF_*` environment variables
//! instead. Every other argument goes to `kubectl diff` unchanged.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::{Command, ExitCode};

use anyhow::{bail, Context};
use clap::ValueEnum;

use crate::parse::Format;

/// The executable name kubectl looks up for `kubectl datadiff`.
const PLUGIN_NAME: &str = "kubectl-datadiff";

/// Set for `kubectl diff`, and so inherited by the diff program it starts.
/// kubectl may start that program under the plugin name too (it is the same
/// file), and the marker makes it compare the two directories instead of
/// running `kubectl diff` again.
const CHILD_MARKER: &str = "DATADIFF_KUBECTL_CHILD";

/// datadiff options the plugin takes, and the variable that carries each one.
/// `Cli` reads every one of these variables through clap's `env`.
const VALUE_OPTIONS: &[(&str, &str)] = &[
    ("--key", "DATADIFF_KEY"),
    ("--format", "DATADIFF_FORMAT"),
    ("--fail-on", "DATADIFF_FAIL_ON"),
    ("--ignore", "DATADIFF_IGNORE"),
];
const FLAG_OPTIONS: &[(&str, &str)] = &[
    ("--show-unchanged", "DATADIFF_SHOW_UNCHANGED"),
    ("--no-color", "DATADIFF_NO_COLOR"),
    ("--exit-zero", "DATADIFF_EXIT_ZERO"),
];
/// Options that may repeat; their values add up, comma-separated, the way
/// `Cli` splits them.
const LIST_OPTIONS: &[&str] = &["--fail-on", "--ignore"];

const HELP: &str = "\
kubectl datadiff: kubectl diff, with datadiff showing what changed

Usage: kubectl datadiff [DATADIFF OPTIONS] [KUBECTL DIFF ARGUMENTS]

Runs `kubectl diff` with datadiff as KUBECTL_EXTERNAL_DIFF. Arguments that
are not datadiff options go to `kubectl diff` unchanged.

datadiff options:
      --key <FIELD>          Match list items such as containers by this field
      --ignore <PATTERNS>    Leave out changes at these paths
      --fail-on <PATTERNS>   Exit 1 only when a change matches these paths
      --format <FORMAT>      Read objects in this format [default: yaml]
      --show-unchanged       Also print unchanged entries
      --no-color             Disable colored output
      --exit-zero            Exit 0 even when there are differences
  -h, --help                 Print this help
  -V, --version              Print the version

Patterns are paths such as spec.replicas or metadata.annotations; --ignore
and --fail-on repeat or take a comma-separated list.

Examples:
  kubectl datadiff -f k8s/ --key name
  kubectl datadiff -k overlays/prod --key name --ignore metadata.generation
";

/// True when this process should act as `kubectl datadiff`.
pub fn invoked_as_plugin() -> bool {
    std::env::var_os(CHILD_MARKER).is_none()
        && std::env::args_os()
            .next()
            .is_some_and(|argv0| Path::new(&argv0).file_stem() == Some(OsStr::new(PLUGIN_NAME)))
}

pub fn run() -> ExitCode {
    match try_run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::from(2)
        }
    }
}

fn try_run() -> anyhow::Result<ExitCode> {
    let (env, kubectl_args) = match plan(std::env::args_os().skip(1))? {
        Plan::Help => {
            print!("{HELP}");
            return Ok(ExitCode::SUCCESS);
        }
        Plan::Version => {
            println!("kubectl-datadiff {}", env!("CARGO_PKG_VERSION"));
            return Ok(ExitCode::SUCCESS);
        }
        Plan::Run { env, kubectl_args } => (env, kubectl_args),
    };

    // kubectl splits KUBECTL_EXTERNAL_DIFF on spaces, so a path with one in
    // it (C:\Users\Jane Doe\.krew\...) cannot go in whole. Such a path is put
    // on PATH and only the file name is passed.
    let exe = std::env::current_exe().context("cannot find the datadiff executable")?;
    let mut kubectl = if exe.to_string_lossy().contains(char::is_whitespace) {
        let (Some(dir), Some(name)) = (exe.parent(), exe.file_name()) else {
            bail!("cannot use '{}' as KUBECTL_EXTERNAL_DIFF", exe.display());
        };
        let path = std::env::var_os("PATH").unwrap_or_default();
        let path = std::env::join_paths(
            std::iter::once(dir.to_path_buf()).chain(std::env::split_paths(&path)),
        )
        .context("cannot add the datadiff directory to PATH")?;
        // Command looks the program up on the PATH it is given, so kubectl is
        // found first, on the PATH the user set.
        let mut kubectl = Command::new(find_on_path("kubectl")?);
        kubectl.env("PATH", path).env("KUBECTL_EXTERNAL_DIFF", name);
        kubectl
    } else {
        let mut kubectl = Command::new("kubectl");
        kubectl.env("KUBECTL_EXTERNAL_DIFF", &exe);
        kubectl
    };
    kubectl
        .arg("diff")
        .args(&kubectl_args)
        .envs(&env)
        .env(CHILD_MARKER, "1");

    let status = kubectl
        .status()
        .context("cannot run kubectl; is it installed and on PATH?")?;
    // kubectl diff exits 0 for no differences, 1 for differences (our own
    // exit code, passed through) and above 1 for errors. A kubectl killed by
    // a signal has no code, which counts as an error.
    let code = status
        .code()
        .and_then(|c| u8::try_from(c).ok())
        .unwrap_or(2);
    Ok(ExitCode::from(code))
}

enum Plan {
    Help,
    Version,
    Run {
        env: BTreeMap<&'static str, String>,
        kubectl_args: Vec<OsString>,
    },
}

/// Sort the plugin's arguments into datadiff options (as variables) and
/// arguments for `kubectl diff`.
fn plan(args: impl IntoIterator<Item = OsString>) -> anyhow::Result<Plan> {
    let mut env: BTreeMap<&'static str, String> = BTreeMap::new();
    let mut kubectl_args = Vec::new();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let Some(text) = arg.to_str() else {
            kubectl_args.push(arg);
            continue;
        };
        match text {
            "-h" | "--help" => return Ok(Plan::Help),
            "-V" | "--version" => return Ok(Plan::Version),
            _ => {}
        }
        let (name, inline_value) = match text.split_once('=') {
            Some((name, value)) => (name, Some(value)),
            None => (text, None),
        };
        if let Some(&(option, var)) = VALUE_OPTIONS.iter().find(|(o, _)| *o == name) {
            let value = match inline_value {
                Some(value) => value.to_string(),
                None => args
                    .next()
                    .and_then(|value| value.into_string().ok())
                    .with_context(|| format!("{option} needs a value"))?,
            };
            if option == "--format" && Format::from_str(&value, false).is_err() {
                let known: Vec<String> = Format::value_variants()
                    .iter()
                    .filter_map(|f| f.to_possible_value())
                    .map(|p| p.get_name().to_string())
                    .collect();
                bail!("unknown --format '{value}' (expected {})", known.join(", "));
            }
            match env.get_mut(var) {
                Some(existing) if LIST_OPTIONS.contains(&option) => {
                    existing.push(',');
                    existing.push_str(&value);
                }
                _ => {
                    env.insert(var, value);
                }
            }
        } else if let Some(&(_, var)) = FLAG_OPTIONS.iter().find(|(o, _)| *o == text) {
            env.insert(var, "true".to_string());
        } else {
            kubectl_args.push(arg);
        }
    }
    Ok(Plan::Run { env, kubectl_args })
}

/// The first `name` executable on PATH, as Command would find it.
fn find_on_path(name: &str) -> anyhow::Result<std::path::PathBuf> {
    let file = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    std::env::var_os("PATH")
        .iter()
        .flat_map(std::env::split_paths)
        .map(|dir| dir.join(&file))
        .find(|candidate| candidate.is_file())
        .with_context(|| format!("cannot find {name} on PATH; is it installed?"))
}
