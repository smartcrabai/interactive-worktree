use anyhow::Result;
use inquire::{Select, Text};

use crate::git;

const CONFIG_KEYS: &[&str] = &[
    "gtr.worktrees.dir",
    "gtr.worktrees.prefix",
    "gtr.defaultBranch",
    "gtr.editor.default",
    "gtr.editor.workspace",
    "gtr.ai.default",
    "gtr.copy.include",
    "gtr.copy.exclude",
    "gtr.copy.includeDirs",
    "gtr.copy.excludeDirs",
    "gtr.hook.postCreate",
    "gtr.hook.preRemove",
    "gtr.hook.postRemove",
    "gtr.hook.postCd",
    "gtr.ui.color",
];

pub fn run() -> Result<()> {
    let actions = vec!["show", "get", "set", "unset"];
    let action = Select::new("Config action:", actions).prompt()?;

    match action {
        "show" => {
            let pairs = git::config_list("^gtr\\.")?;
            if pairs.is_empty() {
                println!("No gtr configuration found.");
            } else {
                for (k, v) in &pairs {
                    println!("{k} = {v}");
                }
            }
            Ok(())
        }
        "get" => {
            let key = Select::new("Config key:", CONFIG_KEYS.to_vec()).prompt()?;
            match git::config_get(key)? {
                Some(v) => println!("{v}"),
                None => println!("(not set)"),
            }
            Ok(())
        }
        "set" => {
            let key = Select::new("Config key:", CONFIG_KEYS.to_vec()).prompt()?;
            let value = Text::new("Value:").prompt()?;
            git::config_set(key, &value)
        }
        "unset" => {
            let key = Select::new("Config key:", CONFIG_KEYS.to_vec()).prompt()?;
            git::config_unset(key)
        }
        _ => unreachable!(),
    }
}
