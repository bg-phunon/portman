mod confirm;
mod detail;
mod help;
mod table;
mod text;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{App, AppState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    Compact,
    Standard,
    Wide,
}

impl LayoutMode {
    pub fn from_width(width: u16) -> Self {
        if width < 110 {
            Self::Compact
        } else if width < 160 {
            Self::Standard
        } else {
            Self::Wide
        }
    }

    pub fn shows_side_panel(self) -> bool {
        matches!(self, Self::Standard | Self::Wide)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Standard => "standard",
            Self::Wide => "wide",
        }
    }
}

pub fn render(frame: &mut Frame, app: &App) {
    let layout_mode = app.layout_mode;
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(frame.area());

    if layout_mode.shows_side_panel() {
        let main_chunks = split_main(outer[0], layout_mode);
        table::render_table(frame, app, main_chunks[0], layout_mode);
        detail::render_detail(frame, app, main_chunks[1]);
    } else {
        table::render_table(frame, app, outer[0], layout_mode);
    }
    render_footer(frame, app, outer[1], layout_mode);

    // Overlays
    match &app.state {
        AppState::Inspect(ref info) => detail::render_inspect(frame, info),
        AppState::Confirm(ref info) => confirm::render_confirm(frame, info),
        AppState::ConfirmMulti(ref targets) => confirm::render_confirm_multi(frame, targets),
        AppState::Help => help::render_help(frame),
        _ => {}
    }
}

fn split_main(area: Rect, layout_mode: LayoutMode) -> Vec<Rect> {
    let constraints = match layout_mode {
        LayoutMode::Standard => [Constraint::Percentage(58), Constraint::Percentage(42)],
        LayoutMode::Wide => [Constraint::Percentage(55), Constraint::Percentage(45)],
        LayoutMode::Compact => [Constraint::Percentage(100), Constraint::Percentage(0)],
    };

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area)
        .to_vec()
}

fn render_footer(frame: &mut Frame, app: &App, area: Rect, layout_mode: LayoutMode) {
    let elapsed = app.last_refresh.elapsed().as_secs();

    let mode_span = match &app.state {
        AppState::FilterInput => Span::styled(
            format!(" /{}▏", app.filter),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
        AppState::Inspect(_) => Span::styled(
            " INSPECT ",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
        _ => {
            if app.filter.is_empty() {
                Span::styled(" NORMAL ", Style::default().fg(Color::DarkGray))
            } else {
                Span::styled(
                    format!(" filter: {} ", app.filter),
                    Style::default().fg(Color::Yellow),
                )
            }
        }
    };

    let sort_span = Span::styled(
        format!(" sort: {} {} ", app.sort_col.label(), app.sort_dir.arrow()),
        Style::default().fg(Color::Cyan),
    );

    let layout_span = Span::styled(
        format!(" layout: {} ", layout_mode.label()),
        Style::default().fg(Color::DarkGray),
    );

    // Mark count
    let mark_span = if app.marked.is_empty() {
        Span::raw("")
    } else {
        Span::styled(
            format!(" ● {} marked ", app.marked.len()),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )
    };

    let refresh_span = Span::styled(
        format!(" {elapsed}s ago "),
        Style::default().fg(Color::DarkGray),
    );

    let msg_span = if let Some(msg) = app.active_message() {
        Span::styled(
            format!(" {msg} "),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )
    } else if let Some(msg) = app.update_message() {
        Span::styled(
            format!(" {msg} "),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )
    } else if let Some(ref err) = app.last_error {
        Span::styled(
            format!(" ERR: {} ", text::truncate_chars(err, 30)),
            Style::default().fg(Color::Red),
        )
    } else {
        Span::raw("")
    };

    let help = if layout_mode.shows_side_panel() {
        Span::styled(" u dismiss  ? help ", Style::default().fg(Color::DarkGray))
    } else {
        Span::styled(
            " Enter inspect  u dismiss  ? help ",
            Style::default().fg(Color::DarkGray),
        )
    };

    let line = Line::from(vec![
        mode_span,
        sort_span,
        layout_span,
        mark_span,
        refresh_span,
        msg_span,
        help,
    ]);
    let bar = Paragraph::new(line).style(Style::default().bg(Color::Rgb(30, 30, 30)));
    frame.render_widget(bar, area);
}
