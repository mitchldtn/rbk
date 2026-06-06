use ntx_core::notes::{load_notes_from_dir, Note};
use ntx_core::terminal::Terminal;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

use crate::{config, paths};

// ── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    /// Sidebar active — browsing projects or notes.
    Normal,
    /// Terminal has focus.
    Terminal,
    /// Filling template variables before executing a block.
    TemplateInput {
        template: String,
        vars: Vec<String>,
        values: Vec<String>,
        current: usize,
    },
    /// Typing a new project name.
    NewProject,
    /// Waiting for delete confirmation.
    ConfirmDelete,
}

/// Which level the Normal-mode sidebar is showing.
#[derive(Debug, Clone, PartialEq)]
pub enum SidebarLevel {
    /// Project list.
    Projects,
    /// Notes list for current project (left) + note preview (right).
    Notes,
    /// Note content focused — navigating code blocks.
    NoteContent,
}

pub enum InputResult {
    Consumed,
    ForwardToPty(Vec<u8>),
    CopyBlock,
    EditNote,
    NewNote,
    DeleteNote,
    None,
}

// ── App ────────────────────────────────────────────────────────────────────

pub struct App {
    pub mode: Mode,
    pub should_quit: bool,
    pub sidebar_level: SidebarLevel,

    // Projects
    pub projects: Vec<String>,
    pub project_selected: usize,
    pub current_project: String,

    // Terminal
    pub terminal: Option<Terminal>,
    pub scroll_offset: u16,

    // Notes
    pub notes: Vec<Note>,
    pub notes_selected: usize,
    pub block_focused: usize,
    /// Right-side notes panel visible in Terminal mode.
    pub notes_panel_open: bool,
    /// Notes panel has keyboard focus in Terminal mode.
    pub notes_panel_focused: bool,
    pub note_view_scroll: u16,

    // UI
    pub status_msg: Option<String>,
    pub status_ttl: u8,
    pub input_buf: String,
    pub command_bar: Option<String>,
    pub pending_exec: Option<String>,

    // Mouse hit areas (set by renderer, used for scroll targeting)
    pub terminal_area: Option<(u16, u16, u16, u16)>,
    pub notes_panel_area: Option<(u16, u16, u16, u16)>,
}

impl App {
    pub fn new_browser() -> Self {
        let projects = paths::list_projects();
        Self {
            mode: Mode::Normal,
            should_quit: false,
            sidebar_level: SidebarLevel::Projects,
            projects,
            project_selected: 0,
            current_project: String::new(),
            terminal: None,
            scroll_offset: 0,
            notes: Vec::new(),
            notes_selected: 0,
            block_focused: 0,
            notes_panel_open: false,
            notes_panel_focused: false,
            note_view_scroll: 0,
            status_msg: None,
            status_ttl: 0,
            input_buf: String::new(),
            command_bar: None,
            pending_exec: None,
            terminal_area: None,
            notes_panel_area: None,
        }
    }

    pub fn new_in_project(project: &str) -> Self {
        let projects = paths::list_projects();
        let project_selected = projects_index(&projects, project);
        let notes = load_notes_for(project);
        Self {
            mode: Mode::Terminal,
            should_quit: false,
            sidebar_level: SidebarLevel::Notes,
            projects,
            project_selected,
            current_project: project.to_string(),
            terminal: None,
            scroll_offset: 0,
            notes,
            notes_selected: 0,
            block_focused: 0,
            notes_panel_open: false,
            notes_panel_focused: false,
            note_view_scroll: 0,
            status_msg: None,
            status_ttl: 0,
            input_buf: String::new(),
            command_bar: None,
            pending_exec: None,
            terminal_area: None,
            notes_panel_area: None,
        }
    }

    // ── Project / notes loading ───────────────────────────────────────────

    pub fn select_project(&mut self) {
        let project = match self.projects.get(self.project_selected).cloned() {
            Some(p) => p,
            None => return,
        };
        self.current_project = project.clone();
        self.notes = load_notes_for(&project);
        self.notes_selected = 0;
        self.block_focused = 0;
        self.note_view_scroll = 0;
        self.sidebar_level = SidebarLevel::Notes;

        if self.is_terminal_alive() {
            let session = config::load(&project);
            if let Some(dir) = session.working_dir.filter(|d| !d.is_empty()) {
                self.pending_exec = Some(format!("cd {dir}"));
            }
        }
    }

