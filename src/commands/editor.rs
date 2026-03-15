use anyhow::Result;

use crate::commands::run_with_tool_selection;
use crate::worktree_ops;

const EDITORS: &[&str] = &[
    "default", "cursor", "vscode", "zed", "idea", "nvim", "vim", "emacs", "sublime", "nano",
];

pub fn run() -> Result<()> {
    run_with_tool_selection(
        "Editor:",
        EDITORS,
        "'default' uses gtr.editor.default config",
        worktree_ops::open_editor,
    )
}
