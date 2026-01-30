//! Output formatting utilities

use tabled::{
    settings::{Style, Width},
    Table, Tabled,
};

use crate::ipc::{MachineInfo, OrchestratorStatus, SessionInfo};

/// Format machine list as a table
pub fn format_machines(machines: &[MachineInfo], detailed: bool) -> String {
    if machines.is_empty() {
        return "No machines connected".to_string();
    }

    #[derive(Tabled)]
    struct MachineRow {
        #[tabled(rename = "ID")]
        id: String,
        #[tabled(rename = "ALIAS")]
        alias: String,
        #[tabled(rename = "HOSTNAME")]
        hostname: String,
        #[tabled(rename = "OS")]
        os: String,
        #[tabled(rename = "STATUS")]
        status: String,
        #[tabled(rename = "SESSIONS")]
        sessions: usize,
    }

    #[derive(Tabled)]
    struct MachineRowDetailed {
        #[tabled(rename = "ID")]
        id: String,
        #[tabled(rename = "ALIAS")]
        alias: String,
        #[tabled(rename = "HOSTNAME")]
        hostname: String,
        #[tabled(rename = "OS/ARCH")]
        os_arch: String,
        #[tabled(rename = "STATUS")]
        status: String,
        #[tabled(rename = "SESSIONS")]
        sessions: usize,
        #[tabled(rename = "CONNECTED")]
        connected: String,
        #[tabled(rename = "LAST HEARTBEAT")]
        heartbeat: String,
    }

    if detailed {
        let rows: Vec<MachineRowDetailed> = machines
            .iter()
            .map(|m| MachineRowDetailed {
                id: truncate(&m.id, 12),
                alias: m.alias.clone().unwrap_or_else(|| "-".to_string()),
                hostname: m.hostname.clone(),
                os_arch: format!("{}/{}", m.os, m.arch),
                status: m.status.clone(),
                sessions: m.session_count,
                connected: m.connected_at.clone().unwrap_or_else(|| "-".to_string()),
                heartbeat: m.last_heartbeat.clone().unwrap_or_else(|| "-".to_string()),
            })
            .collect();

        Table::new(rows)
            .with(Style::rounded())
            .with(Width::wrap(100))
            .to_string()
    } else {
        let rows: Vec<MachineRow> = machines
            .iter()
            .map(|m| MachineRow {
                id: truncate(&m.id, 12),
                alias: m.alias.clone().unwrap_or_else(|| "-".to_string()),
                hostname: m.hostname.clone(),
                os: m.os.clone(),
                status: m.status.clone(),
                sessions: m.session_count,
            })
            .collect();

        Table::new(rows).with(Style::rounded()).to_string()
    }
}

/// Format session list as a table
pub fn format_sessions(sessions: &[SessionInfo]) -> String {
    if sessions.is_empty() {
        return "No active sessions".to_string();
    }

    #[derive(Tabled)]
    struct SessionRow {
        #[tabled(rename = "SESSION ID")]
        id: String,
        #[tabled(rename = "MACHINE")]
        machine: String,
        #[tabled(rename = "SHELL")]
        shell: String,
        #[tabled(rename = "PID")]
        pid: String,
        #[tabled(rename = "CREATED")]
        created: String,
    }

    let rows: Vec<SessionRow> = sessions
        .iter()
        .map(|s| SessionRow {
            id: s.id.clone(),
            machine: truncate(&s.machine_id, 12),
            shell: s.shell.clone().unwrap_or_else(|| "default".to_string()),
            pid: s.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string()),
            created: s.created_at.clone(),
        })
        .collect();

    Table::new(rows).with(Style::rounded()).to_string()
}

/// Format orchestrator status
pub fn format_status(status: &OrchestratorStatus, detailed: bool) -> String {
    let mut output = String::new();

    output.push_str(&format!("Orchestrator Status: {}\n", if status.running { "Running" } else { "Stopped" }));
    output.push_str(&format!("Version: {}\n", status.version));
    output.push_str(&format!("Uptime: {}\n", format_duration(status.uptime_secs)));
    output.push_str(&format!("Connected Machines: {}\n", status.machine_count));
    output.push_str(&format!("Active Sessions: {}\n", status.session_count));

    if detailed {
        output.push_str("\n--- Detailed Metrics ---\n");
        // Additional metrics would be added here in a full implementation
    }

    output
}

/// Format duration in human-readable form
fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        let mins = secs / 60;
        let remaining_secs = secs % 60;
        format!("{}m {}s", mins, remaining_secs)
    } else if secs < 86400 {
        let hours = secs / 3600;
        let remaining_mins = (secs % 3600) / 60;
        format!("{}h {}m", hours, remaining_mins)
    } else {
        let days = secs / 86400;
        let remaining_hours = (secs % 86400) / 3600;
        format!("{}d {}h", days, remaining_hours)
    }
}

/// Truncate a string with ellipsis if too long
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Print success message in green
pub fn print_success(msg: &str) {
    use crossterm::style::{Color, Print, ResetColor, SetForegroundColor};

    let mut stdout = std::io::stdout();
    let _ = crossterm::execute!(
        stdout,
        SetForegroundColor(Color::Green),
        Print("✓ "),
        ResetColor,
        Print(msg),
        Print("\n")
    );
}

/// Print error message in red
pub fn print_error(msg: &str) {
    use crossterm::style::{Color, Print, ResetColor, SetForegroundColor};

    let mut stderr = std::io::stderr();
    let _ = crossterm::execute!(
        stderr,
        SetForegroundColor(Color::Red),
        Print("✗ "),
        ResetColor,
        Print(msg),
        Print("\n")
    );
}

/// Print warning message in yellow
pub fn print_warning(msg: &str) {
    use crossterm::style::{Color, Print, ResetColor, SetForegroundColor};

    let mut stderr = std::io::stderr();
    let _ = crossterm::execute!(
        stderr,
        SetForegroundColor(Color::Yellow),
        Print("⚠ "),
        ResetColor,
        Print(msg),
        Print("\n")
    );
}

/// Print info message
pub fn print_info(msg: &str) {
    use crossterm::style::{Color, Print, ResetColor, SetForegroundColor};

    let mut stdout = std::io::stdout();
    let _ = crossterm::execute!(
        stdout,
        SetForegroundColor(Color::Cyan),
        Print("ℹ "),
        ResetColor,
        Print(msg),
        Print("\n")
    );
}
