use std::collections::HashMap;
use std::process::Command;

use anyhow::{Context, Result};
use serde::Serialize;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

// ---------------------------------------------------------------------------
// Custom errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum ScanError {
    LsofNotFound,
    PermissionDenied,
    Other(String),
}

impl std::fmt::Display for ScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScanError::LsofNotFound => write!(f, "lsof not found in PATH"),
            ScanError::PermissionDenied => write!(f, "permission denied running lsof"),
            ScanError::Other(msg) => write!(f, "lsof error: {msg}"),
        }
    }
}

impl std::error::Error for ScanError {}

// ---------------------------------------------------------------------------
// ProcessInfo
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ProcessInfo {
    pub port: u16,
    pub pid: u32,
    pub proto: String,
    pub local_addr: String,
    pub app: String,
    pub command: String,
    pub cpu: f32,
    pub memory_mb: f64,
    pub status: String,
    pub username: String,
    pub start_time: u64,
}

// ---------------------------------------------------------------------------
// ProcessScanner
// ---------------------------------------------------------------------------

pub struct ProcessScanner {
    system: System,
    last_known: Vec<ProcessInfo>,
}

impl ProcessScanner {
    pub fn new() -> Self {
        Self {
            system: System::new(),
            last_known: Vec::new(),
        }
    }

    /// Run lsof and join with sysinfo data to build a full process list.
    pub fn scan(&mut self) -> Result<Vec<ProcessInfo>> {
        // Refresh sysinfo for CPU + memory data
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::new()
                .with_cpu()
                .with_memory()
                .with_user(UpdateKind::OnlyIfNotSet),
        );

        // Run lsof to discover listening TCP ports
        let lsof_entries = match run_lsof() {
            Ok(entries) => entries,
            Err(e) => {
                // Return last known data as fallback
                if !self.last_known.is_empty() {
                    return Ok(self.last_known.clone());
                }
                return Err(e);
            }
        };

        // Build a lookup from sysinfo by pid
        // IMPORTANT: create Users list ONCE — calling per-process is O(n²) and very slow
        let users = sysinfo::Users::new_with_refreshed_list();
        let mut sysinfo_map: HashMap<u32, &sysinfo::Process> = HashMap::new();
        for (pid, proc) in self.system.processes() {
            sysinfo_map.insert(pid.as_u32(), proc);
        }

        let mut results: Vec<ProcessInfo> = Vec::new();

        for entry in lsof_entries {
            let (cpu, memory_mb, status, sysinfo_user, full_cmd, start_time) =
                if let Some(proc) = sysinfo_map.get(&entry.pid) {
                    let mem = proc.memory() as f64 / (1024.0 * 1024.0);
                    let stat = format!("{:?}", proc.status());
                    let cmd_parts: Vec<String> =
                        proc.cmd().iter().map(|s| s.to_string_lossy().into_owned()).collect();
                    let cmd = if cmd_parts.is_empty() {
                        entry.command.clone()
                    } else {
                        cmd_parts.join(" ")
                    };
                    let uname = proc
                        .user_id()
                        .and_then(|uid| {
                            users
                                .iter()
                                .find(|u| u.id() == uid)
                                .map(|u| u.name().to_string())
                        })
                        .unwrap_or_default();
                    (proc.cpu_usage(), mem, stat, uname, cmd, proc.start_time())
                } else {
                    (0.0, 0.0, "Unknown".to_string(), String::new(), entry.command.clone(), 0)
                };

            // Prefer lsof username (always available), fall back to sysinfo
            let username = if entry.user.is_empty() {
                sysinfo_user
            } else {
                entry.user
            };

            results.push(ProcessInfo {
                port: entry.port,
                pid: entry.pid,
                proto: entry.proto,
                local_addr: entry.local_addr,
                app: entry.command,
                command: full_cmd,
                cpu,
                memory_mb,
                status,
                username,
                start_time,
            });
        }

        // Sort by port ascending
        results.sort_by_key(|p| p.port);

