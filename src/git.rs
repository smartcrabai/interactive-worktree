#![allow(dead_code)]

use std::process::Command;

use anyhow::{Context, Result, bail};

/// Check if we are inside a git repository.
pub fn is_inside_repo() -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "true")
        .unwrap_or(false)
}

/// Get local branch names.
pub fn branch_list() -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(["branch", "--format=%(refname:short)"])
        .output()
        .context("Failed to list branches")?;

    if !output.status.success() {
        bail!(
            "Failed to list branches: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

/// Get remote branch names (stripped of remote prefix).
pub fn remote_branch_list() -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(["branch", "-r", "--format=%(refname:short)"])
        .output()
        .context("Failed to list remote branches")?;

    if !output.status.success() {
        bail!(
            "Failed to list remote branches: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && !s.ends_with("/HEAD"))
        .collect())
}

/// Get the current branch name.
pub fn current_branch() -> Result<String> {
    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .context("Failed to get current branch")?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Worktree info parsed from `git gtr list --porcelain`.
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub branch: String,
    pub path: String,
}

/// Get worktree list from gtr (porcelain format).
pub fn worktree_list() -> Result<Vec<WorktreeInfo>> {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .output()
        .context("Failed to list worktrees")?;

    if !output.status.success() {
        bail!(
            "Failed to list worktrees: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut worktrees = Vec::new();
    let mut current_path = String::new();
    let mut current_branch = String::new();

    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            current_path = path.to_string();
        } else if let Some(branch) = line.strip_prefix("branch refs/heads/") {
            current_branch = branch.to_string();
        } else if line.is_empty() && !current_path.is_empty() {
            if !current_branch.is_empty() {
                worktrees.push(WorktreeInfo {
                    branch: current_branch.clone(),
                    path: current_path.clone(),
                });
            }
            current_path.clear();
            current_branch.clear();
        }
    }

    // Handle last entry if no trailing newline
    if !current_path.is_empty() && !current_branch.is_empty() {
        worktrees.push(WorktreeInfo {
            branch: current_branch,
            path: current_path,
        });
    }

    Ok(worktrees)
}

/// Get worktree branch names (excluding the main worktree at index 0).
pub fn worktree_branches() -> Result<Vec<String>> {
    let wts = worktree_list()?;
    Ok(wts.into_iter().skip(1).map(|w| w.branch).collect())
}

/// Get all worktree branch names including main.
pub fn all_worktree_branches() -> Result<Vec<String>> {
    let wts = worktree_list()?;
    Ok(wts.into_iter().map(|w| w.branch).collect())
}

/// Fetch a specific branch from a remote.
pub fn fetch(remote: &str, branch: &str) -> Result<()> {
    let output = Command::new("git")
        .args(["fetch", remote, branch])
        .output()
        .context("Failed to execute git fetch")?;

    if !output.status.success() {
        bail!(
            "git fetch {} {} failed: {}",
            remote,
            branch,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(())
}
