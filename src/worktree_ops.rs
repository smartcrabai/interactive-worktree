use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::git;

/// Resolve a branch name to its worktree path.
///
/// # Errors
///
/// Returns an error if no worktree is found for the given branch.
pub fn resolve_path(branch: &str) -> Result<String> {
    let wts = git::worktree_list()?;
    wts.into_iter()
        .find(|w| w.branch == branch)
        .map(|w| w.path)
        .ok_or_else(|| anyhow::anyhow!("No worktree found for branch '{branch}'"))
}

/// Calculate the path where a new worktree for `branch` should be created.
///
/// Uses `gtr.worktrees.dir` (defaults to the parent of the repo root) and
/// `gtr.worktrees.prefix` (defaults to empty).
///
/// # Errors
///
/// Returns an error if the repo root cannot be determined.
pub fn worktree_dir_path(branch: &str) -> Result<String> {
    let dir = if let Some(d) = git::config_get("gtr.worktrees.dir")? {
        d
    } else {
        let root = git::repo_root()?;
        std::path::Path::new(&root)
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Repo root has no parent directory"))?
            .to_string_lossy()
            .to_string()
    };

    let prefix = git::config_get("gtr.worktrees.prefix")?.unwrap_or_default();
    let dir_name = branch.replace('/', "-");

    Ok(format!("{dir}/{prefix}{dir_name}"))
}

/// Run a shell command inside the worktree directory at `path`.
///
/// # Errors
///
/// Returns an error if the command cannot be spawned or exits non-zero.
pub fn run_command(branch: &str, command: &[&str]) -> Result<()> {
    let path = resolve_path(branch)?;
    let (cmd, args) = command
        .split_first()
        .ok_or_else(|| anyhow::anyhow!("Empty command"))?;

    let status = Command::new(cmd)
        .args(args)
        .current_dir(&path)
        .status()
        .with_context(|| format!("Failed to run '{cmd}'"))?;

    if !status.success() {
        bail!("{cmd} exited with {}", status.code().unwrap_or(-1));
    }
    Ok(())
}

/// Open an editor for the worktree at `worktree_path`.
///
/// If `editor` is `None`, falls back to the `gtr.editor.default` config key,
/// then to `"code"` (VS Code).
///
/// # Errors
///
/// Returns an error if the editor process cannot be spawned or exits non-zero.
pub fn open_editor(worktree_path: &str, editor: Option<&str>) -> Result<()> {
    let editor_name = match editor {
        Some(e) => e.to_string(),
        None => git::config_get("gtr.editor.default")?.unwrap_or_else(|| "code".to_string()),
    };

    let mut builder = match editor_name.as_str() {
        "vscode" => Command::new("code"),
        "sublime" => Command::new("subl"),
        name => Command::new(name),
    };

    let status = builder
        .arg(worktree_path)
        .status()
        .with_context(|| format!("Failed to launch editor '{editor_name}'"))?;

    if !status.success() {
        bail!(
            "Editor '{editor_name}' exited with {}",
            status.code().unwrap_or(-1)
        );
    }
    Ok(())
}

/// Start an AI tool in the worktree at `worktree_path`.
///
/// If `tool` is `None`, falls back to the `gtr.ai.default` config key,
/// then to `"claude"`.
///
/// # Errors
///
/// Returns an error if the tool process cannot be spawned or exits non-zero.
pub fn start_ai(worktree_path: &str, tool: Option<&str>) -> Result<()> {
    let tool_name = match tool {
        Some(t) => t.to_string(),
        None => git::config_get("gtr.ai.default")?.unwrap_or_else(|| "claude".to_string()),
    };

    let status = Command::new(&tool_name)
        .current_dir(worktree_path)
        .status()
        .with_context(|| format!("Failed to start AI tool '{tool_name}'"))?;

    if !status.success() {
        bail!(
            "AI tool '{tool_name}' exited with {}",
            status.code().unwrap_or(-1)
        );
    }
    Ok(())
}

