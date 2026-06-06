# ntx

A terminal with a notes sidebar. Keep runnable command snippets in markdown notes and execute them directly into your shell — without leaving the terminal.

## Install

```bash
brew tap mitchldtn/tap
brew install ntx
```

Or build from source (requires Rust 1.75+):

```bash
git clone https://github.com/mitchldtn/ntx
cd ntx
cargo build --release --manifest-path ntx/Cargo.toml
```

## Usage

```bash
ntx                  # open project browser
ntx <project>        # open a project directly
```

On first run, a `quickstart` project is created at `~/.ntx/projects/quickstart/` with getting-started notes.

## Project Structure

Projects live in `~/.ntx/projects/`:

```
~/.ntx/projects/
└── my-project/
    ├── config/
    │   └── default.conf
    └── notes/
        ├── setup.md
        └── deploy.md
```

### Config format

```
dir = ~/dev/my-project
shell = /bin/zsh

[env]
NODE_ENV = development
AWS_PROFILE = staging

[script]
echo "project ready"

greet() {
  echo "hey $1, welcome to $(basename $PWD)"
}
```

| Field | Description |
|-------|-------------|
| `dir` | Working directory for the terminal |
| `shell` | Shell binary (defaults to `$SHELL`) |
| `[env]` | Environment variables injected into the session |
| `[script]` | Sourced into the shell on startup — run commands or define functions available for the session |

### Note format

Notes are standard markdown files with optional YAML frontmatter:

````markdown
---
name: Deploy
tags: [deploy, aws]
---

# Deploy to Staging

Push the current branch and trigger a deploy.

```bash
git push origin HEAD
```

Run with a specific tag:

```bash
git push origin <tag>:staging
```
````

Code blocks are individually selectable and executable. Blocks containing `<variable>` placeholders will prompt for values before running.

## Key Bindings

### Project browser
| Key | Action |
|-----|--------|
| `↑↓` / `j`/`k` | Navigate |
| `enter` | Open project |
| `n` | New project |
| `q` | Quit |

### Notes list
| Key | Action |
|-----|--------|
| `↑↓` / `j`/`k` | Navigate notes |
| `enter` | Open terminal with notes panel |
| `→` | Browse note content and code blocks |
| `e` | Edit in `$EDITOR` |
| `n` | New note |
| `d` | Delete note (confirmation required) |
| `←` | Back to projects |
| `esc` | Return to terminal |

### Terminal
| Key | Action |
|-----|--------|
| `ctrl+n` | Toggle notes panel open/closed |
| `ctrl+w` | Move focus between terminal and notes panel |
| `ctrl+b` | Open note browser to switch notes |
| `shift+pageup/down` | Scroll terminal history |

### Notes panel (focused)
| Key | Action |
|-----|--------|
| `↑↓` / `j`/`k` / `tab` | Navigate code blocks |
| `enter` | Run focused block |
| `y` | Copy block to clipboard |
| `e` | Edit note in `$EDITOR` |
| `/` | Jump to block by number |
| `←` | Browse notes list |
| `esc` | Return focus to terminal |

## License

MIT
