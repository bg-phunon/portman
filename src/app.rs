use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::process::Command;
use std::time::Instant;

use anyhow::Result;

use crate::process::{ProcessInfo, ProcessScanner};

// ---------------------------------------------------------------------------
// Sort
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    Proto,
    Port,
    Addr,
    Pid,
    User,
    App,
    Cpu,
    Mem,
}

impl SortColumn {
    pub const ALL: [SortColumn; 8] = [
        SortColumn::Proto,
        SortColumn::Port,
        SortColumn::Addr,
        SortColumn::Pid,
        SortColumn::User,
        SortColumn::App,
        SortColumn::Cpu,
        SortColumn::Mem,
    ];

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|&c| c == self).unwrap_or(0)
    }

    pub fn label(self) -> &'static str {
        match self {
            SortColumn::Proto => "PROTO",
            SortColumn::Port => "PORT",
            SortColumn::Addr => "LOCAL ADDR",
            SortColumn::Pid => "PID",
            SortColumn::User => "USER",
            SortColumn::App => "APP",
            SortColumn::Cpu => "CPU%",
            SortColumn::Mem => "MEM(MB)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDir {
    Asc,
    Desc,
}

impl SortDir {
    pub fn arrow(self) -> &'static str {
        match self {
            SortDir::Asc => "▲",
            SortDir::Desc => "▼",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            SortDir::Asc => SortDir::Desc,
            SortDir::Desc => SortDir::Asc,
        }
    }
}

// ---------------------------------------------------------------------------
// Mark key: (pid, port) uniquely identifies a row
// ---------------------------------------------------------------------------

pub type MarkKey = (u32, u16);

pub fn mark_key(p: &ProcessInfo) -> MarkKey {
    (p.pid, p.port)
}

// ---------------------------------------------------------------------------
// App state (Elm architecture)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum AppState {
    Normal,
    FilterInput,
    Help,
    Confirm(ProcessInfo),
    ConfirmMulti(Vec<ProcessInfo>),
}

pub struct App {
    pub processes: Vec<ProcessInfo>,
    pub selected: usize,
    pub state: AppState,
    pub filter: String,
    pub sort_col: SortColumn,
    pub sort_dir: SortDir,
    pub marked: BTreeSet<MarkKey>,
    pub last_error: Option<String>,
    pub last_message: Option<(String, Instant)>,
    pub last_refresh: Instant,
    pub page_size: usize,
    scanner: ProcessScanner,
    // Cached filtered+sorted view — rebuilt once per frame via rebuild_cache()
    filtered_cache: Vec<ProcessInfo>,
}

impl App {
    pub fn new(scanner: ProcessScanner) -> Self {
        Self {
            processes: Vec::new(),
            selected: 0,
            state: AppState::Normal,
            filter: String::new(),
            sort_col: SortColumn::Port,
            sort_dir: SortDir::Asc,
            marked: BTreeSet::new(),
            last_error: None,
            last_message: None,
            last_refresh: Instant::now(),
            page_size: 20,
            scanner,
            filtered_cache: Vec::new(),
        }
    }

    // ----- Cache (HIGH fix: compute filter+sort once per frame) -----

    /// Rebuild the filtered+sorted cache. Call once before each render.
    pub fn rebuild_cache(&mut self) {
        let mut result: Vec<ProcessInfo> = if self.filter.is_empty() {
            self.processes.clone()
        } else {
            let q = self.filter.to_lowercase();
            self.processes
                .iter()
                .filter(|p| {
                    p.app.to_lowercase().contains(&q)
                        || p.port.to_string().contains(&q)
                        || p.username.to_lowercase().contains(&q)
                        || p.command.to_lowercase().contains(&q)
                        || p.local_addr.contains(&q)
                        || p.proto.to_lowercase().contains(&q)
                })
                .cloned()
                .collect()
        };

        let col = self.sort_col;
        let dir = self.sort_dir;

        result.sort_by(|a, b| {
            let ord = match col {
                SortColumn::Proto => a.proto.cmp(&b.proto),
                SortColumn::Port => a.port.cmp(&b.port),
                SortColumn::Addr => a.local_addr.cmp(&b.local_addr),
                SortColumn::Pid => a.pid.cmp(&b.pid),
                SortColumn::User => a.username.to_lowercase().cmp(&b.username.to_lowercase()),
                SortColumn::App => a.app.to_lowercase().cmp(&b.app.to_lowercase()),
                SortColumn::Cpu => a.cpu.partial_cmp(&b.cpu).unwrap_or(Ordering::Equal),
                SortColumn::Mem => a.memory_mb.partial_cmp(&b.memory_mb).unwrap_or(Ordering::Equal),
            };
            match dir {
                SortDir::Asc => ord,
                SortDir::Desc => ord.reverse(),
            }
        });

        self.filtered_cache = result;
    }

