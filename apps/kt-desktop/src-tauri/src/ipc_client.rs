//! IPC client for communicating with the orchestrator daemon
//!
//! Uses TCP on localhost for cross-platform compatibility.

use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;

use kt_core::ipc::{IpcEvent, IpcRequest, IpcResponse};

/// Default IPC port
pub const DEFAULT_IPC_PORT: u16 = 22230;

/// Get the default IPC address
pub fn default_ipc_address() -> String {
    format!("127.0.0.1:{}", DEFAULT_IPC_PORT)
}

/// Simple IPC client that opens a new connection for each request
///
/// This is less efficient than a persistent connection but simpler and
/// more robust (no connection state to manage).
pub struct SimpleIpcClient {
    address: String,
}

impl SimpleIpcClient {
    pub fn new(address: String) -> Self {
        Self { address }
    }

    pub fn default_address() -> String {
        default_ipc_address()
    }

    /// Send a single request and get response
    ///
    /// Opens a new connection for each request. This is simple and robust,
    /// though not optimal for high-frequency requests.
    pub async fn request(&self, request: IpcRequest) -> Result<IpcResponse> {
        let mut stream = TcpStream::connect(&self.address)
            .await
            .with_context(|| format!("Failed to connect to orchestrator at {}", self.address))?;

        // Send request
        let mut request_json = serde_json::to_string(&request)?;
        request_json.push('\n');
        stream.write_all(request_json.as_bytes()).await?;

        // Read response
        let (reader, _writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut line = String::new();
        reader.read_line(&mut line).await?;

        let response: IpcResponse = serde_json::from_str(line.trim())?;
        Ok(response)
    }

    /// Check if orchestrator is running by attempting to connect and ping
    pub async fn is_orchestrator_running(&self) -> bool {
        matches!(self.request(IpcRequest::Ping).await, Ok(IpcResponse::Pong))
    }
}

/// Persistent IPC client that maintains a connection for receiving events
/// and sending requests.
pub struct EventSubscriber {
    address: String,
    /// Channel for sending outgoing requests
    request_tx: Option<mpsc::Sender<IpcRequest>>,
    /// Cancellation token
    cancel: CancellationToken,
}

impl EventSubscriber {
    pub fn new(address: String) -> Self {
        Self {
            address,
            request_tx: None,
            cancel: CancellationToken::new(),
        }
    }

    /// Start the event subscriber, returning a receiver for events
    ///
    /// This spawns a background task that:
    /// 1. Connects to the orchestrator
    /// 2. Receives events and forwards them to the returned channel
    /// 3. Reconnects automatically on disconnection
    pub fn start(&mut self) -> mpsc::Receiver<IpcEvent> {
        let (event_tx, event_rx) = mpsc::channel::<IpcEvent>(256);
        let (request_tx, request_rx) = mpsc::channel::<IpcRequest>(64);

        self.request_tx = Some(request_tx);

        let address = self.address.clone();
        let cancel = self.cancel.clone();

        tokio::spawn(async move {
            event_loop(address, event_tx, request_rx, cancel).await;
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
    event_tx: mpsc::Sender<IpcEvent>,
    mut request_rx: mpsc::Receiver<IpcRequest>,
    cancel: CancellationToken,
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

        // Process messages until disconnection
        loop {
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
                                // Try to parse as an event first, then as a response
                                if let Ok(event) = serde_json::from_str::<IpcEvent>(trimmed) {
                                    if event_tx.send(event).await.is_err() {
                                        tracing::warn!("Event channel closed");
                                        return;
                                    }
                                } else if let Ok(_response) = serde_json::from_str::<IpcResponse>(trimmed) {
                                    // Response to a request - we don't need to forward these
                                    // since the simple client handles request/response
                                    tracing::trace!("Received response: {}", trimmed);
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