    pub fn back_to_projects(&mut self) {
        self.sidebar_level = SidebarLevel::Projects;
        self.current_project.clear();
        self.notes.clear();
        self.terminal = None;
    }

    /// Go to project list without killing an active terminal session.
    pub fn browse_to_projects(&mut self) {
        self.sidebar_level = SidebarLevel::Projects;
    }

    pub fn refresh_projects(&mut self) {
        self.projects = paths::list_projects();
    }

    pub fn reload_notes(&mut self) {
        if !self.current_project.is_empty() {
            self.notes = load_notes_for(&self.current_project);
            if self.notes_selected >= self.notes.len() {
                self.notes_selected = self.notes.len().saturating_sub(1);
            }
        }
    }

    pub fn create_project(&mut self) {
        let name = self.input_buf.trim().to_string();
        if name.is_empty() {
            self.mode = Mode::Normal;
            return;
        }
        paths::create_project(&name);
        self.input_buf.clear();
        self.refresh_projects();
        if let Some(idx) = self.projects.iter().position(|p| p == &name) {
            self.project_selected = idx;
        }
        self.mode = Mode::Normal;
        self.set_status(&format!("Created: {name}"));
    }

    // ── Terminal ──────────────────────────────────────────────────────────

    pub fn spawn_terminal(&mut self, rows: u16, cols: u16) {
        if self.terminal.as_ref().map_or(false, |t| t.is_alive()) {
            return;
        }
        let session = config::load(&self.current_project);
        match Terminal::spawn(&session, rows, cols) {
            Ok(term) => {
                self.terminal = Some(term);
                self.mode = Mode::Terminal;
                self.scroll_offset = 0;
            }
            Err(e) => {
                self.set_status(&format!("Spawn failed: {e}"));
            }
        }
    }

    pub fn is_terminal_alive(&self) -> bool {
        self.terminal.as_ref().map_or(false, |t| t.is_alive())
    }

    pub fn check_terminal_alive(&mut self) {
        if self.terminal.as_ref().map_or(false, |t| !t.is_alive()) {
            self.terminal = None;
            self.mode = Mode::Normal;
            self.sidebar_level = SidebarLevel::Notes;
            self.notes_panel_open = false;
            self.notes_panel_focused = false;
            self.set_status("Terminal exited");
        }
    }

    pub fn resize_terminal(&mut self, total_cols: u16, total_rows: u16) {
        if let Some(ref mut term) = self.terminal {
            if !term.is_alive() {
                return;
            }
            let (rows, cols) = terminal_pty_size(total_rows, total_cols, self.notes_panel_open);
            let _ = term.resize(rows, cols);
        }
    }

    // ── Notes panel ───────────────────────────────────────────────────────

    /// Toggle notes panel open/closed. Focus is managed separately via ctrl+w.
    pub fn toggle_notes_panel(&mut self) {
        self.notes_panel_open = !self.notes_panel_open;
        self.notes_panel_focused = false;
    }

    /// Open notes panel and switch to Normal/Notes so user can pick a note.
    pub fn open_notes_browser(&mut self) {
        self.notes_panel_open = false;
        self.notes_panel_focused = false;
        self.mode = Mode::Normal;
        self.sidebar_level = SidebarLevel::Notes;
    }

    // ── Note helpers ──────────────────────────────────────────────────────

    pub fn selected_note(&self) -> Option<&Note> {
        self.notes.get(self.notes_selected)
    }

    pub fn focused_block_content(&self) -> Option<String> {
        self.selected_note()
            .and_then(|n| n.code_blocks.get(self.block_focused))
            .map(|b| b.content.clone())
    }

    pub fn execute_focused_block(&mut self) {
        if let Some(content) = self.focused_block_content() {
            self.execute_content(content);
        }
    }

    pub fn execute_block_by_index(&mut self, index: usize) {
        let content = self
            .selected_note()
            .and_then(|n| n.code_blocks.get(index))
            .map(|b| b.content.clone());
        if let Some(content) = content {
            self.block_focused = index;
            self.note_view_scroll = 0;
            self.execute_content(content);
        } else {
            self.set_status(&format!("No block [{index}]"));
        }
    }

    fn execute_content(&mut self, content: String) {
        let vars = extract_template_vars(&content);
        if vars.is_empty() {
            self.pending_exec = Some(content);
            self.mode = Mode::Terminal;
            self.notes_panel_open = true;
            self.notes_panel_focused = false;
        } else {
            let values = vec![String::new(); vars.len()];
            self.mode = Mode::TemplateInput {
                template: content,
                vars,
                values,
                current: 0,
            };
        }
    }

