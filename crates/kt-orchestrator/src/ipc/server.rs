//! IPC server implementation
//!
//! Listens on localhost TCP for requests from the desktop app/CLI.
//! Uses TCP on 127.0.0.1 for cross-platform compatibility (works on Unix, macOS, Windows).
//!
//! # Security Features
//!
//! The IPC server implements several security measures:
//!
//! - **Localhost-only binding**: Only accepts connections from 127.0.0.1/::1
//! - **Input validation**: Session input is limited to 64KB to prevent memory exhaustion
//! - **Session ownership**: Sessions are tracked by creating client, with access control
//! - **Request validation**: All JSON requests are validated before processing
//!
//! # Input Size Limits
//!
//! The `MAX_SESSION_INPUT_SIZE` constant (64KB) limits terminal input per request.
//! This prevents:
//! - Memory exhaustion from malicious clients sending huge payloads
//! - Buffer overflow from accidentally pasting large content
//! - DoS attacks via resource consumption
//!
//! When input exceeds this limit, the request is rejected with an error and
//! the connection remains open for subsequent valid requests.
//!
//! # Session Ownership
//!
//! Sessions track their creating client via `owner_client_id`. This enables:
//! - Only the creating client can subscribe to session output
//! - Session cleanup when the owning client disconnects
//! - Audit logging of session operations

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use bytes::Bytes;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use kt_core::ipc::{
    IpcEvent, IpcEventEnvelope, IpcRequest, IpcResponse, MachineInfo, MachineStatus,
    OrchestratorStatus, SessionInfo,
};
use kt_protocol::TerminalSize;

use crate::connection::AgentCommand;
use crate::session::SessionState;
use crate::state::OrchestratorState;

/// Validate an environment variable name.
///
/// Valid names must:
/// - Start with a letter (a-z, A-Z) or underscore (_)
/// - Contain only alphanumeric characters (a-z, A-Z, 0-9) and underscores
/// - Not be empty
///
/// This prevents environment variable injection attacks where malicious
/// variable names could affect shell behavior or security.
fn is_valid_env_var_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    let mut chars = name.chars();

    // First character must be letter or underscore
    let first = match chars.next() {
        Some(c) => c,
        None => return false,
    };

    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }

    // Rest must be alphanumeric or underscore
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Validate a list of environment variables.
///
/// Returns `Ok(())` if all variables are valid, or an error message
/// describing the first invalid variable found.
fn validate_env_vars(env: &[(String, String)]) -> Result<(), String> {
    for (name, _value) in env {
        if !is_valid_env_var_name(name) {
            return Err(format!(
                "Invalid environment variable name '{}': must start with letter or underscore, \
                 and contain only alphanumeric characters and underscores",
                name
            ));
        }
    }
    Ok(())
}

/// Validate that a client has permission to access a session.
///
/// Returns `Ok(())` if the client is allowed to access the session:
/// - The client owns the session (owner_client_id matches)
/// - The session has no owner (public session)
///
/// Returns `Err(IpcResponse::Error)` if the session is owned by another client.
fn validate_ownership(
    session: &crate::session::SessionHandle,
    client_id: &str,
) -> Result<(), IpcResponse> {
    match &session.owner_client_id {
        Some(owner) if owner != client_id => Err(IpcResponse::Error {
            message: "Permission denied: session owned by another client".into(),
        }),
        _ => Ok(()),
    }
}

/// Maximum size for session input data (64KB).
///
/// This limit prevents memory exhaustion attacks and protects against:
/// - Malicious clients sending huge payloads
/// - Accidental large pastes causing memory pressure
/// - DoS via resource consumption
///
/// The limit is enforced in `handle_request()` when processing `SessionInput`.
/// Requests exceeding this limit are rejected with a clear error message,
/// and the connection remains open for subsequent valid requests.
///
/// # Value Choice
///
/// 64KB is sufficient for any realistic terminal interaction:
/// - Normal typing: single characters
/// - Command pasting: typically < 4KB
/// - Large file paths: < 4KB
/// - Environment variables: typically < 32KB
///
/// Larger inputs (> 64KB) are almost certainly:
/// - Malicious (DoS attempt)
/// - Accidental (pasting a binary file)
/// - Misuse (should use file transfer instead)
const MAX_SESSION_INPUT_SIZE: usize = 65536;

/// Maximum concurrent IPC connections.
///
/// This prevents resource exhaustion from too many connected clients.
/// The limit is generous for normal usage (CLI, desktop app, scripts)
/// but prevents runaway connections from bugs or attacks.
const MAX_IPC_CONNECTIONS: u32 = 100;

/// Rate limit: maximum requests per second per client.
///
/// This prevents individual clients from overwhelming the server.
/// The limit is high enough for interactive terminal use but
/// low enough to prevent DoS from malicious clients.
const RATE_LIMIT_REQUESTS_PER_SECOND: u32 = 1000;

/// Authentication rate limit: maximum failed auth attempts per minute.
///
/// This is stricter than the general rate limit to prevent brute-force
/// attacks on the authentication token. After this many failures in a
/// minute, the client is temporarily locked out from authentication.
const AUTH_RATE_LIMIT_FAILURES_PER_MINUTE: u32 = 10;

/// Lockout duration after exceeding auth failure limit.
const AUTH_LOCKOUT_DURATION_SECS: u64 = 60;

