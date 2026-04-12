use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Row, Table, TableState};
use ratatui::Frame;

use crate::app::{App, SortColumn};
use crate::process::ProcessInfo;

const COLUMNS: [SortColumn; 8] = SortColumn::ALL;

pub fn render_table(frame: &mut Frame, app: &App, area: Rect) {
    let filtered = app.filtered_processes();

    // Build header — first column is the mark indicator "●"
    let mut header_cells: Vec<Span> = vec![Span::styled(
        "●",
        Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
    )];

    header_cells.extend(COLUMNS.iter().map(|&col| {
        let is_active = col == app.sort_col;
        let label = if is_active {
            format!("{} {}", col.label(), app.sort_dir.arrow())
        } else {
            col.label().to_string()
        };

        let base_color = match col {
            SortColumn::Proto | SortColumn::Addr => Color::DarkGray,
            SortColumn::Port => Color::Cyan,
            _ => Color::Reset,
        };

        let style = if is_active {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default()
                .fg(base_color)
                .add_modifier(Modifier::BOLD)
        };

        Span::styled(label, style)
    }));

    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows: Vec<Row> = filtered.iter().map(|p| build_row(p, app.is_marked(p))).collect();

    let filter_hint = if app.filter.is_empty() {
        String::new()
    } else {
        format!("  filter: \"{}\"", app.filter)
    };

    let mark_hint = if app.marked.is_empty() {
        String::new()
    } else {
        format!("  marked: {}", app.marked.len())
    };

    let title = format!(
        " Ports ({}/{}){filter_hint}{mark_hint} ",
        filtered.len(),
        app.processes.len(),
    );

    let widths = [
        Constraint::Length(2),  // mark ●
        Constraint::Length(6),  // PROTO
        Constraint::Length(7),  // PORT
        Constraint::Length(15), // LOCAL ADDR
        Constraint::Length(7),  // PID
        Constraint::Length(10), // USER
        Constraint::Length(14), // APP
        Constraint::Length(7),  // CPU%
        Constraint::Length(9),  // MEM(MB)
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(title))
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let mut state = TableState::default();
    if !filtered.is_empty() {
        state.select(Some(app.selected));
    }

    frame.render_stateful_widget(table, area, &mut state);
}

fn build_row(p: &ProcessInfo, is_marked: bool) -> Row<'static> {
    let mark_span = if is_marked {
        Span::styled("●", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
    } else {
        Span::raw(" ")
    };

    let proto_color = if p.proto.contains('6') {
        Color::Magenta
    } else {
        Color::Green
    };

    let cpu_color = if p.cpu > 80.0 {
        Color::Red
    } else if p.cpu > 50.0 {
        Color::Yellow
    } else {
        Color::Reset
    };

    let mem_color = if p.memory_mb > 1024.0 {
        Color::Red
    } else if p.memory_mb > 500.0 {
        Color::Yellow
    } else {
        Color::Reset
    };

    Row::new(vec![
        mark_span,
        Span::styled(p.proto.clone(), Style::default().fg(proto_color)),
        Span::styled(p.port.to_string(), Style::default().fg(Color::Cyan)),
        Span::styled(truncate_owned(&p.local_addr, 15), Style::default().fg(Color::DarkGray)),
        Span::raw(p.pid.to_string()),
        Span::raw(truncate_owned(&p.username, 10)),
        Span::styled(truncate_owned(&p.app, 14), Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(format!("{:.1}", p.cpu), Style::default().fg(cpu_color)),
        Span::styled(format!("{:.1}", p.memory_mb), Style::default().fg(mem_color)),
    ])
}

fn truncate_owned(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}…", &s[..max.saturating_sub(1)])
    } else {
        s.to_string()
    }
}