    pub fn submit_template_var(&mut self) {
        let (template, vars, values, current) = match &self.mode {
            Mode::TemplateInput { template, vars, values, current } => {
                if values[*current].is_empty() {
                    self.set_status("Value cannot be empty");
                    return;
                }
                (template.clone(), vars.clone(), values.clone(), *current)
            }
            _ => return,
        };

        if current + 1 < vars.len() {
            if let Mode::TemplateInput { current: ref mut c, .. } = self.mode {
                *c += 1;
            }
        } else {
            let mut result = template;
            for (var, val) in vars.iter().zip(values.iter()) {
                result = result.replace(&format!("<{var}>"), val);
            }
            self.pending_exec = Some(result);
            self.mode = Mode::Terminal;
            self.notes_panel_open = true;
            self.notes_panel_focused = false;
        }
    }

    pub fn submit_command_bar(&mut self) {
        if let Some(ref input) = self.command_bar.take() {
            let trimmed = input.trim();
            if let Ok(index) = trimmed.parse::<usize>() {
                self.execute_block_by_index(index);
            } else {
                self.set_status(&format!("Invalid block number: {trimmed}"));
            }
        }
        self.command_bar = None;
    }

    // ── Navigation ────────────────────────────────────────────────────────

    pub fn select_next_project(&mut self) {
        if !self.projects.is_empty() {
            self.project_selected = (self.project_selected + 1) % self.projects.len();
        }
    }

    pub fn select_prev_project(&mut self) {
        if !self.projects.is_empty() {
            self.project_selected = self
                .project_selected
                .checked_sub(1)
                .unwrap_or(self.projects.len() - 1);
        }
    }

    pub fn select_next_note(&mut self) {
        if !self.notes.is_empty() {
            self.notes_selected = (self.notes_selected + 1) % self.notes.len();
            self.block_focused = 0;
            self.note_view_scroll = 0;
        }
    }

    pub fn select_prev_note(&mut self) {
        if !self.notes.is_empty() {
            self.notes_selected = self
                .notes_selected
                .checked_sub(1)
                .unwrap_or(self.notes.len() - 1);
            self.block_focused = 0;
            self.note_view_scroll = 0;
        }
    }

    pub fn next_block(&mut self) {
        if let Some(note) = self.selected_note() {
            let count = note.code_blocks.len();
            if count > 0 {
                self.block_focused = (self.block_focused + 1) % count;
                self.note_view_scroll = 0;
            }
        }
    }

    pub fn prev_block(&mut self) {
        if let Some(note) = self.selected_note() {
            let count = note.code_blocks.len();
            if count > 0 {
                self.block_focused = self.block_focused.checked_sub(1).unwrap_or(count - 1);
                self.note_view_scroll = 0;
            }
        }
    }

    pub fn clamp_scroll(&mut self) {
        if let Some(ref term) = self.terminal {
            let max = term.scrollback_len() as u16;
            self.scroll_offset = self.scroll_offset.min(max);
        }
    }

    // ── Mouse ─────────────────────────────────────────────────────────────

    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        let col = mouse.column;
        let row = mouse.row;

        let over_notes = point_in(self.notes_panel_area, col, row);
        let over_terminal = point_in(self.terminal_area, col, row);

        match mouse.kind {
            MouseEventKind::ScrollUp => {
                if over_notes {
                    if self.notes_panel_focused {
                        self.note_view_scroll = self.note_view_scroll.saturating_sub(3);
                    } else {
                        self.select_prev_note();
                    }
                } else if over_terminal && self.mode == Mode::Terminal {
                    self.scroll_offset = self.scroll_offset.saturating_add(3);
                    self.clamp_scroll();
                }
            }
            MouseEventKind::ScrollDown => {
                if over_notes {
                    if self.notes_panel_focused {
                        self.note_view_scroll = self.note_view_scroll.saturating_add(3);
                    } else {
                        self.select_next_note();
                    }
                } else if over_terminal && self.mode == Mode::Terminal {
                    self.scroll_offset = self.scroll_offset.saturating_sub(3);
                }
            }
            _ => {}
        }
    }

    // ── Status ────────────────────────────────────────────────────────────

    pub fn set_status(&mut self, msg: &str) {
        self.status_msg = Some(msg.to_string());
        self.status_ttl = 40;
    }

    pub fn tick_status(&mut self) {
        if self.status_ttl > 0 {
            self.status_ttl -= 1;
            if self.status_ttl == 0 {
                self.status_msg = None;
            }
        }
    }
}

