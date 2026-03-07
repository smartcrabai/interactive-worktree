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

use anyhow::Result;
use inquire::{Confirm, Select, Text};

use crate::gtr;

/// Prompt for post-creation action and return the extra args to pass to `gtr new`,
/// plus an optional AI tool name to invoke separately via `gtr ai`.
pub(crate) fn prompt_post_args() -> Result<(Vec<String>, Option<String>)> {
    let mut extra: Vec<String> = Vec::new();
    let mut ai_tool: Option<String> = None;

    let post = Select::new(
        "After creation:",
        vec!["None", "Open in editor", "Start AI tool"],
    )
    .with_help_message("Action to take after creating the worktree")
    .prompt()?;

    match post {
        "Open in editor" => extra.push("--editor".to_string()),
        "Start AI tool" => {
            let tool = Text::new("AI tool:")
                .with_placeholder("claude, aider, copilot, codex, ...")
                .with_help_message("Enter tool name, or press Enter for default")
                .prompt()?;
            if tool.is_empty() {
                extra.push("--ai".to_string());
            } else {
                ai_tool = Some(tool);
            }
        }
        _ => {}
    }

    if Confirm::new("Skip file copying?")
        .with_default(false)
        .prompt()?
    {
        extra.push("--no-copy".to_string());
    }

    Ok((extra, ai_tool))
}

/// Prompt for post-creation args, then execute the worktree creation and optional AI launch.
pub(crate) fn run_with_post_prompt(mut args: Vec<String>, branch: &str) -> Result<()> {
    let (extra, ai_tool) = prompt_post_args()?;
    args.extend(extra);

    let args_str: Vec<&str> = args.iter().map(String::as_str).collect();
    gtr::exec(&args_str)?;

    if let Some(tool) = &ai_tool {
        gtr::exec(&["ai", branch, "--ai", tool])?;
    }

    Ok(())
}
