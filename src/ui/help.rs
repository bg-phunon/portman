use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

pub fn render_help(frame: &mut Frame) {
    let area = centered_rect(60, 80, frame.area());
    frame.render_widget(Clear, area);

    let key = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(Color::DarkGray);

    let lines = vec![
        Line::from(Span::styled(
            " Keybindings",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled("── Navigation ──", dim)),
        Line::from(vec![Span::styled("  j / ↓       ", key), Span::raw("Move down")]),
        Line::from(vec![Span::styled("  k / ↑       ", key), Span::raw("Move up")]),
        Line::from(vec![Span::styled("  g           ", key), Span::raw("Go to top")]),
        Line::from(vec![Span::styled("  G           ", key), Span::raw("Go to bottom")]),
        Line::from(vec![Span::styled("  PgUp/PgDn   ", key), Span::raw("Scroll by page")]),
        Line::from(""),
        Line::from(Span::styled("── Select & Kill ──", dim)),
        Line::from(vec![Span::styled("  Space       ", key), Span::raw("Toggle mark on row")]),
        Line::from(vec![Span::styled("  m           ", key), Span::raw("Mark all visible")]),
        Line::from(vec![Span::styled("  M           ", key), Span::raw("Unmark all")]),
        Line::from(vec![Span::styled("  K           ", key), Span::raw("Kill marked (or selected)")]),
        Line::from(""),
        Line::from(Span::styled("── Clipboard ──", dim)),
        Line::from(vec![Span::styled("  y           ", key), Span::raw("Copy port")]),
        Line::from(vec![Span::styled("  Y           ", key), Span::raw("Copy kill command")]),
        Line::from(""),
        Line::from(Span::styled("── Filter & Sort ──", dim)),
        Line::from(vec![Span::styled("  /           ", key), Span::raw("Search / filter")]),
        Line::from(vec![Span::styled("  Esc         ", key), Span::raw("Clear filter")]),
        Line::from(vec![Span::styled("  Tab         ", key), Span::raw("Cycle sort column")]),
        Line::from(vec![Span::styled("  1-8         ", key), Span::raw("Sort by column N")]),
        Line::from(vec![Span::styled("  S           ", key), Span::raw("Reverse sort direction")]),
        Line::from(""),
        Line::from(Span::styled("── General ──", dim)),
        Line::from(vec![Span::styled("  r           ", key), Span::raw("Refresh")]),
        Line::from(vec![Span::styled("  Enter / i   ", key), Span::raw("Open full inspect view")]),
        Line::from(vec![Span::styled("  ?           ", key), Span::raw("Toggle this help")]),
        Line::from(vec![Span::styled("  q / Ctrl+C  ", key), Span::raw("Quit")]),
        Line::from(""),
        Line::from(Span::styled("  Press ? or Esc to close", dim)),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" Help ");

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
