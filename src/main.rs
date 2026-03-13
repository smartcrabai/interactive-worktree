mod commands;
mod gh;
mod git;
mod gtr;

use std::fmt;
use std::process;

use console::style;
use inquire::{InquireError, Select};

#[derive(Debug, Clone, Copy)]
enum Command {
    New,
    Pr,
    List,
    Rm,
    Editor,
    Ai,
    Go,
    Run,
    Copy,
    Clean,
    Mv,
    Config,
    Doctor,
    Quit,
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Command::New => write!(f, "new      - Create a new worktree"),
            Command::Pr => write!(f, "pr       - Create worktree from pull request"),
            Command::List => write!(f, "list     - List all worktrees"),
            Command::Rm => write!(f, "rm       - Remove worktree(s)"),
            Command::Editor => write!(f, "editor   - Open worktree in editor"),
            Command::Ai => write!(f, "ai       - Start AI tool in worktree"),
            Command::Go => write!(f, "go       - Print worktree path"),
            Command::Run => write!(f, "run      - Run command in worktree"),
            Command::Copy => write!(f, "copy     - Copy files to worktree(s)"),
            Command::Clean => write!(f, "clean    - Clean stale worktrees"),
            Command::Mv => write!(f, "mv       - Rename worktree"),
            Command::Config => write!(f, "config   - Manage configuration"),
            Command::Doctor => write!(f, "doctor   - Health check"),
            Command::Quit => write!(f, "quit     - Exit"),
        }
    }
}

const COMMANDS: &[Command] = &[
    Command::New,
    Command::Pr,
    Command::List,
    Command::Rm,
    Command::Editor,
    Command::Ai,
    Command::Go,
    Command::Run,
    Command::Copy,
    Command::Clean,
    Command::Mv,
    Command::Config,
    Command::Doctor,
    Command::Quit,
];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "-v" || a == "--version") {
        println!("iwt {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if !git::is_inside_repo() {
        eprintln!(
            "{} Not inside a git repository.",
            style("error:").red().bold()
        );
        process::exit(1);
    }

    if !gtr::is_available() {
        eprintln!(
            "{} git-gtr is not installed or not in PATH.",
            style("error:").red().bold()
        );
        eprintln!(
            "Install it from: {}",
            style("https://github.com/coderabbitai/git-worktree-runner").cyan()
        );
        process::exit(1);
    }

    println!(
        "{} (Ctrl+C to quit)\n",
        style("interactive-worktree").green().bold()
    );

    loop {
        let selection = Select::new("Command:", COMMANDS.to_vec())
            .with_page_size(COMMANDS.len())
            .prompt();

        match selection {
            Ok(Command::Quit)
            | Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => break,
            Ok(cmd) => {
                let result = match cmd {
                    Command::New => commands::new::run(),
                    Command::Pr => commands::pr::run(),
                    Command::List => commands::list::run(),
                    Command::Rm => commands::rm::run(),
                    Command::Editor => commands::editor::run(),
                    Command::Ai => commands::ai::run(),
                    Command::Go => commands::go::run(),
                    Command::Run => commands::run::run(),
                    Command::Copy => commands::copy::run(),
                    Command::Clean => commands::clean::run(),
                    Command::Mv => commands::mv::run(),
                    Command::Config => commands::config::run(),
                    Command::Doctor => commands::doctor::run(),
                    Command::Quit => unreachable!(),
                };
                if let Err(e) = result {
                    eprintln!("{} {e}", style("error:").red().bold());
                }
                println!();
            }
            Err(e) => {
                eprintln!("{} {e}", style("error:").red().bold());
                break;
            }
        }
    }
}
