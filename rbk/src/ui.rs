use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::{App, Mode, SidebarLevel};
use crate::notes::Note;
use crate::render;

const SIDEBAR_WIDTH: u16 = 32;
const NOTES_PANEL_WIDTH: u16 = 42;

// ── Top-level ──────────────────────────────────────────────────────────────

pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    match &app.mode {
        Mode::Normal | Mode::NewProject | Mode::ConfirmDelete => render_normal(frame, app, area),
        Mode::Terminal | Mode::TemplateInput { .. } => render_terminal_mode(frame, app, area),
    }

    // Popups on top
    match &app.mode {
        Mode::NewProject | Mode::TemplateInput { .. } | Mode::ConfirmDelete => {
            render_dim_overlay(frame, area);
        }
        _ => {}
    }
    match &app.mode {
        Mode::NewProject => render_popup(frame, area, "New Project", &app.input_buf.clone()),
        Mode::TemplateInput { vars, values, current, .. } => {
            let (vars, values, current) = (vars.clone(), values.clone(), *current);
            render_template_popup(frame, area, &vars, &values, current);
        }
        Mode::ConfirmDelete => {
            let name = app.selected_note().map(|n| n.name.clone()).unwrap_or_default();
            render_confirm_delete_popup(frame, area, &name);
        }
        _ => {}
    }

    // Command bar
    if let Some(ref input) = app.command_bar {
        let bar_area = Rect {
            x: area.x,
            y: area.bottom().saturating_sub(1),
            width: area.width,
            height: 1,
        };
        let bar = Paragraph::new(Line::from(vec![
            Span::styled(" /", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(input.clone(), Style::default().fg(Color::White)),
            Span::styled("█", Style::default().fg(Color::Yellow)),
            Span::styled("  enter: exec  esc: cancel", Style::default().fg(Color::DarkGray)),
        ]))
        .style(Style::default().bg(Color::DarkGray));
        frame.render_widget(bar, bar_area);
    }
}

// ── Normal mode ────────────────────────────────────────────────────────────

fn render_normal(frame: &mut Frame, app: &mut App, area: Rect) {
    match app.sidebar_level {
        SidebarLevel::Projects => render_projects_screen(frame, app, area),
        SidebarLevel::Notes | SidebarLevel::NoteContent => render_notes_screen(frame, app, area),
    }
}

/// Full-screen project list (no terminal behind it yet).
fn render_projects_screen(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" rbk ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.projects.is_empty() {
        let msg = Paragraph::new("\n  No projects yet.\n\n  Press 'n' to create one.")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(msg, inner);
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .split(inner);

    let items: Vec<ListItem> = app
        .projects
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let selected = i == app.project_selected;
            let marker = if selected { "▸ " } else { "  " };
            let style = if selected {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            ListItem::new(Line::from(Span::styled(format!("{marker}{name}"), style)))
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(app.project_selected));
    frame.render_stateful_widget(List::new(items), chunks[0], &mut state);

    let help = Paragraph::new(Line::from(vec![
        Span::styled(" ↑↓", Style::default().fg(Color::Cyan)),
        Span::raw(" nav  "),
        Span::styled("enter", Style::default().fg(Color::Cyan)),
        Span::raw(" open  "),
        Span::styled("n", Style::default().fg(Color::Cyan)),
        Span::raw(" new  "),
        Span::styled("q", Style::default().fg(Color::Cyan)),
        Span::raw(" quit"),
    ]))
    .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[1]);
}

/// Left sidebar (notes list) + right note content preview.
fn render_notes_screen(frame: &mut Frame, app: &mut App, area: Rect) {
    let sidebar_w = SIDEBAR_WIDTH.min(area.width / 2);
    let chunks = Layout::horizontal([
        Constraint::Length(sidebar_w),
        Constraint::Min(1),
    ])
    .split(area);

    render_notes_sidebar(frame, app, chunks[0]);
    render_note_preview(frame, app, chunks[1]);
}

