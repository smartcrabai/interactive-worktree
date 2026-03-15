use anyhow::{Result, bail};
use inquire::{Confirm, MultiSelect};

use crate::{git, worktree_ops};

pub fn run() -> Result<()> {
    let wts = git::worktree_list()?;
    let branches: Vec<String> = wts.iter().skip(1).map(|w| w.branch.clone()).collect();
    if branches.is_empty() {
        bail!("No worktrees to remove (only main worktree exists)");
    }

    let selected = MultiSelect::new("Select worktrees to remove:", branches)
        .with_help_message("Space to select, Enter to confirm")
        .prompt()?;

    if selected.is_empty() {
        println!("No worktrees selected.");
        return Ok(());
    }

    let delete_branch = Confirm::new("Also delete the branch(es)?")
        .with_default(true)
        .prompt()?;

    let force = Confirm::new("Force removal (even if dirty)?")
        .with_default(false)
        .prompt()?;

    for branch in &selected {
        let Some(path) = wts
            .iter()
            .find(|w| &w.branch == branch)
            .map(|w| w.path.clone())
        else {
            eprintln!("Worktree not found for branch '{branch}', skipping.");
            continue;
        };

        if let Err(e) = worktree_ops::remove_with_hooks(&path, branch, delete_branch, force) {
            eprintln!("Failed to remove '{branch}': {e}");
        }
    }

    Ok(())
}
