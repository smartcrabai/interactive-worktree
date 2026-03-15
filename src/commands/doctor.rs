use std::process::Command;

use anyhow::Result;

use crate::git;

pub fn run() -> Result<()> {
    // Git version
    let output = Command::new("git").arg("--version").output();
    match output {
        Ok(o) if o.status.success() => {
            let version = String::from_utf8_lossy(&o.stdout);
            println!("[OK] {}", version.trim());
        }
        Ok(_) => {
            println!("[ERR] git returned non-zero exit status");
            return Ok(());
        }
        Err(e) => {
            println!("[ERR] git not found: {e}");
            return Ok(());
        }
    }

    // Worktrees
    let wts = git::worktree_list()?;
    println!("\nWorktrees ({}):", wts.len());
    for wt in &wts {
        println!("  {} — {}", wt.branch, wt.path);
    }

    // Configuration
    let configs = git::config_list("^gtr\\.")?;
    if configs.is_empty() {
        println!("\nConfiguration: (none)");
    } else {
        println!("\nConfiguration:");
        for (k, v) in &configs {
            println!("  {k} = {v}");
        }
    }

    Ok(())
}
