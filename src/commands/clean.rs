use anyhow::Result;
use inquire::Confirm;

use crate::{git, worktree_ops};

pub fn run() -> Result<()> {
    let merged_only = Confirm::new("Only clean merged worktrees?")
        .with_default(true)
        .prompt()?;

    let default_branch = git::default_branch()?;
    let wts = git::worktree_list()?;
    let non_main: Vec<String> = wts.iter().skip(1).map(|w| w.branch.clone()).collect();

    let candidates: Vec<&String> = if merged_only {
        non_main
            .iter()
            .filter(|b| match git::is_merged(b, &default_branch) {
                Ok(merged) => merged,
                Err(e) => {
                    eprintln!("Warning: could not check if '{b}' is merged: {e}");
                    false
                }
            })
            .collect()
    } else {
        non_main.iter().collect()
    };

    if candidates.is_empty() {
        println!("No worktrees to clean.");
        return Ok(());
    }

    println!("\n--- Dry run ---");
    for branch in &candidates {
        println!("  Would remove: {branch}");
    }
    println!();

    let proceed = Confirm::new("Proceed with cleanup?")
        .with_default(false)
        .prompt()?;

    if !proceed {
        println!("Cancelled.");
        return Ok(());
    }

    for branch in &candidates {
        let Some(path) = wts
            .iter()
            .find(|w| &w.branch == *branch)
            .map(|w| w.path.clone())
        else {
            eprintln!("Worktree path not found for '{branch}', skipping.");
            continue;
        };

        if let Err(e) = worktree_ops::remove_with_hooks(&path, branch, true, false) {
            eprintln!("Failed to remove '{branch}': {e}");
        }
    }

    git::worktree_prune()?;
    Ok(())
}
