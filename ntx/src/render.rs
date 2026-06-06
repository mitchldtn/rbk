use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

pub fn vt100_color(color: vt100::Color) -> Color {
    match color {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(idx) => Color::Indexed(idx),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

pub fn render_row(screen: &vt100::Screen, row: u16, visible_cols: u16) -> Line<'static> {
    let spans = (0..visible_cols).fold(Vec::<Span<'static>>::new(), |mut spans, col| {
        let cell = match screen.cell(row, col) {
            Some(c) => c,
            None => return spans,
        };
        let content: String = if cell.contents().is_empty() {
            " ".to_string()
        } else {
            cell.contents().to_string()
        };
        let fg = vt100_color(cell.fgcolor());
        let bg = vt100_color(cell.bgcolor());
        let mut mods = Modifier::empty();
        if cell.bold() { mods |= Modifier::BOLD; }
        if cell.italic() { mods |= Modifier::ITALIC; }
        if cell.underline() { mods |= Modifier::UNDERLINED; }
        if cell.inverse() { mods |= Modifier::REVERSED; }
        let style = Style::default().fg(fg).bg(bg).add_modifier(mods);
        if let Some(last) = spans.last_mut() {
            if last.style == style {
                last.content = (last.content.to_string() + &content).into();
                return spans;
            }
        }
        spans.push(Span::styled(content, style));
        spans
    });
    Line::from(spans)
}

pub fn render_vt100_screen(
    screen: &vt100::Screen,
    area_rows: u16,
    area_cols: u16,
) -> (Vec<Line<'static>>, Option<(u16, u16)>) {
    let lines: Vec<Line<'static>> = (0..area_rows)
        .map(|row| render_row(screen, row, area_cols))
        .collect();
    let cursor = cursor_position(screen, area_rows, area_cols);
    (lines, cursor)
}

pub fn cursor_position(
    screen: &vt100::Screen,
    area_rows: u16,
    area_cols: u16,
) -> Option<(u16, u16)> {
    if screen.hide_cursor() {
        return None;
    }
    let (crow, ccol) = screen.cursor_position();
    if crow < area_rows && ccol < area_cols {
        Some((ccol, crow))
    } else {
        None
    }
}
