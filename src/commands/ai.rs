use anyhow::Result;

use crate::commands::run_with_tool_selection;
use crate::worktree_ops;

const AI_TOOLS: &[&str] = &[
    "default", "claude", "aider", "copilot", "codex", "auggie", "continue", "gemini", "opencode",
];

pub fn run() -> Result<()> {
    run_with_tool_selection(
        "AI tool:",
        AI_TOOLS,
        "'default' uses gtr.ai.default config",
        worktree_ops::start_ai,
    )
}
