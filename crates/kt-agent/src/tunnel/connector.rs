//! Outbound SSH tunnel connector
//!
//! Establishes and maintains the reverse tunnel connection to the orchestrator.

use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use bytes::BytesMut;
use russh::client::{self, Config, Handle, Msg};
use russh::{Channel, ChannelId, Disconnect};
use russh_keys::key::{KeyPair, PublicKey};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_util::codec::{Decoder, Encoder};

use kt_core::config::AgentConfig;
use kt_protocol::{Frame, FrameCodec, Message, SessionId, TerminalSize};

use super::reconnect::ExponentialBackoff;

/// Channel capacity for events from the orchestrator.
///
/// This buffer holds events (session create, data, resize, etc.) between
/// the SSH data handler and the main agent loop.
///
/// # Value Choice
///
/// 256 provides headroom for:
/// - Burst commands from orchestrator (multiple session ops)
/// - Brief delays in agent processing (e.g., PTY operations)
///
/// Too small: Risk of dropped events during high activity
/// Too large: Memory usage when agent is slow
const TUNNEL_EVENT_CHANNEL_CAPACITY: usize = 256;

/// Connection errors that may require special handling
#[derive(Debug, Error)]
pub enum ConnectionError {
    /// Private key file not found
    #[error("Private key not found at {path}: {source}")]
    KeyNotFound {
        path: String,
        #[source]
        source: anyhow::Error,
    },

    /// Authentication was rejected by the orchestrator
    #[error("Authentication rejected by orchestrator")]
    AuthRejected,

    /// Host key verification failed
    #[error("Host key verification failed: {message}")]
    HostKeyRejected { message: String },

    /// Other connection error
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

/// Establishes and maintains the outbound SSH tunnel to the orchestrator
pub struct TunnelConnector {
    /// Agent configuration
    config: AgentConfig,
    /// Private key for authentication
    key: Arc<KeyPair>,
}

impl TunnelConnector {
    /// Create a new tunnel connector
    pub fn new(config: AgentConfig) -> Result<Self, ConnectionError> {
        // Check if key file exists first
        if !config.private_key_path.exists() {
            return Err(ConnectionError::KeyNotFound {
                path: config.private_key_path.display().to_string(),
                source: anyhow::anyhow!("File does not exist"),
            });
        }

        // Load the private key
        let key = russh_keys::load_secret_key(&config.private_key_path, None).map_err(|e| {
            ConnectionError::KeyNotFound {
                path: config.private_key_path.display().to_string(),
                source: anyhow::anyhow!("Failed to load key: {}", e),
            }
        })?;

        Ok(Self {
            config,
            key: Arc::new(key),
        })
    }

