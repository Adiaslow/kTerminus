//! IPC client for communicating with the orchestrator
//!
//! Uses TCP on localhost for cross-platform compatibility.

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyModifiers};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use kt_core::ipc::{
    IpcEvent, IpcRequest, IpcResponse, MachineInfo, OrchestratorStatus, SessionInfo,
};

/// Default IPC port
pub const DEFAULT_IPC_PORT: u16 = 22230;

/// Get the default IPC address
pub fn default_ipc_address() -> String {
    format!("127.0.0.1:{}", DEFAULT_IPC_PORT)
}

/// Client for communicating with the orchestrator daemon
pub struct OrchestratorClient {
    address: String,
    stream: Option<TcpStream>,
}

impl OrchestratorClient {
    /// Create a new client with default address
    pub fn new() -> Self {
        Self::with_address(default_ipc_address())
    }

    /// Create a new client with custom address
    pub fn with_address(address: String) -> Self {
        Self {
            address,
            stream: None,
        }
    }

    /// Get the address
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Connect to the orchestrator
    pub async fn connect(&mut self) -> Result<()> {
        if self.stream.is_some() {
            return Ok(());
        }

        tracing::debug!("Connecting to orchestrator at {}", self.address);

        let stream = TcpStream::connect(&self.address).await.with_context(|| {
            format!(
                "Failed to connect to orchestrator at {}. Is it running?",
                self.address
            )
        })?;

        self.stream = Some(stream);
        Ok(())
    }

    /// Take ownership of the stream for interactive mode
    pub fn take_stream(&mut self) -> Option<TcpStream> {
        self.stream.take()
    }

    /// Check if the orchestrator is running
    pub async fn ping(&mut self) -> Result<bool> {
        self.connect().await?;

        match self.send_request(IpcRequest::Ping).await {
            Ok(IpcResponse::Pong) => Ok(true),
            _ => Ok(false),
        }
    }

    /// Get orchestrator status
    pub async fn status(&mut self) -> Result<OrchestratorStatus> {
        self.connect().await?;

        match self.send_request(IpcRequest::GetStatus).await? {
            IpcResponse::Status(status) => Ok(status),
            IpcResponse::Error { message } => anyhow::bail!("{}", message),
            other => anyhow::bail!("Unexpected response: {:?}", other),
        }
    }

    /// List connected machines
    pub async fn list_machines(&mut self) -> Result<Vec<MachineInfo>> {
        self.connect().await?;

        match self.send_request(IpcRequest::ListMachines).await? {
            IpcResponse::Machines { machines } => Ok(machines),
            IpcResponse::Error { message } => anyhow::bail!("{}", message),
            other => anyhow::bail!("Unexpected response: {:?}", other),
        }
    }

    /// List active sessions
    pub async fn list_sessions(&mut self, machine_id: Option<&str>) -> Result<Vec<SessionInfo>> {
        self.connect().await?;

        let request = IpcRequest::ListSessions {
            machine_id: machine_id.map(String::from),
        };

        match self.send_request(request).await? {
            IpcResponse::Sessions { sessions } => Ok(sessions),
            IpcResponse::Error { message } => anyhow::bail!("{}", message),
            other => anyhow::bail!("Unexpected response: {:?}", other),
        }
    }

    /// Create a new session on a machine
    pub async fn create_session(
        &mut self,
        machine_id: &str,
        shell: Option<&str>,
    ) -> Result<SessionInfo> {
        self.connect().await?;

        let request = IpcRequest::CreateSession {
            machine_id: machine_id.to_string(),
            shell: shell.map(String::from),
        };

        match self.send_request(request).await? {
            IpcResponse::SessionCreated(info) => Ok(info),
            IpcResponse::Error { message } => anyhow::bail!("{}", message),
            other => anyhow::bail!("Unexpected response: {:?}", other),
        }
    }

    /// Kill a session
    pub async fn kill_session(&mut self, session_id: &str, force: bool) -> Result<()> {
        self.connect().await?;

        let request = IpcRequest::CloseSession {
            session_id: session_id.to_string(),
            force,
        };

        match self.send_request(request).await? {
            IpcResponse::Ok => Ok(()),
            IpcResponse::Error { message } => anyhow::bail!("{}", message),
            other => anyhow::bail!("Unexpected response: {:?}", other),
        }
    }

    /// Subscribe to terminal output for a session
    pub async fn subscribe(&mut self, session_id: &str) -> Result<()> {
        self.connect().await?;

        let request = IpcRequest::Subscribe {
            session_id: session_id.to_string(),
        };

        match self.send_request(request).await? {
            IpcResponse::Ok => Ok(()),
            IpcResponse::Error { message } => anyhow::bail!("{}", message),
            other => anyhow::bail!("Unexpected response: {:?}", other),
        }
    }

