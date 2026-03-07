use std::fmt;
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub head_ref_name: String,
    pub author: Option<Author>,
}

#[derive(Debug, Deserialize)]
pub struct Author {
    pub login: String,
}

impl fmt::Display for PullRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let login = self
            .author
            .as_ref()
            .map(|a| a.login.as_str())
            .unwrap_or("ghost");
        write!(
            f,
            "#{} {} ({}) [{}]",
            self.number, self.title, login, self.head_ref_name
        )
    }
}

/// Fetch open pull requests from the current repository.
pub fn pr_list() -> Result<Vec<PullRequest>> {
    let output = Command::new("gh")
        .args([
            "pr",
            "list",
            "--json",
            "number,title,headRefName,author",
            "--limit",
            "50",
        ])
        .output()
        .context("Failed to execute gh pr list")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("gh pr list failed: {}", stderr.trim());
    }

    let prs: Vec<PullRequest> =
        serde_json::from_slice(&output.stdout).context("Failed to parse gh pr list output")?;

    Ok(prs)
}