// ── Input handling ─────────────────────────────────────────────────────────

pub fn handle_key(app: &mut App, key: KeyEvent) -> InputResult {
    // Command bar intercepts all keys
    if app.command_bar.is_some() {
        match key.code {
            KeyCode::Esc => { app.command_bar = None; }
            KeyCode::Enter => { app.submit_command_bar(); }
            KeyCode::Backspace => {
                if let Some(ref mut buf) = app.command_bar {
                    buf.pop();
                    if buf.is_empty() { app.command_bar = None; }
                }
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                if let Some(ref mut buf) = app.command_bar {
                    buf.push(c);
                }
            }
            _ => {}
        }
        return InputResult::Consumed;
    }

    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    // ctrl+c: quit in Normal mode, send ^C to PTY in Terminal mode
    if ctrl && key.code == KeyCode::Char('c') {
        match app.mode {
            Mode::Terminal => return InputResult::ForwardToPty(vec![3]),
            _ => {
                app.should_quit = true;
                return InputResult::Consumed;
            }
        }
    }

    // ctrl+n: toggle notes panel open/closed; ctrl+w moves focus to/from it
    if ctrl && key.code == KeyCode::Char('n') {
        match app.mode {
            Mode::Terminal => {
                app.toggle_notes_panel();
                return InputResult::Consumed;
            }
            Mode::Normal if app.sidebar_level == SidebarLevel::Notes => {
                app.sidebar_level = SidebarLevel::NoteContent;
                return InputResult::Consumed;
            }
            _ => {}
        }
    }

    // ctrl+b: open notes browser from Terminal mode
    if ctrl && key.code == KeyCode::Char('b') && app.mode == Mode::Terminal {
        app.open_notes_browser();
        return InputResult::Consumed;
    }

    match &app.mode {
        Mode::Normal => handle_normal_key(app, key),
        Mode::Terminal => handle_terminal_key(app, key),
        Mode::TemplateInput { .. } => handle_template_input_key(app, key),
        Mode::NewProject => handle_text_input_key(app, key),
        Mode::ConfirmDelete => handle_confirm_delete_key(app, key),
    }
}

fn handle_normal_key(app: &mut App, key: KeyEvent) -> InputResult {
    match app.sidebar_level {
        SidebarLevel::Projects => handle_projects_key(app, key),
        SidebarLevel::Notes => handle_notes_key(app, key),
        SidebarLevel::NoteContent => handle_note_content_key(app, key),
    }
}

fn handle_projects_key(app: &mut App, key: KeyEvent) -> InputResult {
    match key.code {
        KeyCode::Char('q') => {
            app.should_quit = true;
            InputResult::Consumed
        }
        KeyCode::Esc => {
            if app.is_terminal_alive() {
                app.mode = Mode::Terminal;
            }
            InputResult::Consumed
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.select_prev_project();
            InputResult::Consumed
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.select_next_project();
            InputResult::Consumed
        }
        KeyCode::Enter | KeyCode::Right => {
            app.select_project();
            InputResult::Consumed
        }
        KeyCode::Char('n') => {
            app.input_buf.clear();
            app.mode = Mode::NewProject;
            InputResult::Consumed
        }
        _ => InputResult::None,
    }
}

fn handle_notes_key(app: &mut App, key: KeyEvent) -> InputResult {
    match key.code {
        KeyCode::Char('q') => {
            app.should_quit = true;
            InputResult::Consumed
        }
        KeyCode::Esc => {
            if app.is_terminal_alive() {
                app.mode = Mode::Terminal;
            } else {
                app.back_to_projects();
            }
            InputResult::Consumed
        }
        KeyCode::Left => {
            if app.is_terminal_alive() {
                app.browse_to_projects();
            } else {
                app.back_to_projects();
            }
            InputResult::Consumed
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.select_prev_note();
            InputResult::Consumed
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.select_next_note();
            InputResult::Consumed
        }
        KeyCode::Enter => {
            // Collapse sidebar, enter terminal mode with notes panel open
            if app.selected_note().is_some() {
                app.mode = Mode::Terminal;
                app.notes_panel_open = true;
                app.notes_panel_focused = false;
                app.block_focused = 0;
                app.note_view_scroll = 0;
            }
            InputResult::Consumed
        }
        KeyCode::Right | KeyCode::Char('l') => {
            if app.selected_note().is_some() {
                app.sidebar_level = SidebarLevel::NoteContent;
                app.block_focused = 0;
                app.note_view_scroll = 0;
            }
            InputResult::Consumed
        }
        KeyCode::Char('e') => InputResult::EditNote,
        KeyCode::Char('n') => InputResult::NewNote,
        KeyCode::Char('d') => {
            if app.selected_note().is_some() {
                app.mode = Mode::ConfirmDelete;
            }
            InputResult::Consumed
        }
        _ => InputResult::None,
    }
}

