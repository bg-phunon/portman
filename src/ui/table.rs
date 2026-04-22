use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Row, Table, TableState};
use ratatui::Frame;

use crate::app::{App, SortColumn};
use crate::process::{ListenScope, ProcessInfo, RiskLevel};
use crate::ui::text::truncate_chars;
use crate::ui::LayoutMode;

#[derive(Clone, Copy)]
enum DisplayColumn {
    Mark,
    Proto,
    Port,
    Scope,
    App,
    Kind,
    Action,
    Risk,
    Pid,
    Cpu,
    Mem,
}

pub fn render_table(frame: &mut Frame, app: &App, area: Rect, layout_mode: LayoutMode) {
    let filtered = app.filtered();
    let columns = columns_for(layout_mode);
    let header = Row::new(
        columns
            .iter()
            .map(|&col| build_header_cell(col, app))
            .collect::<Vec<_>>(),
    )
    .height(1)
    .bottom_margin(1);

    let rows: Vec<Row> = filtered
        .iter()
        .map(|p| build_row(p, app.is_marked(p), &columns))
        .collect();

    let filter_hint = if app.filter.is_empty() {
        String::new()
    } else {
        format!("  filter: \"{}\"", app.filter)
    };

    let hint = if layout_mode.shows_side_panel() {
        String::new()
    } else {
        "  Enter inspect".to_string()
    };

    let title = format!(
        " Ports ({}/{}){filter_hint}{hint} ",
        filtered.len(),
        app.processes.len(),
    );

    let table = Table::new(rows, widths_for(layout_mode))
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(title))
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let mut state = TableState::default();
    if !filtered.is_empty() {
        state.select(Some(app.selected));
    }

    frame.render_stateful_widget(table, area, &mut state);
}

fn columns_for(layout_mode: LayoutMode) -> Vec<DisplayColumn> {
    match layout_mode {
        LayoutMode::Compact => vec![
            DisplayColumn::Mark,
            DisplayColumn::Port,
            DisplayColumn::App,
            DisplayColumn::Scope,
            DisplayColumn::Action,
        ],
        LayoutMode::Standard => vec![
            DisplayColumn::Mark,
            DisplayColumn::Port,
            DisplayColumn::Scope,
            DisplayColumn::App,
            DisplayColumn::Kind,
            DisplayColumn::Action,
            DisplayColumn::Pid,
            DisplayColumn::Cpu,
        ],
        LayoutMode::Wide => vec![
            DisplayColumn::Mark,
            DisplayColumn::Proto,
            DisplayColumn::Port,
            DisplayColumn::Scope,
            DisplayColumn::App,
            DisplayColumn::Kind,
            DisplayColumn::Action,
            DisplayColumn::Risk,
            DisplayColumn::Pid,
            DisplayColumn::Mem,
        ],
    }
}

fn widths_for(layout_mode: LayoutMode) -> Vec<Constraint> {
    match layout_mode {
        LayoutMode::Compact => vec![
            Constraint::Length(2),
            Constraint::Length(7),
            Constraint::Min(16),
            Constraint::Length(7),
            Constraint::Length(7),
        ],
        LayoutMode::Standard => vec![
            Constraint::Length(2),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Min(14),
            Constraint::Min(14),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(7),
        ],
        LayoutMode::Wide => vec![
            Constraint::Length(2),
            Constraint::Length(6),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Min(14),
            Constraint::Min(14),
            Constraint::Length(7),
            Constraint::Length(6),
            Constraint::Length(7),
            Constraint::Length(6),
            Constraint::Length(9),
        ],
    }
}

fn build_header_cell(col: DisplayColumn, app: &App) -> Span<'static> {
    if matches!(col, DisplayColumn::Mark) {
        return Span::styled(
            "●",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
        );
    }

    let label = display_label(col);
    let style = if let Some(sort_col) = sort_column_for_display(col) {
        let is_active = sort_col == app.sort_col;
        let label = if is_active {
            format!("{label} {}", app.sort_dir.arrow())
        } else {
            label.to_string()
        };
        let base_color = if matches!(col, DisplayColumn::Port) {
            Color::Cyan
        } else if matches!(col, DisplayColumn::Proto) {
            Color::DarkGray
        } else {
            Color::Reset
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
        return Span::styled(label, style);
    } else if matches!(col, DisplayColumn::Scope | DisplayColumn::Risk) {
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    };

    Span::styled(label, style)
}

