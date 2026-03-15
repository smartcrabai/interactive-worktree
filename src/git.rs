use std::process::Command;

use anyhow::{Context, Result, bail};

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Captured output from a git command.
struct GitOutput {
    code: i32,
    stdout: String,
    stderr: String,
}

/// Run a git command and capture its exit code, stdout, and stderr.
/// Only fails if the process cannot be spawned.
fn run_git_raw(args: &[&str]) -> Result<GitOutput> {
    let output = Command::new("git")
        .args(args)
        .output()
        .with_context(|| format!("Failed to run: git {}", args.join(" ")))?;

    Ok(GitOutput {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    })
}

/// Run a git command, assert success, and return trimmed stdout.
fn run_git(args: &[&str]) -> Result<String> {
    let out = run_git_raw(args)?;
    if out.code != 0 {
        bail!("git {} failed: {}", args.join(" "), out.stderr);
    }
    Ok(out.stdout)
}

/// Check if we are inside a git repository.
#[must_use]
pub fn is_inside_repo() -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "true")
        .unwrap_or(false)
}

/// Get local branch names.
///
/// # Errors
///
/// Returns an error if the `git branch` command fails.
pub fn branch_list() -> Result<Vec<String>> {
    let stdout = run_git(&["branch", "--format=%(refname:short)"])?;
    Ok(stdout
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

/// Worktree info parsed from `git worktree list --porcelain`.
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub branch: String,
    pub path: String,
}

/// Parse `git worktree list --porcelain` output into a list of `WorktreeInfo`.
/// Entries without a branch (e.g. detached HEAD) are silently skipped.
fn parse_worktree_porcelain(output: &str) -> Vec<WorktreeInfo> {
    let mut worktrees = Vec::new();
    let mut current_path = String::new();
    let mut current_branch = String::new();

    for line in output.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            current_path = path.to_string();
        } else if let Some(branch) = line.strip_prefix("branch refs/heads/") {
            current_branch = branch.to_string();
        } else if line.is_empty() && !current_path.is_empty() {
            if current_branch.is_empty() {
                current_path.clear();
            } else {
                worktrees.push(WorktreeInfo {
                    branch: std::mem::take(&mut current_branch),
                    path: std::mem::take(&mut current_path),
                });
            }
        }
    }

    // Handle last entry if no trailing newline
    if !current_path.is_empty() && !current_branch.is_empty() {
        worktrees.push(WorktreeInfo {
            branch: current_branch,
            path: current_path,
        });
    }

    worktrees
}

/// Get worktree list from git (porcelain format).
///
/// # Errors
///
/// Returns an error if the `git worktree list` command fails.
pub fn worktree_list() -> Result<Vec<WorktreeInfo>> {
    let stdout = run_git(&["worktree", "list", "--porcelain"])?;
    Ok(parse_worktree_porcelain(&stdout))
}

/// Add a new worktree at `path` for `branch`.
/// If `start_point` is `None`, uses the current HEAD.
/// If `branch` already exists locally, do not pass `-b` (checkout existing branch).
/// Note: `start_point` is silently ignored when the branch already exists locally.
///
/// # Errors
///
/// Returns an error if the `git worktree add` command fails.
pub fn worktree_add(path: &str, branch: &str, start_point: Option<&str>) -> Result<()> {
    let branch_exists =
        run_git_raw(&["rev-parse", "--verify", &format!("refs/heads/{branch}")])?.code == 0;

    let mut args: Vec<&str> = vec!["worktree", "add"];
    if branch_exists {
        args.extend([path, branch]);
    } else {
        args.extend(["-b", branch, path]);
        if let Some(sp) = start_point {
            args.push(sp);
        }
    }
    run_git(&args)?;
    Ok(())
}

/// Remove the worktree at `path`.
///
/// # Errors
///
/// Returns an error if the `git worktree remove` command fails.
pub fn worktree_remove(path: &str, force: bool) -> Result<()> {
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(path);
    run_git(&args)?;
    Ok(())
}

