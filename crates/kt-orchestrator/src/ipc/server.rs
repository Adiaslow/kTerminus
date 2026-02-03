//! IPC server implementation
//!
//! Listens on localhost TCP for requests from the desktop app/CLI.
//! Uses TCP on 127.0.0.1 for cross-platform compatibility (works on Unix, macOS, Windows).

use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use bytes::Bytes;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use kt_core::ipc::{
    IpcEvent, IpcRequest, IpcResponse, MachineInfo, MachineStatus, OrchestratorStatus, SessionInfo,
};
use kt_protocol::TerminalSize;

use crate::connection::AgentCommand;
use crate::state::OrchestratorState;

/// IPC server for CLI/GUI communication
///
/// Listens on localhost (127.0.0.1) only - not accessible from network.
pub struct IpcServer {
    /// Address to bind (127.0.0.1:port)
    pub address: String,
    /// Orchestrator state
    state: Arc<OrchestratorState>,
    /// When the orchestrator started
    start_time: Instant,
    /// Event broadcast channel
    event_tx: broadcast::Sender<IpcEvent>,
    /// Cancellation token for shutdown
    shutdown_token: Option<CancellationToken>,
}

impl IpcServer {
    /// Create a new IPC server
    pub fn new(address: String, state: Arc<OrchestratorState>) -> Self {
        let (event_tx, _) = broadcast::channel(1024);
        Self {
            address,
            state,
            start_time: Instant::now(),
            event_tx,
            shutdown_token: None,
        }
    }

    /// Set the shutdown token (call before run)
    pub fn with_shutdown_token(mut self, token: CancellationToken) -> Self {
        self.shutdown_token = Some(token);
        self
    }

    /// Get a sender for broadcasting events
    pub fn event_sender(&self) -> broadcast::Sender<IpcEvent> {
        self.event_tx.clone()
    }

    /// Start the IPC server
    pub async fn run(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.address)
            .await
            .with_context(|| format!("Failed to bind IPC server to {}", self.address))?;

        tracing::info!("IPC server listening on {}", self.address);