/// Minimum terminal columns/rows for resize requests.
///
/// A terminal with 0 columns or rows would be unusable. The minimum of 1
/// allows for edge cases while preventing completely invalid sizes.
const MIN_TERMINAL_SIZE: u16 = 1;

/// Maximum terminal columns/rows for resize requests.
///
/// 10,000 is far larger than any realistic terminal size (typical max is 300-500).
/// This limit prevents resource exhaustion from extremely large buffers while
/// still allowing for unusual but valid use cases like virtual terminals.
const MAX_TERMINAL_SIZE: u16 = 10000;

/// Broadcast channel capacity for IPC events.
///
/// This determines how many events can be queued before slow clients start
/// lagging. When a client lags:
/// - The lagged events are skipped (not delivered)
/// - An `EventsDropped` notification is sent to the client
/// - The client should refresh its state to resynchronize
///
/// # Value Choice
///
/// 1024 provides a buffer for:
/// - Burst terminal output (many small events in quick succession)
/// - Multiple concurrent sessions generating output
/// - Brief client pauses (e.g., UI rendering, GC)
///
/// Too small: Frequent lag events, poor UX with fast output
/// Too large: Memory usage grows with slow/hung clients
///
/// Consider making this configurable via orchestrator config if deployments
/// have significantly different workload characteristics.
const IPC_EVENT_CHANNEL_CAPACITY: usize = 1024;

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
    /// Event broadcast channel (uses IpcEventEnvelope for sequencing)
    event_tx: broadcast::Sender<IpcEventEnvelope>,
    /// Cancellation token for shutdown
    shutdown_token: Option<CancellationToken>,
    /// Current number of active connections (for rate limiting)
    active_connections: Arc<AtomicU32>,
    /// Authentication token for IPC clients
    auth_token: String,
}

impl IpcServer {
    /// Create a new IPC server
    ///
    /// Attempts to acquire ownership of the IPC token. If another orchestrator
    /// is already running and owns the token, returns an error.
    ///
    /// Clients must read this token and authenticate before making requests.
    pub fn new(address: String, state: Arc<OrchestratorState>) -> Result<Self> {
        let (event_tx, _) = broadcast::channel(IPC_EVENT_CHANNEL_CAPACITY);

        // Acquire token ownership - this ensures we don't overwrite a running orchestrator's token
        let auth_token = match kt_core::acquire_token_ownership(&address)
            .context("Failed to acquire IPC token ownership")?
        {
            kt_core::TokenOwnership::Acquired { token } => {
                tracing::info!("Acquired IPC token ownership");
                token
            }
            kt_core::TokenOwnership::External { pid, address: ext_addr, .. } => {
                // Another orchestrator is running - we should not start
                return Err(anyhow::anyhow!(
                    "Another orchestrator (PID {}) is already running at {}. \
                     Stop it first or connect to it instead.",
                    pid,
                    ext_addr
                ));
            }
        };

        Ok(Self {
            address,
            state,
            start_time: Instant::now(),
            event_tx,
            shutdown_token: None,
            active_connections: Arc::new(AtomicU32::new(0)),
            auth_token,
        })
    }

    /// Set the shutdown token (call before run)
    pub fn with_shutdown_token(mut self, token: CancellationToken) -> Self {
        self.shutdown_token = Some(token);
        self
    }

    /// Get a sender for broadcasting events
    pub fn event_sender(&self) -> broadcast::Sender<IpcEventEnvelope> {
        self.event_tx.clone()
    }

    /// Get the authentication token
    ///
    /// This is primarily for testing purposes. In production, clients
    /// should read the token from the token file.
    pub fn auth_token(&self) -> &str {
        &self.auth_token
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
                        // Explicitly close the stream to prevent half-open connection accumulation
                        drop(stream);
                        continue;
                    }

                    // Check connection limit
                    let current = self.active_connections.load(Ordering::SeqCst);
                    if current >= MAX_IPC_CONNECTIONS {
                        tracing::warn!(
                            "Rejected IPC connection: limit exceeded ({}/{})",
                            current,
                            MAX_IPC_CONNECTIONS
                        );
                        // Drop the stream to close the connection
                        continue;
                    }

                    // Increment connection counter
                    self.active_connections.fetch_add(1, Ordering::SeqCst);

                    let state = Arc::clone(&self.state);
                    let start_time = self.start_time;
                    let event_tx = self.event_tx.clone();
                    let shutdown_token = self.shutdown_token.clone();
                    let active_connections = Arc::clone(&self.active_connections);
                    let auth_token = self.auth_token.clone();

