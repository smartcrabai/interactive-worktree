use anyhow::{Result, bail};
use inquire::Select;

use crate::commands::{NewWorktreeOpts, run_with_post_prompt};
use crate::{gh, git};

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

    let start_point = Some(format!("origin/{branch}"));

    run_with_post_prompt(&NewWorktreeOpts {
        branch,
        start_point,
    })
}
