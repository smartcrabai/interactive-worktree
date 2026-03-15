use anyhow::{Result, bail};
use inquire::{MultiSelect, Text};

use crate::{git, worktree_ops};

pub fn run() -> Result<()> {
    let wts = git::worktree_list()?;
    let branches: Vec<String> = wts.iter().skip(1).map(|w| w.branch.clone()).collect();
    if branches.is_empty() {
        bail!("No worktrees to copy to (only main worktree exists)");
    }

    let targets = MultiSelect::new("Copy files to which worktrees:", branches)
        .with_help_message("Space to select, Enter to confirm")
        .prompt()?;

    if targets.is_empty() {
        println!("No worktrees selected.");
        return Ok(());
    }

    let pattern = Text::new("File pattern (optional, Enter to skip):")
        .with_help_message("e.g. *.json, package-lock.json")
        .prompt()?;

    let pattern_opt = if pattern.is_empty() {
        None
    } else {
        Some(pattern.as_str())
    };

    for target in &targets {
        let Some(path) = wts
            .iter()
            .find(|w| &w.branch == target)
            .map(|w| w.path.clone())
        else {
            eprintln!("Worktree not found for '{target}', skipping.");
            continue;
        };

        if let Err(e) = worktree_ops::copy_files(&path, pattern_opt) {
            eprintln!("Failed to copy to '{target}': {e}");
        }
    }

    Ok(())
}
