mod app;
mod process;
mod ui;
mod update;

use std::io;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use app::{App, AppState, SortColumn};
use process::ProcessScanner;
use update::{spawn_update_check, UpdateNotice};

const REFRESH_INTERVAL: Duration = Duration::from_secs(5);
const POLL_TIMEOUT: Duration = Duration::from_millis(250);

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

struct CliArgs {
    json: bool,
    filter: Option<String>,
}

fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut cli = CliArgs {
        json: false,
        filter: None,
    };

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" | "-j" => cli.json = true,
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            other => {
                if !other.starts_with('-') {
                    cli.filter = Some(other.to_string());
                }
            }
        }
        i += 1;
    }
    cli
}

fn print_usage() {
    eprintln!(
        "\
portman — TUI port manager

USAGE:
    portman                  Launch interactive TUI
    portman 3000             Launch with filter on port 3000
    portman --json           One-shot JSON output of all listening ports
    portman --json 3000      JSON output filtered to port 3000

OPTIONS:
    -j, --json    Output as JSON (non-interactive, exits immediately)
    -h, --help    Show this help"
    );
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    let cli = parse_args();

    let mut scanner = ProcessScanner::new();

    if cli.json {
        return run_json(&mut scanner, cli.filter.as_deref());
    }

    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(scanner);
    let update_rx = spawn_update_check(env!("CARGO_PKG_VERSION"));
    if let Some(f) = cli.filter {
        app.filter = f;
    }
    app.refresh();

    let result = run_loop(&mut terminal, &mut app, update_rx);

    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_json(scanner: &mut ProcessScanner, filter: Option<&str>) -> Result<()> {
    let procs = scanner.scan()?;
    let filtered: Vec<_> = if let Some(q) = filter {
        let q = q.to_lowercase();
        procs.into_iter().filter(|p| p.matches_filter_lower(&q)).collect()
    } else {
        procs
    };

    let json = serde_json::to_string_pretty(&filtered)?;
    println!("{json}");
    Ok(())
}

// ---------------------------------------------------------------------------
// Event loop
// ---------------------------------------------------------------------------

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    update_rx: Option<Receiver<UpdateNotice>>,
) -> Result<()> {
    loop {
        poll_update(app, update_rx.as_ref());

        // Rebuild cache once per frame (not per-widget)
        app.rebuild_cache();

        terminal.draw(|frame| {
            app.set_layout_mode(ui::LayoutMode::from_width(frame.area().width));
            let table_h = frame.area().height.saturating_sub(5) as usize;
            app.page_size = table_h.max(1);
            ui::render(frame, app);
        })?;

        if app.last_refresh.elapsed() >= REFRESH_INTERVAL {
            app.refresh();
        }

        if event::poll(POLL_TIMEOUT)? {
            if let Event::Key(key) = event::read()? {
                if handle_key(app, key) {
                    break;
                }
            }
        }
    }
    Ok(())
}

fn poll_update(app: &mut App, update_rx: Option<&Receiver<UpdateNotice>>) {
    let Some(update_rx) = update_rx else {
        return;
    };

    if let Ok(notice) = update_rx.try_recv() {
        app.set_update_notice(notice);
    }
}

