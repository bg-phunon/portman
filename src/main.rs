mod app;
mod process;
mod ui;

use std::io;
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
    if let Some(f) = cli.filter {
        app.filter = f;
    }
    app.refresh();

    let result = run_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_json(scanner: &mut ProcessScanner, filter: Option<&str>) -> Result<()> {
    let procs = scanner.scan()?;
    let filtered: Vec<_> = if let Some(q) = filter {
        let q = q.to_lowercase();
        procs
            .into_iter()
            .filter(|p| {
                p.port.to_string().contains(&q)
                    || p.app.to_lowercase().contains(&q)
                    || p.local_addr.contains(&q)
            })
            .collect()
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

fn run_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        // Rebuild cache once per frame (not per-widget)
        app.rebuild_cache();

        terminal.draw(|frame| {
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
            KeyCode::Char('?') => {
                app.state = AppState::Help;
            }

            // Filter
            KeyCode::Char('/') => {
                app.state = AppState::FilterInput;
            }
            KeyCode::Esc => {
                if !app.filter.is_empty() {
                    app.clear_filter();
                }
            }

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
