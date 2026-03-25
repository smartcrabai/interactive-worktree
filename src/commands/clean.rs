use anyhow::Result;

use crate::{git, worktree_ops};

pub fn run() -> Result<()> {
    let default_branch = git::default_branch()?;
    let wts = git::worktree_list()?;
    let candidates: Vec<(String, String)> = wts
        .iter()
        .skip(1)
        .filter_map(|w| match git::is_merged(&w.branch, &default_branch) {
            Ok(true) => Some((w.branch.clone(), w.path.clone())),
            Ok(false) => None,
            Err(e) => {
                eprintln!("Warning: could not check if '{}' is merged: {e}", w.branch);
                None
            }
        })
        .collect();

    if candidates.is_empty() {
        println!("No worktrees to clean.");
        return Ok(());
    }

    println!("\nRemoving merged worktrees:");
    for (branch, path) in &candidates {
        println!("  {branch}");

        if let Err(e) = worktree_ops::remove_with_hooks(
            path, branch, /* delete_branch= */ true, /* force= */ false,
        ) {
            eprintln!("Failed to remove '{branch}': {e}");
        }
    }

    git::worktree_prune()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::run;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::{Mutex, MutexGuard};

    static SERIAL: Mutex<()> = Mutex::new(());

    struct TempRepo {
        path: PathBuf,
        original_cwd: PathBuf,
        _lock: MutexGuard<'static, ()>,
    }

    impl TempRepo {
        fn new() -> Self {
            let lock: MutexGuard<'static, ()> = SERIAL
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let original_cwd = std::env::current_dir().expect("failed to get cwd");

            let tmp_root = std::env::temp_dir().join(format!(
                "iwt-clean-test-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ));
            fs::create_dir_all(&tmp_root).expect("failed to create temp dir");

            git_in(&tmp_root, &["init", "-b", "main"]);
            git_in(&tmp_root, &["config", "user.email", "test@example.com"]);
            git_in(&tmp_root, &["config", "user.name", "Test"]);

            let readme = tmp_root.join("README.md");
            fs::write(&readme, "# test\n").expect("write README failed");
            git_in(&tmp_root, &["add", "README.md"]);
            git_in(&tmp_root, &["commit", "-m", "initial commit"]);

            std::env::set_current_dir(&tmp_root).expect("chdir failed");

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

    fn git_in(dir: &Path, args: &[&str]) {
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

    fn add_worktree(repo: &TempRepo, branch: &str) -> PathBuf {
        let wt_path = repo
            .path()
            .parent()
            .expect("repo has no parent dir")
            .join(format!("wt-{branch}"));
        // Clean up any leftover from a previous failed run.
        let _ = fs::remove_dir_all(&wt_path);
        crate::git::worktree_add(&wt_path.to_string_lossy(), branch, None)
            .expect("worktree_add failed");
        wt_path
    }

    /// Add a worktree with a commit that diverges from main so `is_merged` returns false.
    fn make_unmerged_worktree(repo: &TempRepo, branch: &str) -> PathBuf {
        let wt_path = add_worktree(repo, branch);
        let extra = wt_path.join("extra.txt");
        fs::write(&extra, "extra content").expect("write extra.txt failed");
        git_in(&wt_path, &["add", "extra.txt"]);
        git_in(&wt_path, &["commit", "-m", "diverging commit"]);
        wt_path
    }

    // ─────────────────────────────────────────────────────────────────────────
    // clean::run — behaviour tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_clean_no_worktrees_to_clean_returns_ok() {
        let _repo = TempRepo::new();
        assert!(run().is_ok());
    }

    #[test]
    fn test_clean_removes_merged_worktree() {
        let repo = TempRepo::new();
        // A branch with no extra commits is considered merged by is_merged.
        let _wt_path = add_worktree(&repo, "merged-feature");
        let list_before = crate::git::worktree_list().expect("list_before failed");
        assert!(
            list_before.iter().any(|w| w.branch == "merged-feature"),
            "setup: 'merged-feature' worktree not found in {list_before:?}"
        );

        assert!(run().is_ok());

        let list_after = crate::git::worktree_list().expect("list_after failed");
        assert!(
            !list_after.iter().any(|w| w.branch == "merged-feature"),
            "expected 'merged-feature' to be removed; list={list_after:?}"
        );
    }

    #[test]
    fn test_clean_does_not_remove_unmerged_worktree() {
        let repo = TempRepo::new();
        let wt_path = make_unmerged_worktree(&repo, "unmerged-feature");

        assert!(run().is_ok());

        let list_after = crate::git::worktree_list().expect("list_after failed");
        assert!(
            list_after.iter().any(|w| w.branch == "unmerged-feature"),
            "expected 'unmerged-feature' to remain; list={list_after:?}"
        );

        crate::git::worktree_remove(&wt_path.to_string_lossy(), false).ok();
        let _ = fs::remove_dir_all(&wt_path);
    }

    #[test]
    fn test_clean_removes_merged_and_keeps_unmerged() {
        let repo = TempRepo::new();
        let _merged_path = add_worktree(&repo, "merged-wt");
        let unmerged_path = make_unmerged_worktree(&repo, "unmerged-wt");

        assert!(run().is_ok());

        let list_after = crate::git::worktree_list().expect("list_after failed");
        assert!(
            !list_after.iter().any(|w| w.branch == "merged-wt"),
            "expected 'merged-wt' to be removed; list={list_after:?}"
        );
        assert!(
            list_after.iter().any(|w| w.branch == "unmerged-wt"),
            "expected 'unmerged-wt' to remain; list={list_after:?}"
        );

        crate::git::worktree_remove(&unmerged_path.to_string_lossy(), false).ok();
        let _ = fs::remove_dir_all(&unmerged_path);
    }
}
