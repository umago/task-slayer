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
    /// Edit a task's description.
    Edit {
        /// The id of the task to edit.
        id: u64,
        /// The new task description.
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
    /// Self-update: download and install the latest release from GitHub.
    Update,
}

/// Parse a single selector token, which may be a single id (`5`) or a
/// closed range (`3-7`). Ranges expand into the inclusive list of ids.
/// Duplicate ids are deduplicated by the caller.
fn parse_selector(s: &str) -> Result<Vec<u64>, String> {
    if let Some((a, b)) = s.split_once('-') {
        let lo: u64 = a
            .parse()
            .map_err(|_| format!("invalid range start: {a:?}"))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_single_id() {
        let s: Vec<String> = vec!["5".into()];
        assert_eq!(expand_selectors(&s).unwrap(), vec![5]);
    }

    #[test]
    fn selector_multiple_ids() {
        let s: Vec<String> = vec!["1".into(), "4".into(), "8".into()];
        assert_eq!(expand_selectors(&s).unwrap(), vec![1, 4, 8]);
    }

    #[test]
    fn selector_range() {
        let s: Vec<String> = vec!["3-7".into()];
        assert_eq!(expand_selectors(&s).unwrap(), vec![3, 4, 5, 6, 7]);
    }

    #[test]
    fn selector_mixed() {
        let s: Vec<String> = vec!["1".into(), "3-5".into(), "8".into()];
        assert_eq!(expand_selectors(&s).unwrap(), vec![1, 3, 4, 5, 8]);
    }

    #[test]
    fn selector_dedup() {
        let s: Vec<String> = vec!["1".into(), "1".into(), "1-3".into(), "2".into()];
        assert_eq!(expand_selectors(&s).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn selector_range_single() {
        // 5-5 should produce just [5]
        let s: Vec<String> = vec!["5-5".into()];
        assert_eq!(expand_selectors(&s).unwrap(), vec![5]);
    }

    #[test]
    fn selector_invalid_id() {
        let s: Vec<String> = vec!["abc".into()];
        assert!(expand_selectors(&s).is_err());
    }

    #[test]
    fn selector_reversed_range() {
        let s: Vec<String> = vec!["5-2".into()];
        assert!(expand_selectors(&s).is_err());
    }

    #[test]
    fn selector_empty_string() {
        let s: Vec<String> = vec!["".into()];
        assert!(expand_selectors(&s).is_err());
    }
}