fn render_notes_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    let focused_notes = app.sidebar_level == SidebarLevel::Notes;
    let border_color = if focused_notes { Color::Cyan } else { Color::DarkGray };

    let block = Block::default()
        .title(format!(" {} ", app.current_project))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(3),
    ])
    .split(inner);

    if app.notes.is_empty() {
        let msg = Paragraph::new("  No notes.\n  Press 'n' to create one.")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(msg, chunks[0]);
    } else {
        let items: Vec<ListItem> = app
            .notes
            .iter()
            .enumerate()
            .map(|(i, note)| {
                let selected = i == app.notes_selected;
                let marker = if selected { "▸ " } else { "  " };
                let style = if selected {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                ListItem::new(Line::from(Span::styled(
                    format!("{marker}{}", truncate(&note.name, (area.width as usize).saturating_sub(4))),
                    style,
                )))
            })
            .collect();

        let mut state = ListState::default();
        state.select(Some(app.notes_selected));
        frame.render_stateful_widget(List::new(items), chunks[0], &mut state);
    }

    let help_lines = match app.sidebar_level {
        SidebarLevel::Notes => vec![
            Line::from(vec![
                Span::styled(" ↑↓", Style::default().fg(Color::Cyan)),
                Span::raw(" nav  "),
                Span::styled("→/enter", Style::default().fg(Color::Cyan)),
                Span::raw(" open"),
            ]),
            Line::from(vec![
                Span::styled(" e", Style::default().fg(Color::Cyan)),
                Span::raw(" edit  "),
                Span::styled("n", Style::default().fg(Color::Cyan)),
                Span::raw(" new  "),
                Span::styled("d", Style::default().fg(Color::Cyan)),
                Span::raw(" del"),
            ]),
            Line::from(vec![
                Span::styled(" esc", Style::default().fg(Color::Cyan)),
                Span::raw(" back"),
            ]),
        ],
        SidebarLevel::NoteContent => vec![
            Line::from(vec![
                Span::styled(" ↑↓/tab", Style::default().fg(Color::Cyan)),
                Span::raw(" block  "),
                Span::styled("enter", Style::default().fg(Color::Cyan)),
                Span::raw(" exec"),
            ]),
            Line::from(vec![
                Span::styled(" y", Style::default().fg(Color::Cyan)),
                Span::raw(" copy  "),
                Span::styled("e", Style::default().fg(Color::Cyan)),
                Span::raw(" edit"),
            ]),
            Line::from(vec![
                Span::styled(" ←/esc", Style::default().fg(Color::Cyan)),
                Span::raw(" back"),
            ]),
        ],
        _ => vec![],
    };

    let help = Paragraph::new(help_lines).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[1]);
}

fn render_note_preview(frame: &mut Frame, app: &mut App, area: Rect) {
    let note = match app.notes.get(app.notes_selected) {
        Some(n) => n,
        None => {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray));
            frame.render_widget(block, area);
            return;
        }
    };

    let focused = app.sidebar_level == SidebarLevel::NoteContent;
    let border_color = if focused { Color::Yellow } else { Color::DarkGray };

    let title = note.title.as_deref().unwrap_or(&note.name);
    let block = Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    render_note_blocks(frame, app, inner, note, focused);
}

// ── Terminal mode ──────────────────────────────────────────────────────────

