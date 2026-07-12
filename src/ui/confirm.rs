use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::process::ProcessInfo;

/// Single-kill confirm dialog.
pub fn render_confirm(frame: &mut Frame, info: &ProcessInfo) {
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

    let title = " Confirm Kill ";
    let area = centered_for_content(&lines, title, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .title(title);

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Multi-kill confirm dialog.
pub fn render_confirm_multi(frame: &mut Frame, targets: &[ProcessInfo]) {
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

    let title = " Confirm Multi Kill ";
    let area = centered_for_content(&lines, title, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .title(title);

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Center a box sized to its content (plus borders), clamped to the terminal.
fn centered_for_content(lines: &[Line], title: &str, area: Rect) -> Rect {
    let content_width = lines
        .iter()
        .map(Line::width)
        .max()
        .unwrap_or(0)
        .max(title.chars().count());
    let width = (content_width as u16 + 4).min(area.width);
    let height = (lines.len() as u16 + 2).min(area.height);
    let x = area.x + (area.width - width) / 2;
    let y = area.y + (area.height - height) / 2;
    Rect::new(x, y, width, height)
}

#[cfg(test)]
mod tests {
    use super::{render_confirm, render_confirm_multi};
    use crate::process::{
        ListenScope, ProcessInfo, RecommendedAction, RiskLevel, WorkspaceRelation,
    };
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn sample_process(pid: u32, port: u16, app_name: &str) -> ProcessInfo {
        ProcessInfo {
            port,
            pid,
            proto: "IPv4".to_string(),
            local_addr: "127.0.0.1".to_string(),
            app: app_name.to_string(),
            command: format!("/tmp/{app_name}"),
            cpu: 0.0,
            memory_mb: 10.0,
            status: "Run".to_string(),
            username: "tester".to_string(),
            start_time: 1,
            listen_scope: ListenScope::Localhost,
            risk_level: RiskLevel::Low,
            inferred_kind: "Go service".to_string(),
            project_type: "Go".to_string(),
            project_root: "/tmp/project".to_string(),
            exe_path: format!("/tmp/{app_name}"),
            cwd: "/tmp/project".to_string(),
            parent_pid: Some(1),
            parent_command: "launchd".to_string(),
            guidance: "safe".to_string(),
            recommended_action: RecommendedAction::Keep,
            workspace_relation: WorkspaceRelation::CurrentWorkspace,
            origin_summary: "Go workspace: /tmp/project".to_string(),
        }
    }

    fn render_single(width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        let info = sample_process(4242, 3000, "node");
        terminal
            .draw(|frame| render_confirm(frame, &info))
            .unwrap();
        buffer_text(&terminal)
    }

    fn render_multi(width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        let targets = vec![
            sample_process(1001, 3000, "alpha"),
            sample_process(1002, 3001, "beta"),
            sample_process(1003, 3002, "gamma"),
        ];
        terminal
            .draw(|frame| render_confirm_multi(frame, &targets))
            .unwrap();
        buffer_text(&terminal)
    }

    fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn single_confirm_shows_full_content_on_wide_short_terminal() {
        let text = render_single(200, 17);
        assert!(text.contains("Kill this process?"), "missing question");
        assert!(text.contains("App:"), "missing app detail");
        assert!(text.contains("4242"), "missing pid");
        assert!(text.contains("[Y]"), "missing kill/cancel keys");
    }

    #[test]
    fn multi_confirm_shows_full_content_on_wide_short_terminal() {
        let text = render_multi(200, 17);
        assert!(text.contains("Kill 3 process(es)?"), "missing question");
        assert!(text.contains("alpha"), "missing first target");
        assert!(text.contains("gamma"), "missing last target");
        assert!(text.contains("[Y]"), "missing kill/cancel keys");
    }

    #[test]
    fn single_confirm_still_complete_on_standard_terminal() {
        let text = render_single(80, 24);
        assert!(text.contains("Kill this process?"));
        assert!(text.contains("[Y]"));
    }
}