    /// Read-only access to cached filtered+sorted processes.
    pub fn filtered(&self) -> &[ProcessInfo] {
        &self.filtered_cache
    }

    pub fn filtered_len(&self) -> usize {
        self.filtered_cache.len()
    }

    // ----- Data -----

    pub fn refresh(&mut self) {
        match self.scanner.scan() {
            Ok(procs) => {
                self.processes = procs;
                self.last_error = None;
                let valid: BTreeSet<MarkKey> = self.processes.iter().map(mark_key).collect();
                self.marked.retain(|k| valid.contains(k));
            }
            Err(e) => {
                self.last_error = Some(e.to_string());
            }
        }
        self.last_refresh = Instant::now();
        self.rebuild_cache();
        self.clamp_selection();
    }

    pub fn selected_process(&self) -> Option<&ProcessInfo> {
        self.filtered_cache.get(self.selected)
    }

    // ----- Mark / multi-select -----

    pub fn is_marked(&self, p: &ProcessInfo) -> bool {
        self.marked.contains(&mark_key(p))
    }

    pub fn toggle_mark(&mut self) {
        if let Some(info) = self.filtered_cache.get(self.selected).cloned() {
            let key = mark_key(&info);
            if !self.marked.remove(&key) {
                self.marked.insert(key);
            }
        }
        self.move_down();
    }

    pub fn mark_all_visible(&mut self) {
        let keys: Vec<MarkKey> = self.filtered_cache.iter().map(mark_key).collect();
        self.marked.extend(keys);
    }

    pub fn unmark_all(&mut self) {
        self.marked.clear();
    }

    // ----- Kill -----

    pub fn request_kill(&mut self) {
        if self.marked.is_empty() {
            if let Some(info) = self.selected_process().cloned() {
                self.state = AppState::Confirm(info);
            }
        } else {
            let targets: Vec<ProcessInfo> = self
                .processes
                .iter()
                .filter(|p| self.marked.contains(&mark_key(p)))
                .cloned()
                .collect();
            if !targets.is_empty() {
                self.state = AppState::ConfirmMulti(targets);
            }
        }
    }

    pub fn kill_confirmed_single(&mut self) -> Result<()> {
        if let AppState::Confirm(ref info) = self.state {
            let pid = info.pid;
            let result = self.scanner.kill(pid);
            self.state = AppState::Normal;
            if result.is_ok() {
                self.processes.retain(|p| p.pid != pid);
                self.marked.retain(|&(p, _)| p != pid);
                self.rebuild_cache();
                self.clamp_selection();
                self.set_message("Process killed".to_string());
            }
            result
        } else {
            Ok(())
        }
    }

    pub fn kill_confirmed_multi(&mut self) -> Result<()> {
        if let AppState::ConfirmMulti(ref targets) = self.state {
            let mut pids: Vec<u32> = targets.iter().map(|p| p.pid).collect();
            pids.sort();
            pids.dedup();

            let mut killed = 0u32;
            let mut errors = Vec::new();

            for pid in &pids {
                match self.scanner.kill(*pid) {
                    Ok(()) => killed += 1,
                    Err(e) => errors.push(format!("pid {pid}: {e}")),
                }
            }

            self.state = AppState::Normal;
            self.processes.retain(|p| !pids.contains(&p.pid));
            self.marked.clear();
            self.rebuild_cache();
            self.clamp_selection();

            if errors.is_empty() {
                self.set_message(format!("Killed {killed} process(es)"));
                Ok(())
            } else {
                let msg = format!("Killed {killed}, failed: {}", errors.join("; "));
                self.set_message(msg.clone());
                anyhow::bail!(msg)
            }
        } else {
            Ok(())
        }
    }