fn handle_confirm_delete_key(app: &mut App, key: KeyEvent) -> InputResult {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            app.mode = Mode::Normal;
            InputResult::DeleteNote
        }
        _ => {
            app.mode = Mode::Normal;
            InputResult::Consumed
        }
    }
}

fn handle_note_content_key(app: &mut App, key: KeyEvent) -> InputResult {
    match key.code {
        KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => {
            app.sidebar_level = SidebarLevel::Notes;
            InputResult::Consumed
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.prev_block();
            InputResult::Consumed
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.next_block();
            InputResult::Consumed
        }
        KeyCode::Tab => {
            app.next_block();
            InputResult::Consumed
        }
        KeyCode::BackTab => {
            app.prev_block();
            InputResult::Consumed
        }
        KeyCode::Enter => {
            // Execute block: collapses sidebar, enters Terminal mode, opens notes panel
            if let Some(note) = app.selected_note() {
                if !note.code_blocks.is_empty() {
                    app.execute_focused_block();
                }
            }
            InputResult::Consumed
        }
        KeyCode::Char('y') => InputResult::CopyBlock,
        KeyCode::Char('e') => InputResult::EditNote,
        KeyCode::PageUp => {
            app.note_view_scroll = app.note_view_scroll.saturating_sub(5);
            InputResult::Consumed
        }
        KeyCode::PageDown => {
            app.note_view_scroll = app.note_view_scroll.saturating_add(5);
            InputResult::Consumed
        }
        KeyCode::Char('/') => {
            app.command_bar = Some(String::new());
            InputResult::Consumed
        }
        _ => InputResult::None,
    }
}

fn handle_terminal_key(app: &mut App, key: KeyEvent) -> InputResult {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    // Shift+PageUp/Down for scrollback
    if key.modifiers.intersects(KeyModifiers::SHIFT | KeyModifiers::CONTROL) {
        if key.code == KeyCode::PageUp {
            app.scroll_offset = app.scroll_offset.saturating_add(5);
            app.clamp_scroll();
            return InputResult::Consumed;
        }
        if key.code == KeyCode::PageDown {
            app.scroll_offset = app.scroll_offset.saturating_sub(5);
            return InputResult::Consumed;
        }
    }

    // ctrl+w: toggle focus between terminal and notes panel (checked before routing)
    if ctrl && key.code == KeyCode::Char('w') && app.notes_panel_open {
        app.notes_panel_focused = !app.notes_panel_focused;
        return InputResult::Consumed;
    }

    // Route to notes panel navigation when notes panel is focused
    if app.notes_panel_open && app.notes_panel_focused {
        return handle_notes_panel_key(app, key);
    }

    app.scroll_offset = 0;
    let bytes = key_to_bytes(key);
    if bytes.is_empty() {
        InputResult::None
    } else {
        InputResult::ForwardToPty(bytes)
    }
}

fn handle_notes_panel_key(app: &mut App, key: KeyEvent) -> InputResult {
    match key.code {
        KeyCode::Esc => {
            app.notes_panel_focused = false;
            InputResult::Consumed
        }
        KeyCode::Left => {
            app.open_notes_browser();
            InputResult::Consumed
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.prev_block();
            InputResult::Consumed
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.next_block();
            InputResult::Consumed
        }
        KeyCode::Tab => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                app.prev_block();
            } else {
                app.next_block();
            }
            InputResult::Consumed
        }
        KeyCode::Enter => {
            app.execute_focused_block();
            InputResult::Consumed
        }
        KeyCode::Char('y') => InputResult::CopyBlock,
        KeyCode::Char('e') => InputResult::EditNote,
        KeyCode::PageUp => {
            app.note_view_scroll = app.note_view_scroll.saturating_sub(5);
            InputResult::Consumed
        }
        KeyCode::PageDown => {
            app.note_view_scroll = app.note_view_scroll.saturating_add(5);
            InputResult::Consumed
        }
        KeyCode::Char('/') => {
            app.command_bar = Some(String::new());
            InputResult::Consumed
        }
        _ => InputResult::Consumed, // Don't leak to PTY when notes focused
    }
}

