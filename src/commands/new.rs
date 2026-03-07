use anyhow::Result;
use inquire::{Confirm, Select, Text};

use crate::gtr;

pub fn run() -> Result<()> {
    let branch = Text::new("Branch name:")
        .with_help_message("Name for the new worktree branch")
        .prompt()?;

    let from_options = vec!["Default (main/master)", "Current branch", "Specific ref"];
    let from = Select::new("Starting point:", from_options).prompt()?;

    let mut args = vec!["new", &branch];
    let ref_value: String;

    match from {
        "Current branch" => args.push("--from-current"),
        "Specific ref" => {
            ref_value = Text::new("Ref (branch/tag/commit):").prompt()?;
            args.push("--from");
            args.push(&ref_value);
        }
        _ => {}
    }

    let post_options = vec!["None", "Open in editor", "Start AI tool"];
    let post = Select::new("After creation:", post_options)
        .with_help_message("Action to take after creating the worktree")
        .prompt()?;

    let ai_tool = match post {
        "Open in editor" => {
            args.push("--editor");
            None
        }
        "Start AI tool" => {
            let tool = Text::new("AI tool:")
                .with_placeholder("claude, aider, copilot, codex, ...")
                .with_help_message("Enter tool name, or press Enter for default")
                .prompt()?;
            if tool.is_empty() {
                args.push("--ai");
                None
            } else {
                Some(tool)
            }
        }
        _ => None,
    };

    let no_copy = Confirm::new("Skip file copying?")
        .with_default(false)
        .prompt()?;

    if no_copy {
        args.push("--no-copy");
    }

    gtr::exec(&args)?;

    // ツール名を指定した場合、別途 git gtr ai で指定ツールを起動
    if let Some(tool) = &ai_tool {
        gtr::exec(&["ai", &branch, "--ai", tool])?;
    }

    Ok(())
}
