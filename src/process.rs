use std::collections::HashMap;
use std::path::{Path, PathBuf};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ListenScope {
    Localhost,
    PrivateNetwork,
    Public,
    Unknown,
}

impl ListenScope {
    pub fn label(self) -> &'static str {
        match self {
            ListenScope::Localhost => "LOCAL",
            ListenScope::PrivateNetwork => "LAN",
            ListenScope::Public => "PUBLIC",
            ListenScope::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

impl RiskLevel {
    pub fn label(self) -> &'static str {
        match self {
            RiskLevel::Low => "LOW",
            RiskLevel::Medium => "MED",
            RiskLevel::High => "HIGH",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendedAction {
    Keep,
    Inspect,
    CloseIfUnused,
}

impl RecommendedAction {
    pub fn label(self) -> &'static str {
        match self {
            RecommendedAction::Keep => "KEEP",
            RecommendedAction::Inspect => "CHECK",
            RecommendedAction::CloseIfUnused => "CLOSE",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceRelation {
    CurrentWorkspace,
    ExternalProject,
    AppBundle,
    SystemPath,
    ExternalBinary,
    Unknown,
}

impl WorkspaceRelation {
    pub fn label(self) -> &'static str {
        match self {
            WorkspaceRelation::CurrentWorkspace => "WORKSPACE",
            WorkspaceRelation::ExternalProject => "PROJECT",
            WorkspaceRelation::AppBundle => "APP",
            WorkspaceRelation::SystemPath => "SYSTEM",
            WorkspaceRelation::ExternalBinary => "EXTERNAL",
            WorkspaceRelation::Unknown => "UNKNOWN",
        }
    }
}

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
    pub listen_scope: ListenScope,
    pub risk_level: RiskLevel,
    pub inferred_kind: String,
    pub project_type: String,
    pub project_root: String,
    pub exe_path: String,
    pub cwd: String,
    pub parent_pid: Option<u32>,
    pub parent_command: String,
    pub guidance: String,
    pub recommended_action: RecommendedAction,
    pub workspace_relation: WorkspaceRelation,
    pub origin_summary: String,
}

impl ProcessInfo {
    pub fn matches_filter_lower(&self, query: &str) -> bool {
        let query = query.trim();
        if query.is_empty() {
            return true;
        }

        self.port.to_string().contains(query)
            || self.pid.to_string().contains(query)
            || self.app.to_lowercase().contains(query)
            || self.command.to_lowercase().contains(query)
            || self.local_addr.to_lowercase().contains(query)
            || self.proto.to_lowercase().contains(query)
            || self.username.to_lowercase().contains(query)
            || self.listen_scope.label().to_lowercase().contains(query)
            || self.risk_level.label().to_lowercase().contains(query)
            || self.inferred_kind.to_lowercase().contains(query)
            || self.project_type.to_lowercase().contains(query)
            || self.project_root.to_lowercase().contains(query)
            || self.exe_path.to_lowercase().contains(query)
            || self.cwd.to_lowercase().contains(query)
            || self.parent_command.to_lowercase().contains(query)
            || self.recommended_action.label().to_lowercase().contains(query)
            || self.workspace_relation.label().to_lowercase().contains(query)
            || self.origin_summary.to_lowercase().contains(query)
    }
}

// ---------------------------------------------------------------------------
// ProcessScanner
// ---------------------------------------------------------------------------

pub struct ProcessScanner {
    system: System,
    last_known: Vec<ProcessInfo>,
    workspace_dir: Option<PathBuf>,
}

impl ProcessScanner {
    pub fn new() -> Self {
        let workspace_dir = std::env::current_dir().ok();
        Self {
            system: System::new(),
            last_known: Vec::new(),
            workspace_dir,
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
                .with_cmd(UpdateKind::OnlyIfNotSet)
                .with_cwd(UpdateKind::OnlyIfNotSet)
                .with_exe(UpdateKind::OnlyIfNotSet)
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
            let (cpu, memory_mb, status, sysinfo_user, full_cmd, start_time, exe_path, cwd, parent_pid, parent_command) =
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
                    let exe_path = proc
                        .exe()
                        .map(path_to_string)
                        .unwrap_or_default();
                    let cwd = proc
                        .cwd()
                        .map(path_to_string)
                        .unwrap_or_default();
                    let parent_pid = proc.parent().map(|pid| pid.as_u32());
                    let parent_command = proc
                        .parent()
                        .and_then(|pid| self.system.process(pid))
                        .map(|parent| {
                            let parts: Vec<String> = parent
                                .cmd()
                                .iter()
                                .map(|s| s.to_string_lossy().into_owned())
                                .collect();
                            if parts.is_empty() {
                                parent.name().to_string_lossy().into_owned()
                            } else {
                                parts.join(" ")
                            }
                        })
                        .unwrap_or_default();

                    (
                        proc.cpu_usage(),
                        mem,
                        stat,
                        uname,
                        cmd,
                        proc.start_time(),
                        exe_path,
                        cwd,
                        parent_pid,
                        parent_command,
                    )
                } else {
                    (
                        0.0,
                        0.0,
                        "Unknown".to_string(),
                        String::new(),
                        entry.command.clone(),
                        0,
                        String::new(),
                        String::new(),
                        None,
                        String::new(),
                    )
                };

            // Prefer lsof username (always available), fall back to sysinfo
            let username = if entry.user.is_empty() {
                sysinfo_user
            } else {
                entry.user
            };

            let listen_scope = infer_listen_scope(&entry.local_addr);
            let project = infer_project_origin(&cwd, &exe_path);
            let inferred_kind = infer_process_kind(
                &entry.command,
                &full_cmd,
                &exe_path,
                &cwd,
                project.as_ref().map(|(_, kind)| *kind),
            );
            let risk_level =
                infer_risk_level(listen_scope, &username, project.as_ref().map(|(_, kind)| *kind), &inferred_kind);
            let workspace_relation = infer_workspace_relation(
                self.workspace_dir.as_deref(),
                &cwd,
                &exe_path,
                project.as_ref().map(|(root, _)| root.as_path()),
            );
            let origin_summary = build_origin_summary(
                workspace_relation,
                project.as_ref(),
                &exe_path,
                &cwd,
                &parent_command,
            );
            let recommended_action = infer_recommended_action(
                listen_scope,
                risk_level,
                workspace_relation,
                &inferred_kind,
            );
            let guidance = build_guidance(
                listen_scope,
                risk_level,
                recommended_action,
                workspace_relation,
                &inferred_kind,
                project.as_ref(),
            );
            let (project_root, project_type) = project
                .map(|(root, kind)| (path_to_string(&root), kind.to_string()))
                .unwrap_or_default();

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
                listen_scope,
                risk_level,
                inferred_kind,
                project_type,
                project_root,
                exe_path,
                cwd,
                parent_pid,
                parent_command,
                guidance,
                recommended_action,
                workspace_relation,
                origin_summary,
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
            if proc.kill() {
                return Ok(());
            }
        }

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

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn infer_listen_scope(local_addr: &str) -> ListenScope {
    let addr = local_addr
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();

    if addr.is_empty() {
        return ListenScope::Unknown;
    }

    if matches!(addr.as_str(), "*" | "0.0.0.0" | "::") {
        return ListenScope::Public;
    }

    if addr == "localhost" || addr == "::1" || addr.starts_with("127.") {
        return ListenScope::Localhost;
    }

    if addr.starts_with("10.")
        || addr.starts_with("192.168.")
        || addr.starts_with("169.254.")
        || addr.starts_with("fe80:")
    {
        return ListenScope::PrivateNetwork;
    }

    if let Some(second_octet) = addr
        .strip_prefix("172.")
        .and_then(|rest| rest.split('.').next())
        .and_then(|part| part.parse::<u8>().ok())
    {
        if (16..=31).contains(&second_octet) {
            return ListenScope::PrivateNetwork;
        }
    }

    ListenScope::Public
}

fn infer_project_origin(cwd: &str, exe_path: &str) -> Option<(PathBuf, &'static str)> {
    find_project_root(Path::new(cwd)).or_else(|| {
        Path::new(exe_path)
            .parent()
            .and_then(find_project_root)
    })
}

fn find_project_root(start: &Path) -> Option<(PathBuf, &'static str)> {
    if start.as_os_str().is_empty() {
        return None;
    }

    const MARKERS: [(&str, &str); 6] = [
        ("go.mod", "Go"),
        ("Cargo.toml", "Rust"),
        ("package.json", "Node"),
        ("pyproject.toml", "Python"),
        ("Gemfile", "Ruby"),
        ("pom.xml", "Java"),
    ];

    start.ancestors().find_map(|dir| {
        MARKERS.iter().find_map(|(marker, kind)| {
            dir.join(marker)
                .exists()
                .then(|| (dir.to_path_buf(), *kind))
        })
    })
}

fn infer_process_kind(
    app: &str,
    command: &str,
    exe_path: &str,
    cwd: &str,
    project_type: Option<&str>,
) -> String {
    let haystack = format!("{app} {command} {exe_path} {cwd}").to_ascii_lowercase();

    if haystack.contains("docker-proxy") || haystack.contains("com.docker") {
        return "Docker published port".to_string();
    }
    if (haystack.contains("electron")
        || haystack.contains(" helper ")
        || haystack.contains(".app/contents/frameworks/"))
        && (haystack.contains("node")
            || haystack.contains("chromium")
            || haystack.contains("renderer")
            || haystack.contains("utility"))
    {
        return "Electron app".to_string();
    }
    if haystack.contains("nginx") {
        return "nginx".to_string();
    }
    if haystack.contains("postgres") || haystack.contains("postmaster") {
        return "PostgreSQL".to_string();
    }
    if haystack.contains("redis") {
        return "Redis".to_string();
    }
    if haystack.contains("mysqld") || haystack.contains("mysql") {
        return "MySQL".to_string();
    }
    if haystack.contains("node")
        || haystack.contains("bun ")
        || haystack.contains("deno ")
        || matches!(project_type, Some("Node"))
    {
        return "Node service".to_string();
    }
    if haystack.contains("python")
        || haystack.contains("uvicorn")
        || haystack.contains("gunicorn")
        || matches!(project_type, Some("Python"))
    {
        return "Python service".to_string();
    }
    if haystack.contains("java")
        || haystack.contains("spring")
        || haystack.contains("jetty")
        || matches!(project_type, Some("Java"))
    {
        return "Java service".to_string();
    }
    if haystack.contains("ruby")
        || haystack.contains("puma")
        || haystack.contains("rails")
        || matches!(project_type, Some("Ruby"))
    {
        return "Ruby service".to_string();
    }
    if haystack.contains("go run ") {
        return "Go service (go run)".to_string();
    }
    if matches!(project_type, Some("Go")) {
        return "Go service".to_string();
    }
    if exe_path.starts_with("/System/")
        || exe_path.starts_with("/usr/libexec/")
        || exe_path.starts_with("/usr/sbin/")
    {
        return "macOS system service".to_string();
    }
    if exe_path.contains(".app/Contents/") {
        return "macOS app".to_string();
    }
    if exe_path.contains("/target/") || matches!(project_type, Some("Rust")) {
        return "Rust service".to_string();
    }
    if !exe_path.is_empty() {
        return "Native binary".to_string();
    }
    "Unknown listener".to_string()
}

fn infer_risk_level(
    scope: ListenScope,
    username: &str,
    project_type: Option<&str>,
    inferred_kind: &str,
) -> RiskLevel {
    let privileged = matches!(username, "root" | "_www" | "_postgres");
    match scope {
        ListenScope::Public if privileged => RiskLevel::High,
        ListenScope::Public if project_type.is_none() && inferred_kind == "Unknown listener" => {
            RiskLevel::High
        }
        ListenScope::Public => RiskLevel::Medium,
        ListenScope::PrivateNetwork if privileged => RiskLevel::Medium,
        ListenScope::PrivateNetwork => RiskLevel::Medium,
        ListenScope::Localhost => RiskLevel::Low,
        ListenScope::Unknown => RiskLevel::Medium,
    }
}

fn infer_workspace_relation(
    workspace_dir: Option<&Path>,
    cwd: &str,
    exe_path: &str,
    project_root: Option<&Path>,
) -> WorkspaceRelation {
    if let Some(workspace_dir) = workspace_dir {
        if let Some(project_root) = project_root {
            if same_or_nested(workspace_dir, project_root) || same_or_nested(project_root, workspace_dir) {
                return WorkspaceRelation::CurrentWorkspace;
            }
        }

        let cwd_path = Path::new(cwd);
        if !cwd.is_empty() && same_or_nested(workspace_dir, cwd_path) {
            return WorkspaceRelation::CurrentWorkspace;
        }

        let exe = Path::new(exe_path);
        if !exe_path.is_empty() && same_or_nested(workspace_dir, exe) {
            return WorkspaceRelation::CurrentWorkspace;
        }
    }

    if project_root.is_some() {
        return WorkspaceRelation::ExternalProject;
    }

    if exe_path.starts_with("/System/")
        || exe_path.starts_with("/usr/libexec/")
        || exe_path.starts_with("/usr/sbin/")
    {
        return WorkspaceRelation::SystemPath;
    }

    if exe_path.contains(".app/Contents/") {
        return WorkspaceRelation::AppBundle;
    }

    if !exe_path.is_empty() {
        return WorkspaceRelation::ExternalBinary;
    }

    WorkspaceRelation::Unknown
}

fn same_or_nested(base: &Path, candidate: &Path) -> bool {
    candidate.starts_with(base)
}

fn build_origin_summary(
    relation: WorkspaceRelation,
    project: Option<&(PathBuf, &'static str)>,
    exe_path: &str,
    cwd: &str,
    parent_command: &str,
) -> String {
    match relation {
        WorkspaceRelation::CurrentWorkspace => {
            if let Some((root, kind)) = project {
                format!("{kind} workspace: {}", path_to_string(root))
            } else if !cwd.is_empty() {
                format!("Current workspace path: {cwd}")
            } else {
                "Current workspace process".to_string()
            }
        }
        WorkspaceRelation::ExternalProject => {
            if let Some((root, kind)) = project {
                format!("External {kind} project: {}", path_to_string(root))
            } else {
                "External project".to_string()
            }
        }
        WorkspaceRelation::SystemPath => {
            if !exe_path.is_empty() {
                format!("macOS system path: {exe_path}")
            } else {
                "macOS system process".to_string()
            }
        }
        WorkspaceRelation::AppBundle => {
            let app_root = exe_path
                .split(".app/Contents/")
                .next()
                .map(|prefix| format!("{prefix}.app"))
                .unwrap_or_else(|| exe_path.to_string());
            format!("App bundle: {app_root}")
        }
        WorkspaceRelation::ExternalBinary => {
            if !cwd.is_empty() {
                format!("External binary from {cwd}")
            } else if !exe_path.is_empty() {
                format!("External binary: {exe_path}")
            } else if !parent_command.is_empty() {
                format!("Spawned by {parent_command}")
            } else {
                "External binary".to_string()
            }
        }
        WorkspaceRelation::Unknown => {
            if !parent_command.is_empty() {
                format!("Parent: {parent_command}")
            } else {
                "Origin unclear".to_string()
            }
        }
    }
}

fn infer_recommended_action(
    scope: ListenScope,
    risk: RiskLevel,
    relation: WorkspaceRelation,
    inferred_kind: &str,
) -> RecommendedAction {
    match (scope, risk, relation) {
        (_, RiskLevel::High, _) => RecommendedAction::CloseIfUnused,
        (ListenScope::Public, _, WorkspaceRelation::CurrentWorkspace) => RecommendedAction::Inspect,
        (ListenScope::Public, _, WorkspaceRelation::SystemPath | WorkspaceRelation::AppBundle) => {
            RecommendedAction::Inspect
        }
        (ListenScope::Public, _, _) => RecommendedAction::CloseIfUnused,
        (ListenScope::PrivateNetwork, _, WorkspaceRelation::CurrentWorkspace) => RecommendedAction::Inspect,
        (ListenScope::PrivateNetwork, _, WorkspaceRelation::SystemPath | WorkspaceRelation::AppBundle) => {
            RecommendedAction::Keep
        }
        (ListenScope::Localhost, _, WorkspaceRelation::CurrentWorkspace) => RecommendedAction::Keep,
        (ListenScope::Localhost, _, WorkspaceRelation::SystemPath | WorkspaceRelation::AppBundle) => {
            RecommendedAction::Keep
        }
        (ListenScope::Localhost, _, _) if inferred_kind.contains("Docker") => RecommendedAction::Inspect,
        (ListenScope::Localhost, _, WorkspaceRelation::Unknown) => RecommendedAction::Inspect,
        (ListenScope::Unknown, _, _) => RecommendedAction::Inspect,
        _ => RecommendedAction::Inspect,
    }
}

fn build_guidance(
    scope: ListenScope,
    risk: RiskLevel,
    action: RecommendedAction,
    relation: WorkspaceRelation,
    inferred_kind: &str,
    project: Option<&(PathBuf, &'static str)>,
) -> String {
    match (scope, risk, action, relation) {
        (
            ListenScope::Localhost,
            RiskLevel::Low,
            RecommendedAction::Keep,
            WorkspaceRelation::CurrentWorkspace,
        ) if project.is_some() => {
            format!("Current-workspace {inferred_kind}; keeping it is usually correct unless you are freeing the port.")
        }
        (ListenScope::Localhost, _, RecommendedAction::Keep, WorkspaceRelation::SystemPath | WorkspaceRelation::AppBundle) => {
            "Local-only app/system listener; usually safe to leave running unless it is blocking the port you need."
                .to_string()
        }
        (ListenScope::PrivateNetwork, _, RecommendedAction::Keep, _) => {
            "Visible on your private network but looks expected; keep it only if LAN exposure is intentional."
                .to_string()
        }
        (_, RiskLevel::High, RecommendedAction::CloseIfUnused, _) => {
            "High-risk listener; if you are not actively using it, close it and verify why it was started."
                .to_string()
        }
        (ListenScope::Public, _, RecommendedAction::CloseIfUnused, _) => {
            "Public listener outside the current workspace; close it if you do not explicitly need remote access."
                .to_string()
        }
        (ListenScope::Public, _, RecommendedAction::Inspect, _) => {
            "Public listener; inspect bind address, project root, and parent process before deciding to keep it."
                .to_string()
        }
        (ListenScope::PrivateNetwork, _, RecommendedAction::Inspect, _) => {
            "LAN-visible listener; inspect whether this should stay private-network accessible or bind to localhost instead."
                .to_string()
        }
        (ListenScope::Localhost, _, RecommendedAction::Inspect, _) => {
            "Local-only listener with unclear ownership; inspect binary path, cwd, and parent process before killing."
                .to_string()
        }
        (ListenScope::Unknown, _, _, _) => {
            "Exposure is unclear; inspect the binary path, cwd, and parent process before acting."
                .to_string()
        }
        _ => "Inspect the process details before deciding whether to keep or close the port.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        build_origin_summary, find_project_root, infer_listen_scope, infer_process_kind,
        infer_recommended_action, infer_workspace_relation, split_name, ListenScope,
        RecommendedAction, RiskLevel, WorkspaceRelation,
    };

    #[test]
    fn split_name_handles_ipv4_ipv6_and_wildcard() {
        assert_eq!(split_name("127.0.0.1:3000"), ("127.0.0.1".to_string(), Some(3000)));
        assert_eq!(split_name("[::1]:8080"), ("[::1]".to_string(), Some(8080)));
        assert_eq!(split_name("*:443"), ("*".to_string(), Some(443)));
    }

    #[test]
    fn infer_scope_distinguishes_local_private_and_public() {
        assert_eq!(infer_listen_scope("127.0.0.1"), ListenScope::Localhost);
        assert_eq!(infer_listen_scope("[::1]"), ListenScope::Localhost);
        assert_eq!(infer_listen_scope("192.168.1.9"), ListenScope::PrivateNetwork);
        assert_eq!(infer_listen_scope("0.0.0.0"), ListenScope::Public);
        assert_eq!(infer_listen_scope("*"), ListenScope::Public);
    }

    #[test]
    fn infer_kind_uses_project_type_and_runtime_hints() {
        assert_eq!(
            infer_process_kind("api", "go run ./cmd/api", "", "", Some("Go")),
            "Go service (go run)"
        );
        assert_eq!(
            infer_process_kind("node", "node server.js", "", "", None),
            "Node service"
        );
        assert_eq!(
            infer_process_kind(
                "Discord Helper (Renderer)",
                "/Applications/Discord.app/Contents/Frameworks/Discord Helper (Renderer).app/Contents/MacOS/Discord Helper (Renderer) --type=renderer",
                "/Applications/Discord.app/Contents/Frameworks/Discord Helper (Renderer).app/Contents/MacOS/Discord Helper (Renderer)",
                "/",
                None,
            ),
            "Electron app"
        );
        assert_eq!(
            infer_process_kind("svc", "", "/tmp/app/target/debug/svc", "", None),
            "Rust service"
        );
    }

    #[test]
    fn workspace_relation_detects_current_workspace() {
        let workspace = Path::new("/tmp/project");
        let relation = infer_workspace_relation(
            Some(workspace),
            "/tmp/project/cmd/api",
            "/tmp/project/bin/api",
            Some(Path::new("/tmp/project")),
        );
        assert_eq!(relation, WorkspaceRelation::CurrentWorkspace);
    }

    #[test]
    fn workspace_relation_does_not_treat_root_cwd_as_current_workspace() {
        let workspace = Path::new("/tmp/project");
        let relation = infer_workspace_relation(
            Some(workspace),
            "/",
            "/Applications/Ollama.app/Contents/Resources/ollama",
            None,
        );
        assert_eq!(relation, WorkspaceRelation::AppBundle);
    }

    #[test]
    fn recommended_action_prefers_close_for_public_unknown() {
        let action = infer_recommended_action(
            ListenScope::Public,
            RiskLevel::Medium,
            WorkspaceRelation::ExternalBinary,
            "Unknown listener",
        );
        assert_eq!(action, RecommendedAction::CloseIfUnused);
    }

    #[test]
    fn origin_summary_mentions_workspace_project() {
        let root = PathBuf::from("/tmp/project");
        let summary = build_origin_summary(
            WorkspaceRelation::CurrentWorkspace,
            Some(&(root.clone(), "Go")),
            "",
            "",
            "",
        );
        assert_eq!(summary, "Go workspace: /tmp/project");
    }

    #[test]
    fn find_project_root_walks_up_tree() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("portman-test-{unique}"));
        let nested = root.join("cmd").join("api");
        fs::create_dir_all(&nested).unwrap();
        fs::write(root.join("go.mod"), "module example.com/test\n").unwrap();

        let found = find_project_root(&nested).unwrap();
        assert_eq!(found.0, root);
        assert_eq!(found.1, "Go");

        let _ = fs::remove_dir_all(&found.0);
    }
}
