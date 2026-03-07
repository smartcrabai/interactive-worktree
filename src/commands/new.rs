use anyhow::Result;
use inquire::{Select, Text};

use crate::commands::run_with_post_prompt;

pub fn run() -> Result<()> {
    let branch = Text::new("Branch name:")
        .with_help_message("Name for the new worktree branch")
        .prompt()?;

    let from_options = vec!["Default (main/master)", "Current branch", "Specific ref"];
    let from = Select::new("Starting point:", from_options).prompt()?;

    let mut args = vec!["new".to_string(), branch.clone()];

    match from {
        "Current branch" => args.push("--from-current".to_string()),
        "Specific ref" => {
            let ref_value = Text::new("Ref (branch/tag/commit):").prompt()?;
            args.push("--from".to_string());
            args.push(ref_value);
        }
        _ => {}
    }

    run_with_post_prompt(args, &branch)
}