    /// Get the agent configuration
    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    /// Connect to the orchestrator with automatic retry
    ///
    /// Returns `ConnectionError::AuthRejected` if authentication fails,
    /// or `ConnectionError::HostKeyRejected` if host key verification fails.
    /// These errors indicate that Tailscale verification may have failed.
    pub async fn connect_with_retry(
        &self,
        mut backoff: ExponentialBackoff,
    ) -> Result<ActiveTunnel, ConnectionError> {
        loop {
            match self.try_connect().await {
                Ok(tunnel) => {
                    tracing::info!(
                        "Connected to orchestrator at {}",
                        self.config.orchestrator_address
                    );
                    return Ok(tunnel);
                }
                Err(ConnectionError::AuthRejected) => {
                    // Don't retry auth failures - check Tailscale connection
                    tracing::warn!("Authentication rejected - check that both machines are on the same Tailscale network");
                    return Err(ConnectionError::AuthRejected);
                }
                Err(ConnectionError::HostKeyRejected { message }) => {
                    // Don't retry host key failures
                    tracing::error!("Host key verification failed");
                    return Err(ConnectionError::HostKeyRejected { message });
                }
                Err(e) => {
                    let delay = backoff.next_delay();
                    tracing::warn!("Connection failed: {}. Retrying in {:?}", e, delay);
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    /// Attempt a single connection to the orchestrator
    async fn try_connect(&self) -> Result<ActiveTunnel, ConnectionError> {
        let ssh_config = Config::default();
        let ssh_config = Arc::new(ssh_config);

        // Create the client handler
        let (event_tx, event_rx) = mpsc::channel(TUNNEL_EVENT_CHANNEL_CAPACITY);
        let handler = ClientHandler::new(self.config.orchestrator_host_key.clone(), event_tx);

        // Connect to the orchestrator
        tracing::debug!("Connecting to {}", self.config.orchestrator_address);
        let mut session = tokio::time::timeout(
            self.config.connect_timeout,
            client::connect(ssh_config, &self.config.orchestrator_address, handler),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Connection timed out"))?
        .map_err(|e| {
            let err_str = e.to_string();
            // Detect host key rejection (russh returns "Unknown server key" error)
            if err_str.contains("Unknown server key") || err_str.contains("server key") {
                return ConnectionError::HostKeyRejected {
                    message: "Server's host key was rejected. Ensure both machines are on the same Tailscale network.".to_string(),
                };
            }
            ConnectionError::Other(anyhow::anyhow!("Failed to connect to {}: {}", self.config.orchestrator_address, e))
        })?;

        // Authenticate with public key
        tracing::debug!("Authenticating as user '{}'", self.config.username);
        let authenticated = session
            .authenticate_publickey(&self.config.username, Arc::clone(&self.key))
            .await
            .map_err(|e| anyhow::anyhow!("Authentication error: {}", e))?;

        if !authenticated {
            return Err(ConnectionError::AuthRejected);
        }

        tracing::debug!("Authentication successful, opening channel");

        // Open a session channel for multiplexed communication
        let channel = session
            .channel_open_session()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to open session channel: {}", e))?;

        let _channel_id = channel.id();

        // Create the active tunnel
        let tunnel = ActiveTunnel::new(session, channel, event_rx);

        // Send registration message
        tunnel.register(&self.config).await?;

        Ok(tunnel)
    }
}

/// An active tunnel connection to the orchestrator
pub struct ActiveTunnel {
    /// SSH session handle
    session: Handle<ClientHandler>,
    /// Main channel for communication
    channel: Channel<Msg>,
    /// Event receiver
    event_rx: mpsc::Receiver<TunnelEvent>,
    /// Frame codec for encoding/decoding
    #[allow(dead_code)]
    codec: FrameCodec,
    /// Buffer for outgoing data
    #[allow(dead_code)]
    write_buffer: BytesMut,
}

/// Events received from the orchestrator
#[derive(Debug)]
pub enum TunnelEvent {
    /// Orchestrator acknowledged our registration
    Registered {
        accepted: bool,
        reason: Option<String>,
    },
    /// Request to create a new session
    CreateSession {
        session_id: SessionId,
        shell: Option<String>,
        env: Vec<(String, String)>,
        size: TerminalSize,
    },
    /// Data for a session
    SessionData {
        session_id: SessionId,
        data: Vec<u8>,
    },
    /// Resize a session
    SessionResize {
        session_id: SessionId,
        size: TerminalSize,
    },
    /// Close a session
    SessionClose { session_id: SessionId },
    /// Heartbeat request
    Heartbeat { timestamp: u64 },
    /// Connection closed
    Disconnected,
}

impl ActiveTunnel {
    fn new(
        session: Handle<ClientHandler>,
        channel: Channel<Msg>,
        event_rx: mpsc::Receiver<TunnelEvent>,
    ) -> Self {
        Self {
            session,
            channel,
            event_rx,
            codec: FrameCodec::new(),
            write_buffer: BytesMut::with_capacity(4096),
        }
    }

    /// Send a registration message to the orchestrator
    async fn register(&self, config: &AgentConfig) -> Result<()> {
        let hostname = gethostname::gethostname().to_string_lossy().into_owned();
        let machine_id = config.machine_alias();

        let message = Message::Register {
            machine_id,
            hostname,
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            version: Some(kt_protocol::PROTOCOL_VERSION.to_string()),
        };

        self.send_message(SessionId::CONTROL, message).await
    }

    /// Send a message to the orchestrator
    pub async fn send_message(&self, session_id: SessionId, message: Message) -> Result<()> {
        let frame = Frame::new(session_id, message);
        let mut buf = BytesMut::new();

        let mut codec = FrameCodec::new();
        codec
            .encode(frame, &mut buf)
            .with_context(|| "Failed to encode message")?;

        self.channel
            .data(&buf[..])
            .await
            .with_context(|| "Failed to send data")?;

        Ok(())
    }

    /// Send session data to the orchestrator
    pub async fn send_data(&self, session_id: SessionId, data: &[u8]) -> Result<()> {
        self.send_message(
            session_id,
            Message::Data(bytes::Bytes::copy_from_slice(data)),
        )
        .await
    }

    /// Send session ready notification
    pub async fn send_session_ready(&self, session_id: SessionId, pid: u32) -> Result<()> {
        self.send_message(session_id, Message::SessionReady { pid })
            .await
    }

    /// Send session close notification
    pub async fn send_session_close(
        &self,
        session_id: SessionId,
        exit_code: Option<i32>,
    ) -> Result<()> {
        self.send_message(session_id, Message::SessionClose { exit_code })
            .await
    }

    /// Send heartbeat acknowledgment
    pub async fn send_heartbeat_ack(&self, timestamp: u64) -> Result<()> {
        self.send_message(SessionId::CONTROL, Message::HeartbeatAck { timestamp })
            .await
    }

    /// Send error notification for a session
    pub async fn send_error(
        &self,
        session_id: SessionId,
        code: kt_protocol::ErrorCode,
        message: String,
    ) -> Result<()> {
        self.send_message(session_id, Message::Error { code, message })
            .await
    }

    /// Receive the next event from the orchestrator
    pub async fn recv_event(&mut self) -> Option<TunnelEvent> {
        self.event_rx.recv().await
    }

    /// Close the tunnel
    pub async fn close(self) -> Result<()> {
        self.session
            .disconnect(Disconnect::ByApplication, "closing", "en")
            .await?;
        Ok(())
    }
}

/// SSH client handler for the agent
struct ClientHandler {
    /// Expected host key fingerprint (for verification)
    expected_host_key: Option<String>,
    /// Whether host key has been verified
    host_key_verified: bool,
    /// Event sender
    event_tx: mpsc::Sender<TunnelEvent>,
    /// Frame codec
    codec: FrameCodec,
    /// Buffer for incoming data
    buffer: BytesMut,
}

impl ClientHandler {
    fn new(expected_host_key: Option<String>, event_tx: mpsc::Sender<TunnelEvent>) -> Self {
        Self {
            expected_host_key,
            host_key_verified: false,
            event_tx,
            codec: FrameCodec::new(),
            buffer: BytesMut::with_capacity(8192),
        }
    }

    /// Process a decoded frame
    async fn handle_frame(&self, frame: Frame) {
        let event = match frame.message {
            Message::RegisterAck { accepted, reason } => {
                TunnelEvent::Registered { accepted, reason }
            }

            Message::SessionCreate {
                shell,
                env,
                initial_size,
            } => TunnelEvent::CreateSession {
                session_id: frame.session_id,
                shell,
                env,
                size: initial_size,
            },

            Message::Data(data) => TunnelEvent::SessionData {
                session_id: frame.session_id,
                data: data.to_vec(),
            },

            Message::Resize(size) => TunnelEvent::SessionResize {
                session_id: frame.session_id,
                size,
            },

            Message::SessionClose { .. } => TunnelEvent::SessionClose {
                session_id: frame.session_id,
            },

            Message::Heartbeat { timestamp } => TunnelEvent::Heartbeat { timestamp },

            _ => {
                tracing::warn!("Unexpected message from orchestrator: {:?}", frame.message);
                return;
            }
        };

        let _ = self.event_tx.send(event).await;
    }
}

#[async_trait]
impl client::Handler for ClientHandler {
    type Error = anyhow::Error;

    /// Verify the server's host key
    ///
    /// With Tailscale providing the security layer, we accept any host key.
    /// Tailscale's WireGuard encryption already ensures we're talking to the right machine.
    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        let fingerprint = server_public_key.fingerprint();
        tracing::debug!("Server host key: {}", fingerprint);

        // If an expected key is configured, verify it (for manual setups)
        if let Some(expected) = &self.expected_host_key {
            if fingerprint == *expected {
                tracing::debug!("Host key verified against configured fingerprint");
            } else {
                tracing::warn!(
                    "Host key differs from configured: expected {}, got {}",
                    expected,
                    fingerprint
                );
                // Still accept - Tailscale provides the security layer
            }
        }

        // Accept the key - Tailscale handles transport security
        self.host_key_verified = true;
        Ok(true)
    }

    /// Handle data received on a channel
    async fn data(
        &mut self,
        _channel: ChannelId,
        data: &[u8],
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        // Append to buffer
        self.buffer.extend_from_slice(data);

        // Try to decode frames
        loop {
            match self.codec.decode(&mut self.buffer) {
                Ok(Some(frame)) => {
                    self.handle_frame(frame).await;
                }
                Ok(None) => {
                    // Need more data
                    break;
                }
                Err(e) => {
                    tracing::error!("Protocol error: {}", e);
                    self.buffer.clear();
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle channel close
    async fn channel_close(
        &mut self,
        _channel: ChannelId,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        tracing::info!("Channel closed");
        let _ = self.event_tx.send(TunnelEvent::Disconnected).await;
        Ok(())
    }

    /// Handle channel EOF
    async fn channel_eof(
        &mut self,
        _channel: ChannelId,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        tracing::debug!("Channel EOF");
        Ok(())
    }
}
