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

pub(crate) const WORKTREEINCLUDE_FILE: &str = ".worktreeinclude";

/// Read `.worktreeinclude` from the repo root and return include patterns.
///
/// Returns an empty Vec if the file does not exist.
///
/// # Errors
///
/// Returns an error if the file cannot be read (other than not-found).
fn read_worktreeinclude(root: &str) -> Result<Vec<String>> {
    let path = std::path::Path::new(root).join(WORKTREEINCLUDE_FILE);

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(anyhow::anyhow!(
                "Failed to read {WORKTREEINCLUDE_FILE}: {e}"
            ));
        }
    };

    Ok(content
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect())
}

/// Copy files from the main worktree into `target_path`.
///
/// If `pattern` is given it overrides both `.worktreeinclude` and the `gtr.copy.include` config.
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
        let mut patterns = read_worktreeinclude(&source)?;

        if let Some(config_inc) = git::config_get("gtr.copy.include")? {
            patterns.extend(
                config_inc
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty()),
            );
        }

        if patterns.is_empty() {
            return Ok(());
        }
        patterns
    };

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

/// Write the given `path` to the file specified by `IWT_CD_FILE`.
///
/// This enables the shell integration function to `cd` into the worktree
/// after the binary exits. If `IWT_CD_FILE` is not set, this is a no-op
/// so that the binary works correctly without shell integration.
///
/// # Errors
///
/// Returns an error if `IWT_CD_FILE` is set but the file cannot be written.
pub fn request_cd(path: &str) -> Result<()> {
    let Ok(cd_file) = std::env::var("IWT_CD_FILE") else {
        return Ok(());
    };
    std::fs::write(&cd_file, path)
        .with_context(|| format!("Failed to write cd file '{cd_file}'"))?;
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

#[cfg(test)]
#[expect(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::{Mutex, MutexGuard};

    static SERIAL: Mutex<()> = Mutex::new(());
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    struct TempRepo {
        path: PathBuf,
        original_cwd: PathBuf,
        _lock: MutexGuard<'static, ()>,
    }

    impl TempRepo {
        fn new() -> Self {
            let lock = SERIAL
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let original_cwd = std::env::current_dir().expect("failed to get cwd");

            let tmp_root = std::env::temp_dir().join(format!(
                "iwt-worktreeops-test-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .subsec_nanos()
            ));
            fs::create_dir_all(&tmp_root).expect("failed to create temp dir");

            run_git(&tmp_root, &["init", "-b", "main"]);
            run_git(&tmp_root, &["config", "user.email", "test@example.com"]);
            run_git(&tmp_root, &["config", "user.name", "Test"]);

            let readme = tmp_root.join("README.md");
            fs::write(&readme, "# test repo\n").expect("failed to write README");
            run_git(&tmp_root, &["add", "README.md"]);
            run_git(&tmp_root, &["commit", "-m", "initial commit"]);

            std::env::set_current_dir(&tmp_root).expect("failed to chdir");

            Self {
                path: tmp_root,
                original_cwd,
                _lock: lock,
            }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original_cwd);
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn run_git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap_or_else(|e| panic!("failed to spawn git: {e}"));
        assert!(
            status.success(),
            "git {} failed (exit {:?})",
            args.join(" "),
            status.code()
        );
    }

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        ENV_MUTEX
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// RAII guard: sets `IWT_CD_FILE` on construction, clears it (and an optional
    /// temp file) on drop so cleanup is panic-safe.
    struct CdFileGuard {
        file: Option<std::path::PathBuf>,
    }

    impl CdFileGuard {
        fn set(val: &str) -> Self {
            unsafe { std::env::set_var("IWT_CD_FILE", val) };
            Self { file: None }
        }

        fn set_file(path: &std::path::Path) -> Self {
            unsafe { std::env::set_var("IWT_CD_FILE", path.to_string_lossy().as_ref()) };
            Self {
                file: Some(path.to_owned()),
            }
        }
    }

    impl Drop for CdFileGuard {
        fn drop(&mut self) {
            unsafe { std::env::remove_var("IWT_CD_FILE") };
            if let Some(f) = &self.file {
                let _ = std::fs::remove_file(f);
            }
        }
    }

    #[test]
    fn test_copy_files_no_worktreeinclude_uses_git_config() {
        // Given: a repo without .worktreeinclude, but gtr.copy.include is set via git config
        let repo = TempRepo::new();
        fs::write(repo.path().join("notes.txt"), "notes").expect("write failed");
        let target = repo.path().join("target");
        fs::create_dir_all(&target).expect("mkdir target failed");
        run_git(repo.path(), &["config", "gtr.copy.include", "*.txt"]);

        // When: copying files without an explicit pattern
        super::copy_files(target.to_str().unwrap(), None).expect("copy_files failed");

        // Then: notes.txt is copied via the git config pattern
        assert!(
            target.join("notes.txt").exists(),
            "notes.txt should be copied when gtr.copy.include=*.txt"
        );
    }

    #[test]
    fn test_copy_files_worktreeinclude_copies_matching_files() {
        // Given: a repo with .worktreeinclude containing *.env, no git config set
        let repo = TempRepo::new();
        fs::write(repo.path().join(".worktreeinclude"), "*.env\n").expect("write failed");
        fs::write(repo.path().join("secret.env"), "SECRET=1").expect("write failed");
        fs::write(repo.path().join("other.txt"), "other").expect("write failed");
        let target = repo.path().join("target");
        fs::create_dir_all(&target).expect("mkdir target failed");

        // When: copying files without an explicit pattern and no git config
        super::copy_files(target.to_str().unwrap(), None).expect("copy_files failed");

        // Then: secret.env is copied, other.txt is not
        assert!(
            target.join("secret.env").exists(),
            "secret.env should be copied via .worktreeinclude"
        );
        assert!(
            !target.join("other.txt").exists(),
            "other.txt should NOT be copied (not in .worktreeinclude)"
        );
    }

    #[test]
    fn test_copy_files_merges_worktreeinclude_and_git_config() {
        // Given: .worktreeinclude has *.env, git config has *.txt — both should apply
        let repo = TempRepo::new();
        fs::write(repo.path().join(".worktreeinclude"), "*.env\n").expect("write failed");
        fs::write(repo.path().join("secret.env"), "SECRET=1").expect("write failed");
        fs::write(repo.path().join("config.txt"), "config").expect("write failed");
        fs::write(repo.path().join("readme.md"), "readme").expect("write failed");
        let target = repo.path().join("target");
        fs::create_dir_all(&target).expect("mkdir target failed");
        run_git(repo.path(), &["config", "gtr.copy.include", "*.txt"]);

        // When: copying without an explicit pattern
        super::copy_files(target.to_str().unwrap(), None).expect("copy_files failed");

        // Then: both *.env and *.txt files are copied; *.md is not
        assert!(
            target.join("secret.env").exists(),
            "secret.env should be copied via .worktreeinclude"
        );
        assert!(
            target.join("config.txt").exists(),
            "config.txt should be copied via gtr.copy.include"
        );
        assert!(
            !target.join("readme.md").exists(),
            "readme.md should NOT be copied"
        );
    }

    #[test]
    fn test_copy_files_worktreeinclude_ignores_comments_and_blank_lines() {
        // Given: .worktreeinclude with comment lines and blank lines mixed in
        let repo = TempRepo::new();
        let content = "# This is a comment\n\n*.env\n\n# Another comment\n";
        fs::write(repo.path().join(".worktreeinclude"), content).expect("write failed");
        fs::write(repo.path().join("secret.env"), "SECRET=1").expect("write failed");
        let target = repo.path().join("target");
        fs::create_dir_all(&target).expect("mkdir target failed");

        // When: copying files
        super::copy_files(target.to_str().unwrap(), None).expect("copy_files failed");

        // Then: only the real pattern (*.env) takes effect
        assert!(
            target.join("secret.env").exists(),
            "secret.env should be copied (only real pattern in .worktreeinclude)"
        );
    }

    #[test]
    fn test_copy_files_explicit_pattern_ignores_worktreeinclude() {
        // Given: .worktreeinclude has *.env, but caller passes an explicit pattern *.txt
        let repo = TempRepo::new();
        fs::write(repo.path().join(".worktreeinclude"), "*.env\n").expect("write failed");
        fs::write(repo.path().join("secret.env"), "SECRET=1").expect("write failed");
        fs::write(repo.path().join("config.txt"), "config").expect("write failed");
        let target = repo.path().join("target");
        fs::create_dir_all(&target).expect("mkdir target failed");

        // When: copying with an explicit pattern
        super::copy_files(target.to_str().unwrap(), Some("*.txt")).expect("copy_files failed");

        // Then: only *.txt is copied; .worktreeinclude patterns are ignored
        assert!(
            target.join("config.txt").exists(),
            "config.txt should be copied via explicit pattern"
        );
        assert!(
            !target.join("secret.env").exists(),
            "secret.env should NOT be copied when explicit pattern overrides .worktreeinclude"
        );
    }

    #[test]
    fn test_request_cd_noop_when_env_var_not_set() {
        let _lock = lock_env();
        unsafe { std::env::remove_var("IWT_CD_FILE") };
        assert!(request_cd("/some/worktree/path").is_ok());
    }

    #[test]
    fn test_request_cd_writes_path_to_file_when_env_var_set() -> Result<()> {
        let _lock = lock_env();
        let tmp_file = std::env::temp_dir().join("iwt_cd_write_test.txt");
        let _guard = CdFileGuard::set_file(&tmp_file);
        request_cd("/repo/worktrees/my-feature")?;
        let written = std::fs::read_to_string(&tmp_file)?;
        assert_eq!(written, "/repo/worktrees/my-feature");
        Ok(())
    }

    #[test]
    fn test_request_cd_returns_error_when_write_fails() {
        let _lock = lock_env();
        let _guard = CdFileGuard::set("/nonexistent/directory/cd_file.txt");
        let Err(e) = request_cd("/some/path") else {
            panic!("expected request_cd to return an error");
        };
        assert!(
            e.to_string().contains("Failed to write cd file"),
            "expected write-failure message, got: {e}"
        );
    }

    #[test]
    fn test_request_cd_overwrites_existing_file_content() -> Result<()> {
        let _lock = lock_env();
        let tmp_file = std::env::temp_dir().join("iwt_cd_overwrite_test.txt");
        let _ = std::fs::write(&tmp_file, "stale_path");
        let _guard = CdFileGuard::set_file(&tmp_file);
        request_cd("/new/worktree/path")?;
        let written = std::fs::read_to_string(&tmp_file)?;
        assert_eq!(written, "/new/worktree/path");
        Ok(())
    }
}
