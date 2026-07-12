use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::update::UPGRADE_COMMAND;

/// Minimum number of help lines kept on screen when scrolled to the bottom.
const MIN_VISIBLE_LINES: usize = 10;

pub fn max_scroll() -> u16 {
    help_lines().len().saturating_sub(MIN_VISIBLE_LINES) as u16
}

pub fn render_help(frame: &mut Frame, scroll: u16) {
    let area = centered_rect(60, 80, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" Help ")
        .title_bottom(
            Line::from(" j/k scroll · ? close ").style(Style::default().fg(Color::DarkGray)),
        );

    let paragraph = Paragraph::new(help_lines())
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((scroll.min(max_scroll()), 0));
    frame.render_widget(paragraph, area);
}

fn help_lines() -> Vec<Line<'static>> {
    let key = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(Color::DarkGray);

    vec![
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
        Line::from(vec![Span::styled("  u           ", key), Span::raw("Dismiss update notice")]),
        Line::from(vec![
            Span::styled("  Upgrade     ", key),
            Span::raw(UPGRADE_COMMAND),
        ]),
        Line::from(vec![Span::styled("  ?           ", key), Span::raw("Toggle this help")]),
        Line::from(vec![Span::styled("  q / Ctrl+C  ", key), Span::raw("Quit")]),
        Line::from(""),
        Line::from(Span::styled("  Press ? or Esc to close", dim)),
    ]
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

#[cfg(test)]
mod tests {
    use super::{max_scroll, render_help};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn help_text(width: u16, height: u16, scroll: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render_help(frame, scroll)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn help_screen_shows_upgrade_command() {
        let text = help_text(120, 55, 0);
        assert!(
            text.contains("brew update && brew upgrade"),
            "help screen did not show the upgrade command"
        );
    }

    #[test]
    fn help_clips_upgrade_command_on_short_terminal_without_scroll() {
        let text = help_text(100, 24, 0);
        assert!(
            !text.contains("brew update && brew upgrade"),
            "expected upgrade command to be below the fold at 24 rows"
        );
    }

    #[test]
    fn help_scroll_reveals_upgrade_command_on_short_terminal() {
        let text = help_text(100, 24, max_scroll());
        assert!(
            text.contains("brew update && brew upgrade"),
            "scrolling to bottom should reveal the upgrade command"
        );
    }

    #[test]
    fn help_scroll_hint_is_always_visible() {
        let text = help_text(100, 24, 0);
        assert!(
            text.contains("j/k scroll"),
            "scroll hint should be pinned to the help border"
        );
    }
}