    // ----- Navigation -----

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        let len = self.filtered_len();
        if len > 0 && self.selected < len - 1 {
            self.selected += 1;
        }
    }

    pub fn go_top(&mut self) {
        self.selected = 0;
    }

    pub fn go_bottom(&mut self) {
        let len = self.filtered_len();
        self.selected = if len > 0 { len - 1 } else { 0 };
    }

    pub fn page_up(&mut self) {
        self.selected = self.selected.saturating_sub(self.page_size);
    }

    pub fn page_down(&mut self) {
        let len = self.filtered_len();
        if len == 0 {
            return;
        }
        self.selected = (self.selected + self.page_size).min(len - 1);
    }

    // ----- Sort (rebuild cache on change) -----

    pub fn cycle_sort(&mut self) {
        let idx = self.sort_col.index();
        let next = (idx + 1) % SortColumn::ALL.len();
        self.sort_col = SortColumn::ALL[next];
        self.sort_dir = SortDir::Asc;
        self.selected = 0;
        self.rebuild_cache();
    }

    pub fn set_sort(&mut self, col: SortColumn) {
        if self.sort_col == col {
            self.sort_dir = self.sort_dir.toggle();
        } else {
            self.sort_col = col;
            self.sort_dir = SortDir::Asc;
        }
        self.selected = 0;
        self.rebuild_cache();
    }

    pub fn toggle_sort_dir(&mut self) {
        self.sort_dir = self.sort_dir.toggle();
        self.selected = 0;
        self.rebuild_cache();
    }

    // ----- Filter (rebuild cache on change) -----

    pub fn set_filter_char(&mut self, c: char) {
        self.filter.push(c);
        self.selected = 0;
        self.rebuild_cache();
    }

    pub fn backspace_filter(&mut self) {
        self.filter.pop();
        self.selected = 0;
        self.rebuild_cache();
    }

    pub fn clear_filter(&mut self) {
        self.filter.clear();
        self.selected = 0;
        self.rebuild_cache();
    }

    // ----- Clipboard -----

    pub fn copy_port(&mut self) {
        if let Some(info) = self.selected_process() {
            let text = info.port.to_string();
            if clipboard_copy(&text) {
                self.set_message(format!("Copied port: {text}"));
            } else {
                self.set_message("Clipboard failed (pbcopy not found?)".to_string());
            }
        }
    }

    pub fn copy_kill_cmd(&mut self) {
        if self.marked.is_empty() {
            if let Some(info) = self.selected_process() {
                let text = format!("kill -9 {}", info.pid);
                if clipboard_copy(&text) {
                    self.set_message(format!("Copied: {text}"));
                }
            }
        } else {
            let mut pids: Vec<u32> = self
                .processes
                .iter()
                .filter(|p| self.marked.contains(&mark_key(p)))
                .map(|p| p.pid)
                .collect();
            pids.sort();
            pids.dedup();
            let pid_strs: Vec<String> = pids.iter().map(|p| p.to_string()).collect();
            let text = format!("kill -9 {}", pid_strs.join(" "));
            if clipboard_copy(&text) {
                self.set_message(format!("Copied: {text}"));
            }
        }
    }

    // ----- Message -----

    pub fn set_message(&mut self, msg: String) {
        self.last_message = Some((msg, Instant::now()));
    }

    pub fn active_message(&self) -> Option<&str> {
        self.last_message.as_ref().and_then(|(msg, t)| {
            if t.elapsed().as_secs() < 3 {
                Some(msg.as_str())
            } else {
                None
            }
        })
    }

    fn clamp_selection(&mut self) {
        let len = self.filtered_len();
        if len == 0 {
            self.selected = 0;
        } else if self.selected >= len {
            self.selected = len - 1;
        }
    }
}

fn clipboard_copy(text: &str) -> bool {
    use std::io::Write;
    let child = Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn();
    match child {
        Ok(mut c) => {
            if let Some(ref mut stdin) = c.stdin {
                let _ = stdin.write_all(text.as_bytes());
            }
            c.wait().is_ok()
        }
        Err(_) => false,
    }
}
