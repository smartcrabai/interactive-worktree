use anyhow::{Result, bail};
use inquire::{Select, Text};

use crate::{git, worktree_ops};

pub fn run() -> Result<()> {
    let branches = git::all_worktree_branches()?;
    if branches.is_empty() {
        bail!("No worktrees found");
    }

    let branch = Select::new("Select worktree:", branches).prompt()?;

    let command = Text::new("Command to run:")
        .with_help_message("e.g. npm test, cargo build, git status")
        .prompt()?;

    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        bail!("No command provided");
    }

    worktree_ops::run_command(&branch, &parts)
}