fn display_label(col: DisplayColumn) -> &'static str {
    match col {
        DisplayColumn::Mark => "●",
        DisplayColumn::Proto => "PROTO",
        DisplayColumn::Port => "PORT",
        DisplayColumn::Scope => "SCOPE",
        DisplayColumn::App => "APP",
        DisplayColumn::Kind => "TYPE",
        DisplayColumn::Action => "ACT",
        DisplayColumn::Risk => "RISK",
        DisplayColumn::Pid => "PID",
        DisplayColumn::Cpu => "CPU%",
        DisplayColumn::Mem => "MEM",
    }
}

fn sort_column_for_display(col: DisplayColumn) -> Option<SortColumn> {
    match col {
        DisplayColumn::Proto => Some(SortColumn::Proto),
        DisplayColumn::Port => Some(SortColumn::Port),
        DisplayColumn::Pid => Some(SortColumn::Pid),
        DisplayColumn::App => Some(SortColumn::App),
        DisplayColumn::Cpu => Some(SortColumn::Cpu),
        DisplayColumn::Mem => Some(SortColumn::Mem),
        DisplayColumn::Mark
        | DisplayColumn::Scope
        | DisplayColumn::Kind
        | DisplayColumn::Action
        | DisplayColumn::Risk => None,
    }
}

fn build_row(p: &ProcessInfo, is_marked: bool, columns: &[DisplayColumn]) -> Row<'static> {
    Row::new(
        columns
            .iter()
            .map(|&column| build_cell(column, p, is_marked))
            .collect::<Vec<_>>(),
    )
}

fn build_cell(column: DisplayColumn, p: &ProcessInfo, is_marked: bool) -> Span<'static> {
    match column {
        DisplayColumn::Mark => {
            if is_marked {
                Span::styled("●", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
            } else {
                Span::raw(" ")
            }
        }
        DisplayColumn::Proto => Span::styled(p.proto.clone(), Style::default().fg(proto_color(p))),
        DisplayColumn::Port => Span::styled(p.port.to_string(), Style::default().fg(Color::Cyan)),
        DisplayColumn::Scope => Span::styled(
            p.listen_scope.label(),
            Style::default().fg(scope_color(p.listen_scope)),
        ),
        DisplayColumn::App => Span::styled(
            truncate_chars(&p.app, 18),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        DisplayColumn::Kind => Span::styled(
            truncate_chars(&p.inferred_kind, 18),
            Style::default().fg(Color::Magenta),
        ),
        DisplayColumn::Action => Span::styled(
            p.recommended_action.label(),
            Style::default().fg(action_color(p.recommended_action)),
        ),
        DisplayColumn::Risk => Span::styled(
            p.risk_level.label(),
            Style::default().fg(risk_color(p.risk_level)),
        ),
        DisplayColumn::Pid => Span::raw(p.pid.to_string()),
        DisplayColumn::Cpu => Span::styled(format!("{:.1}", p.cpu), Style::default().fg(cpu_color(p))),
        DisplayColumn::Mem => Span::styled(
            format!("{:.1}", p.memory_mb),
            Style::default().fg(mem_color(p)),
        ),
    }
}

fn proto_color(p: &ProcessInfo) -> Color {
    if p.proto.contains('6') {
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

fn action_color(action: crate::process::RecommendedAction) -> Color {
    match action {
        crate::process::RecommendedAction::Keep => Color::Green,
        crate::process::RecommendedAction::Inspect => Color::Yellow,
        crate::process::RecommendedAction::CloseIfUnused => Color::Red,
    }
}

fn cpu_color(p: &ProcessInfo) -> Color {
    if p.cpu > 80.0 {
        Color::Red
    } else if p.cpu > 50.0 {
        Color::Yellow
    } else {
        Color::Reset
    }
}

fn mem_color(p: &ProcessInfo) -> Color {
    if p.memory_mb > 1024.0 {
        Color::Red
    } else if p.memory_mb > 500.0 {
        Color::Yellow
    } else {
        Color::Reset
    }
}
