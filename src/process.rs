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
    let output = Command::new("lsof")
        .args(["-iTCP", "-sTCP:LISTEN", "-n", "-P"])
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
        // lsof returns exit 1 when no results — that's okay
        if output.status.code() == Some(1) && output.stdout.is_empty() {
            return Ok(Vec::new());
        }
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_lsof_output(&stdout)
}

fn parse_lsof_output(output: &str) -> Result<Vec<LsofEntry>> {
    let mut entries = Vec::new();

    for line in output.lines().skip(1) {
        // skip header
        let fields: Vec<&str> = line.split_whitespace().collect();
        // lsof output: COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME (LISTEN)
        if fields.len() < 9 {
            continue;
        }

        let command = fields[0].to_string();
        let pid: u32 = match fields[1].parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let user = fields[2].to_string();

        // TYPE field (index 4): "IPv4" or "IPv6"
        let proto = fields[4].to_string();

        // NAME field: find the field containing ':' that isn't "(LISTEN)"
        // e.g. "127.0.0.1:3000", "*:8080", "[::1]:443"
        let name = fields
            .iter()
            .rev()
            .find(|f| f.contains(':') && !f.starts_with('('))
            .copied()
            .unwrap_or("");

        let (local_addr, port) = split_name(name);

        if let Some(port) = port {
            entries.push(LsofEntry {
                pid,
                port,
                proto,
                local_addr,
                user,
                command,
            });
        }
    }

    // Deduplicate by (pid, port, proto)
    entries.sort_by(|a, b| a.pid.cmp(&b.pid).then(a.port.cmp(&b.port)).then(a.proto.cmp(&b.proto)));
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
