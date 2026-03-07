use anyhow::{Result, bail};
use inquire::Select;

use crate::{commands::run_with_post_prompt, gh, git};

pub fn run() -> Result<()> {
    let prs = gh::pr_list()?;
    if prs.is_empty() {
        bail!("No open pull requests found.");
    }

    let pr = Select::new("Select a pull request:", prs)
        .with_page_size(10)
        .prompt()?;

    let branch = pr.head_ref_name;
    git::fetch("origin", &branch)?;

    let from = format!("origin/{branch}");
    let args = vec![
        "new".to_string(),
        branch.clone(),
        "--from".to_string(),
        from,
    ];

    run_with_post_prompt(args, &branch)
}
