use std::io::{self, Stderr};

use anyhow::Result;
use crossterm::{
    cursor::{Hide, Show},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::{Color, Line, Modifier, Span, Style},
    widgets::{Block, BorderType, Borders, Paragraph},
};

pub const BG: Color = Color::Rgb(8, 13, 10);
pub const PANEL: Color = Color::Rgb(13, 22, 16);
pub const PANEL_ALT: Color = Color::Rgb(18, 31, 21);
pub const ACCENT: Color = Color::Rgb(151, 214, 92);
pub const ACCENT_SOFT: Color = Color::Rgb(109, 160, 72);
pub const ACCENT_DIM: Color = Color::Rgb(71, 105, 53);
pub const TEXT: Color = Color::Rgb(232, 239, 221);
pub const MUTED: Color = Color::Rgb(134, 152, 129);
pub const WARNING: Color = Color::Rgb(255, 187, 71);
pub const DANGER: Color = Color::Rgb(255, 107, 107);

type Backend = CrosstermBackend<Stderr>;

pub struct TuiSession {
    terminal: Terminal<Backend>,
    restored: bool,
}

impl TuiSession {
    pub fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stderr = io::stderr();
        execute!(stderr, EnterAlternateScreen, Hide)?;
        let backend = CrosstermBackend::new(stderr);
        let terminal = Terminal::new(backend)?;
        Ok(Self {
            terminal,
            restored: false,
        })
    }

    pub fn draw<F>(&mut self, render: F) -> Result<()>
    where
        F: FnOnce(&mut Frame<'_>),
    {
        self.terminal.draw(render)?;
        Ok(())
    }

    pub fn restore(&mut self) -> Result<()> {
        if self.restored {
            return Ok(());
        }
        self.restored = true;
        disable_raw_mode()?;
        execute!(self.terminal.backend_mut(), Show, LeaveAlternateScreen)?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}

impl Drop for TuiSession {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

pub fn frame_layout(area: Rect) -> [Rect; 4] {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Min(16),
            Constraint::Length(5),
            Constraint::Length(2),
        ])
        .split(area);
    [areas[0], areas[1], areas[2], areas[3]]
}

pub fn split_main(area: Rect) -> [Rect; 2] {
    let areas = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Min(40)])
        .split(area);
    [areas[0], areas[1]]
}

pub fn page_background() -> Block<'static> {
    Block::default().style(Style::default().bg(BG))
}

pub fn card(title: impl Into<String>) -> Block<'static> {
    Block::default()
        .title(Line::from(Span::styled(
            title.into(),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(PANEL).fg(TEXT))
}

pub fn muted_card(title: impl Into<String>) -> Block<'static> {
    Block::default()
        .title(Line::from(Span::styled(
            title.into(),
            Style::default().fg(MUTED).add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(PANEL_ALT).fg(TEXT))
}

pub fn footer(text: impl Into<String>) -> Paragraph<'static> {
    Paragraph::new(text.into()).style(Style::default().fg(MUTED).bg(BG))
}

pub fn brand() -> Paragraph<'static> {
    Paragraph::new(vec![
        Line::from(vec![
            Span::styled("      ", Style::default().bg(BG)),
            Span::styled("▇", Style::default().fg(ACCENT)),
        ]),
        Line::from(vec![
            Span::styled("   ▅  ", Style::default().fg(ACCENT_DIM)),
            Span::styled("▇", Style::default().fg(ACCENT)),
        ]),
        Line::from(vec![
            Span::styled(" ▃ ▆ ", Style::default().fg(ACCENT_SOFT)),
            Span::styled("▇", Style::default().fg(ACCENT)),
            Span::raw("  "),
            Span::styled(
                "Anne",
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "al",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(" installer", Style::default().fg(MUTED)),
            Span::raw("  "),
            Span::styled("native setup", Style::default().fg(ACCENT_DIM)),
        ]),
    ])
    .style(Style::default().bg(BG))
}
