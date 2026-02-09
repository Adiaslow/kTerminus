//! IPC client for communicating with the orchestrator daemon
//!
//! Uses TCP on localhost for cross-platform compatibility.
//! Clients must authenticate using a token read from the token file.
//!
//! ## Event Sequencing
//!
//! The `EventSubscriber` tracks sequence numbers from `IpcEventEnvelope` messages
//! to detect gaps (missing events). If a gap is detected, the client will request
//! a state snapshot to recover.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use parking_lot::RwLock;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex, oneshot};
use tokio_util::sync::CancellationToken;

use kt_core::ipc::{IpcEvent, IpcEventEnvelope, IpcRequest, IpcResponse};
use kt_core::read_ipc_token;

/// Default IPC port
pub const DEFAULT_IPC_PORT: u16 = 22230;

/// Get the default IPC address
pub fn default_ipc_address() -> String {
    format!("127.0.0.1:{}", DEFAULT_IPC_PORT)
}

/// Internal connection state
struct Connection {
    reader: BufReader<OwnedReadHalf>,
    writer: OwnedWriteHalf,
}

/// Persistent IPC client that maintains a single connection for all requests
///
/// Features:
/// - Single TCP connection maintained across multiple requests
/// - Automatic reconnection on connection loss
/// - Logical client ID for session ownership (survives reconnections)
/// - Thread-safe: can be shared across async tasks
/// - Lazy initialization: connection task spawns on first use
pub struct PersistentIpcClient {
    #[allow(dead_code)] // Stored for debugging/display purposes
    address: String,
    /// Logical client ID (UUID) for session ownership tracking
    client_id: String,
    /// Channel to send requests to the connection task
    request_tx: mpsc::Sender<(IpcRequest, oneshot::Sender<IpcResponse>)>,
    /// Receiver for the connection loop (consumed on first use)
    request_rx: std::sync::Mutex<Option<mpsc::Receiver<(IpcRequest, oneshot::Sender<IpcResponse>)>>>,
    /// Cancellation token for shutdown
    cancel: CancellationToken,
}

impl PersistentIpcClient {
    /// Create a new persistent IPC client
    ///
    /// # Arguments
    /// * `address` - The orchestrator IPC address (e.g., "127.0.0.1:22230")
    /// * `client_id` - A unique client ID (UUID recommended) for session ownership
    ///
    /// Note: The connection task is spawned lazily on first request, so this can
    /// be called before the tokio runtime is fully initialized.
    pub fn new(address: String, client_id: String) -> Self {
        let (request_tx, request_rx) = mpsc::channel(64);
        let cancel = CancellationToken::new();

        Self {
            address,
            client_id,
            request_tx,
            request_rx: std::sync::Mutex::new(Some(request_rx)),
            cancel,
        }
    }

    /// Ensure the connection loop is running
    fn ensure_started(&self) {
        // Take the receiver if we have it (only happens once)
        let maybe_rx = self.request_rx.lock().unwrap().take();
        if let Some(request_rx) = maybe_rx {
            let addr = self.address.clone();
            let cid = self.client_id.clone();
            let cancel_clone = self.cancel.clone();

            tokio::spawn(async move {
                connection_loop(addr, cid, request_rx, cancel_clone).await;
            });
        }
    }

    pub fn default_address() -> String {
        default_ipc_address()
    }

    /// Get the client ID
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// Send a request and wait for a response
    pub async fn request(&self, request: IpcRequest) -> Result<IpcResponse> {
        // Ensure the connection loop is running (lazy start)
        self.ensure_started();

        let (response_tx, response_rx) = oneshot::channel();

        self.request_tx.send((request, response_tx))
            .await
            .map_err(|_| anyhow::anyhow!("IPC connection task is not running"))?;

        response_rx
            .await
            .map_err(|_| anyhow::anyhow!("IPC connection dropped before response"))
    }

    /// Check if orchestrator is running by sending a ping
    pub async fn is_orchestrator_running(&self) -> bool {
        matches!(self.request(IpcRequest::Ping).await, Ok(IpcResponse::Pong))
    }