                    tokio::spawn(async move {
                        let result =
                            handle_client(stream, state, start_time, event_tx, shutdown_token, auth_token).await;

                        // Decrement connection counter on disconnect
                        active_connections.fetch_sub(1, Ordering::SeqCst);

                        if let Err(e) = result {
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
    /// Unique identifier for this TCP connection (for logging/debugging)
    connection_id: String,
    /// Logical client ID (survives reconnections, used for session ownership)
    /// Set when client provides client_id during authentication.
    /// If None, falls back to connection_id for legacy behavior.
    logical_client_id: Option<String>,
    /// Whether this client has authenticated
    authenticated: bool,
    /// Session IDs this client has subscribed to for terminal output
    subscribed_sessions: std::collections::HashSet<String>,
    /// Session IDs this client has created (for ownership tracking)
    owned_sessions: std::collections::HashSet<String>,
    /// Rate limiter state: request count in current window
    request_count: u32,
    /// Rate limiter state: start of current window
    rate_window_start: Instant,
    /// Auth rate limiter state: failed auth attempts in current window
    auth_failure_count: u32,
    /// Auth rate limiter state: start of current auth window
    auth_window_start: Instant,
    /// Auth lockout: time until which this client is locked out from auth
    auth_lockout_until: Option<Instant>,
}

impl ClientState {
    fn new() -> Self {
        // Generate a unique connection ID using timestamp and random component
        // Use monotonic Instant for uniqueness as fallback if system time fails
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros())
            .unwrap_or_else(|_| {
                // System time before UNIX epoch is extremely rare but possible
                // Use a fallback based on random values
                tracing::warn!("System time before UNIX epoch, using random connection ID");
                rand::random::<u128>()
            });
        let connection_id = format!("conn-{}-{}", timestamp, rand::random::<u32>());
        let now = Instant::now();
        Self {
            connection_id,
            logical_client_id: None, // Set during authentication if client provides one
            authenticated: false,
            subscribed_sessions: std::collections::HashSet::new(),
            owned_sessions: std::collections::HashSet::new(),
            request_count: 0,
            rate_window_start: now,
            auth_failure_count: 0,
            auth_window_start: now,
            auth_lockout_until: None,
        }
    }

    /// Get the effective client ID for session ownership.
    /// Uses logical_client_id if set (survives reconnections), otherwise connection_id.
    fn effective_client_id(&self) -> &str {
        self.logical_client_id.as_deref().unwrap_or(&self.connection_id)
    }

    /// Check if the request is allowed under rate limiting.
    /// Returns true if allowed, false if rate limited.
    fn check_rate_limit(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.rate_window_start);

        // Reset window if more than 1 second has passed
        if elapsed.as_secs() >= 1 {
            self.request_count = 0;
            self.rate_window_start = now;
        }

        // Check if under limit
        if self.request_count >= RATE_LIMIT_REQUESTS_PER_SECOND {
            return false;
        }

        // Increment and allow
        self.request_count += 1;
        true
    }

    /// Check if authentication is allowed (not locked out due to too many failures).
    /// Returns true if allowed, false if locked out.
    fn check_auth_rate_limit(&mut self) -> bool {
        let now = Instant::now();

        // Check if currently locked out
        if let Some(lockout_until) = self.auth_lockout_until {
            if now < lockout_until {
                // Still locked out
                return false;
            }
            // Lockout expired, reset
            self.auth_lockout_until = None;
            self.auth_failure_count = 0;
            self.auth_window_start = now;
        }

        // Check if the auth window has expired (1 minute)
        let elapsed = now.duration_since(self.auth_window_start);
        if elapsed.as_secs() >= 60 {
            self.auth_failure_count = 0;
            self.auth_window_start = now;
        }

        // Check if under limit
        self.auth_failure_count < AUTH_RATE_LIMIT_FAILURES_PER_MINUTE
    }

    /// Record a failed authentication attempt.
    /// If the limit is exceeded, sets a lockout period.
    fn record_auth_failure(&mut self) {
        let now = Instant::now();

        // Check if the auth window has expired (1 minute)
        let elapsed = now.duration_since(self.auth_window_start);
        if elapsed.as_secs() >= 60 {
            self.auth_failure_count = 0;
            self.auth_window_start = now;
        }

        self.auth_failure_count += 1;

        // Check if we've exceeded the limit
        if self.auth_failure_count >= AUTH_RATE_LIMIT_FAILURES_PER_MINUTE {
            self.auth_lockout_until =
                Some(now + std::time::Duration::from_secs(AUTH_LOCKOUT_DURATION_SECS));
            tracing::warn!(
                "Connection {} locked out for {} seconds due to {} failed auth attempts",
                self.connection_id,
                AUTH_LOCKOUT_DURATION_SECS,
                self.auth_failure_count
            );
        }
    }

    /// Check if this client should receive the given event envelope
    fn should_receive_event(&self, envelope: &IpcEventEnvelope) -> bool {
        match &envelope.event {
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
    event_tx: broadcast::Sender<IpcEventEnvelope>,
    shutdown_token: Option<CancellationToken>,
    auth_token: String,
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

                        // Check rate limit before processing request
                        let response = if !client_state.check_rate_limit() {
                            tracing::warn!(
                                "Rate limit exceeded for connection {} ({} req/s)",
                                client_state.connection_id,
                                RATE_LIMIT_REQUESTS_PER_SECOND
                            );
                            IpcResponse::Error {
                                message: format!(
                                    "Rate limit exceeded: max {} requests per second",
                                    RATE_LIMIT_REQUESTS_PER_SECOND
                                ),
                            }
                        } else {
                            match serde_json::from_str::<IpcRequest>(trimmed) {
                                Ok(request) => {
                                    // Handle authentication
                                    match &request {
                                        IpcRequest::Authenticate { token, client_id } => {
                                            // Check auth-specific rate limit first
                                            if !client_state.check_auth_rate_limit() {
                                                tracing::warn!(
                                                    "Auth rate limit exceeded for connection {}",
                                                    client_state.connection_id
                                                );
                                                IpcResponse::Error {
                                                    message: format!(
                                                        "Too many failed authentication attempts. Try again in {} seconds.",
                                                        AUTH_LOCKOUT_DURATION_SECS
                                                    ),
                                                }
                                            } else if kt_core::validate_ipc_token(token, &auth_token) {
                                                client_state.authenticated = true;

                                                // Set logical client ID if provided (for session ownership)
                                                if let Some(id) = client_id {
                                                    client_state.logical_client_id = Some(id.clone());
                                                    // Reclaim any orphaned sessions for this client
                                                    reclaim_orphaned_sessions(&state, id, &mut client_state);
                                                }

                                                tracing::debug!(
                                                    "Connection {} authenticated (logical_client: {:?})",
                                                    client_state.connection_id,
                                                    client_state.logical_client_id
                                                );
                                                // Return epoch info for client synchronization
                                                IpcResponse::Authenticated {
                                                    epoch_id: state.epoch.epoch_id_string(),
                                                    current_seq: state.epoch.current_sequence(),
                                                }
                                            } else {
                                                // Record the failed attempt for auth rate limiting
                                                client_state.record_auth_failure();
                                                tracing::warn!(
                                                    "Connection {} authentication failed ({} failures). Expected: {}...{}, got: {}...{}",
                                                    client_state.connection_id,
                                                    client_state.auth_failure_count,
                                                    &auth_token[..8],
                                                    &auth_token[auth_token.len()-8..],
                                                    &token[..std::cmp::min(8, token.len())],
                                                    if token.len() > 8 { &token[token.len()-8..] } else { "" }
                                                );
                                                IpcResponse::Error {
                                                    message: "Invalid authentication token".to_string(),
                                                }
                                            }
                                        }
                                        // Ping is allowed without authentication (for health checks)
                                        IpcRequest::Ping => IpcResponse::Pong,
                                        // VerifyPairingCode is allowed without auth (for agent discovery)
                                        IpcRequest::VerifyPairingCode { ref code } => {
                                            IpcResponse::PairingCodeValid {
                                                valid: state.verify_pairing_code(code),
                                            }
                                        }
                                        // All other requests require authentication
                                        _ if !client_state.authenticated => {
                                            IpcResponse::AuthenticationRequired
                                        }
                                        // Authenticated - process normally
                                        _ => handle_request_with_state(
                                            request,
                                            &state,
                                            start_time,
                                            &mut client_state,
                                            shutdown_token.as_ref(),
                                        ).await,
                                    }
                                }
                                Err(e) => IpcResponse::Error {
                                    message: format!("Invalid request: {}", e),
                                },
                            }
                        };

                        let mut response_json = serde_json::to_string(&response)?;
                        response_json.push('\n');
                        writer.write_all(response_json.as_bytes()).await?;

                        line.clear();
                    }
                    Err(e) => {
                        // Clean up owned sessions before returning error
                        cleanup_owned_sessions(&state, &client_state);
                        return Err(e.into());
                    }
                }
            }