    /// Unsubscribe from session events
    pub async fn unsubscribe(&mut self, session_id: &str) -> Result<()> {
        self.connect().await?;

        let request = IpcRequest::Unsubscribe {
            session_id: session_id.to_string(),
        };

        match self.send_request(request).await? {
            IpcResponse::Ok => Ok(()),
            IpcResponse::Error { message } => anyhow::bail!("{}", message),
            other => anyhow::bail!("Unexpected response: {:?}", other),
        }
    }

    /// Send input to a session
    pub async fn send_input(&mut self, session_id: &str, data: &[u8]) -> Result<()> {
        self.connect().await?;

        let request = IpcRequest::SessionInput {
            session_id: session_id.to_string(),
            data: data.to_vec(),
        };

        match self.send_request(request).await? {
            IpcResponse::Ok => Ok(()),
            IpcResponse::Error { message } => anyhow::bail!("{}", message),
            other => anyhow::bail!("Unexpected response: {:?}", other),
        }
    }

    /// Resize a session's terminal
    pub async fn resize_session(&mut self, session_id: &str, cols: u16, rows: u16) -> Result<()> {
        self.connect().await?;

        let request = IpcRequest::SessionResize {
            session_id: session_id.to_string(),
            cols,
            rows,
        };

        match self.send_request(request).await? {
            IpcResponse::Ok => Ok(()),
            IpcResponse::Error { message } => anyhow::bail!("{}", message),
            other => anyhow::bail!("Unexpected response: {:?}", other),
        }
    }

    /// Shutdown the orchestrator
    pub async fn shutdown(&mut self) -> Result<()> {
        self.connect().await?;

        match self.send_request(IpcRequest::Shutdown).await? {
            IpcResponse::Ok => Ok(()),
            IpcResponse::Error { message } => anyhow::bail!("{}", message),
            other => anyhow::bail!("Unexpected response: {:?}", other),
        }
    }

    /// Send a request and receive response
    async fn send_request(&mut self, request: IpcRequest) -> Result<IpcResponse> {
        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Not connected"))?;

        // Send request as JSON line
        let mut request_json = serde_json::to_string(&request)?;
        request_json.push('\n');
        stream.write_all(request_json.as_bytes()).await?;

        // Read response line
        let (reader, _writer) = stream.split();
        let mut reader = BufReader::new(reader);
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await?;

        // Parse response
        let response: IpcResponse = serde_json::from_str(&response_line)?;
        Ok(response)
    }
}

impl Default for OrchestratorClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Interactive terminal session handler
pub struct TerminalSession {
    session_id: String,
    stream: TcpStream,
}

impl TerminalSession {
    /// Create a new terminal session from a connected client
    pub async fn new(mut client: OrchestratorClient, session_id: String) -> Result<Self> {
        // Subscribe to terminal output
        client.subscribe(&session_id).await?;

        // Take the stream for interactive mode
        let stream = client
            .take_stream()
            .ok_or_else(|| anyhow::anyhow!("No connection"))?;

        Ok(Self { session_id, stream })
    }