/// Prune stale worktree administrative files.
///
/// # Errors
///
/// Returns an error if the `git worktree prune` command fails.
pub fn worktree_prune() -> Result<()> {
    run_git(&["worktree", "prune"])?;
    Ok(())
}

/// Delete a local branch.
/// If `force` is true, uses `-D` (force delete); otherwise uses `-d`.
///
/// # Errors
///
/// Returns an error if the branch does not exist or the delete command fails.
pub fn branch_delete(branch: &str, force: bool) -> Result<()> {
    let flag = if force { "-D" } else { "-d" };
    run_git(&["branch", flag, branch])?;
    Ok(())
}

/// Rename a local branch from `old` to `new`.
///
/// # Errors
///
/// Returns an error if the rename command fails.
pub fn branch_rename(old: &str, new: &str) -> Result<()> {
    run_git(&["branch", "-m", old, new])?;
    Ok(())
}

/// Get a git config value. Returns `Ok(None)` when the key does not exist.
///
/// # Errors
///
/// Returns an error if `git config` exits with a code other than 0 or 1.
pub fn config_get(key: &str) -> Result<Option<String>> {
    let out = run_git_raw(&["config", "--get", key])?;
    match out.code {
        0 => Ok(Some(out.stdout)),
        // Exit code 1 means the key was not found — that is not an error.
        1 => Ok(None),
        _ => bail!("git config --get failed: {}", out.stderr),
    }
}

/// Set a git config value.
///
/// # Errors
///
/// Returns an error if the `git config` command fails.
pub fn config_set(key: &str, value: &str) -> Result<()> {
    run_git(&["config", key, value])?;
    Ok(())
}

/// Remove a git config key.
///
/// # Errors
///
/// Returns an error if the key does not exist or the command fails.
pub fn config_unset(key: &str) -> Result<()> {
    run_git(&["config", "--unset", key])?;
    Ok(())
}

/// List all git config entries matching `pattern` (passed to `--get-regexp`).
/// Returns `(key, value)` pairs.
///
/// # Errors
///
/// Returns an error if `git config --get-regexp` fails.
pub fn config_list(pattern: &str) -> Result<Vec<(String, String)>> {
    let out = run_git_raw(&["config", "--get-regexp", pattern])?;
    match out.code {
        // Exit code 1 means no keys matched — return empty list.
        1 => return Ok(Vec::new()),
        0 => {}
        _ => bail!("git config --get-regexp failed: {}", out.stderr),
    }

    let pairs = out
        .stdout
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let mut parts = line.splitn(2, ' ');
            let key = parts.next()?.to_string();
            let value = parts.next()?.to_string();
            Some((key, value))
        })
        .collect();

    Ok(pairs)
}

/// Detect the default branch name.
/// Tries `gtr.defaultBranch` config, then `git symbolic-ref refs/remotes/origin/HEAD`,
/// falling back to `"main"`.
///
/// # Errors
///
/// Returns an error if the underlying git or config commands fail.
pub fn default_branch() -> Result<String> {
    if let Some(branch) = config_get("gtr.defaultBranch")? {
        return Ok(branch);
    }

    let out = run_git_raw(&["symbolic-ref", "refs/remotes/origin/HEAD"])?;
    if out.code == 0
        && let Some(branch) = out.stdout.strip_prefix("refs/remotes/origin/")
    {
        return Ok(branch.to_string());
    }

    Ok("main".to_string())
}

/// Return the absolute path to the repository root (`git rev-parse --show-toplevel`).
///
/// # Errors
///
/// Returns an error if not inside a git repository.
pub fn repo_root() -> Result<String> {
    run_git(&["rev-parse", "--show-toplevel"])
}

