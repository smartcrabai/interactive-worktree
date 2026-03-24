use anyhow::{Result, bail};
use inquire::Select;

use crate::{git, worktree_ops};

pub fn run() -> Result<()> {
    let wts = git::worktree_list()?;
    if wts.is_empty() {
        bail!("No worktrees found");
    }

    let branches: Vec<String> = wts.iter().map(|w| w.branch.clone()).collect();
    let branch = Select::new("Select worktree:", branches).prompt()?;

    let path = wts
        .iter()
        .find(|w| w.branch == branch)
        .map(|w| &w.path)
        .ok_or_else(|| anyhow::anyhow!("Worktree not found for branch '{branch}'"))?;

    println!("{path}");
    worktree_ops::request_cd(path)?;
    Ok(())
}