/// Copy files from the main worktree into `target_path`.
///
/// If `pattern` is given it overrides the `gtr.copy.include` config.
/// Files matching `gtr.copy.exclude` patterns are skipped.
/// Returns `Ok(())` immediately when no include patterns are configured.
///
/// # Errors
///
/// Returns an error if the source directory cannot be read or a file copy fails.
pub fn copy_files(target_path: &str, pattern: Option<&str>) -> Result<()> {
    let source = git::repo_root()?;

    let includes: Vec<String> = if let Some(p) = pattern {
        vec![p.to_string()]
    } else {
        let inc = git::config_get("gtr.copy.include")?.unwrap_or_default();
        if inc.is_empty() {
            return Ok(());
        }
        inc.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };

    if includes.is_empty() {
        return Ok(());
    }

    let excludes: Vec<String> = git::config_get("gtr.copy.exclude")?
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    copy_matching(
        std::path::Path::new(&source),
        std::path::Path::new(target_path),
        &includes,
        &excludes,
    )
}

fn pattern_matches(filename: &str, pattern: &str) -> bool {
    if let Some(ext) = pattern.strip_prefix("*.") {
        filename.ends_with(&format!(".{ext}"))
    } else {
        filename == pattern
    }
}

/// Read `source_dir` once and copy every file that matches any of `includes`
/// (and is not excluded). Processes all patterns in a single directory pass.
fn copy_matching(
    source_dir: &std::path::Path,
    target_dir: &std::path::Path,
    includes: &[String],
    excludes: &[String],
) -> Result<()> {
    let entries = std::fs::read_dir(source_dir)
        .with_context(|| format!("Failed to read directory {}", source_dir.display()))?;

    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if excludes.iter().any(|e| pattern_matches(&name_str, e)) {
            continue;
        }

        if includes.iter().any(|p| pattern_matches(&name_str, p)) {
            if !entry.file_type()?.is_file() {
                continue;
            }
            let target_file = target_dir.join(&name);
            std::fs::copy(entry.path(), &target_file)
                .with_context(|| format!("Failed to copy '{name_str}'"))?;
        }
    }

    Ok(())
}

/// Run a git config hook command in `worktree_path`.
///
/// If the config key `hook_key` is not set, this is a no-op.
///
/// # Errors
///
/// Returns an error if the hook process cannot be spawned or exits non-zero.
pub fn run_hook(hook_key: &str, worktree_path: &str) -> Result<()> {
    let Some(hook_cmd) = git::config_get(hook_key)? else {
        return Ok(());
    };

    if hook_cmd.is_empty() {
        return Ok(());
    }

    let status = Command::new("/bin/sh")
        .args(["-c", &hook_cmd])
        .current_dir(worktree_path)
        .status()
        .with_context(|| format!("Failed to run hook '{hook_key}'"))?;

    if !status.success() {
        bail!(
            "Hook '{hook_key}' exited with {}",
            status.code().unwrap_or(-1)
        );
    }

    Ok(())
}

/// Remove a worktree and optionally its branch, running pre/post hooks.
///
/// If `worktree_remove` fails the error is propagated. `branch_delete` failures
/// are printed but do not abort. Hook failures are silently ignored.
///
/// # Errors
///
/// Returns an error if the `git worktree remove` command fails.
pub fn remove_with_hooks(path: &str, branch: &str, delete_branch: bool, force: bool) -> Result<()> {
    run_hook("gtr.hook.preRemove", path).ok();

    let repo_root = git::repo_root()?;
    git::worktree_remove(path, force)?;

    if delete_branch && let Err(e) = git::branch_delete(branch, force) {
        eprintln!("Failed to delete branch '{branch}': {e}");
    }

    run_hook("gtr.hook.postRemove", &repo_root).ok();
    Ok(())
}
