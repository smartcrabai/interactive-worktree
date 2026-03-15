# interactive-worktree

Interactive CLI for managing git worktrees.

![Demo](demo.gif)

## Requirements

- Git 2.17+

## Installation

### Homebrew (macOS / Linux) - recommended

```sh
brew install smartcrabai/tap/interactive-worktree
```

### Shell script installer

Installs a prebuilt binary to `~/.cargo/bin`:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/smartcrabai/interactive-worktree/releases/latest/download/interactive-worktree-installer.sh | sh
```

Supported platforms: macOS (ARM64, x86_64), Linux (x86_64)

### Build from source

```bash
cargo install --git https://github.com/smartcrabai/interactive-worktree
```

## Usage

```bash
iwt
```

## Development

### Library

`interactive-worktree` can also be used as a library crate and exposes the `git` and `gh` modules.

### Running tests

```bash
cargo test
```