    /// Run the interactive terminal session
    ///
    /// Returns when the user detaches (Ctrl+]) or the session closes
    pub async fn run(self) -> Result<()> {
        use crossterm::{
            event::{self, Event, KeyEvent},
            terminal::{
                disable_raw_mode, enable_raw_mode, size, EnterAlternateScreen, LeaveAlternateScreen,
            },
            ExecutableCommand,
        };
        use std::io::{stdout, Write};

        let (reader, writer) = self.stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut writer = BufWriter::new(writer);
        let session_id = self.session_id;

        // Enter raw mode
        enable_raw_mode()?;
        let mut stdout = stdout();
        stdout.execute(EnterAlternateScreen)?;

        // Send initial terminal size
        if let Ok((cols, rows)) = size() {
            let request = IpcRequest::SessionResize {
                session_id: session_id.clone(),
                cols,
                rows,
            };
            let mut json = serde_json::to_string(&request)?;
            json.push('\n');
            writer.write_all(json.as_bytes()).await?;
            writer.flush().await?;
        }

        // Create channel for terminal events
        let (event_tx, mut event_rx) = mpsc::channel::<Event>(256);

        // Spawn terminal event reader
        let event_handle = tokio::task::spawn_blocking(move || loop {
            if event::poll(std::time::Duration::from_millis(10)).unwrap_or(false) {
                if let Ok(evt) = event::read() {
                    if event_tx.blocking_send(evt).is_err() {
                        break;
                    }
                }
            }
        });

        let mut line_buf = String::new();

        loop {
            tokio::select! {
                // Handle terminal events (keyboard, resize)
                Some(evt) = event_rx.recv() => {
                    match evt {
                        Event::Key(KeyEvent { code, modifiers, .. }) => {
                            // Ctrl+] to detach
                            if modifiers.contains(KeyModifiers::CONTROL)
                                && code == KeyCode::Char(']')
                            {
                                break;
                            }

                            // Convert key to bytes and send
                            let data = key_to_bytes(code, modifiers);
                            if !data.is_empty() {
                                let request = IpcRequest::SessionInput {
                                    session_id: session_id.clone(),
                                    data,
                                };
                                let mut json = serde_json::to_string(&request)?;
                                json.push('\n');
                                writer.write_all(json.as_bytes()).await?;
                                writer.flush().await?;
                            }
                        }
                        Event::Resize(cols, rows) => {
                            let request = IpcRequest::SessionResize {
                                session_id: session_id.clone(),
                                cols,
                                rows,
                            };
                            let mut json = serde_json::to_string(&request)?;
                            json.push('\n');
                            writer.write_all(json.as_bytes()).await?;
                            writer.flush().await?;
                        }
                        _ => {}
                    }
                }

                // Handle IPC events (terminal output)
                result = reader.read_line(&mut line_buf) => {
                    match result {
                        Ok(0) => break, // EOF
                        Ok(_) => {
                            // Try to parse as IPC event
                            if let Ok(event) = serde_json::from_str::<IpcEvent>(&line_buf) {
                                match event {
                                    IpcEvent::TerminalOutput { data, .. } => {
                                        stdout.write_all(&data)?;
                                        stdout.flush()?;
                                    }
                                    IpcEvent::SessionClosed { session_id: sid } if sid == session_id => {
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                            line_buf.clear();
                        }
                        Err(e) => {
                            tracing::warn!("Error reading from IPC: {}", e);
                            break;
                        }
                    }
                }
            }
        }

        // Cleanup
        event_handle.abort();
        stdout.execute(LeaveAlternateScreen)?;
        disable_raw_mode()?;

        Ok(())
    }
}

/// Convert a key event to bytes to send to the terminal
fn key_to_bytes(code: KeyCode, modifiers: KeyModifiers) -> Vec<u8> {
    use KeyCode::*;

    match code {
        Char(c) => {
            if modifiers.contains(KeyModifiers::CONTROL) {
                // Ctrl+A = 0x01, Ctrl+B = 0x02, etc.
                let ctrl_char = (c.to_ascii_lowercase() as u8).wrapping_sub(b'a' - 1);
                vec![ctrl_char]
            } else if modifiers.contains(KeyModifiers::ALT) {
                // Alt+key sends ESC followed by the key
                vec![0x1b, c as u8]
            } else {
                c.to_string().into_bytes()
            }
        }
        Enter => vec![b'\r'],
        Tab => vec![b'\t'],
        Backspace => vec![0x7f],
        Esc => vec![0x1b],
        Up => vec![0x1b, b'[', b'A'],
        Down => vec![0x1b, b'[', b'B'],
        Right => vec![0x1b, b'[', b'C'],
        Left => vec![0x1b, b'[', b'D'],
        Home => vec![0x1b, b'[', b'H'],
        End => vec![0x1b, b'[', b'F'],
        PageUp => vec![0x1b, b'[', b'5', b'~'],
        PageDown => vec![0x1b, b'[', b'6', b'~'],
        Delete => vec![0x1b, b'[', b'3', b'~'],
        Insert => vec![0x1b, b'[', b'2', b'~'],
        F(n) => {
            // F1-F12 escape sequences
            match n {
                1 => vec![0x1b, b'O', b'P'],
                2 => vec![0x1b, b'O', b'Q'],
                3 => vec![0x1b, b'O', b'R'],
                4 => vec![0x1b, b'O', b'S'],
                5 => vec![0x1b, b'[', b'1', b'5', b'~'],
                6 => vec![0x1b, b'[', b'1', b'7', b'~'],
                7 => vec![0x1b, b'[', b'1', b'8', b'~'],
                8 => vec![0x1b, b'[', b'1', b'9', b'~'],
                9 => vec![0x1b, b'[', b'2', b'0', b'~'],
                10 => vec![0x1b, b'[', b'2', b'1', b'~'],
                11 => vec![0x1b, b'[', b'2', b'3', b'~'],
                12 => vec![0x1b, b'[', b'2', b'4', b'~'],
                _ => vec![],
            }
        }
        _ => vec![],
    }
}