        // Cache for fallback
        self.last_known = results.clone();
        Ok(results)
    }

    /// Kill a process by pid.
    pub fn kill(&self, pid: u32) -> Result<()> {
        let sys_pid = Pid::from_u32(pid);
        if let Some(proc) = self.system.process(sys_pid) {
            proc.kill();
            Ok(())
        } else {
            // Fallback to kill command
            let status = Command::new("kill")
                .arg("-9")
                .arg(pid.to_string())
                .status()
                .context("failed to execute kill command")?;
            if status.success() {
                Ok(())
            } else {
                anyhow::bail!("kill command failed for pid {pid}")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// lsof helpers
// ---------------------------------------------------------------------------

struct LsofEntry {
    pid: u32,
    port: u16,
    proto: String,
    local_addr: String,
    user: String,
    command: String,
}

fn run_lsof() -> Result<Vec<LsofEntry>> {
    // Use -F field mode for robust parsing (handles spaces in command names)
    // Field selectors: p=PID, c=command, L=login name, t=type, n=name
    let output = Command::new("lsof")
        .args(["-nP", "-iTCP", "-sTCP:LISTEN", "-FpcLtn"])
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow::Error::new(ScanError::LsofNotFound)
            } else if e.kind() == std::io::ErrorKind::PermissionDenied {
                anyhow::Error::new(ScanError::PermissionDenied)
            } else {
                anyhow::Error::new(ScanError::Other(e.to_string()))
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("Permission denied") {
            anyhow::bail!(ScanError::PermissionDenied);
        }
        if output.status.code() == Some(1) && output.stdout.is_empty() {
            return Ok(Vec::new());
        }
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_lsof_field_output(&stdout)
}

/// Parse lsof -F field mode output.
///
/// Each line starts with a field identifier character:
///   p = PID, c = command, L = login name, t = type (IPv4/IPv6), n = name
///
/// Process context (p, c, L) repeats per process; fd context (t, n) repeats per socket.
fn parse_lsof_field_output(output: &str) -> Result<Vec<LsofEntry>> {
    let mut entries = Vec::new();

    // Current process context
    let mut cur_pid: u32 = 0;
    let mut cur_cmd = String::new();
    let mut cur_user = String::new();

    // Current fd context
    let mut cur_type = String::new();

    for line in output.lines() {
        if line.is_empty() {
            continue;
        }

        let tag = line.as_bytes()[0];
        let value = &line[1..];

        match tag {
            b'p' => {
                cur_pid = value.parse().unwrap_or(0);
                // Reset per-process fields
                cur_cmd.clear();
                cur_user.clear();
            }
            b'c' => cur_cmd = value.to_string(),
            b'L' => cur_user = value.to_string(),
            b't' => cur_type = value.to_string(),
            b'n' => {
                // name field: "127.0.0.1:3000", "*:8080", "[::1]:443"
                let (local_addr, port) = split_name(value);
                if let Some(port) = port {
                    entries.push(LsofEntry {
                        pid: cur_pid,
                        port,
                        proto: cur_type.clone(),
                        local_addr,
                        user: cur_user.clone(),
                        command: cur_cmd.clone(),
                    });
                }
            }
            _ => {} // f (fd), etc. — ignored
        }
    }

    // Deduplicate by (pid, port, proto)
    entries.sort_by(|a, b| {
        a.pid
            .cmp(&b.pid)
            .then(a.port.cmp(&b.port))
            .then(a.proto.cmp(&b.proto))
    });
    entries.dedup_by(|a, b| a.pid == b.pid && a.port == b.port && a.proto == b.proto);

    Ok(entries)
}

/// Split a NAME field like "127.0.0.1:3000" into (address, port).
fn split_name(name: &str) -> (String, Option<u16>) {
    if name.is_empty() {
        return (String::new(), None);
    }

    // IPv6 bracketed: "[::1]:443"
    if let Some(bracket_end) = name.rfind("]:") {
        let addr = &name[..bracket_end + 1]; // "[::1]"
        let port_str = &name[bracket_end + 2..];
        return (addr.to_string(), port_str.parse().ok());
    }

    // IPv4 or wildcard: "127.0.0.1:3000", "*:8080"
    if let Some(colon_pos) = name.rfind(':') {
        let addr = &name[..colon_pos];
        let port_str = &name[colon_pos + 1..];
        return (addr.to_string(), port_str.parse().ok());
    }

    (name.to_string(), None)
}
