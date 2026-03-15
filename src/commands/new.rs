use anyhow::Result;
use inquire::{Select, Text};

use crate::commands::{NewWorktreeOpts, run_with_post_prompt};
use crate::git;

pub fn run() -> Result<()> {
    let branch = Text::new("Branch name:")
        .with_help_message("Name for the new worktree branch")
        .prompt()?;

    let from_options = vec!["Default (main/master)", "Current branch", "Specific ref"];
    let from = Select::new("Starting point:", from_options).prompt()?;

    let start_point = match from {
        "Current branch" => None,
        "Specific ref" => {
            let ref_value = Text::new("Ref (branch/tag/commit):").prompt()?;
            Some(ref_value)
        }
        _ => {
            let default = git::default_branch()?;
            Some(format!("origin/{default}"))
        }
    };

    run_with_post_prompt(&NewWorktreeOpts {
        branch,
        start_point,
    })
}
