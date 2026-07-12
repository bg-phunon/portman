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

pub use help::max_scroll as help_max_scroll;

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
        AppState::Help => help::render_help(frame, app.help_scroll),
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

    let (msg_text, msg_style) = if let Some(msg) = app.active_message() {
        (
            msg.to_string(),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )
    } else if let Some(msg) = app.update_message() {
        (
            msg.to_string(),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )
    } else if let Some(ref err) = app.last_error {
        (
            format!("ERR: {}", text::truncate_chars(err, 30)),
            Style::default().fg(Color::Red),
        )
    } else {
        (String::new(), Style::default())
    };

    let help_text = match (layout_mode.shows_side_panel(), app.update_notice.is_some()) {
        (true, true) => " u dismiss  ? help ",
        (true, false) => " ? help ",
        (false, true) => " Enter inspect  u dismiss  ? help ",
        (false, false) => " Enter inspect  ? help ",
    };
    let help = Span::styled(help_text, Style::default().fg(Color::DarkGray));

    // Fit to width: drop low-priority spans first (layout, refresh, sort),
    // then truncate the message into whatever room remains.
    let width = area.width as usize;
    let fixed = mode_span.width() + mark_span.width() + help.width();
    let msg_width = if msg_text.is_empty() {
        0
    } else {
        msg_text.chars().count() + 2
    };

    let mut show_sort = true;
    let mut show_layout = true;
    let mut show_refresh = true;
    let mut total =
        fixed + msg_width + sort_span.width() + layout_span.width() + refresh_span.width();
    if total > width {
        show_layout = false;
        total -= layout_span.width();
    }
    if total > width {
        show_refresh = false;
        total -= refresh_span.width();
    }
    if total > width {
        show_sort = false;
        total -= sort_span.width();
    }

    let msg_span = if msg_text.is_empty() {
        Span::raw("")
    } else if total > width {
        let avail = width.saturating_sub(fixed).saturating_sub(2);
        if avail == 0 {
            Span::raw("")
        } else {
            Span::styled(
                format!(" {} ", text::truncate_chars(&msg_text, avail)),
                msg_style,
            )
        }
    } else {
        Span::styled(format!(" {msg_text} "), msg_style)
    };

    let mut spans = vec![mode_span];
    if show_sort {
        spans.push(sort_span);
    }
    if show_layout {
        spans.push(layout_span);
    }
    spans.push(mark_span);
    if show_refresh {
        spans.push(refresh_span);
    }
    spans.push(msg_span);
    spans.push(help);

    let bar = Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::Rgb(30, 30, 30)));
    frame.render_widget(bar, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::process::ProcessScanner;
    use crate::update::UpdateNotice;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn test_app() -> App {
        App::new(ProcessScanner::new())
    }

    fn app_with_notice(msg: &str) -> App {
        let mut app = test_app();
        app.set_update_notice(UpdateNotice {
            message: msg.to_string(),
        });
        app
    }

    fn footer_text(app: &App, width: u16) -> String {
        let backend = TestBackend::new(width, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_footer(frame, app, area, LayoutMode::from_width(width));
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..width).map(|x| buffer[(x, 0)].symbol()).collect()
    }

    const LONG_NOTICE: &str =
        "Update available: 0.9.9  Run: brew update && brew upgrade bg-phunon/tap/portman";

    #[test]
    fn footer_keeps_help_hint_when_update_notice_present_at_80_cols() {
        let app = app_with_notice(LONG_NOTICE);
        let text = footer_text(&app, 80);
        assert!(text.contains("? help"), "footer was: {text:?}");
    }

    #[test]
    fn long_update_message_is_truncated_with_ellipsis() {
        let app = app_with_notice(LONG_NOTICE);
        let text = footer_text(&app, 80);
        assert!(text.contains('…'), "footer was: {text:?}");
    }

    #[test]
    fn dismiss_hint_hidden_without_update_notice() {
        let app = test_app();
        let text = footer_text(&app, 120);
        assert!(!text.contains("u dismiss"), "footer was: {text:?}");
    }

    #[test]
    fn dismiss_hint_shown_with_update_notice() {
        let app = app_with_notice("Update available: 0.9.9");
        let text = footer_text(&app, 120);
        assert!(text.contains("u dismiss"), "footer was: {text:?}");
    }

    #[test]
    fn low_priority_spans_dropped_before_help_when_narrow() {
        let app = app_with_notice("Update available: 0.9.9");
        let text = footer_text(&app, 60);
        assert!(!text.contains("layout:"), "footer was: {text:?}");
        assert!(text.contains("? help"), "footer was: {text:?}");
    }
}
