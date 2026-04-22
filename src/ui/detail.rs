use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::process::{
    ListenScope, ProcessInfo, RecommendedAction, RiskLevel, WorkspaceRelation,
};
pub fn render_detail(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title(" Inspect ");

    match app.selected_process() {
        Some(info) => render_process_detail(frame, info, area, block),
        None => {
            let placeholder = Paragraph::new(Span::styled(
                "  select a process",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::DIM),
            ))
            .block(block);
            frame.render_widget(placeholder, area);
        }
    }
}

pub fn render_inspect(frame: &mut Frame, info: &ProcessInfo) {
    let area = centered_rect(78, 80, frame.area());
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" Process Inspect ");
    render_process_detail(frame, info, area, block);
}

fn render_process_detail(frame: &mut Frame, info: &ProcessInfo, area: Rect, block: Block<'static>) {
    let section = |title: &str| -> Line<'static> {
        Line::from(Span::styled(
            format!("── {title} ──"),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ))
    };

    let label = |name: &str| -> Span<'static> {
        Span::styled(
            format!("{name:<10}"),
            Style::default().add_modifier(Modifier::BOLD),
        )
    };

    let uptime_str = format_uptime(info.start_time);
    let parent_pid = info
        .parent_pid
        .map(|pid| pid.to_string())
        .unwrap_or_else(|| "–".to_string());
    let project_type = if info.project_type.is_empty() {
        "Unknown".to_string()
    } else {
        info.project_type.clone()
    };
    let project_root = if info.project_root.is_empty() {
        "–".to_string()
    } else {
        info.project_root.clone()
    };
    let exe_path = if info.exe_path.is_empty() {
        "–".to_string()
    } else {
        info.exe_path.clone()
    };
    let cwd = if info.cwd.is_empty() {
        "–".to_string()
    } else {
        info.cwd.clone()
    };
    let parent_cmd = if info.parent_command.is_empty() {
        "–".to_string()
    } else {
        info.parent_command.clone()
    };
    let origin_summary = if info.origin_summary.is_empty() {
        "–".to_string()
    } else {
        info.origin_summary.clone()
    };

    let lines = vec![
        section("Network"),
        Line::from(vec![label("Port"), Span::styled(info.port.to_string(), Style::default().fg(Color::Cyan))]),
        Line::from(vec![label("Bind"), Span::raw(info.local_addr.clone())]),
        Line::from(vec![
            label("Scope"),
            Span::styled(info.listen_scope.label(), Style::default().fg(scope_color(info.listen_scope))),
        ]),
        Line::from(vec![
            label("Risk"),
            Span::styled(info.risk_level.label(), Style::default().fg(risk_color(info.risk_level))),
        ]),
        Line::from(vec![
            label("Proto"),
            Span::styled(info.proto.clone(), Style::default().fg(proto_color(info))),
        ]),
        Line::from(""),
        section("Identity"),
        Line::from(vec![label("App"), Span::raw(info.app.clone())]),
        Line::from(vec![
            label("Type"),
            Span::styled(info.inferred_kind.clone(), Style::default().fg(Color::Magenta)),
        ]),
        Line::from(vec![
            label("Action"),
            Span::styled(
                info.recommended_action.label(),
                Style::default().fg(action_color(info.recommended_action)),
            ),
        ]),
        Line::from(vec![label("PID"), Span::raw(info.pid.to_string())]),
        Line::from(vec![label("User"), Span::raw(info.username.clone())]),
        Line::from(vec![label("Parent"), Span::raw(parent_pid)]),
        Line::from(""),
        section("Provenance"),
        Line::from(vec![label("Project"), Span::raw(project_type)]),
        Line::from(vec![
            label("Relation"),
            Span::styled(
                info.workspace_relation.label(),
                Style::default().fg(relation_color(info.workspace_relation)),
            ),
        ]),
        Line::from(vec![label("Origin"), Span::raw(origin_summary)]),
        Line::from(vec![label("Root"), Span::raw(project_root)]),
        Line::from(vec![label("CWD"), Span::raw(cwd)]),
        Line::from(vec![label("Binary"), Span::raw(exe_path)]),
        Line::from(vec![label("ParentCmd"), Span::raw(parent_cmd)]),
        Line::from(""),
        section("Resources"),
        Line::from(vec![label("Status"), Span::raw(info.status.clone())]),
        Line::from(vec![label("Uptime"), Span::raw(uptime_str)]),
        Line::from(vec![label("CPU"), styled_cpu(info.cpu)]),
        Line::from(vec![label("Memory"), styled_mem(info.memory_mb)]),
        Line::from(""),
        section("Guidance"),
        Line::from(Span::raw(info.guidance.clone())),
        Line::from(""),
        section("Command"),
        Line::from(Span::raw(info.command.clone())),
    ];

    let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn proto_color(info: &ProcessInfo) -> Color {
    if info.proto.contains('6') {
        Color::Magenta
    } else {
        Color::Green
    }
}

fn scope_color(scope: ListenScope) -> Color {
    match scope {
        ListenScope::Localhost => Color::Green,
        ListenScope::PrivateNetwork => Color::Yellow,
        ListenScope::Public => Color::Red,
        ListenScope::Unknown => Color::DarkGray,
    }
}

fn risk_color(risk: RiskLevel) -> Color {
    match risk {
        RiskLevel::Low => Color::Green,
        RiskLevel::Medium => Color::Yellow,
        RiskLevel::High => Color::Red,
    }
}

fn action_color(action: RecommendedAction) -> Color {
    match action {
        RecommendedAction::Keep => Color::Green,
        RecommendedAction::Inspect => Color::Yellow,
        RecommendedAction::CloseIfUnused => Color::Red,
    }
}

fn relation_color(relation: WorkspaceRelation) -> Color {
    match relation {
        WorkspaceRelation::CurrentWorkspace => Color::Green,
        WorkspaceRelation::ExternalProject => Color::Magenta,
        WorkspaceRelation::AppBundle => Color::Cyan,
        WorkspaceRelation::SystemPath => Color::Blue,
        WorkspaceRelation::ExternalBinary => Color::Yellow,
        WorkspaceRelation::Unknown => Color::DarkGray,
    }
}

fn styled_cpu(cpu: f32) -> Span<'static> {
    let color = if cpu > 80.0 {
        Color::Red
    } else if cpu > 50.0 {
        Color::Yellow
    } else {
        Color::Green
    };
    Span::styled(format!("{cpu:.1}%"), Style::default().fg(color))
}

fn styled_mem(mb: f64) -> Span<'static> {
    let color = if mb > 1024.0 {
        Color::Red
    } else if mb > 500.0 {
        Color::Yellow
    } else {
        Color::Green
    };
    Span::styled(format!("{mb:.1} MB"), Style::default().fg(color))
}

fn format_uptime(start_epoch: u64) -> String {
    if start_epoch == 0 {
        return "–".to_string();
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if now <= start_epoch {
        return "just started".to_string();
    }
    let secs = now - start_epoch;
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else if secs < 86400 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d {}h", secs / 86400, (secs % 86400) / 3600)
    }
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