    /// Shutdown the client
    pub fn shutdown(&self) {
        self.cancel.cancel();
    }
}

impl Drop for PersistentIpcClient {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

/// Internal connection loop that maintains a persistent connection
async fn connection_loop(
    address: String,
    client_id: String,
    mut request_rx: mpsc::Receiver<(IpcRequest, oneshot::Sender<IpcResponse>)>,
    cancel: CancellationToken,
) {
    loop {
        if cancel.is_cancelled() {
            tracing::info!("Persistent IPC client cancelled");
            break;
        }

        // Try to connect and authenticate
        let conn = match connect_and_authenticate(&address, &client_id).await {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!("Failed to connect to orchestrator: {}", e);
                // Wait before retry, but check for cancellation
                tokio::select! {
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(2)) => {}
                    _ = cancel.cancelled() => break,
                }
                continue;
            }
        };

        tracing::info!("Persistent IPC client connected (client_id: {})", client_id);

        // Process requests until connection drops
        let result = handle_requests(conn, &mut request_rx, &cancel).await;

        if result.is_err() {
            tracing::debug!("Connection lost, will reconnect...");
        }

        // Brief delay before reconnecting
        tokio::select! {
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {}
            _ = cancel.cancelled() => break,
        }
    }

    tracing::info!("Persistent IPC client shutdown");
}

/// Connect to the orchestrator and authenticate
async fn connect_and_authenticate(address: &str, client_id: &str) -> Result<Connection> {
    let stream = TcpStream::connect(address)
        .await
        .with_context(|| format!("Failed to connect to orchestrator at {}", address))?;

    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    // Read token for authentication
    let token = read_ipc_token().with_context(|| "Failed to read IPC authentication token")?;

    tracing::debug!(
        "Authenticating with IPC token: {}...{} (client_id: {})",
        &token[..8],
        &token[token.len() - 8..],
        client_id
    );

    // Authenticate with client_id for session ownership
    let auth_request = IpcRequest::Authenticate {
        token,
        client_id: Some(client_id.to_string()),
    };
    let mut auth_json = serde_json::to_string(&auth_request)?;
    auth_json.push('\n');
    writer.write_all(auth_json.as_bytes()).await?;

    // Read auth response
    let mut line = String::new();
    reader.read_line(&mut line).await?;

    let auth_response: IpcResponse = serde_json::from_str(line.trim())?;
    match auth_response {
        IpcResponse::Authenticated { epoch_id, current_seq } => {
            tracing::debug!(
                "IPC authentication successful (client_id: {}, epoch: {}, seq: {})",
                client_id, epoch_id, current_seq
            );
        }
        IpcResponse::Error { message } => {
            return Err(anyhow::anyhow!("Authentication failed: {}", message));
        }
        other => {
            return Err(anyhow::anyhow!("Unexpected auth response: {:?}", other));
        }
    }

    Ok(Connection { reader, writer })
}

/// Handle requests on an established connection
async fn handle_requests(
    mut conn: Connection,
    request_rx: &mut mpsc::Receiver<(IpcRequest, oneshot::Sender<IpcResponse>)>,
    cancel: &CancellationToken,
) -> Result<()> {
    let mut line = String::new();

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                return Ok(());
            }

            // Wait for a request to send
            Some((request, response_tx)) = request_rx.recv() => {
                // Process the request and always send a response (even on error)
                let result = process_request(&mut conn, &request, &mut line).await;

                match result {
                    Ok(response) => {
                        let _ = response_tx.send(response);
                    }
                    Err(e) => {
                        // Send error response so caller doesn't hang
                        let _ = response_tx.send(IpcResponse::Error {
                            message: format!("Connection error: {}", e),
                        });
                        // Return error to trigger reconnect
                        return Err(e);
                    }
                }
            }
        }
    }
}

