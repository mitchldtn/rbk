# ntx

A terminal with a notes sidebar. Keep runnable command snippets in markdown notes and execute them directly into your shell.

## Features

- Project browser with per-project working directory, env vars, and startup script
- Notes panel alongside the terminal — navigate and run code blocks without leaving the shell
- Template variables in code blocks — `<var>` prompts for input before running
- Scrollback support
- First-run quickstart project included

## Install

```bash
cargo install --path ntx
```

Requires Rust 1.75+.

## Usage

```bash
ntx                  # open project browser
ntx <project>        # open project directly
```

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
| `→` | Browse note content |
| `e` | Edit in `$EDITOR` |
| `n` | New note |
| `d` | Delete note (confirmation required) |
| `←` / `esc` | Back |

### Terminal
| Key | Action |
|-----|--------|
| `ctrl+n` | Toggle notes panel |
| `ctrl+w` | Move focus between terminal and notes |
| `ctrl+b` | Open note browser |
| `shift+pageup/down` | Scroll terminal history |

### Notes panel (focused)
| Key | Action |
|-----|--------|
| `↑↓` / `j`/`k` / `tab` | Navigate code blocks |
| `enter` | Run focused block |
| `y` | Copy block to clipboard |
| `e` | Edit note |
| `/` | Jump to block by number |
| `←` | Browse notes list |
| `esc` | Return focus to terminal |

## Project Configuration

Projects live in `~/.ntx/projects/`. Each project reads its config from:

```
~/.ntx/projects/<project>/config/default.conf
```

```
dir = ~/dev/my-project
shell = /bin/zsh

[env]
NODE_ENV = development

[script]
echo "ready"
```

Notes go in:

```
~/.ntx/projects/<project>/notes/
```

## Template Variables

Code blocks can contain `<variable>` placeholders. When run, ntx prompts
for each value before sending the command to the terminal.

```bash
git checkout -b <branch-name>
```

## Building from Source

```bash
git clone https://github.com/mitchldtn/ntx
cd ntx
cargo build --release
```

The binary is at `target/release/ntx`.
