mod app;
mod config;
mod notes;
mod paths;
mod render;
mod ui;

use std::io;
use std::time::Duration;

use anyhow::{bail, Result};
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;

use crate::app::{App, InputResult, Mode, SidebarLevel};

fn main() -> Result<()> {
    first_run();
    let args: Vec<String> = std::env::args().collect();
    match args.len() {
        1 => run(App::new_browser()),
        2 => run(App::new_in_project(&args[1])),
        _ => {
            eprintln!("Usage:");
            eprintln!("  ntx                  Project browser");
            eprintln!("  ntx <project>        Open project directly");
            Ok(())
        }
    }
}

fn run(mut app: App) -> Result<()> {
    let mut tui = setup_terminal()?;

    // Spawn terminal immediately when launched directly into a project
    if app.mode == Mode::Terminal {
        let area = tui.get_frame().area();
        let (rows, cols) = app::terminal_pty_size(area.height, area.width, false);
        app.spawn_terminal(rows, cols);
    }

    let result = event_loop(&mut tui, &mut app);
    restore_terminal(&mut tui)?;
    result
}

fn event_loop(
    tui: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    loop {
        if app.should_quit {
            return Ok(());
        }

        app.check_terminal_alive();

        tui.draw(|f| ui::render(f, app))?;

        // Send pending command to PTY
        if let Some(cmd) = app.pending_exec.take() {
            if let Some(ref mut term) = app.terminal {
                for line in cmd.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() || trimmed.starts_with('#') {
                        continue;
                    }
                    let _ = term.write_input(format!("{line}\r").as_bytes());
                }
            }
        }

        // Resize terminal to match layout
        {
            let area = tui.get_frame().area();
            app.resize_terminal(area.width, area.height);
        }

        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    // Capture whether we should spawn after handling the key
                    let should_spawn = should_spawn_terminal(app, &key);

                    let was_template = matches!(app.mode, Mode::TemplateInput { .. });

                    match app::handle_key(app, key) {
                        InputResult::ForwardToPty(bytes) => {
                            if let Some(ref mut term) = app.terminal {
                                let _ = term.write_input(&bytes);
                            }
                        }
                        InputResult::CopyBlock => {
                            if let Some(content) = app.focused_block_content() {
                                match copy_to_clipboard(&content) {
                                    Ok(()) => app.set_status("Copied to clipboard"),
                                    Err(e) => app.set_status(&format!("Copy failed: {e}")),
                                }
                            }
                        }
                        InputResult::EditNote => {
                            if let Some(note) = app.selected_note() {
                                let path = note.path.clone();
                                restore_terminal(tui)?;
                                open_in_editor(&path)?;
                                *tui = setup_terminal()?;
                                app.reload_notes();
                            }
                        }
                        InputResult::NewNote => {
                            let notes_dir = paths::notes_dir(&app.current_project);
                            std::fs::create_dir_all(&notes_dir)?;
                            let path = create_new_note(&notes_dir)?;
                            restore_terminal(tui)?;
                            open_in_editor(&path)?;
                            *tui = setup_terminal()?;
                            app.reload_notes();
                        }
                        InputResult::DeleteNote => {
                            if let Some(note) = app.selected_note() {
                                let path = note.path.clone();
                                let name = note.name.clone();
                                if std::fs::remove_file(&path).is_ok() {
                                    app.set_status(&format!("Deleted: {name}"));
                                    app.reload_notes();
                                }
                            }
                        }
                        InputResult::Consumed | InputResult::None => {}
                    }

                    // Toggle mouse capture when entering/leaving TemplateInput so
                    // the user can select terminal text to copy-paste into the prompt.
                    let is_template = matches!(app.mode, Mode::TemplateInput { .. });
                    if !was_template && is_template {
                        let _ = io::stdout().execute(crossterm::event::DisableMouseCapture);
                    } else if was_template && !is_template {
                        let _ = io::stdout().execute(crossterm::event::EnableMouseCapture);
                    }

                    if should_spawn {
                        let area = tui.get_frame().area();
                        let (rows, cols) = app::terminal_pty_size(
                            area.height,
                            area.width,
                            app.notes_panel_open,
                        );
                        app.spawn_terminal(rows, cols);
                    }
                }
                Event::Mouse(mouse) => {
                    app.handle_mouse(mouse);
                }
                Event::Resize(_cols, _rows) => {
                    // Resize handled above
                }
                _ => {}
            }
        }

        app.tick_status();
    }
}

/// Returns true if the current key + app state should trigger a terminal spawn.
/// Spawning happens after key handling so app state is updated first.
fn should_spawn_terminal(app: &App, key: &crossterm::event::KeyEvent) -> bool {
    use crossterm::event::KeyCode;
    if key.code != KeyCode::Enter || app.is_terminal_alive() {
        return false;
    }
    match (&app.mode, &app.sidebar_level) {
        // Enter on a note in the notes list → go to terminal + notes panel
        (Mode::Normal, SidebarLevel::Notes) => app.selected_note().is_some(),
        // Enter on a block in NoteContent → execute block
        (Mode::Normal, SidebarLevel::NoteContent) => {
            app.selected_note().map_or(false, |n| !n.code_blocks.is_empty())
        }
        _ => false,
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    terminal::enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    io::stdout().execute(crossterm::event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(io::stdout());
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(tui: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    io::stdout().execute(crossterm::event::DisableMouseCapture)?;
    terminal::disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    tui.show_cursor()?;
    Ok(())
}

fn copy_to_clipboard(text: &str) -> Result<()> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| anyhow::anyhow!("clipboard init failed: {e}"))?;
    clipboard
        .set_text(text)
        .map_err(|e| anyhow::anyhow!("clipboard set failed: {e}"))?;
    Ok(())
}

fn open_in_editor(path: &std::path::Path) -> Result<()> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
    let status = std::process::Command::new(&editor).arg(path).status()?;
    if !status.success() {
        bail!("editor exited with status: {status}");
    }
    Ok(())
}

fn create_new_note(notes_dir: &std::path::Path) -> Result<std::path::PathBuf> {
    let template = "---\nname: \ntags: []\n---\n\n# \n\n```bash\n\n```\n";
    let name = format!("new-note-{}", chrono::Utc::now().timestamp());
    let path = notes_dir.join(format!("{name}.md"));
    std::fs::write(&path, template)?;
    Ok(path)
}

/// Seeds a quickstart project on first run if no projects exist yet.
fn first_run() {
    if !paths::list_projects().is_empty() {
        return;
    }

    let project = "quickstart";
    paths::create_project(project);

    let notes_dir = paths::notes_dir(project);
    let _ = std::fs::create_dir_all(&notes_dir);

    for (filename, content) in QUICKSTART_NOTES {
        let _ = std::fs::write(notes_dir.join(filename), content);
    }
}

const QUICKSTART_NOTES: &[(&str, &str)] = &[
    ("1-welcome.md",        include_str!("quickstart/1-welcome.md")),
    ("2-navigation.md",     include_str!("quickstart/2-navigation.md")),
    ("3-code-blocks.md",    include_str!("quickstart/3-code-blocks.md")),
    ("4-your-projects.md",  include_str!("quickstart/4-your-projects.md")),
];