fn render_terminal_mode(frame: &mut Frame, app: &mut App, area: Rect) {
    let show_status = app.status_msg.is_some();
    let (content_area, status_area) = if show_status {
        let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    if app.notes_panel_open {
        let panel_w = NOTES_PANEL_WIDTH.min(content_area.width.saturating_sub(20));
        let term_w = content_area.width.saturating_sub(panel_w);
        let chunks = Layout::horizontal([
            Constraint::Length(term_w),
            Constraint::Length(panel_w),
        ])
        .split(content_area);

        render_terminal_pane(frame, app, chunks[0]);
        render_notes_panel(frame, app, chunks[1]);
    } else {
        render_terminal_pane(frame, app, content_area);
    }

    if let Some(status) = status_area {
        render_status_bar(frame, app, status);
    }
}

fn render_terminal_pane(frame: &mut Frame, app: &mut App, area: Rect) {
    let inner = area;
    app.terminal_area = Some((inner.x, inner.y, inner.width, inner.height));

    let term = match app.terminal.as_ref() {
        Some(t) => t,
        None => return,
    };

    let rows = inner.height;
    let cols = inner.width;

    if app.scroll_offset > 0 {
        term.with_scrollback(app.scroll_offset, |screen| {
            let (lines, _cursor) = render::render_vt100_screen(screen, rows, cols);
            frame.render_widget(Paragraph::new(lines), inner);
        });
    } else {
        let cursor_pos = term.with_screen(|screen| {
            let (lines, cursor) = render::render_vt100_screen(screen, rows, cols);
            frame.render_widget(Paragraph::new(lines), inner);
            cursor
        });
        if let Some(Some((cx, cy))) = cursor_pos {
            frame.set_cursor_position((inner.x + cx, inner.y + cy));
        }
    }
}

fn render_notes_panel(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.notes_panel_focused;
    let border_color = if focused { Color::Yellow } else { Color::DarkGray };

    let note = match app.notes.get(app.notes_selected) {
        Some(n) => n,
        None => {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color));
            let inner = block.inner(area);
            frame.render_widget(block, area);
            app.notes_panel_area = Some((inner.x, inner.y, inner.width, inner.height));
            return;
        }
    };

    let title = note.title.as_deref().unwrap_or(&note.name);
    let help = if focused { " ctrl+n:close  esc:unfocus " } else { " ctrl+n:cycle  ctrl+w:focus " };
    let block = Block::default()
        .title(format!(" {title} "))
        .title_bottom(Line::from(Span::styled(help, Style::default().fg(Color::DarkGray))))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    app.notes_panel_area = Some((inner.x, inner.y, inner.width, inner.height));

    render_note_blocks(frame, app, inner, note, focused);
}

// ── Note content renderer (shared) ────────────────────────────────────────

fn render_note_blocks(frame: &mut Frame, app: &App, area: Rect, note: &Note, focused: bool) {
    let (lines, focused_line) = build_note_lines(&note.body, app.block_focused, area.width);

    let scroll = if app.note_view_scroll > 0 {
        app.note_view_scroll
    } else if focused {
        let target = focused_line.unwrap_or(0) as u16;
        let visible = area.height;
        if target >= visible {
            target.saturating_sub(visible / 3)
        } else {
            0
        }
    } else {
        0
    };

    let para = Paragraph::new(lines)
        .scroll((scroll, 0));
    frame.render_widget(para, area);
}

/// Walk note body line by line, rendering headings, prose, and fenced code
/// blocks with [N] labels. Returns lines and the line index of the focused block.
fn build_note_lines(body: &str, focused_block: usize, width: u16) -> (Vec<Line<'static>>, Option<usize>) {
    let w = width as usize;
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut block_idx = 0usize;
    let mut in_block = false;
    let mut focused_line: Option<usize> = None;

    for line in body.lines() {
        let trimmed = line.trim();

        if !in_block && trimmed.starts_with("```") {
            in_block = true;
            let is_focused = block_idx == focused_block;
            if is_focused {
                focused_line = Some(lines.len());
            }
            let lang = trimmed[3..].trim();
            let lang_display = if lang.is_empty() { String::new() } else { format!(" {lang}") };
            let marker = if is_focused { ">" } else { " " };
            let label = format!("{marker}[{block_idx}]{lang_display}");
            let label_style = if is_focused {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            lines.push(Line::from(Span::styled(label, label_style)));
            let border = "\u{2500}".repeat(w.saturating_sub(4));
            lines.push(Line::from(Span::styled(
                format!(" \u{250c}{border}\u{2510}"),
                Style::default().fg(Color::DarkGray),
            )));
            continue;
        }

        if in_block && trimmed.starts_with("```") {
            let border = "\u{2500}".repeat(w.saturating_sub(4));
            lines.push(Line::from(Span::styled(
                format!(" \u{2514}{border}\u{2518}"),
                Style::default().fg(Color::DarkGray),
            )));
            block_idx += 1;
            in_block = false;
            continue;
        }

        if in_block {
            let is_focused = block_idx == focused_block;
            let code_style = if is_focused {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            };
            let max_content = w.saturating_sub(3);
            let cont_indent = "  ";
            let cont_width = max_content.saturating_sub(cont_indent.len());
            let mut wrapped: Vec<String> = Vec::new();
            let mut current = String::new();
            for word in line.split(' ') {
                let max = if wrapped.is_empty() { max_content } else { cont_width };
                if current.is_empty() {
                    current = word.to_string();
                } else if current.len() + 1 + word.len() <= max {
                    current.push(' ');
                    current.push_str(word);
                } else {
                    wrapped.push(current);
                    current = word.to_string();
                }
            }
            if !current.is_empty() { wrapped.push(current); }
            for (i, chunk) in wrapped.iter().enumerate() {
                let text = if i == 0 { chunk.clone() } else { format!("{cont_indent}{chunk}") };
                lines.push(Line::from(vec![
                    Span::styled(" \u{2502} ", Style::default().fg(Color::DarkGray)),
                    Span::styled(text, code_style),
                ]));
            }
        } else {
            lines.push(style_markdown_line(line));
        }
    }

    (lines, focused_line)
}

fn style_markdown_line(line: &str) -> Line<'static> {
    let trimmed = line.trim();
    if trimmed.starts_with("### ") {
        Line::from(Span::styled(
            format!(" {trimmed}"),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        ))
    } else if trimmed.starts_with("## ") {
        Line::from(Span::styled(
            format!(" {trimmed}"),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ))
    } else if trimmed.starts_with("# ") {
        Line::from(Span::styled(
            format!(" {trimmed}"),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ))
    } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        Line::from(Span::styled(
            format!("  {trimmed}"),
            Style::default().fg(Color::White),
        ))
    } else if trimmed.is_empty() {
        Line::raw("")
    } else {
        Line::from(Span::styled(
            format!(" {trimmed}"),
            Style::default().fg(Color::White),
        ))
    }
}