fn handle_key(app: &mut App, key: KeyEvent) -> bool {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return true;
    }

    match &app.state {
        // ----- Help -----
        AppState::Help => match key.code {
            KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q') => {
                app.state = AppState::Normal;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                app.help_scroll = app.help_scroll.saturating_add(1).min(ui::help_max_scroll());
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.help_scroll = app.help_scroll.saturating_sub(1);
            }
            KeyCode::PageDown => {
                app.help_scroll = app.help_scroll.saturating_add(10).min(ui::help_max_scroll());
            }
            KeyCode::PageUp => {
                app.help_scroll = app.help_scroll.saturating_sub(10);
            }
            _ => {}
        },

        // ----- Inspect -----
        AppState::Inspect(_) => match key.code {
            KeyCode::Char('i') | KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') => {
                app.state = AppState::Normal;
            }
            _ => {}
        },

        // ----- Filter input -----
        AppState::FilterInput => match key.code {
            KeyCode::Esc => {
                app.clear_filter();
                app.state = AppState::Normal;
            }
            KeyCode::Enter => {
                app.state = AppState::Normal;
            }
            KeyCode::Backspace => app.backspace_filter(),
            KeyCode::Char(c) => app.set_filter_char(c),
            _ => {}
        },

        // ----- Normal -----
        AppState::Normal => match key.code {
            KeyCode::Char('q') => return true,

            // Navigation
            KeyCode::Char('j') | KeyCode::Down => app.move_down(),
            KeyCode::Char('k') | KeyCode::Up => app.move_up(),
            KeyCode::Char('g') => app.go_top(),
            KeyCode::Char('G') => app.go_bottom(),
            KeyCode::PageUp => app.page_up(),
            KeyCode::PageDown => app.page_down(),

            // Mark / multi-select
            KeyCode::Char(' ') => app.toggle_mark(),
            KeyCode::Char('m') => {
                app.mark_all_visible();
                app.set_message(format!("Marked {} entries", app.marked.len()));
            }
            KeyCode::Char('M') => {
                app.unmark_all();
                app.set_message("Unmarked all".to_string());
            }

            // Kill (single or multi depending on marks)
            KeyCode::Char('K') => app.request_kill(),

            // Clipboard
            KeyCode::Char('y') => app.copy_port(),
            KeyCode::Char('Y') => app.copy_kill_cmd(),

            // Misc
            KeyCode::Char('r') => app.refresh(),
            KeyCode::Char('i') | KeyCode::Enter => app.request_inspect(),
            KeyCode::Char('u') => app.dismiss_update_notice(),
            KeyCode::Char('?') => {
                app.help_scroll = 0;
                app.state = AppState::Help;
            }

            // Filter
            KeyCode::Char('/') => {
                app.state = AppState::FilterInput;
            }
            KeyCode::Esc if !app.filter.is_empty() => app.clear_filter(),

            // Sort
            KeyCode::Tab => app.cycle_sort(),
            KeyCode::Char('S') => app.toggle_sort_dir(),
            KeyCode::Char('1') => app.set_sort(SortColumn::Proto),
            KeyCode::Char('2') => app.set_sort(SortColumn::Port),
            KeyCode::Char('3') => app.set_sort(SortColumn::Addr),
            KeyCode::Char('4') => app.set_sort(SortColumn::Pid),
            KeyCode::Char('5') => app.set_sort(SortColumn::User),
            KeyCode::Char('6') => app.set_sort(SortColumn::App),
            KeyCode::Char('7') => app.set_sort(SortColumn::Cpu),
            KeyCode::Char('8') => app.set_sort(SortColumn::Mem),

            _ => {}
        },

        // ----- Confirm single kill -----
        AppState::Confirm(_) => match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                if let Err(e) = app.kill_confirmed_single() {
                    app.last_error = Some(e.to_string());
                }
                app.refresh();
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                app.state = AppState::Normal;
            }
            _ => {}
        },

        // ----- Confirm multi kill -----
        AppState::ConfirmMulti(_) => match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                if let Err(e) = app.kill_confirmed_multi() {
                    app.last_error = Some(e.to_string());
                }
                app.refresh();
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                app.state = AppState::Normal;
            }
            _ => {}
        },
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::{
        ListenScope, ProcessInfo, RecommendedAction, RiskLevel, WorkspaceRelation,
    };

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

    fn app_with_processes() -> App {
        let mut app = App::new(ProcessScanner::new());
        app.processes = vec![
            sample_process(1001, 3000, "alpha"),
            sample_process(1002, 3001, "beta"),
            sample_process(1003, 3002, "gamma"),
        ];
        app.rebuild_cache();
        app
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl_c() -> KeyEvent {
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
    }

    #[test]
    fn navigation_and_mark_keys_work_in_normal_mode() {
        let mut app = app_with_processes();

        assert!(!handle_key(&mut app, key(KeyCode::Down)));
        assert_eq!(app.selected, 1);

        assert!(!handle_key(&mut app, key(KeyCode::Char(' '))));
        assert_eq!(app.marked.len(), 1);
        assert_eq!(app.selected, 2);

        assert!(!handle_key(&mut app, key(KeyCode::Char('g'))));
        assert_eq!(app.selected, 0);

        assert!(!handle_key(&mut app, key(KeyCode::Char('G'))));
        assert_eq!(app.selected, 2);

        assert!(!handle_key(&mut app, key(KeyCode::Char('m'))));
        assert_eq!(app.marked.len(), 3);

        assert!(!handle_key(&mut app, key(KeyCode::Char('M'))));
        assert!(app.marked.is_empty());
    }

    #[test]
    fn filter_keys_edit_and_clear_filter() {
        let mut app = app_with_processes();

        assert!(!handle_key(&mut app, key(KeyCode::Char('/'))));
        assert!(matches!(app.state, AppState::FilterInput));

        assert!(!handle_key(&mut app, key(KeyCode::Char('b'))));
        assert!(!handle_key(&mut app, key(KeyCode::Char('e'))));
        assert_eq!(app.filter, "be");

        assert!(!handle_key(&mut app, key(KeyCode::Backspace)));
        assert_eq!(app.filter, "b");

        assert!(!handle_key(&mut app, key(KeyCode::Enter)));
        assert!(matches!(app.state, AppState::Normal));
        assert_eq!(app.filter, "b");

        assert!(!handle_key(&mut app, key(KeyCode::Esc)));
        assert!(app.filter.is_empty());
    }

    #[test]
    fn inspect_and_help_keys_open_and_close_views() {
        let mut app = app_with_processes();

        assert!(!handle_key(&mut app, key(KeyCode::Enter)));
        assert!(matches!(app.state, AppState::Inspect(_)));

        assert!(!handle_key(&mut app, key(KeyCode::Esc)));
        assert!(matches!(app.state, AppState::Normal));

        assert!(!handle_key(&mut app, key(KeyCode::Char('?'))));
        assert!(matches!(app.state, AppState::Help));

        assert!(!handle_key(&mut app, key(KeyCode::Char('?'))));
        assert!(matches!(app.state, AppState::Normal));
    }

    #[test]
    fn sort_and_quit_keys_work() {
        let mut app = app_with_processes();

        assert_eq!(app.sort_col, SortColumn::Port);
        assert!(!handle_key(&mut app, key(KeyCode::Tab)));
        assert_eq!(app.sort_col, SortColumn::App);

        assert!(!handle_key(&mut app, key(KeyCode::Char('1'))));
        assert_eq!(app.sort_col, SortColumn::Proto);

        assert!(!handle_key(&mut app, key(KeyCode::Char('S'))));
        assert_eq!(app.sort_dir, app::SortDir::Desc);

        assert!(handle_key(&mut app, key(KeyCode::Char('q'))));
    }

    #[test]
    fn tab_cycles_only_visible_sort_columns_for_layout() {
        let mut app = app_with_processes();

        app.set_layout_mode(ui::LayoutMode::Compact);
        app.sort_col = SortColumn::Port;
        assert!(!handle_key(&mut app, key(KeyCode::Tab)));
        assert_eq!(app.sort_col, SortColumn::App);
        assert!(!handle_key(&mut app, key(KeyCode::Tab)));
        assert_eq!(app.sort_col, SortColumn::Port);

        app.set_layout_mode(ui::LayoutMode::Standard);
        app.sort_col = SortColumn::Port;
        assert!(!handle_key(&mut app, key(KeyCode::Tab)));
        assert_eq!(app.sort_col, SortColumn::App);
        assert!(!handle_key(&mut app, key(KeyCode::Tab)));
        assert_eq!(app.sort_col, SortColumn::Pid);
        assert!(!handle_key(&mut app, key(KeyCode::Tab)));
        assert_eq!(app.sort_col, SortColumn::Cpu);
        assert!(!handle_key(&mut app, key(KeyCode::Tab)));
        assert_eq!(app.sort_col, SortColumn::Port);
    }

    #[test]
    fn layout_change_resets_hidden_sort_column_to_visible_default() {
        let mut app = app_with_processes();
        app.sort_col = SortColumn::Mem;
        app.rebuild_cache();

        app.set_layout_mode(ui::LayoutMode::Compact);
        assert_eq!(app.sort_col, SortColumn::Port);
        assert_eq!(app.sort_dir, app::SortDir::Asc);
    }

    #[test]
    fn ctrl_c_and_confirm_cancel_work() {
        let mut app = app_with_processes();
        app.state = AppState::Confirm(app.processes[0].clone());

        assert!(!handle_key(&mut app, key(KeyCode::Esc)));
        assert!(matches!(app.state, AppState::Normal));

        app.state = AppState::ConfirmMulti(vec![app.processes[0].clone(), app.processes[1].clone()]);
        assert!(!handle_key(&mut app, key(KeyCode::Char('n'))));
        assert!(matches!(app.state, AppState::Normal));

        assert!(handle_key(&mut app, ctrl_c()));
    }

    #[test]
    fn help_scroll_keys_adjust_offset_and_reset_on_reopen() {
        let mut app = app_with_processes();

        assert!(!handle_key(&mut app, key(KeyCode::Char('?'))));
        assert!(matches!(app.state, AppState::Help));
        assert_eq!(app.help_scroll, 0);

        assert!(!handle_key(&mut app, key(KeyCode::Char('j'))));
        assert_eq!(app.help_scroll, 1);
        assert!(!handle_key(&mut app, key(KeyCode::Down)));
        assert_eq!(app.help_scroll, 2);

        assert!(!handle_key(&mut app, key(KeyCode::Char('k'))));
        assert_eq!(app.help_scroll, 1);
        assert!(!handle_key(&mut app, key(KeyCode::Up)));
        assert_eq!(app.help_scroll, 0);
        assert!(!handle_key(&mut app, key(KeyCode::Char('k'))));
        assert_eq!(app.help_scroll, 0, "scroll must saturate at 0");

        // scrolling down is clamped so the box never goes fully blank
        for _ in 0..500 {
            handle_key(&mut app, key(KeyCode::Char('j')));
        }
        assert_eq!(app.help_scroll, ui::help_max_scroll());

        assert!(!handle_key(&mut app, key(KeyCode::Esc)));
        assert!(matches!(app.state, AppState::Normal));
        assert!(!handle_key(&mut app, key(KeyCode::Char('?'))));
        assert_eq!(app.help_scroll, 0, "reopening help must reset scroll");
    }

    #[test]
    fn dismiss_update_notice_key_works() {
        let mut app = app_with_processes();
        app.set_update_notice(UpdateNotice {
            message: "Update available".to_string(),
        });

        assert!(app.update_notice.is_some());
        assert!(!handle_key(&mut app, key(KeyCode::Char('u'))));
        assert!(app.update_notice.is_none());
    }
}
