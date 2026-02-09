//! PTY session management
//!
//! Manages pseudo-terminal sessions using the portable-pty crate.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;

use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize, PtySystem};

use kt_protocol::{SessionId, TerminalSize};

/// Allowed shell paths for security (prevents arbitrary command execution)
const ALLOWED_SHELLS_UNIX: &[&str] = &[
    "/bin/sh",
    "/bin/bash",
    "/bin/zsh",
    "/bin/fish",
    "/bin/dash",
    "/bin/ksh",
    "/bin/tcsh",
    "/bin/csh",
    "/usr/bin/sh",
    "/usr/bin/bash",
    "/usr/bin/zsh",
    "/usr/bin/fish",
    "/usr/bin/dash",
    "/usr/bin/ksh",
    "/usr/bin/tcsh",
    "/usr/bin/csh",
    "/usr/local/bin/bash",
    "/usr/local/bin/zsh",
    "/usr/local/bin/fish",
    "/opt/homebrew/bin/bash",
    "/opt/homebrew/bin/zsh",
    "/opt/homebrew/bin/fish",
];

const ALLOWED_SHELLS_WINDOWS: &[&str] = &[
    "cmd.exe",
    "powershell.exe",
    "pwsh.exe",
    "C:\\Windows\\System32\\cmd.exe",
    "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
];

/// Validate that a shell path is allowed and exists
fn validate_shell_path(shell: &str) -> Result<String> {
    let allowed = if cfg!(windows) {
        ALLOWED_SHELLS_WINDOWS
    } else {
        ALLOWED_SHELLS_UNIX
    };

    // Check if shell is in the allowed list
    let shell_lower = shell.to_lowercase();
    let is_allowed = allowed.iter().any(|s| {
        let s_lower = s.to_lowercase();
        shell_lower == s_lower || shell_lower.ends_with(&format!("/{}", s_lower.split('/').next_back().unwrap_or("")))
    });

    if !is_allowed {
        // For Unix, also check if it's in /etc/shells
        #[cfg(unix)]
        if let Ok(shells) = std::fs::read_to_string("/etc/shells") {
            if shells.lines().any(|line| {
                let line = line.trim();
                !line.starts_with('#') && line == shell
            }) {
                // Shell is in /etc/shells, verify it exists
                if Path::new(shell).exists() {
                    return Ok(shell.to_string());
                }
            }
        }

        anyhow::bail!(
            "Shell '{}' is not in the allowed shell list. Allowed shells: {:?}",
            shell,
            allowed
        );
    }

    // Verify the shell exists on disk
    let path = Path::new(shell);
    if !path.exists() && !cfg!(windows) {
        anyhow::bail!("Shell '{}' does not exist", shell);
    }

    Ok(shell.to_string())
}

/// Manages PTY sessions on the local machine
pub struct PtyManager {
    /// The PTY system
    pty_system: Box<dyn PtySystem + Send>,
    /// Active sessions
    sessions: HashMap<SessionId, PtySession>,
    /// Default shell
    default_shell: Option<String>,
    /// Default environment variables
    default_env: Vec<(String, String)>,
}

/// A PTY session with its associated I/O handles
pub struct PtySession {
    /// Session ID
    pub session_id: SessionId,
    /// Process ID of the shell
    pub pid: Option<u32>,
    /// The PTY pair (master + slave)
    pty_pair: PtyPair,
    /// Child process handle
    child: Box<dyn portable_pty::Child + Send + Sync>,
    /// Writer to send data to the PTY
    writer: Box<dyn Write + Send>,
    /// Reader to receive data from the PTY
    reader: Box<dyn Read + Send>,
}

/// Output from a PTY session
///
/// This enum represents the possible outputs from a PTY session.
/// Currently marked as `#[allow(dead_code)]` because:
/// - The enum is part of the public API for future async PTY reading support
/// - It will be used when implementing event-driven PTY output handling
/// - Keeping it allows the API to be designed upfront without forcing immediate use
#[derive(Debug)]
#[allow(dead_code)]
pub enum PtyOutput {
    /// Data read from the PTY master
    Data(Vec<u8>),
    /// Session process exited with optional exit code
    Exited(Option<i32>),
}

impl PtyManager {
    /// Create a new PTY manager
    pub fn new() -> Self {
        Self {
            pty_system: native_pty_system(),
            sessions: HashMap::new(),
            default_shell: None,
            default_env: vec![("TERM".to_string(), "xterm-256color".to_string())],
        }
    }

    /// Create a new PTY manager with custom defaults
    pub fn with_defaults(
        default_shell: Option<String>,
        default_env: Vec<(String, String)>,
    ) -> Self {
        let mut env = vec![("TERM".to_string(), "xterm-256color".to_string())];
        env.extend(default_env);

        Self {
            pty_system: native_pty_system(),
            sessions: HashMap::new(),
            default_shell,
            default_env: env,
        }
    }

