mod confirm;
mod detail;
mod help;
mod table;

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{App, AppState};

pub fn render(frame: &mut Frame, app: &App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(frame.area());

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(outer[0]);

    table::render_table(frame, app, main_chunks[0]);
    detail::render_detail(frame, app, main_chunks[1]);
    render_footer(frame, app, outer[1]);

    // Overlays
    match &app.state {
        AppState::Confirm(ref info) => confirm::render_confirm(frame, info),
        AppState::ConfirmMulti(ref targets) => confirm::render_confirm_multi(frame, targets),
        AppState::Help => help::render_help(frame),
        _ => {}
    }
}

fn render_footer(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let elapsed = app.last_refresh.elapsed().as_secs();

    let mode_span = match &app.state {
        AppState::FilterInput => Span::styled(
            format!(" /{}▏", app.filter),
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
    } else if let Some(ref err) = app.last_error {
        Span::styled(
            format!(" ERR: {} ", truncate_str(err, 30)),
            Style::default().fg(Color::Red),
        )
    } else {
        Span::raw("")
    };

    let help = Span::styled(" ? help ", Style::default().fg(Color::DarkGray));

    let line = Line::from(vec![mode_span, sort_span, mark_span, refresh_span, msg_span, help]);
    let bar = Paragraph::new(line).style(Style::default().bg(Color::Rgb(30, 30, 30)));
    frame.render_widget(bar, area);
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}…", &s[..max.saturating_sub(1)])
    } else {
        s.to_string()
    }
}
