use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::process::ProcessInfo;

/// Single-kill confirm dialog.
pub fn render_confirm(frame: &mut Frame, info: &ProcessInfo) {
    let area = centered_rect(50, 30, frame.area());
    frame.render_widget(Clear, area);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            " Kill this process? ",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  App:  ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(&info.app),
        ]),
        Line::from(vec![
            Span::styled("  PID:  ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(info.pid.to_string()),
        ]),
        Line::from(vec![
            Span::styled("  Port: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(info.port.to_string(), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [Y]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw(" kill   "),
            Span::styled("[N]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" cancel"),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .title(" Confirm Kill ");

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Multi-kill confirm dialog.
pub fn render_confirm_multi(frame: &mut Frame, targets: &[ProcessInfo]) {
    let area = centered_rect(55, 50, frame.area());
    frame.render_widget(Clear, area);

    // Collect unique PIDs for count
    let mut unique_pids: Vec<u32> = targets.iter().map(|p| p.pid).collect();
    unique_pids.sort();
    unique_pids.dedup();

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!(" Kill {} process(es)? ({} entries) ", unique_pids.len(), targets.len()),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    // Show up to 8 entries, then "...and N more"
    let max_show = 8;
    for (i, info) in targets.iter().enumerate() {
        if i >= max_show {
            lines.push(Line::from(Span::styled(
                format!("  ...and {} more", targets.len() - max_show),
                Style::default().fg(Color::DarkGray),
            )));
            break;
        }
        lines.push(Line::from(vec![
            Span::styled("  ● ", Style::default().fg(Color::Red)),
            Span::styled(format!(":{:<6}", info.port), Style::default().fg(Color::Cyan)),
            Span::raw(format!(" pid={:<7} ", info.pid)),
            Span::styled(&info.app, Style::default().add_modifier(Modifier::BOLD)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  [Y]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::raw(" kill all   "),
        Span::styled("[N]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(" cancel"),
    ]));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .title(" Confirm Multi Kill ");

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
