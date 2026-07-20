//! Command dispatch and output formatting.
use anyhow::Result;

use crate::cli::{self, Command};
use crate::repository::TaskRepository;
use crate::storage::Storage;
use crate::storage::default_storage;

pub fn dispatch(command: Option<Command>) -> Result<()> {
    let repo = TaskRepository::new(default_storage()?);

    match command {
        None => list_pending(&repo),
        Some(Command::Add { description }) => add(&repo, description),
        Some(Command::All) => list_all(&repo),
        Some(Command::Done { selectors }) => set_completed(&repo, &selectors, true),
        Some(Command::Undo { selectors }) => set_completed(&repo, &selectors, false),
        Some(Command::Rm { selectors }) => remove(&repo, &selectors),
    }
}

fn add<S: Storage>(repo: &TaskRepository<S>, description: String) -> Result<()> {
    let task = repo.add(description)?;
    println!("Created task {}.", task.id);
    Ok(())
}

fn list_pending<S: Storage>(repo: &TaskRepository<S>) -> Result<()> {
    let tasks = repo.list_pending()?;
    print_pending(&tasks);
    Ok(())
}

fn list_all<S: Storage>(repo: &TaskRepository<S>) -> Result<()> {
    let tasks = repo.list_all()?;
    print_all(&tasks);
    Ok(())
}

fn set_completed<S: Storage>(
    repo: &TaskRepository<S>,
    selectors: &[String],
    completed: bool,
) -> Result<()> {
    let ids = cli::expand_selectors(selectors)?;
    let touched = repo.set_completed(&ids, completed)?;
    let action = if completed { "done" } else { "undone" };
    print_marked(action, touched.len());
    Ok(())
}

fn remove<S: Storage>(repo: &TaskRepository<S>, selectors: &[String]) -> Result<()> {
    let ids = cli::expand_selectors(selectors)?;
    let touched = repo.remove(&ids)?;
    print_removed(touched.len());
    Ok(())
}

// ---- Output formatting ----------------------------------------------------

/// Render the `id` column width from the largest id.
fn id_width(tasks: &[crate::model::Task]) -> usize {
    let max = tasks.iter().map(|t| t.id).max().unwrap_or(0);
    let s = max.to_string().len();
    s.max(2)
}

fn print_pending(tasks: &[crate::model::Task]) {
    if tasks.is_empty() {
        println!("0 tasks.");
        return;
    }
    let w = id_width(tasks);
    println!("{:width$} Description", "ID", width = w);
    println!("{:width$} -----------", "-".repeat(w), width = w);
    for t in tasks {
        println!("{:width$} {}", t.id, t.description, width = w);
    }
    println!();
    println!(
        "{} {}.",
        tasks.len(),
        if tasks.len() == 1 { "task" } else { "tasks" }
    );
}

fn print_all(tasks: &[crate::model::Task]) {
    if tasks.is_empty() {
        println!("0 tasks.");
        return;
    }
    let w = id_width(tasks);
    println!("{:width$} ✓ Description", "ID", width = w);
    println!("{:width$} - -----------", "-".repeat(w), width = w);
    for t in tasks {
        let mark = if t.completed { "✓" } else { "" };
        println!("{:width$} {:<1} {}", t.id, mark, t.description, width = w);
    }
    println!();
    println!(
        "{} {}.",
        tasks.len(),
        if tasks.len() == 1 { "task" } else { "tasks" }
    );
}

fn print_marked(action: &str, n: usize) {
    println!(
        "Marked {} {} {}.",
        n,
        if n == 1 { "task" } else { "tasks" },
        action
    );
}

fn print_removed(n: usize) {
    println!("Removed {} {}.", n, if n == 1 { "task" } else { "tasks" });
}