// ── Status bar ─────────────────────────────────────────────────────────────

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let content = if let Some(ref msg) = app.status_msg {
        Line::from(vec![
            Span::raw(" "),
            Span::styled(msg.clone(), Style::default().fg(Color::Yellow)),
        ])
    } else {
        let project = if app.current_project.is_empty() {
            "rbk".to_string()
        } else {
            app.current_project.clone()
        };
        Line::from(vec![
            Span::styled(format!(" {project}"), Style::default().fg(Color::DarkGray)),
            Span::styled(
                "  ctrl+n: notes  ctrl+b: browse  ctrl+c: quit",
                Style::default().fg(Color::DarkGray),
            ),
        ])
    };
    let bar = Paragraph::new(content).style(Style::default().bg(Color::Reset));
    frame.render_widget(bar, area);
}

// ── Popups ─────────────────────────────────────────────────────────────────

fn render_popup(frame: &mut Frame, area: Rect, title: &str, value: &str) {
    let popup = centered_rect(50, 3, area);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let display = format!("{value}█");
    let para = Paragraph::new(display).style(Style::default().fg(Color::White));
    frame.render_widget(para, inner);
}

fn render_template_popup(
    frame: &mut Frame,
    area: Rect,
    vars: &[String],
    values: &[String],
    current: usize,
) {
    let var_name = vars.get(current).map(|s| s.as_str()).unwrap_or("?");
    let value = values.get(current).map(|s| s.as_str()).unwrap_or("");
    let title = format!(" {var_name} ({}/{}) ", current + 1, vars.len());
    let popup = centered_rect(60, 3, area);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let display = format!("{value}█");
    let para = Paragraph::new(display).style(Style::default().fg(Color::White));
    frame.render_widget(para, inner);
}

// ── Utilities ──────────────────────────────────────────────────────────────

fn render_dim_overlay(frame: &mut Frame, area: Rect) {
    let overlay = Block::default().style(Style::default().bg(Color::Rgb(10, 10, 10)));
    frame.render_widget(overlay, area);
}

fn render_confirm_delete_popup(frame: &mut Frame, area: Rect, note_name: &str) {
    let popup = centered_rect(60, 3, area);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .title(" Delete Note ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let msg = format!(" Delete '{note_name}'?  y = confirm   any key = cancel");
    let para = Paragraph::new(msg).style(Style::default().fg(Color::White));
    frame.render_widget(para, inner);
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let w = area.width * percent_x / 100;
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + area.height / 2 - height / 2;
    Rect { x, y, width: w, height }
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{truncated}…")
    }
}