            // Forward events to client (filtered by subscription, only if authenticated)
            result = event_rx.recv() => {
                match result {
                    Ok(envelope) => {
                        // Only send events to authenticated clients
                        if client_state.authenticated && client_state.should_receive_event(&envelope) {
                            let mut event_json = serde_json::to_string(&envelope)?;
                            event_json.push('\n');
                            writer.write_all(event_json.as_bytes()).await?;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        // Issue #9: Handle event queue lag by logging and notifying client
                        tracing::warn!(
                            "IPC connection {} lagged by {} events - client may have missed updates",
                            client_state.connection_id,
                            n
                        );
                        // Send a sync notification to the client so they know to refresh state
                        // Wrap in IpcEventEnvelope for consistency
                        if client_state.authenticated {
                            let sync_envelope = state.epoch.wrap_event(IpcEvent::EventsDropped { count: n as u32 });
                            if let Ok(mut event_json) = serde_json::to_string(&sync_envelope) {
                                event_json.push('\n');
                                // Best effort - don't fail if we can't send the notification
                                let _ = writer.write_all(event_json.as_bytes()).await;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        }
    }

    // Issue #10: Clean up sessions owned by this client when they disconnect
    cleanup_owned_sessions(&state, &client_state);

    Ok(())
}

/// Get current time in milliseconds since UNIX epoch.
fn current_time_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Mark sessions as orphaned when a client disconnects.
///
/// Instead of immediately deleting sessions, we mark them as orphaned with a timestamp.
/// This allows sessions to be reclaimed if the client reconnects within the grace period.
/// Sessions that remain orphaned after the grace period are cleaned up by the cleanup task.
fn cleanup_owned_sessions(state: &OrchestratorState, client_state: &ClientState) {
    if client_state.owned_sessions.is_empty() {
        return;
    }

    let effective_id = client_state.effective_client_id();
    let now = current_time_millis();

    tracing::info!(
        "Marking {} sessions as orphaned for disconnecting client {} (connection: {})",
        client_state.owned_sessions.len(),
        effective_id,
        client_state.connection_id
    );

    // Mark sessions as orphaned by owner_client_id (not just the ones in owned_sessions)
    // This handles the case where the client's owned_sessions set might be incomplete
    // Use coordinator.sessions for proper state management
    for session in state.coordinator.sessions.list() {
        if session.owner_client_id.as_deref() == Some(effective_id) {
            // Use try_orphan for CAS-based state transition
            if session.try_orphan(now) {
                tracing::debug!(
                    "Marked session {} as orphaned (owner: {})",
                    session.id,
                    effective_id
                );
            }
        }
    }
}

/// Reclaim orphaned sessions when a client reconnects.
///
/// Called during authentication when a client provides a logical client ID.
/// Finds all sessions owned by this client that are currently orphaned and reclaims them.
fn reclaim_orphaned_sessions(
    state: &OrchestratorState,
    client_id: &str,
    client_state: &mut ClientState,
) {
    let mut reclaimed_count = 0;

    // Use coordinator.sessions for proper state management
    for session in state.coordinator.sessions.list() {
        if session.owner_client_id.as_deref() == Some(client_id) {
            // Use try_reclaim for CAS-based state transition
            if session.is_orphaned() {
                if session.try_reclaim() {
                    tracing::info!(
                        "Session {} reclaimed by reconnected client {}",
                        session.id,
                        client_id
                    );
                    reclaimed_count += 1;
                }
            }
            // Track in this connection's owned_sessions
            client_state.owned_sessions.insert(session.id.to_string());
        }
    }

    if reclaimed_count > 0 {
        tracing::info!(
            "Client {} reclaimed {} orphaned sessions",
            client_id,
            reclaimed_count
        );
    }
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
            // Verify the session exists
            // Use coordinator.sessions for proper state management
            let Some(session) = state.coordinator.sessions.get_by_string_id(session_id) else {
                return IpcResponse::Error {
                    message: format!("Session not found: {}", session_id),
                };
            };

            // Check ownership: allow if client owns the session, or if session has no owner
            if let Err(err) = validate_ownership(&session, client_state.effective_client_id()) {
                tracing::warn!(
                    "Connection {} (client: {:?}) attempted to subscribe to session {} owned by {:?}",
                    client_state.connection_id,
                    client_state.logical_client_id,
                    session_id,
                    session.owner_client_id
                );
                return err;
            }

            client_state.subscribed_sessions.insert(session_id.clone());
            tracing::debug!(
                "Connection {} subscribed to session {}",
                client_state.connection_id,
                session_id
            );
            return IpcResponse::Ok;
        }
        IpcRequest::Unsubscribe { session_id } => {
            client_state.subscribed_sessions.remove(session_id);
            tracing::debug!(
                "Connection {} unsubscribed from session {}",
                client_state.connection_id,
                session_id
            );
            return IpcResponse::Ok;
        }
        _ => {}
    }

    // Handle all other requests with client state for ownership tracking
    handle_request_with_client(request, state, start_time, client_state, shutdown_token).await
}

/// Handle requests that need client state for ownership tracking
async fn handle_request_with_client(
    request: IpcRequest,
    state: &OrchestratorState,
    start_time: Instant,
    client_state: &mut ClientState,
    shutdown_token: Option<&CancellationToken>,
) -> IpcResponse {
    // Handle CreateSession specially to track ownership
    // Use coordinator.connections and coordinator.sessions for proper state management
    if let IpcRequest::CreateSession { machine_id, shell } = request {
        // Look up by machine ID or alias
        let Some(conn) = state.coordinator.connections.get_by_id_or_alias(&machine_id) else {
            return IpcResponse::Error {
                message: format!("Machine not found: {}", machine_id),
            };
        };

        // Use the actual machine ID from the connection (in case lookup was by alias)
        let machine_id_parsed = conn.machine_id.clone();

        // Environment variables to pass to the session.
        // Currently empty, but this is where custom env vars would be added.
        // They must be validated before being sent to the agent.
        let env: Vec<(String, String)> = vec![];

        // Validate environment variable names to prevent injection attacks
        if let Err(e) = validate_env_vars(&env) {
            return IpcResponse::Error { message: e };
        }

        // Create a new session with this client as owner
        // Use effective_client_id (logical ID if set, otherwise connection ID)
        let owner_id = client_state.effective_client_id().to_string();
        let session_id = state.coordinator.sessions.create_with_owner(
            machine_id_parsed.clone(),
            shell.clone(),
            Some(owner_id.clone()),
        );

        // Track ownership in client state
        client_state.owned_sessions.insert(session_id.to_string());

        // Send create session command to the agent
        let command = AgentCommand::CreateSession {
            session_id,
            shell,
            env,
            size: TerminalSize::default(),
        };

        if let Err(e) = conn.command_tx.send(command).await {
            // Remove the session since creation failed
            state.coordinator.sessions.remove(session_id);
            client_state.owned_sessions.remove(&session_id.to_string());
            return IpcResponse::Error {
                message: format!("Failed to send command to agent: {}", e),
            };
        }

        tracing::info!(
            "Created session {} on machine {} (owner: {}, connection: {})",
            session_id,
            machine_id,
            owner_id,
            client_state.connection_id
        );

        // Get the session to retrieve created_at
        let created_at = state
            .coordinator
            .sessions
            .get(session_id)
            .map(|s| s.created_at_iso())
            .unwrap_or_default();

        return IpcResponse::SessionCreated(SessionInfo {
            id: session_id.to_string(),
            machine_id,
            shell: None,
            created_at,
            pid: None,
            size: None,
        });
    }

    // Handle SessionInput with ownership validation
    if let IpcRequest::SessionInput { session_id, data } = &request {
        // Validate input size to prevent memory exhaustion
        if data.len() > MAX_SESSION_INPUT_SIZE {
            return IpcResponse::Error {
                message: format!(
                    "Session input too large: {} bytes (max {})",
                    data.len(),
                    MAX_SESSION_INPUT_SIZE
                ),
            };
        }

        // Look up the session to find which machine it belongs to
        let Some(session) = state.coordinator.sessions.get_by_string_id(session_id) else {
            return IpcResponse::Error {
                message: format!("Session not found: {}", session_id),
            };
        };

        // Validate ownership
        if let Err(err) = validate_ownership(&session, client_state.effective_client_id()) {
            return err;
        }

        // Check session state
        if session.state() == SessionState::Closing {
            return IpcResponse::Error {
                message: "Session is closing".into(),
            };
        }

        // Get the connection for this machine
        let Some(conn) = state.coordinator.connections.get(&session.machine_id) else {
            return IpcResponse::Error {
                message: format!("Machine not connected: {}", session.machine_id),
            };
        };

        // Send input command to the agent
        let command = AgentCommand::SessionInput {
            session_id: session.id,
            data: Bytes::from(data.clone()),
        };

        if let Err(e) = conn.command_tx.send(command).await {
            return IpcResponse::Error {
                message: format!("Failed to send input to agent: {}", e),
            };
        }

        return IpcResponse::Ok;
    }

    // Handle SessionResize with ownership validation
    if let IpcRequest::SessionResize {
        session_id,
        cols,
        rows,
    } = &request
    {
        // Issue #13: Validate terminal resize dimensions
        let valid_range = MIN_TERMINAL_SIZE..=MAX_TERMINAL_SIZE;
        if !valid_range.contains(cols) {
            return IpcResponse::Error {
                message: format!(
                    "Invalid terminal columns: {} (must be {}-{})",
                    cols, MIN_TERMINAL_SIZE, MAX_TERMINAL_SIZE
                ),
            };
        }
        if !valid_range.contains(rows) {
            return IpcResponse::Error {
                message: format!(
                    "Invalid terminal rows: {} (must be {}-{})",
                    rows, MIN_TERMINAL_SIZE, MAX_TERMINAL_SIZE
                ),
            };
        }

        // Look up the session to find which machine it belongs to
        let Some(session) = state.coordinator.sessions.get_by_string_id(session_id) else {
            return IpcResponse::Error {
                message: format!("Session not found: {}", session_id),
            };
        };

        // Validate ownership
        if let Err(err) = validate_ownership(&session, client_state.effective_client_id()) {
            return err;
        }

        // Check session state
        if session.state() == SessionState::Closing {
            return IpcResponse::Error {
                message: "Session is closing".into(),
            };
        }

        // Get the connection for this machine
        let Some(conn) = state.coordinator.connections.get(&session.machine_id) else {
            return IpcResponse::Error {
                message: format!("Machine not connected: {}", session.machine_id),
            };
        };

        // Send resize command to the agent
        let command = AgentCommand::SessionResize {
            session_id: session.id,
            size: TerminalSize::new(*rows, *cols),
        };

        if let Err(e) = conn.command_tx.send(command).await {
            return IpcResponse::Error {
                message: format!("Failed to send resize to agent: {}", e),
            };
        }

        tracing::debug!("Resized session {} to {}x{}", session_id, cols, rows);
        return IpcResponse::Ok;
    }

    // Handle CloseSession with ownership validation
    if let IpcRequest::CloseSession { session_id, force: _ } = &request {
        // Look up the session to find which machine it belongs to
        let Some(session) = state.coordinator.sessions.get_by_string_id(session_id) else {
            return IpcResponse::Error {
                message: format!("Session not found: {}", session_id),
            };
        };

        // Validate ownership
        if let Err(err) = validate_ownership(&session, client_state.effective_client_id()) {
            return err;
        }

        // Check if already closing (idempotent - return success)
        if session.state() == SessionState::Closing {
            return IpcResponse::Ok;
        }

        // Transition to Closing state
        session.try_close();

        // Get the connection for this machine
        let Some(conn) = state.coordinator.connections.get(&session.machine_id) else {
            // Machine disconnected - just remove the session
            state.coordinator.sessions.remove(session.id);
            client_state.owned_sessions.remove(session_id);
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
        state.coordinator.sessions.remove(session.id);
        client_state.owned_sessions.remove(session_id);

        tracing::info!("Closed session {}", session_id);
        return IpcResponse::Ok;
    }

    // All other requests don't need client state
    handle_request(request, state, start_time, shutdown_token).await
}

async fn handle_request(
    request: IpcRequest,
    state: &OrchestratorState,
    start_time: Instant,
    shutdown_token: Option<&CancellationToken>,
) -> IpcResponse {
    // Use coordinator.connections and coordinator.sessions for proper state management
    match request {
        IpcRequest::GetStatus => {
            let machines = state.coordinator.connections.list();
            let sessions = state.coordinator.sessions.list();

            IpcResponse::Status(OrchestratorStatus {
                running: true,
                uptime_secs: start_time.elapsed().as_secs(),
                machine_count: machines.len(),
                session_count: sessions.len(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                tailscale_hostname: state.config.tailscale_hostname.clone(),
                bind_address: state.config.bind_address.clone(),
                pairing_code: Some(state.pairing_code().to_string()),
            })
        }

        IpcRequest::ListMachines => {
            let connections = state.coordinator.connections.list();
            let machines: Vec<MachineInfo> = connections
                .iter()
                .map(|conn| {
                    let session_count = state.coordinator.sessions.list_for_machine(&conn.machine_id).len();
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
            // Look up by machine ID or alias
            match state.coordinator.connections.get_by_id_or_alias(&machine_id) {
                Some(conn) => {
                    let session_count = state.coordinator.sessions.list_for_machine(&conn.machine_id).len();
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
                // Resolve alias to actual machine ID if needed
                let actual_machine_id = state
                    .coordinator
                    .connections
                    .get_by_id_or_alias(&mid)
                    .map(|conn| conn.machine_id.clone())
                    .unwrap_or_else(|| kt_core::MachineId::new(mid));
                state.coordinator.sessions.list_for_machine(&actual_machine_id)
            } else {
                state.coordinator.sessions.list()
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

        // CreateSession is handled in handle_request_with_client for ownership tracking
        IpcRequest::CreateSession { .. } => {
            // This branch should not be reached - CreateSession goes through handle_request_with_client
            IpcResponse::Error {
                message: "Internal error: CreateSession should be handled with client state".to_string(),
            }
        }

        // SessionInput is handled in handle_request_with_client for ownership validation
        IpcRequest::SessionInput { .. } => {
            // This branch should not be reached - SessionInput goes through handle_request_with_client
            IpcResponse::Error {
                message: "Internal error: SessionInput should be handled with client state".to_string(),
            }
        }

        // SessionResize is handled in handle_request_with_client for ownership validation
        IpcRequest::SessionResize { .. } => {
            // This branch should not be reached - SessionResize goes through handle_request_with_client
            IpcResponse::Error {
                message: "Internal error: SessionResize should be handled with client state".to_string(),
            }
        }

        // CloseSession is handled in handle_request_with_client for ownership validation
        IpcRequest::CloseSession { .. } => {
            // This branch should not be reached - CloseSession goes through handle_request_with_client
            IpcResponse::Error {
                message: "Internal error: CloseSession should be handled with client state".to_string(),
            }
        }

        // Subscribe/Unsubscribe are handled in handle_request_with_state
        IpcRequest::Subscribe { .. } | IpcRequest::Unsubscribe { .. } => {
            // This shouldn't be reached - handled by handle_request_with_state
            IpcResponse::Ok
        }

        IpcRequest::DisconnectMachine { machine_id } => {
            // Look up by machine ID or alias
            let Some(conn) = state.coordinator.connections.get_by_id_or_alias(&machine_id) else {
                return IpcResponse::Error {
                    message: format!("Machine not found: {}", machine_id),
                };
            };

            // Get the actual machine ID (in case lookup was by alias)
            let actual_machine_id = conn.machine_id.clone();

            // Signal the connection to close
            conn.disconnect();

            // Remove from connection pool
            state.coordinator.connections.remove(&actual_machine_id);

            tracing::info!("Disconnected machine {} (requested: {})", actual_machine_id, machine_id);
            IpcResponse::Ok
        }

        IpcRequest::Ping => IpcResponse::Pong,

        // Authenticate is handled in handle_client before this function is called
        // This branch should never be reached but is needed for exhaustiveness
        IpcRequest::Authenticate { .. } => IpcResponse::Error {
            message: "Internal error: Authenticate should be handled before this point".to_string(),
        },

        IpcRequest::GetPairingCode => {
            IpcResponse::PairingCode {
                code: state.pairing_code().to_string(),
            }
        }

        IpcRequest::VerifyPairingCode { code } => {
            IpcResponse::PairingCodeValid {
                valid: state.verify_pairing_code(&code),
            }
        }

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

        IpcRequest::GetStateSnapshot => {
            // Get all machines
            let connections = state.coordinator.connections.list();
            let machines: Vec<MachineInfo> = connections
                .iter()
                .map(|conn| {
                    let session_count = state.coordinator.sessions.list_for_machine(&conn.machine_id).len();
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

            // Get all sessions
            let all_sessions = state.coordinator.sessions.list();
            let sessions: Vec<SessionInfo> = all_sessions
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

            IpcResponse::StateSnapshot {
                epoch_id: state.epoch.epoch_id_string(),
                current_seq: state.epoch.current_sequence(),
                machines,
                sessions,
            }
        }

        IpcRequest::GetEventsSince { since_seq: _ } => {
            // TODO: Implement event buffer for replay
            // For now, return empty with truncated=true to force full resync
            IpcResponse::EventsSince {
                events: vec![],
                truncated: true,
                oldest_available_seq: Some(state.epoch.current_sequence()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_env_var_names() {
        // Valid names
        assert!(is_valid_env_var_name("PATH"));
        assert!(is_valid_env_var_name("_PATH"));
        assert!(is_valid_env_var_name("my_var"));
        assert!(is_valid_env_var_name("MY_VAR_123"));
        assert!(is_valid_env_var_name("_"));
        assert!(is_valid_env_var_name("_123"));
        assert!(is_valid_env_var_name("a"));
        assert!(is_valid_env_var_name("TERM"));
        assert!(is_valid_env_var_name("HOME"));
        assert!(is_valid_env_var_name("LD_LIBRARY_PATH"));
    }

    #[test]
    fn test_invalid_env_var_names() {
        // Invalid names - empty
        assert!(!is_valid_env_var_name(""));

        // Invalid names - starts with number
        assert!(!is_valid_env_var_name("123"));
        assert!(!is_valid_env_var_name("1PATH"));

        // Invalid names - contains invalid characters
        assert!(!is_valid_env_var_name("MY-VAR"));
        assert!(!is_valid_env_var_name("MY.VAR"));
        assert!(!is_valid_env_var_name("MY VAR"));
        assert!(!is_valid_env_var_name("MY=VAR"));
        assert!(!is_valid_env_var_name("MY$VAR"));
        assert!(!is_valid_env_var_name("MY@VAR"));
        assert!(!is_valid_env_var_name("$(whoami)"));
        assert!(!is_valid_env_var_name("`command`"));

        // Invalid names - starts with invalid characters
        assert!(!is_valid_env_var_name("-PATH"));
        assert!(!is_valid_env_var_name(".PATH"));
        assert!(!is_valid_env_var_name(" PATH"));
    }

    #[test]
    fn test_validate_env_vars() {
        // Empty list is valid
        assert!(validate_env_vars(&[]).is_ok());

        // Valid variables
        assert!(validate_env_vars(&[
            ("PATH".to_string(), "/usr/bin".to_string()),
            ("TERM".to_string(), "xterm-256color".to_string()),
            ("_CUSTOM_VAR".to_string(), "value".to_string()),
        ])
        .is_ok());

        // Invalid variable name
        let result = validate_env_vars(&[
            ("PATH".to_string(), "/usr/bin".to_string()),
            ("INVALID-VAR".to_string(), "value".to_string()),
        ]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("INVALID-VAR"));

        // Injection attempt
        let result = validate_env_vars(&[("$(whoami)".to_string(), "value".to_string())]);
        assert!(result.is_err());
    }

    #[test]
    fn test_auth_rate_limit_allows_initial_attempts() {
        let mut state = ClientState::new();

        // First several attempts should be allowed
        for _ in 0..AUTH_RATE_LIMIT_FAILURES_PER_MINUTE {
            assert!(state.check_auth_rate_limit());
            state.record_auth_failure();
        }
    }

    #[test]
    fn test_auth_rate_limit_blocks_after_limit() {
        let mut state = ClientState::new();

        // Exhaust the limit
        for _ in 0..AUTH_RATE_LIMIT_FAILURES_PER_MINUTE {
            state.record_auth_failure();
        }

        // Next attempt should be blocked
        assert!(!state.check_auth_rate_limit());
    }

    #[test]
    fn test_auth_rate_limit_lockout_set() {
        let mut state = ClientState::new();

        // Exhaust the limit
        for _ in 0..AUTH_RATE_LIMIT_FAILURES_PER_MINUTE {
            state.record_auth_failure();
        }

        // Lockout should be set
        assert!(state.auth_lockout_until.is_some());
    }

    #[test]
    fn test_client_state_initial_auth_state() {
        let state = ClientState::new();

        // Initially not authenticated
        assert!(!state.authenticated);
        // No failures yet
        assert_eq!(state.auth_failure_count, 0);
        // No lockout
        assert!(state.auth_lockout_until.is_none());
    }

    #[test]
    fn test_terminal_size_constants() {
        // Verify constants are reasonable
        assert!(MIN_TERMINAL_SIZE >= 1, "Min size should be at least 1");
        assert!(
            MAX_TERMINAL_SIZE <= 10000,
            "Max size should be reasonable (<=10000)"
        );
        assert!(
            MIN_TERMINAL_SIZE < MAX_TERMINAL_SIZE,
            "Min should be less than max"
        );
    }

    #[test]
    fn test_terminal_size_typical_values() {
        // Typical terminal sizes should be within bounds
        let typical_cols = [80, 120, 132, 200];
        let typical_rows = [24, 25, 40, 50, 80];

        for cols in typical_cols {
            assert!(
                cols >= MIN_TERMINAL_SIZE && cols <= MAX_TERMINAL_SIZE,
                "Typical cols {} should be valid",
                cols
            );
        }

        for rows in typical_rows {
            assert!(
                rows >= MIN_TERMINAL_SIZE && rows <= MAX_TERMINAL_SIZE,
                "Typical rows {} should be valid",
                rows
            );
        }
    }

    #[test]
    fn test_client_state_owned_sessions_tracking() {
        let mut state = ClientState::new();

        // Initially no owned sessions
        assert!(state.owned_sessions.is_empty());

        // Add some sessions
        state
            .owned_sessions
            .insert("session-1".to_string());
        state
            .owned_sessions
            .insert("session-2".to_string());

        assert_eq!(state.owned_sessions.len(), 2);
        assert!(state.owned_sessions.contains("session-1"));
        assert!(state.owned_sessions.contains("session-2"));

        // Remove a session
        state.owned_sessions.remove("session-1");
        assert_eq!(state.owned_sessions.len(), 1);
        assert!(!state.owned_sessions.contains("session-1"));
    }
}
