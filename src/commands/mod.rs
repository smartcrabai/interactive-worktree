pub mod ai;
pub mod clean;
pub mod config;
pub mod copy;
pub mod doctor;
pub mod editor;
pub mod go;
pub mod list;
pub mod mv;
pub mod new;
pub mod pr;
pub mod rm;
pub mod run;

use anyhow::{Result, bail};
use inquire::{Confirm, Select, Text};

use crate::{git, worktree_ops};

/// Action to take after creating a worktree.
pub(crate) enum PostAction {
    None,
    Editor,
    Ai(Option<String>),
}

/// Options for creating a new worktree.
pub(crate) struct NewWorktreeOpts {
    pub branch: String,
    /// Start point for the new branch. `None` means current HEAD.
    pub start_point: Option<String>,
}

/// Prompt the user for a post-creation action and whether to skip file copying.
///
/// Returns `(PostAction, skip_copy)`.
pub(crate) fn prompt_post_args() -> Result<(PostAction, bool)> {
    let post = Select::new(
        "After creation:",
        vec!["None", "Open in editor", "Start AI tool"],
    )
    .with_help_message("Action to take after creating the worktree")
    .prompt()?;

    let action = match post {
        "Open in editor" => PostAction::Editor,
        "Start AI tool" => {
            let tool = Text::new("AI tool:")
                .with_placeholder("claude, aider, copilot, codex, ...")
                .with_help_message("Enter tool name, or press Enter for default")
                .prompt()?;
            let tool_opt = if tool.is_empty() { None } else { Some(tool) };
            PostAction::Ai(tool_opt)
        }
        _ => PostAction::None,
    };

    let skip_copy = Confirm::new("Skip file copying?")
        .with_default(false)
        .prompt()?;

    Ok((action, skip_copy))
}

/// Prompt for post-creation preferences, then create the worktree and run post-actions.
pub(crate) fn run_with_post_prompt(opts: &NewWorktreeOpts) -> Result<()> {
    let (post_action, skip_copy) = prompt_post_args()?;

    let path = worktree_ops::worktree_dir_path(&opts.branch)?;

    git::worktree_add(&path, &opts.branch, opts.start_point.as_deref())?;

    if !skip_copy {
        worktree_ops::copy_files(&path, None)?;
    }

    if let Err(e) = worktree_ops::run_hook("gtr.hook.postCreate", &path) {
        eprintln!("Warning: hook 'gtr.hook.postCreate' failed: {e}");
    }

    match post_action {
        PostAction::Editor => worktree_ops::open_editor(&path, None)?,
        PostAction::Ai(tool) => worktree_ops::start_ai(&path, tool.as_deref())?,
        PostAction::None => {}
    }

    Ok(())
}

/// Prompt the user to select a worktree and a tool, then run `action(path, tool)`.
///
/// `tools` must contain `"default"` as the first item; selecting it passes `None`
/// to `action`.
pub(crate) fn run_with_tool_selection(
    tool_prompt: &str,
    tools: &[&str],
    help_msg: &str,
    action: fn(&str, Option<&str>) -> Result<()>,
) -> Result<()> {
    let wts = git::worktree_list()?;
    let branches: Vec<String> = wts.iter().map(|w| w.branch.clone()).collect();
    if branches.is_empty() {
        bail!("No worktrees found");
    }

    let branch = Select::new("Select worktree:", branches).prompt()?;
    let path = wts
        .iter()
        .find(|w| w.branch == branch)
        .map(|w| w.path.clone())
        .ok_or_else(|| anyhow::anyhow!("No worktree for branch '{branch}'"))?;

    let tool = Select::new(tool_prompt, tools.to_vec())
        .with_help_message(help_msg)
        .prompt()?;
    let tool_arg = if tool == "default" { None } else { Some(tool) };

    action(&path, tool_arg)
}
