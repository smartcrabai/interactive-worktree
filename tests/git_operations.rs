//! Integration tests for `git::*` operations that require a real git repository.
//!
//! Each test creates a temporary git repository, performs operations within it,
//! and asserts the expected state. Tests are serialised via a process-wide mutex
//! to avoid `std::env::set_current_dir` races.

#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard};

// ─────────────────────────────────────────────────────────────────────────────
// Test infrastructure
// ─────────────────────────────────────────────────────────────────────────────

/// Global mutex to serialise tests that mutate the process working directory.
static SERIAL: Mutex<()> = Mutex::new(());

/// Temporary git repository that restores the original cwd on drop.
struct TempRepo {
    path: PathBuf,
    original_cwd: PathBuf,
    /// Keeps the global mutex locked for the lifetime of this struct.
    _lock: MutexGuard<'static, ()>,
}

impl TempRepo {
    /// Create a new temp directory, initialise a git repo with one commit,
    /// and change the process cwd into it.
    fn new() -> Self {
        // Acquire the lock before touching the cwd so that no two
        // TempRepos change the cwd at the same time.
        let lock: MutexGuard<'static, ()> = SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let original_cwd = std::env::current_dir().expect("failed to get cwd");

        // Create a unique temp directory under the system temp root.
        let tmp_root = std::env::temp_dir().join(format!(
            "iwt-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
        ));
        fs::create_dir_all(&tmp_root).expect("failed to create temp dir");

        // Initialise repo.
        run_git(&tmp_root, &["init", "-b", "main"]);
        run_git(&tmp_root, &["config", "user.email", "test@example.com"]);
        run_git(&tmp_root, &["config", "user.name", "Test"]);

        // Make an initial commit so that HEAD points to main.
        let readme = tmp_root.join("README.md");
        fs::write(&readme, "# test repo\n").expect("failed to write README");
        run_git(&tmp_root, &["add", "README.md"]);
        run_git(&tmp_root, &["commit", "-m", "initial commit"]);

        std::env::set_current_dir(&tmp_root).expect("failed to chdir into temp repo");

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
        // Restore cwd before removing the directory.
        let _ = std::env::set_current_dir(&self.original_cwd);
        // Best-effort cleanup.
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Run a git sub-command in `dir`, asserting success.
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

// ─────────────────────────────────────────────────────────────────────────────
// git::config_get / config_set / config_unset
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_config_get_missing_key_returns_none() {
    // Given: a fresh repo with no gtr config
    let _repo = TempRepo::new();
    // When: reading a key that was never set
    let result = interactive_worktree::git::config_get("gtr.nonexistent.key");
    // Then: Ok(None) — not an error, simply absent
    assert!(matches!(result, Ok(None)));
}

#[test]
fn test_config_set_get_roundtrip() {
    // Given: a fresh repo
    let _repo = TempRepo::new();
    // When: setting a config key
    interactive_worktree::git::config_set("gtr.worktrees.dir", "/tmp/worktrees")
        .expect("config_set failed");
    // Then: reading back the same key returns the value
    let val =
        interactive_worktree::git::config_get("gtr.worktrees.dir").expect("config_get failed");
    assert_eq!(val, Some("/tmp/worktrees".to_string()));
}

#[test]
fn test_config_unset_removes_key() {
    // Given: a key is set
    let _repo = TempRepo::new();
    interactive_worktree::git::config_set("gtr.test.key", "hello").expect("config_set failed");
    // When: the key is unset
    interactive_worktree::git::config_unset("gtr.test.key").expect("config_unset failed");
    // Then: the key is gone
    let val = interactive_worktree::git::config_get("gtr.test.key").expect("config_get failed");
    assert_eq!(val, None);
}

#[test]
fn test_config_list_returns_matching_keys() {
    // Given: multiple gtr.* keys are set
    let _repo = TempRepo::new();
    interactive_worktree::git::config_set("gtr.copy.include", "*.env").expect("set failed");
    interactive_worktree::git::config_set("gtr.copy.exclude", "secrets").expect("set failed");
    // When: listing with pattern "gtr.copy.*"
    let pairs = interactive_worktree::git::config_list("gtr.copy").expect("config_list failed");
    // Then: both keys appear in the result
    let keys: Vec<&str> = pairs.iter().map(|(k, _)| k.as_str()).collect();
    assert!(
        keys.contains(&"gtr.copy.include"),
        "expected gtr.copy.include in {keys:?}"
    );
    assert!(
        keys.contains(&"gtr.copy.exclude"),
        "expected gtr.copy.exclude in {keys:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// git::repo_root
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_repo_root_returns_existing_directory() {
    // Given: we are inside a git repo
    let repo = TempRepo::new();
    // When: querying the repo root
    let root = interactive_worktree::git::repo_root().expect("repo_root failed");
    // Then: the returned path exists and is a directory
    let root_path = std::path::Path::new(&root);
    assert!(root_path.exists(), "repo root '{root}' does not exist");
    assert!(root_path.is_dir(), "repo root '{root}' is not a directory");
    // And the temp dir should be a prefix of the returned root (canonicalised).
    let canon_repo = repo.path().canonicalize().expect("canonicalize failed");
    let canon_root = root_path.canonicalize().expect("canonicalize failed");
    assert_eq!(
        canon_repo, canon_root,
        "expected repo root {canon_repo:?}, got {canon_root:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// git::default_branch
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_default_branch_falls_back_to_main() {
    // Given: a fresh repo with no gtr.defaultBranch config and no remote
    let _repo = TempRepo::new();
    // When: resolving the default branch
    let branch = interactive_worktree::git::default_branch().expect("default_branch failed");
    // Then: falls back to "main"
    assert_eq!(branch, "main");
}

#[test]
fn test_default_branch_reads_gtr_config() {
    // Given: gtr.defaultBranch is explicitly configured
    let _repo = TempRepo::new();
    interactive_worktree::git::config_set("gtr.defaultBranch", "develop")
        .expect("config_set failed");
    // When: resolving the default branch
    let branch = interactive_worktree::git::default_branch().expect("default_branch failed");
    // Then: the configured value is returned
    assert_eq!(branch, "develop");
}

// ─────────────────────────────────────────────────────────────────────────────
// git::branch_delete
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_branch_delete_removes_branch() {
    // Given: a branch "to-delete" that is fully merged into main
    let repo = TempRepo::new();
    run_git(repo.path(), &["checkout", "-b", "to-delete"]);
    run_git(repo.path(), &["checkout", "main"]);
    // When: deleting it (safe delete, already merged)
    interactive_worktree::git::branch_delete("to-delete", false).expect("branch_delete failed");
    // Then: the branch no longer appears in the branch list
    let branches = interactive_worktree::git::branch_list().expect("branch_list failed");
    assert!(
        !branches.contains(&"to-delete".to_string()),
        "expected 'to-delete' to be absent from {branches:?}"
    );
}

#[test]
fn test_branch_delete_force_removes_unmerged_branch() {
    // Given: a branch with an extra commit not merged into main
    let repo = TempRepo::new();
    run_git(repo.path(), &["checkout", "-b", "unmerged"]);
    let extra = repo.path().join("extra.txt");
    fs::write(&extra, "extra").expect("write failed");
    run_git(repo.path(), &["add", "extra.txt"]);
    run_git(repo.path(), &["commit", "-m", "extra commit"]);
    run_git(repo.path(), &["checkout", "main"]);
    // When: force-deleting
    interactive_worktree::git::branch_delete("unmerged", true).expect("branch_delete force failed");
    // Then: branch is gone
    let branches = interactive_worktree::git::branch_list().expect("branch_list failed");
    assert!(!branches.contains(&"unmerged".to_string()));
}

// ─────────────────────────────────────────────────────────────────────────────
// git::branch_rename
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_branch_rename_renames_branch() {
    // Given: a branch "old-name"
    let repo = TempRepo::new();
    run_git(repo.path(), &["checkout", "-b", "old-name"]);
    run_git(repo.path(), &["checkout", "main"]);
    // When: renaming to "new-name"
    interactive_worktree::git::branch_rename("old-name", "new-name").expect("branch_rename failed");
    // Then: "old-name" is gone and "new-name" exists
    let branches = interactive_worktree::git::branch_list().expect("branch_list failed");
    assert!(
        !branches.contains(&"old-name".to_string()),
        "old name should be gone; branches={branches:?}"
    );
    assert!(
        branches.contains(&"new-name".to_string()),
        "new name should exist; branches={branches:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// git::is_merged
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_is_merged_returns_true_for_merged_branch() {
    // Given: a branch that was created from main and has no extra commits
    let repo = TempRepo::new();
    run_git(repo.path(), &["checkout", "-b", "merged-branch"]);
    run_git(repo.path(), &["checkout", "main"]);
    // When: checking if it is merged into main
    let result =
        interactive_worktree::git::is_merged("merged-branch", "main").expect("is_merged failed");
    // Then: true (no divergence)
    assert!(result, "expected merged-branch to be merged into main");
}

#[test]
fn test_is_merged_returns_false_for_unmerged_branch() {
    // Given: a branch with a commit not in main
    let repo = TempRepo::new();
    run_git(repo.path(), &["checkout", "-b", "feature-branch"]);
    let f = repo.path().join("feature.txt");
    fs::write(&f, "feature content").expect("write failed");
    run_git(repo.path(), &["add", "feature.txt"]);
    run_git(repo.path(), &["commit", "-m", "feature commit"]);
    run_git(repo.path(), &["checkout", "main"]);
    // When: checking merge status
    let result =
        interactive_worktree::git::is_merged("feature-branch", "main").expect("is_merged failed");
    // Then: false
    assert!(
        !result,
        "expected feature-branch to NOT be merged into main"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// git::worktree_add / worktree_remove / worktree_prune
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_worktree_add_creates_new_worktree() {
    // Given: a repo with an initial commit
    let repo = TempRepo::new();
    let wt_path = repo
        .path()
        .join("../iwt-test-wt-add")
        .to_string_lossy()
        .to_string();
    // When: adding a worktree for a new branch
    interactive_worktree::git::worktree_add(&wt_path, "feature/new", None)
        .expect("worktree_add failed");
    // Then: the worktree appears in the list
    let list = interactive_worktree::git::worktree_list().expect("worktree_list failed");
    assert!(
        list.iter().any(|w| w.branch == "feature/new"),
        "expected 'feature/new' worktree; list={list:?}"
    );
    // Cleanup
    let _ = fs::remove_dir_all(&wt_path);
}

#[test]
fn test_worktree_add_existing_branch_checks_out_without_dash_b() {
    // Given: an existing local branch
    let repo = TempRepo::new();
    run_git(repo.path(), &["checkout", "-b", "existing-branch"]);
    run_git(repo.path(), &["checkout", "main"]);
    let wt_path = repo
        .path()
        .join("../iwt-test-wt-existing")
        .to_string_lossy()
        .to_string();
    // When: adding a worktree for the EXISTING branch (no start point)
    interactive_worktree::git::worktree_add(&wt_path, "existing-branch", None)
        .expect("worktree_add for existing branch failed");
    // Then: the worktree exists
    let list = interactive_worktree::git::worktree_list().expect("worktree_list failed");
    assert!(list.iter().any(|w| w.branch == "existing-branch"));
    // Cleanup
    let _ = fs::remove_dir_all(&wt_path);
}

#[test]
fn test_worktree_remove_removes_worktree() {
    // Given: a worktree for "to-remove"
    let repo = TempRepo::new();
    let wt_path = repo
        .path()
        .join("../iwt-test-wt-remove")
        .to_string_lossy()
        .to_string();
    interactive_worktree::git::worktree_add(&wt_path, "to-remove", None)
        .expect("worktree_add failed");
    // Confirm it exists
    let list_before = interactive_worktree::git::worktree_list().expect("list failed");
    assert!(list_before.iter().any(|w| w.branch == "to-remove"));
    // When: removing the worktree
    interactive_worktree::git::worktree_remove(&wt_path, false).expect("worktree_remove failed");
    // Then: the worktree is gone from the list
    let list_after = interactive_worktree::git::worktree_list().expect("list failed");
    assert!(
        !list_after.iter().any(|w| w.branch == "to-remove"),
        "expected 'to-remove' to be gone; list={list_after:?}"
    );
    let _ = fs::remove_dir_all(&wt_path);
}

#[test]
fn test_worktree_prune_succeeds_on_clean_repo() {
    // Given: a repo with no stale worktree admin files
    let _repo = TempRepo::new();
    // When / Then: prune succeeds without error
    interactive_worktree::git::worktree_prune().expect("worktree_prune failed");
}