/// Process a single request on the connection
async fn process_request(
    conn: &mut Connection,
    request: &IpcRequest,
    line: &mut String,
) -> Result<IpcResponse> {
    // Send the request
    let mut request_json = serde_json::to_string(request)?;
    request_json.push('\n');
    conn.writer.write_all(request_json.as_bytes()).await?;

    // Read lines until we get a response (skip event envelopes)
    // Event envelopes have "seq" field, responses have "type" field
    loop {
        line.clear();
        conn.reader.read_line(line).await?;

        if line.is_empty() {
            // EOF - connection closed
            return Err(anyhow::anyhow!("Connection closed"));
        }

        let trimmed = line.trim();

        // Skip event envelopes (they have "seq" at the start, not "type")
        // Event format: {"seq":N,"timestamp":N,"event":{...}}
        // Response format: {"type":"...","...}
        if trimmed.starts_with("{\"seq\":") {
            tracing::trace!("Skipping event envelope in request/response channel");
            continue;
        }

        let response: IpcResponse = serde_json::from_str(trimmed)?;
        return Ok(response);
    }
}

/// Result of processing an event envelope
enum ProcessResult {
    /// Event processed successfully
    Ok(IpcEvent),
    /// Gap detected, need to recover from this sequence number
    NeedsRecovery { from_seq: u64 },
}

/// Persistent IPC client that maintains a connection for receiving events
/// and sending requests (subscriptions).
///
/// Tracks sequence numbers for gap detection and recovery.
pub struct EventSubscriber {
    address: String,
    /// Logical client ID for session ownership
    client_id: Option<String>,
    /// Channel for sending outgoing requests
    request_tx: Option<mpsc::Sender<IpcRequest>>,
    /// Cancellation token
    cancel: CancellationToken,
    /// Last seen sequence number for gap detection
    last_seen_seq: Arc<AtomicU64>,
    /// Current epoch ID (changes on orchestrator restart)
    epoch_id: Arc<RwLock<Option<String>>>,
}

impl EventSubscriber {
    pub fn new(address: String) -> Self {
        Self {
            address,
            client_id: None,
            request_tx: None,
            cancel: CancellationToken::new(),
            last_seen_seq: Arc::new(AtomicU64::new(0)),
            epoch_id: Arc::new(RwLock::new(None)),
        }
    }

    /// Set the client ID to use for authentication
    pub fn with_client_id(mut self, client_id: String) -> Self {
        self.client_id = Some(client_id);
        self
    }

    /// Get the current epoch ID
    pub fn epoch_id(&self) -> Option<String> {
        self.epoch_id.read().clone()
    }

    /// Get the last seen sequence number
    pub fn last_seen_seq(&self) -> u64 {
        self.last_seen_seq.load(Ordering::SeqCst)
    }

    /// Process an event envelope, tracking sequence numbers
    fn process_envelope(last_seen_seq: &AtomicU64, envelope: IpcEventEnvelope) -> ProcessResult {
        let last = last_seen_seq.load(Ordering::SeqCst);
        let expected = last + 1;

        // Check for gap (but allow first event after reconnect)
        if envelope.seq > expected && last > 0 {
            tracing::warn!(
                "Event sequence gap detected: expected {}, got {} (gap of {} events)",
                expected,
                envelope.seq,
                envelope.seq - expected
            );
            return ProcessResult::NeedsRecovery { from_seq: last };
        }

        // Update last seen sequence
        last_seen_seq.store(envelope.seq, Ordering::SeqCst);
        ProcessResult::Ok(envelope.event)
    }

    /// Start the event subscriber, returning a receiver for events
    ///
    /// This spawns a background task that:
    /// 1. Connects to the orchestrator
    /// 2. Receives events and forwards them to the returned channel
    /// 3. Reconnects automatically on disconnection
    /// 4. Tracks sequence numbers for gap detection
    pub fn start(&mut self) -> mpsc::Receiver<IpcEvent> {
        let (event_tx, event_rx) = mpsc::channel::<IpcEvent>(256);
        let (request_tx, request_rx) = mpsc::channel::<IpcRequest>(64);

        self.request_tx = Some(request_tx);

        let address = self.address.clone();
        let client_id = self.client_id.clone();
        let cancel = self.cancel.clone();
        let last_seen_seq = self.last_seen_seq.clone();
        let epoch_id = self.epoch_id.clone();

        tokio::spawn(async move {
            event_loop(address, client_id, event_tx, request_rx, cancel, last_seen_seq, epoch_id).await;
        });

        event_rx
    }