fn handle_text_input_key(app: &mut App, key: KeyEvent) -> InputResult {
    match key.code {
        KeyCode::Esc => {
            app.input_buf.clear();
            app.mode = Mode::Normal;
            InputResult::Consumed
        }
        KeyCode::Enter => {
            app.create_project();
            InputResult::Consumed
        }
        KeyCode::Backspace => {
            app.input_buf.pop();
            InputResult::Consumed
        }
        KeyCode::Char(c) => {
            app.input_buf.push(c);
            InputResult::Consumed
        }
        _ => InputResult::None,
    }
}

fn handle_template_input_key(app: &mut App, key: KeyEvent) -> InputResult {
    match key.code {
        KeyCode::Esc => {
            app.mode = Mode::Terminal;
            app.set_status("Cancelled");
            InputResult::Consumed
        }
        KeyCode::Enter => {
            app.submit_template_var();
            InputResult::Consumed
        }
        KeyCode::Backspace => {
            if let Mode::TemplateInput { ref mut values, current, .. } = app.mode {
                values[current].pop();
            }
            InputResult::Consumed
        }
        KeyCode::Char(c) => {
            if let Mode::TemplateInput { ref mut values, current, .. } = app.mode {
                values[current].push(c);
            }
            InputResult::Consumed
        }
        _ => InputResult::Consumed,
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn load_notes_for(project: &str) -> Vec<Note> {
    load_notes_from_dir(&paths::notes_dir(project))
}

fn projects_index(projects: &[String], name: &str) -> usize {
    projects.iter().position(|p| p == name).unwrap_or(0)
}

fn point_in(area: Option<(u16, u16, u16, u16)>, col: u16, row: u16) -> bool {
    if let Some((x, y, w, h)) = area {
        col >= x && col < x + w && row >= y && row < y + h
    } else {
        false
    }
}

/// Calculate PTY dimensions given total terminal size and whether notes panel is open.
pub fn terminal_pty_size(total_rows: u16, total_cols: u16, notes_open: bool) -> (u16, u16) {
    let status_rows: u16 = 1;
    let notes_border: u16 = 2;
    let rows = total_rows.saturating_sub(status_rows).max(1);
    let cols = if notes_open {
        let panel_w = 42u16.min(total_cols.saturating_sub(20));
        total_cols.saturating_sub(panel_w + notes_border).max(1)
    } else {
        total_cols.max(1)
    };
    (rows, cols)
}

fn extract_template_vars(content: &str) -> Vec<String> {
    let mut vars = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut chars = content.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '<' {
            let var: String = chars.by_ref().take_while(|&c| c != '>').collect();
            if !var.is_empty()
                && var.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
                && seen.insert(var.clone())
            {
                vars.push(var);
            }
        }
    }
    vars
}

fn key_to_bytes(key: KeyEvent) -> Vec<u8> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char(c) if ctrl => {
            vec![(c as u8).wrapping_sub(b'a').wrapping_add(1)]
        }
        KeyCode::Char(c) => {
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            s.as_bytes().to_vec()
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![127],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::Esc => vec![27],
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::F(n) => f_key_bytes(n),
        _ => Vec::new(),
    }
}

fn f_key_bytes(n: u8) -> Vec<u8> {
    match n {
        1 => b"\x1bOP".to_vec(),
        2 => b"\x1bOQ".to_vec(),
        3 => b"\x1bOR".to_vec(),
        4 => b"\x1bOS".to_vec(),
        5 => b"\x1b[15~".to_vec(),
        6 => b"\x1b[17~".to_vec(),
        7 => b"\x1b[18~".to_vec(),
        8 => b"\x1b[19~".to_vec(),
        9 => b"\x1b[20~".to_vec(),
        10 => b"\x1b[21~".to_vec(),
        11 => b"\x1b[23~".to_vec(),
        12 => b"\x1b[24~".to_vec(),
        _ => Vec::new(),
    }
}
