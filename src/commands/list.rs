use anyhow::Result;

use crate::git;

pub fn run() -> Result<()> {
    let wts = git::worktree_list()?;
    for wt in &wts {
        println!("  {} — {}", wt.branch, wt.path);
    }
    Ok(())
}
