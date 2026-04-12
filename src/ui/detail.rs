use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::App;

pub fn render_detail(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Details ");

    match app.selected_process() {
        Some(info) => {
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

            // Uptime calculation
            let uptime_str = format_uptime(info.start_time);

            let lines = vec![
                section("Network"),
                Line::from(vec![
                    label("Port"),
                    Span::styled(info.port.to_string(), Style::default().fg(Color::Cyan)),
                ]),
                Line::from(vec![
                    label("Proto"),
                    Span::styled(
                        info.proto.clone(),
                        Style::default().fg(if info.proto.contains('6') {
                            Color::Magenta
                        } else {
                            Color::Green
                        }),
                    ),
                ]),
                Line::from(vec![
                    label("Address"),
                    Span::raw(info.local_addr.clone()),
                ]),
                Line::from(""),
                section("Process"),
                Line::from(vec![label("PID"), Span::raw(info.pid.to_string())]),
                Line::from(vec![label("App"), Span::raw(info.app.clone())]),
                Line::from(vec![label("Status"), Span::raw(info.status.clone())]),
                Line::from(vec![label("User"), Span::raw(info.username.clone())]),
                Line::from(vec![label("Uptime"), Span::raw(uptime_str)]),
                Line::from(""),
                section("Resources"),
                Line::from(vec![
                    label("CPU"),
                    styled_cpu(info.cpu),
                ]),
                Line::from(vec![
                    label("Memory"),
                    styled_mem(info.memory_mb),
                ]),
                Line::from(""),
                section("Command"),
                Line::from(Span::raw(info.command.clone())),
            ];

            let paragraph = Paragraph::new(lines).block(block).wrap(ratatui::widgets::Wrap { trim: false });
            frame.render_widget(paragraph, area);
        }
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
