//! Command-line interface.
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "tslay",
    bin_name = "tslay",
    version,
    about = "A small, fast command-line todo manager."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Add a new task.
    Add {
        /// The task description.
        description: String,
    },
    /// List all tasks, including completed ones.
    All,
    /// Mark tasks as done. Accepts ids and ranges, e.g. `1 3-5 8`.
    Done {
        #[arg(num_args = 1..)]
        selectors: Vec<String>,
    },
    /// Mark completed tasks as not done. Accepts ids and ranges.
    Undo {
        #[arg(num_args = 1..)]
        selectors: Vec<String>,
    },
    /// Remove tasks permanently. Accepts ids and ranges.
    Rm {
        #[arg(num_args = 1..)]
        selectors: Vec<String>,
    },
}

/// Parse a single selector token, which may be a single id (`5`) or a
/// closed range (`3-7`). Ranges expand into the inclusive list of ids.
/// Duplicate ids are deduplicated by the caller.
fn parse_selector(s: &str) -> Result<Vec<u64>, String> {
    if let Some((a, b)) = s.split_once('-') {
        let lo: u64 = a.parse().map_err(|_| format!("invalid range start: {a:?}"))?;
        let hi: u64 = b.parse().map_err(|_| format!("invalid range end: {b:?}"))?;
        if hi < lo {
            return Err(format!("range end is less than start: {s}"));
        }
        Ok((lo..=hi).collect())
    } else {
        let id: u64 = s.parse().map_err(|_| format!("invalid id: {s:?}"))?;
        Ok(vec![id])
    }
}

/// Expand a list of selector tokens into a deduplicated, sorted list of ids.
pub fn expand_selectors(selectors: &[String]) -> anyhow::Result<Vec<u64>> {
    let mut ids: Vec<u64> = Vec::new();
    for s in selectors {
        ids.extend(parse_selector(s).map_err(anyhow::Error::msg)?);
    }
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

pub fn run() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            let _ = e.print();
            return match e.kind() {
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                    ExitCode::SUCCESS
                }
                _ => ExitCode::from(2),
            };
        }
    };

    match crate::commands::dispatch(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tslay: {e}");
            ExitCode::FAILURE
        }
    }
}