/// Return `true` if `branch` has been fully merged into `into`.
///
/// # Errors
///
/// Returns an error if the underlying git command fails.
pub fn is_merged(branch: &str, into: &str) -> Result<bool> {
    let out = run_git_raw(&["merge-base", "--is-ancestor", branch, into])?;
    match out.code {
        0 => Ok(true),
        1 => Ok(false),
        _ => bail!("git merge-base --is-ancestor failed: {}", out.stderr),
    }
}

/// Get worktree branch names (excluding the main worktree at index 0).
///
/// # Errors
///
/// Returns an error if `worktree_list` fails.
pub fn worktree_branches() -> Result<Vec<String>> {
    let wts = worktree_list()?;
    Ok(wts.into_iter().skip(1).map(|w| w.branch).collect())
}

/// Get all worktree branch names including main.
///
/// # Errors
///
/// Returns an error if `worktree_list` fails.
pub fn all_worktree_branches() -> Result<Vec<String>> {
    let wts = worktree_list()?;
    Ok(wts.into_iter().map(|w| w.branch).collect())
}

/// Move a worktree from `old_path` to `new_path`.
///
/// # Errors
///
/// Returns an error if the `git worktree move` command fails.
pub fn worktree_move(old_path: &str, new_path: &str) -> Result<()> {
    run_git(&["worktree", "move", old_path, new_path])?;
    Ok(())
}

/// Fetch a specific branch from a remote.
///
/// # Errors
///
/// Returns an error if the `git fetch` command fails.
pub fn fetch(remote: &str, branch: &str) -> Result<()> {
    run_git(&["fetch", remote, branch])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // parse_worktree_porcelain — pure parsing tests (no git required)
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_worktree_porcelain_empty_input() {
        // Given: empty porcelain output
        // When: parsed
        let result = parse_worktree_porcelain("");
        // Then: no entries
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_worktree_porcelain_single_entry_with_trailing_newline() {
        // Given: one worktree entry with a trailing blank line
        let output = "worktree /repo/main\nHEAD abc1234\nbranch refs/heads/main\n\n";
        // When: parsed
        let result = parse_worktree_porcelain(output);
        // Then: one entry with correct fields
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].branch, "main");
        assert_eq!(result[0].path, "/repo/main");
    }

    #[test]
    fn test_parse_worktree_porcelain_single_entry_without_trailing_newline() {
        // Given: one worktree entry with NO trailing blank line
        let output = "worktree /repo/main\nHEAD abc1234\nbranch refs/heads/main";
        // When: parsed
        let result = parse_worktree_porcelain(output);
        // Then: entry is still captured
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].branch, "main");
    }

    #[test]
    fn test_parse_worktree_porcelain_multiple_entries() {
        // Given: porcelain output with two worktrees
        let output = concat!(
            "worktree /repo/main\nHEAD aaaaaaa\nbranch refs/heads/main\n\n",
            "worktree /repo/feature\nHEAD bbbbbbb\nbranch refs/heads/feature/my-feature\n\n",
        );
        // When: parsed
        let result = parse_worktree_porcelain(output);
        // Then: two entries in order
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].branch, "main");
        assert_eq!(result[0].path, "/repo/main");
        assert_eq!(result[1].branch, "feature/my-feature");
        assert_eq!(result[1].path, "/repo/feature");
    }

    #[test]
    fn test_parse_worktree_porcelain_detached_head_is_excluded() {
        // Given: a worktree in detached HEAD state (no `branch` line)
        let output = "worktree /repo/detached\nHEAD deadbeef\ndetached\n\n";
        // When: parsed
        let result = parse_worktree_porcelain(output);
        // Then: detached entry is not included
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_worktree_porcelain_mixed_entries() {
        // Given: one normal worktree and one detached HEAD
        let output = concat!(
            "worktree /repo/main\nHEAD aaaaaaa\nbranch refs/heads/main\n\n",
            "worktree /repo/detached\nHEAD deadbeef\ndetached\n\n",
        );
        // When: parsed
        let result = parse_worktree_porcelain(output);
        // Then: only the normal worktree appears
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].branch, "main");
    }
}