    /// Send a request through the persistent connection
    pub async fn send(&self, request: IpcRequest) -> Result<()> {
        if let Some(tx) = &self.request_tx {
            tx.send(request)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to send request: {}", e))
        } else {
            Err(anyhow::anyhow!("Event subscriber not started"))
        }
    }

    /// Stop the event subscriber
    pub fn stop(&self) {
        self.cancel.cancel();
    }
}

/// Internal event loop for the persistent connection
async fn event_loop(
    address: String,
    client_id: Option<String>,
    event_tx: mpsc::Sender<IpcEvent>,
    mut request_rx: mpsc::Receiver<IpcRequest>,
    cancel: CancellationToken,
    last_seen_seq: Arc<AtomicU64>,
    epoch_id: Arc<RwLock<Option<String>>>,
) {
    loop {
        if cancel.is_cancelled() {
            tracing::info!("Event subscriber cancelled");
            break;
        }

        // Try to connect
        let stream = match TcpStream::connect(&address).await {
            Ok(s) => s,
            Err(e) => {
                tracing::debug!("Failed to connect to orchestrator: {}", e);
                // Wait before retry
                tokio::select! {
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(2)) => {}
                    _ = cancel.cancelled() => break,
                }
                continue;
            }
        };

        tracing::info!("Connected to orchestrator for events");

        let (reader, writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let writer = Arc::new(Mutex::new(writer));
        let mut line = String::new();

        // Authenticate with the orchestrator
        let token = match read_ipc_token() {
            Ok(t) => {
                tracing::debug!(
                    "Read IPC token for authentication: {}...{}",
                    &t[..std::cmp::min(8, t.len())],
                    if t.len() > 8 { &t[t.len()-8..] } else { "" }
                );
                t
            }
            Err(e) => {
                tracing::error!("Failed to read IPC token: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                continue;
            }
        };

        // Send auth request with client_id if available
        let auth_request = IpcRequest::Authenticate {
            token,
            client_id: client_id.clone(),
        };
        if let Err(e) = send_request(&writer, auth_request).await {
            tracing::error!("Failed to send auth request: {}", e);
            continue;
        }

        // Read auth response
        match reader.read_line(&mut line).await {
            Ok(0) => {
                tracing::warn!("Disconnected during authentication");
                continue;
            }
            Ok(_) => {
                match serde_json::from_str::<IpcResponse>(line.trim()) {
                    Ok(IpcResponse::Authenticated { epoch_id: new_epoch, current_seq }) => {
                        // Check if epoch changed (orchestrator restarted)
                        let old_epoch = epoch_id.read().clone();
                        if let Some(ref old) = old_epoch {
                            if old != &new_epoch {
                                tracing::warn!(
                                    "Orchestrator epoch changed: {} -> {} (restart detected)",
                                    old, new_epoch
                                );
                                // Reset sequence tracking - full resync needed
                                last_seen_seq.store(0, Ordering::SeqCst);
                            }
                        }

                        // Store new epoch and sequence
                        *epoch_id.write() = Some(new_epoch.clone());
                        last_seen_seq.store(current_seq, Ordering::SeqCst);

                        tracing::debug!(
                            "Authenticated with orchestrator (client_id: {:?}, epoch: {}, seq: {})",
                            client_id, new_epoch, current_seq
                        );
                    }
                    Ok(other) => {
                        tracing::error!("Authentication failed: {:?}", other);
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                        continue;
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse auth response: {}", e);
                        continue;
                    }
                }
                line.clear();
            }
            Err(e) => {
                tracing::error!("Failed to read auth response: {}", e);
                continue;
            }
        }

        // Track if we need recovery
        let mut needs_recovery = false;

        // Process messages until disconnection
        loop {
            // If recovery is needed, request state snapshot
            if needs_recovery {
                tracing::info!("Requesting state snapshot for recovery");
                if let Err(e) = send_request(&writer, IpcRequest::GetStateSnapshot).await {
                    tracing::warn!("Failed to request state snapshot: {}", e);
                    break; // Reconnect
                }
                needs_recovery = false;
            }

            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("Event subscriber cancelled during connection");
                    return;
                }

                // Read incoming messages (events or responses)
                result = reader.read_line(&mut line) => {
                    match result {
                        Ok(0) => {
                            tracing::info!("Orchestrator disconnected");
                            break; // EOF, reconnect
                        }
                        Ok(_) => {
                            let trimmed = line.trim();
                            if !trimmed.is_empty() {
                                // Try to parse as an event envelope first
                                if let Ok(envelope) = serde_json::from_str::<IpcEventEnvelope>(trimmed) {
                                    match EventSubscriber::process_envelope(&last_seen_seq, envelope) {
                                        ProcessResult::Ok(event) => {
                                            if event_tx.send(event).await.is_err() {
                                                tracing::warn!("Event channel closed");
                                                return;
                                            }
                                        }
                                        ProcessResult::NeedsRecovery { from_seq } => {
                                            tracing::warn!(
                                                "Gap detected at seq {}, will request snapshot",
                                                from_seq
                                            );
                                            needs_recovery = true;
                                        }
                                    }
                                } else if let Ok(response) = serde_json::from_str::<IpcResponse>(trimmed) {
                                    // Handle state snapshot response for recovery
                                    match response {
                                        IpcResponse::StateSnapshot { epoch_id: snap_epoch, current_seq, machines, sessions } => {
                                            tracing::info!(
                                                "Received state snapshot: epoch={}, seq={}, {} machines, {} sessions",
                                                snap_epoch, current_seq, machines.len(), sessions.len()
                                            );

                                            // Update epoch and sequence
                                            *epoch_id.write() = Some(snap_epoch);
                                            last_seen_seq.store(current_seq, Ordering::SeqCst);

                                            // Emit synthetic events for current state
                                            for machine in machines {
                                                let event = IpcEvent::MachineConnected(machine);
                                                if event_tx.send(event).await.is_err() {
                                                    tracing::warn!("Event channel closed");
                                                    return;
                                                }
                                            }
                                            for session in sessions {
                                                let event = IpcEvent::SessionCreated(session);
                                                if event_tx.send(event).await.is_err() {
                                                    tracing::warn!("Event channel closed");
                                                    return;
                                                }
                                            }
                                        }
                                        IpcResponse::EventsSince { events, truncated, oldest_available_seq } => {
                                            if truncated {
                                                tracing::warn!(
                                                    "Events truncated (oldest available: {:?}), requesting full snapshot",
                                                    oldest_available_seq
                                                );
                                                needs_recovery = true;
                                            } else {
                                                // Process missed events
                                                for envelope in events {
                                                    last_seen_seq.store(envelope.seq, Ordering::SeqCst);
                                                    if event_tx.send(envelope.event).await.is_err() {
                                                        tracing::warn!("Event channel closed");
                                                        return;
                                                    }
                                                }
                                            }
                                        }
                                        _ => {
                                            // Other responses - we don't need to forward these
                                            // since the persistent client handles request/response
                                            tracing::trace!("Received response: {}", trimmed);
                                        }
                                    }
                                } else {
                                    tracing::warn!("Unknown message type: {}", trimmed);
                                }
                            }
                            line.clear();
                        }
                        Err(e) => {
                            tracing::warn!("Read error: {}", e);
                            break; // Reconnect
                        }
                    }
                }

                // Handle outgoing requests
                Some(request) = request_rx.recv() => {
                    if let Err(e) = send_request(&writer, request).await {
                        tracing::warn!("Failed to send request: {}", e);
                        break; // Reconnect
                    }
                }
            }
        }

        // Wait before reconnecting
        tokio::select! {
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {}
            _ = cancel.cancelled() => break,
        }
    }
}

async fn send_request(writer: &Arc<Mutex<OwnedWriteHalf>>, request: IpcRequest) -> Result<()> {
    let mut json = serde_json::to_string(&request)?;
    json.push('\n');

    let mut writer = writer.lock().await;
    writer.write_all(json.as_bytes()).await?;
    Ok(())
}