        loop {
            match listener.accept().await {
                Ok((stream, peer_addr)) => {
                    // Only accept connections from localhost
                    if !peer_addr.ip().is_loopback() {
                        tracing::warn!("Rejected non-localhost connection from {}", peer_addr);
                        continue;
                    }

                    let state = Arc::clone(&self.state);
                    let start_time = self.start_time;
                    let event_tx = self.event_tx.clone();
                    let shutdown_token = self.shutdown_token.clone();

                    tokio::spawn(async move {
                        if let Err(e) =
                            handle_client(stream, state, start_time, event_tx, shutdown_token).await
                        {
                            tracing::warn!("IPC client error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    tracing::error!("Failed to accept IPC connection: {}", e);
                }
            }
        }
    }
}

/// State for a single IPC client connection
struct ClientState {
    /// Session IDs this client has subscribed to for terminal output
    subscribed_sessions: std::collections::HashSet<String>,
}

impl ClientState {
    fn new() -> Self {
        Self {
            subscribed_sessions: std::collections::HashSet::new(),
        }
    }

    /// Check if this client should receive the given event
    fn should_receive_event(&self, event: &IpcEvent) -> bool {
        match event {
            // Terminal output is only sent to subscribed clients
            IpcEvent::TerminalOutput { session_id, .. } => {
                self.subscribed_sessions.contains(session_id)
            }
            // All other events are broadcast to all clients
            _ => true,
        }
    }
}

async fn handle_client(
    stream: TcpStream,
    state: Arc<OrchestratorState>,
    start_time: Instant,
    event_tx: broadcast::Sender<IpcEvent>,
    shutdown_token: Option<CancellationToken>,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let mut client_state = ClientState::new();

    // Subscribe to events
    let mut event_rx = event_tx.subscribe();

    loop {
        tokio::select! {
            // Handle incoming requests
            result = reader.read_line(&mut line) => {
                match result {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            line.clear();
                            continue;
                        }

                        let response = match serde_json::from_str::<IpcRequest>(trimmed) {
                            Ok(request) => handle_request_with_state(
                                request,
                                &state,
                                start_time,
                                &mut client_state,
                                shutdown_token.as_ref(),
                            ).await,
                            Err(e) => IpcResponse::Error {
                                message: format!("Invalid request: {}", e),
                            },
                        };

                        let mut response_json = serde_json::to_string(&response)?;
                        response_json.push('\n');
                        writer.write_all(response_json.as_bytes()).await?;

                        line.clear();
                    }
                    Err(e) => {
                        return Err(e.into());
                    }
                }
            }

            // Forward events to client (filtered by subscription)
            result = event_rx.recv() => {
                match result {
                    Ok(event) => {
                        if client_state.should_receive_event(&event) {
                            let mut event_json = serde_json::to_string(&event)?;
                            event_json.push('\n');
                            writer.write_all(event_json.as_bytes()).await?;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("IPC client lagged by {} events", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn handle_request_with_state(
    request: IpcRequest,
    state: &OrchestratorState,
    start_time: Instant,
    client_state: &mut ClientState,
    shutdown_token: Option<&CancellationToken>,
) -> IpcResponse {
    // Handle subscription requests that modify client state
    match &request {
        IpcRequest::Subscribe { session_id } => {
            client_state.subscribed_sessions.insert(session_id.clone());
            tracing::debug!("Client subscribed to session {}", session_id);
            return IpcResponse::Ok;
        }
        IpcRequest::Unsubscribe { session_id } => {
            client_state.subscribed_sessions.remove(session_id);
            tracing::debug!("Client unsubscribed from session {}", session_id);
            return IpcResponse::Ok;
        }
        _ => {}
    }

    // Handle all other requests
    handle_request(request, state, start_time, shutdown_token).await
}

async fn handle_request(
    request: IpcRequest,
    state: &OrchestratorState,
    start_time: Instant,
    shutdown_token: Option<&CancellationToken>,
) -> IpcResponse {
    match request {
        IpcRequest::GetStatus => {
            let machines = state.connections.list();
            let sessions = state.sessions.list();

            IpcResponse::Status(OrchestratorStatus {
                running: true,
                uptime_secs: start_time.elapsed().as_secs(),
                machine_count: machines.len(),
                session_count: sessions.len(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                tailscale_hostname: state.config.tailscale_hostname.clone(),
                bind_address: state.config.bind_address.clone(),
            })
        }

        IpcRequest::ListMachines => {
            let connections = state.connections.list();
            let machines: Vec<MachineInfo> = connections
                .iter()
                .map(|conn| {
                    let session_count = state.sessions.list_for_machine(&conn.machine_id).len();
                    MachineInfo {
                        id: conn.machine_id.to_string(),
                        alias: conn.alias.clone(),
                        hostname: conn
                            .hostname
                            .clone()
                            .unwrap_or_else(|| conn.machine_id.to_string()),
                        os: conn.os.clone(),
                        arch: conn.arch.clone(),
                        status: MachineStatus::Connected,
                        connected_at: None,
                        last_heartbeat: None,
                        session_count,
                        tags: vec![],
                    }
                })
                .collect();

            IpcResponse::Machines { machines }
        }

        IpcRequest::GetMachine { machine_id } => {
            let machine_id_parsed = kt_core::MachineId::new(machine_id.clone());
            match state.connections.get(&machine_id_parsed) {
                Some(conn) => {
                    let session_count = state.sessions.list_for_machine(&conn.machine_id).len();
                    IpcResponse::Machine(MachineInfo {
                        id: conn.machine_id.to_string(),
                        alias: conn.alias.clone(),
                        hostname: conn
                            .hostname
                            .clone()
                            .unwrap_or_else(|| conn.machine_id.to_string()),
                        os: conn.os.clone(),
                        arch: conn.arch.clone(),
                        status: MachineStatus::Connected,
                        connected_at: None,
                        last_heartbeat: None,
                        session_count,
                        tags: vec![],
                    })
                }
                None => IpcResponse::Error {
                    message: format!("Machine not found: {}", machine_id),
                },
            }
        }

        IpcRequest::ListSessions { machine_id } => {
            let sessions = if let Some(mid) = machine_id {
                let machine_id_parsed = kt_core::MachineId::new(mid);
                state.sessions.list_for_machine(&machine_id_parsed)
            } else {
                state.sessions.list()
            };

            let session_infos: Vec<SessionInfo> = sessions
                .iter()
                .map(|s| SessionInfo {
                    id: s.id.to_string(),
                    machine_id: s.machine_id.to_string(),
                    shell: s.shell.clone(),
                    created_at: s.created_at_iso(),
                    pid: s.pid(),
                    size: None,
                })
                .collect();

            IpcResponse::Sessions {
                sessions: session_infos,
            }
        }

        IpcRequest::CreateSession { machine_id, shell } => {
            let machine_id_parsed = kt_core::MachineId::new(machine_id.clone());

            // Get the connection for this machine
            let Some(conn) = state.connections.get(&machine_id_parsed) else {
                return IpcResponse::Error {
                    message: format!("Machine not found: {}", machine_id),
                };
            };

            // Create a new session in the session manager
            let session_id = state
                .sessions
                .create(machine_id_parsed.clone(), shell.clone());

            // Send create session command to the agent
            let command = AgentCommand::CreateSession {
                session_id,
                shell,
                env: vec![],
                size: TerminalSize::default(),
            };

            if let Err(e) = conn.command_tx.send(command).await {
                // Remove the session since creation failed
                state.sessions.remove(session_id);
                return IpcResponse::Error {
                    message: format!("Failed to send command to agent: {}", e),
                };
            }

            tracing::info!("Created session {} on machine {}", session_id, machine_id);

            // Get the session to retrieve created_at
            let created_at = state
                .sessions
                .get(session_id)
                .map(|s| s.created_at_iso())
                .unwrap_or_default();

            IpcResponse::SessionCreated(SessionInfo {
                id: session_id.to_string(),
                machine_id,
                shell: None,
                created_at,
                pid: None,
                size: None,
            })
        }

        IpcRequest::SessionInput { session_id, data } => {
            // Look up the session to find which machine it belongs to
            let Some(session) = state.sessions.get_by_string_id(&session_id) else {
                return IpcResponse::Error {
                    message: format!("Session not found: {}", session_id),
                };
            };

            // Get the connection for this machine
            let Some(conn) = state.connections.get(&session.machine_id) else {
                return IpcResponse::Error {
                    message: format!("Machine not connected: {}", session.machine_id),
                };
            };

            // Send input command to the agent
            let command = AgentCommand::SessionInput {
                session_id: session.id,
                data: Bytes::from(data),
            };

            if let Err(e) = conn.command_tx.send(command).await {
                return IpcResponse::Error {
                    message: format!("Failed to send input to agent: {}", e),
                };
            }

            IpcResponse::Ok
        }

        IpcRequest::SessionResize {
            session_id,
            cols,
            rows,
        } => {
            // Look up the session to find which machine it belongs to
            let Some(session) = state.sessions.get_by_string_id(&session_id) else {
                return IpcResponse::Error {
                    message: format!("Session not found: {}", session_id),
                };
            };

            // Get the connection for this machine
            let Some(conn) = state.connections.get(&session.machine_id) else {
                return IpcResponse::Error {
                    message: format!("Machine not connected: {}", session.machine_id),
                };
            };

            // Send resize command to the agent
            let command = AgentCommand::SessionResize {
                session_id: session.id,
                size: TerminalSize::new(rows, cols),
            };

            if let Err(e) = conn.command_tx.send(command).await {
                return IpcResponse::Error {
                    message: format!("Failed to send resize to agent: {}", e),
                };
            }

            tracing::debug!("Resized session {} to {}x{}", session_id, cols, rows);
            IpcResponse::Ok
        }

        IpcRequest::CloseSession {
            session_id,
            force: _,
        } => {
            // Look up the session to find which machine it belongs to
            let Some(session) = state.sessions.get_by_string_id(&session_id) else {
                return IpcResponse::Error {
                    message: format!("Session not found: {}", session_id),
                };
            };

            // Get the connection for this machine
            let Some(conn) = state.connections.get(&session.machine_id) else {
                // Machine disconnected - just remove the session
                state.sessions.remove(session.id);
                return IpcResponse::Ok;
            };

            // Send close command to the agent
            let command = AgentCommand::CloseSession {
                session_id: session.id,
            };

            if let Err(e) = conn.command_tx.send(command).await {
                tracing::warn!("Failed to send close to agent: {}", e);
            }

            // Remove from session manager
            state.sessions.remove(session.id);

            tracing::info!("Closed session {}", session_id);
            IpcResponse::Ok
        }

        // Subscribe/Unsubscribe are handled in handle_request_with_state
        IpcRequest::Subscribe { .. } | IpcRequest::Unsubscribe { .. } => {
            // This shouldn't be reached - handled by handle_request_with_state
            IpcResponse::Ok
        }

        IpcRequest::DisconnectMachine { machine_id } => {
            let machine_id_parsed = kt_core::MachineId::new(machine_id.clone());

            // Get the connection and disconnect it
            let Some(conn) = state.connections.get(&machine_id_parsed) else {
                return IpcResponse::Error {
                    message: format!("Machine not found: {}", machine_id),
                };
            };

            // Signal the connection to close
            conn.disconnect();

            // Remove from connection pool
            state.connections.remove(&machine_id_parsed);

            tracing::info!("Disconnected machine {}", machine_id);
            IpcResponse::Ok
        }

        IpcRequest::Ping => IpcResponse::Pong,

        IpcRequest::Shutdown => {
            tracing::info!("Shutdown requested via IPC");
            if let Some(token) = shutdown_token {
                token.cancel();
                IpcResponse::Ok
            } else {
                IpcResponse::Error {
                    message: "Shutdown not supported (no shutdown token configured)".to_string(),
                }
            }
        }
    }
}