    /// Create a new PTY session
    pub fn create_session(
        &mut self,
        session_id: SessionId,
        shell: Option<String>,
        env: Vec<(String, String)>,
        size: TerminalSize,
    ) -> Result<u32> {
        tracing::info!(
            "Creating PTY session {} with size {}x{}",
            session_id,
            size.cols,
            size.rows
        );

        // Open a PTY pair
        let pty_pair = self
            .pty_system
            .openpty(PtySize {
                rows: size.rows,
                cols: size.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .with_context(|| "Failed to open PTY")?;

        // Determine which shell to use with security validation
        let requested_shell = shell
            .or_else(|| self.default_shell.clone())
            .or_else(|| std::env::var("SHELL").ok())
            .unwrap_or_else(|| {
                if cfg!(windows) {
                    "cmd.exe".to_string()
                } else {
                    "/bin/sh".to_string()
                }
            });

        // Validate the shell path for security
        let shell_path = validate_shell_path(&requested_shell)
            .with_context(|| format!("Invalid shell requested: {}", requested_shell))?;

        tracing::debug!("Using validated shell: {}", shell_path);

        // Build the command
        let mut cmd = CommandBuilder::new(&shell_path);

        // Add environment variables
        for (key, value) in &self.default_env {
            cmd.env(key, value);
        }
        for (key, value) in &env {
            cmd.env(key, value);
        }

        // Spawn the shell process
        let child = pty_pair
            .slave
            .spawn_command(cmd)
            .with_context(|| format!("Failed to spawn shell: {}", shell_path))?;

        // Get the process ID
        let pid = child.process_id();
        tracing::info!("Spawned shell process with PID: {:?}", pid);

        // Get reader/writer handles for the master side
        let reader = pty_pair
            .master
            .try_clone_reader()
            .with_context(|| "Failed to clone PTY reader")?;

        let writer = pty_pair
            .master
            .take_writer()
            .with_context(|| "Failed to take PTY writer")?;

        // Create the session
        let session = PtySession {
            session_id,
            pid,
            pty_pair,
            child,
            writer,
            reader,
        };

        self.sessions.insert(session_id, session);

        Ok(pid.unwrap_or(0))
    }

    /// Write data to a session's PTY
    pub fn write(&mut self, session_id: SessionId, data: &[u8]) -> Result<()> {
        let session = self
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        session
            .writer
            .write_all(data)
            .with_context(|| "Failed to write to PTY")?;

        session
            .writer
            .flush()
            .with_context(|| "Failed to flush PTY")?;

        Ok(())
    }

    /// Read data from a session's PTY (non-blocking)
    pub fn try_read(&mut self, session_id: SessionId, buf: &mut [u8]) -> Result<Option<usize>> {
        let session = self
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        // Use non-blocking read
        // Note: portable-pty readers are blocking by default, so we need to handle this carefully
        match session.reader.read(buf) {
            Ok(0) => Ok(None), // EOF
            Ok(n) => Ok(Some(n)),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(Some(0)),
            Err(e) => Err(e.into()),
        }
    }

    /// Take the reader from a session (for async I/O)
    pub fn take_reader(&mut self, session_id: SessionId) -> Result<Box<dyn Read + Send>> {
        let session = self
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        // We need to clone the reader - this is a limitation of portable-pty
        let reader = session
            .pty_pair
            .master
            .try_clone_reader()
            .with_context(|| "Failed to clone PTY reader")?;

        Ok(reader)
    }

    /// Resize a session's PTY
    pub fn resize(&mut self, session_id: SessionId, size: TerminalSize) -> Result<()> {
        let session = self
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        tracing::debug!(
            "Resizing session {} to {}x{}",
            session_id,
            size.cols,
            size.rows
        );

        session
            .pty_pair
            .master
            .resize(PtySize {
                rows: size.rows,
                cols: size.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .with_context(|| "Failed to resize PTY")?;

        Ok(())
    }

    /// Check if a session's process has exited
    pub fn try_wait(&mut self, session_id: SessionId) -> Result<Option<i32>> {
        let session = self
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        match session.child.try_wait() {
            Ok(Some(status)) => {
                let code = status.exit_code() as i32;
                tracing::info!("Session {} exited with code {}", session_id, code);
                Ok(Some(code))
            }
            Ok(None) => Ok(None), // Still running
            Err(e) => {
                tracing::warn!("Failed to check session {} status: {}", session_id, e);
                Err(e.into())
            }
        }
    }

    /// Close a session
    pub fn close(&mut self, session_id: SessionId) -> Option<i32> {
        tracing::info!("Closing PTY session {}", session_id);

        if let Some(mut session) = self.sessions.remove(&session_id) {
            // Try to kill the child process
            let _ = session.child.kill();

            // Wait for it to exit and get the exit code
            match session.child.wait() {
                Ok(status) => Some(status.exit_code() as i32),
                Err(_) => None,
            }
        } else {
            None
        }
    }

    /// Get a session by ID
    pub fn get(&self, session_id: SessionId) -> Option<&PtySession> {
        self.sessions.get(&session_id)
    }

    /// Get a mutable session by ID
    pub fn get_mut(&mut self, session_id: SessionId) -> Option<&mut PtySession> {
        self.sessions.get_mut(&session_id)
    }

    /// List all session IDs
    pub fn list_sessions(&self) -> Vec<SessionId> {
        self.sessions.keys().copied().collect()
    }

    /// Number of active sessions
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

impl Default for PtyManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PtySession {
    /// Get the writer for this session
    pub fn writer(&mut self) -> &mut Box<dyn Write + Send> {
        &mut self.writer
    }
}
