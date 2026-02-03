//! SSH client handler implementation
//!
//! Implements the russh server handler for accepting reverse tunnel connections
//! from remote agents.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::BytesMut;
use russh::server::{Auth, Handle, Handler, Msg, Session};
use russh::{Channel, ChannelId, CryptoVec};
use russh_keys::key::PublicKey;
use tokio::sync::mpsc;
use tokio_util::codec::{Decoder, Encoder};

use kt_core::types::MachineId;
use kt_protocol::{Frame, FrameCodec, Message, SessionId};

use crate::connection::AgentCommand;
use crate::state::OrchestratorState;

/// Events emitted by connection handlers
pub enum ConnectionEvent {
    /// A new machine has connected and registered
    MachineConnected {
        machine_id: MachineId,
        alias: String,
        hostname: String,
        /// Operating system
        os: String,
        /// CPU architecture
        arch: String,
        /// Channel for sending commands to this agent
        command_tx: mpsc::Sender<AgentCommand>,
        /// Token to cancel/disconnect this connection
        cancel: tokio_util::sync::CancellationToken,
    },
    /// A machine has disconnected
    MachineDisconnected { machine_id: MachineId },
    /// A new session was created (agent confirmed with PID)
    SessionCreated {
        machine_id: MachineId,
        session_id: SessionId,
        pid: u32,
    },
    /// A session was closed
    SessionClosed {
        machine_id: MachineId,
        session_id: SessionId,
    },
    /// Data received from a session
    SessionData {
        machine_id: MachineId,
        session_id: SessionId,
        data: Vec<u8>,
    },
}

/// Handler for a single SSH client connection
pub struct ClientHandler {
    /// Shared orchestrator state
    state: Arc<OrchestratorState>,
    /// Peer address of the connecting client
    peer_addr: SocketAddr,
    /// Machine ID derived from public key (set after auth)
    machine_id: Option<MachineId>,
    /// Machine alias (set after registration)
    alias: Option<String>,
    /// Codec for decoding frames
    codec: FrameCodec,
    /// Buffer for incoming data
    buffer: BytesMut,
    /// Active SSH channels
    channels: HashSet<ChannelId>,
    /// Sender to notify orchestrator of events
    event_tx: mpsc::Sender<ConnectionEvent>,
    /// Session handle for sending data (captured when channel opens)
    session_handle: Option<Handle>,
    /// Channel for receiving commands from orchestrator
    command_rx: Option<mpsc::Receiver<AgentCommand>>,
    /// Sender side of command channel (to pass to orchestrator)
    command_tx: Option<mpsc::Sender<AgentCommand>>,
    /// Handle to the command processor task
    command_processor_handle: Option<tokio::task::JoinHandle<()>>,
    /// Cancellation token for this connection (to allow external disconnect)
    cancel: tokio_util::sync::CancellationToken,
}

impl ClientHandler {
    /// Create a new client handler with an external cancellation token
    pub fn new(
        state: Arc<OrchestratorState>,
        event_tx: mpsc::Sender<ConnectionEvent>,
        cancel: tokio_util::sync::CancellationToken,
        peer_addr: SocketAddr,
    ) -> Self {
        // Create command channel for this connection
        let (command_tx, command_rx) = mpsc::channel(256);

        Self {
            state,
            peer_addr,
            machine_id: None,
            alias: None,
            codec: FrameCodec::new(),
            buffer: BytesMut::with_capacity(8192),
            channels: HashSet::new(),
            event_tx,
            session_handle: None,
            command_rx: Some(command_rx),
            command_tx: Some(command_tx),
            command_processor_handle: None,
            cancel,
        }
    }

    /// Get the machine ID (panics if not authenticated)
    fn machine_id(&self) -> &MachineId {
        self.machine_id.as_ref().expect("not authenticated")
    }

    /// Process a decoded frame
    async fn handle_frame(&mut self, frame: Frame, session: &mut Session) {
        let machine_id = self.machine_id().clone();

        match frame.message {
            Message::Register {
                machine_id: reported_id,
                hostname,
                os,
                arch,
            } => {
                tracing::info!(
                    "Machine registered: {} ({}) - {} {}",
                    reported_id,
                    hostname,
                    os,
                    arch
                );

                self.alias = Some(hostname.clone());

                // Send registration acknowledgment
                let ack = Message::RegisterAck {
                    accepted: true,
                    reason: None,
                };
                self.send_message(session, SessionId::CONTROL, ack);

                // Take the command_tx to pass to the orchestrator
                let command_tx = self.command_tx.take().expect("command_tx already taken");

                // Notify orchestrator with command channel
                let _ = self
                    .event_tx
                    .send(ConnectionEvent::MachineConnected {
                        machine_id,
                        alias: reported_id,
                        hostname,
                        os,
                        arch,
                        command_tx,
                        cancel: self.cancel.clone(),
                    })
                    .await;

                // Start the command processing task
                self.start_command_processor();
            }

            Message::SessionReady { pid } => {
                tracing::debug!(
                    "Session {} ready on {}, pid={}",
                    frame.session_id,
                    machine_id,
                    pid
                );

                let _ = self
                    .event_tx
                    .send(ConnectionEvent::SessionCreated {
                        machine_id,
                        session_id: frame.session_id,
                        pid,
                    })
                    .await;
            }

            Message::Data(data) => {
                let _ = self
                    .event_tx
                    .send(ConnectionEvent::SessionData {
                        machine_id,
                        session_id: frame.session_id,
                        data: data.to_vec(),
                    })
                    .await;
            }

            Message::SessionClose { exit_code } => {
                tracing::debug!(
                    "Session {} closed on {}, exit_code={:?}",
                    frame.session_id,
                    machine_id,
                    exit_code
                );

                let _ = self
                    .event_tx
                    .send(ConnectionEvent::SessionClosed {
                        machine_id,
                        session_id: frame.session_id,
                    })
                    .await;
            }

            Message::HeartbeatAck { timestamp } => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                let latency = now.saturating_sub(timestamp);
                tracing::trace!("Heartbeat ack from {}, latency={}ms", machine_id, latency);

                // Record heartbeat on the connection
                if let Some(conn) = self.state.connections.get(&machine_id) {
                    conn.record_heartbeat();
                }
            }

            _ => {
                tracing::warn!(
                    "Unexpected message type from {}: {:?}",
                    machine_id,
                    frame.message
                );
            }
        }
    }

    /// Send a message to the client
    fn send_message(&self, session: &mut Session, session_id: SessionId, message: Message) {
        let frame = Frame::new(session_id, message);
        let mut buf = BytesMut::new();

        // Use the codec to encode
        let mut codec = FrameCodec::new();
        if let Err(e) = codec.encode(frame, &mut buf) {
            tracing::error!("Failed to encode message: {}", e);
            return;
        }

        // Send on the first channel (we use a single channel for multiplexing)
        if let Some(&channel_id) = self.channels.iter().next() {
            session.data(channel_id, CryptoVec::from_slice(&buf));
        }
    }

    /// Start a background task to process incoming commands
    fn start_command_processor(&mut self) {
        let Some(mut command_rx) = self.command_rx.take() else {
            tracing::warn!("Command processor already started");
            return;
        };

        let Some(handle) = self.session_handle.clone() else {
            tracing::error!("No session handle available for command processor");
            return;
        };

        let Some(&channel_id) = self.channels.iter().next() else {
            tracing::error!("No channel available for command processor");
            return;
        };

        let machine_id = self.machine_id().clone();

        let task_handle = tokio::spawn(async move {
            tracing::debug!("Command processor started for {}", machine_id);

            while let Some(command) = command_rx.recv().await {
                let (session_id, message) = command.to_message();

                // Encode the frame
                let frame = Frame::new(session_id, message);
                let mut buf = BytesMut::new();
                let mut codec = FrameCodec::new();

                if let Err(e) = codec.encode(frame, &mut buf) {
                    tracing::error!("Failed to encode command: {}", e);
                    continue;
                }

                // Send via the session handle
                if let Err(e) = handle.data(channel_id, CryptoVec::from_slice(&buf)).await {
                    tracing::error!("Failed to send command to {}: {:?}", machine_id, e);
                    break;
                }

                tracing::debug!("Sent command to {} on session {}", machine_id, session_id);
            }

            tracing::debug!("Command processor stopped for {}", machine_id);
        });

        self.command_processor_handle = Some(task_handle);
    }
}

impl Drop for ClientHandler {
    fn drop(&mut self) {
        // Abort the command processor task if it's running
        if let Some(handle) = self.command_processor_handle.take() {
            handle.abort();
            tracing::debug!("Aborted command processor task on handler drop");
        }
    }
}

#[async_trait]
impl Handler for ClientHandler {
    type Error = anyhow::Error;

    /// Handle public key authentication
    ///
    /// Authentication is based on Tailscale network membership.
    /// Loopback connections (127.0.0.1) are always trusted since they
    /// can only originate from the same machine.
    async fn auth_publickey(
        &mut self,
        user: &str,
        public_key: &PublicKey,
    ) -> Result<Auth, Self::Error> {
        let fingerprint = public_key.fingerprint();
        let peer_ip = self.peer_addr.ip();

        tracing::info!(
            "Auth attempt from {} ({}), key fingerprint: {}",
            peer_ip,
            user,
            fingerprint
        );

        // Loopback connections are always trusted (same machine)
        if peer_ip.is_loopback() {
            tracing::info!("Loopback connection accepted from {}", peer_ip);
            // Use fingerprint-based ID for local connections
            self.machine_id = Some(MachineId::new(format!("local-{}", &fingerprint[..8])));
            return Ok(Auth::Accept);
        }

        // Tailscale verification: peer must be in our tailnet
        if let Some(peer_info) = self.state.tailscale.verify_peer(peer_ip) {
            tracing::info!(
                "Tailscale peer verified: {} ({})",
                peer_info.device_name,
                peer_ip
            );
            // Use the Tailscale device name as the machine ID
            self.machine_id = Some(MachineId::new(&peer_info.device_name));
            return Ok(Auth::Accept);
        }

        // Reject: Not in tailnet and not loopback
        tracing::warn!(
            "Authentication REJECTED for {} ({}): not in Tailscale network",
            peer_ip,
            fingerprint
        );
        Ok(Auth::Reject {
            proceed_with_methods: None,
        })
    }

    /// Handle channel open request
    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        session: &mut Session,
    ) -> Result<bool, Self::Error> {
        let channel_id = channel.id();
        tracing::debug!("Channel opened: {:?}", channel_id);

        self.channels.insert(channel_id);

        // Capture the session handle for later use
        if self.session_handle.is_none() {
            self.session_handle = Some(session.handle());
        }

        Ok(true)
    }

    /// Handle incoming data on a channel
    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        tracing::trace!("Received {} bytes on channel {:?}", data.len(), channel);

        // Append to buffer
        self.buffer.extend_from_slice(data);

        // Try to decode frames
        loop {
            match self.codec.decode(&mut self.buffer) {
                Ok(Some(frame)) => {
                    self.handle_frame(frame, session).await;
                }
                Ok(None) => {
                    // Need more data
                    break;
                }
                Err(e) => {
                    tracing::error!("Protocol error: {}", e);
                    // Clear buffer on error to try to recover
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
        channel: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        tracing::debug!("Channel closed: {:?}", channel);
        self.channels.remove(&channel);

        // If all channels closed, the connection is done
        if self.channels.is_empty() {
            if let Some(machine_id) = &self.machine_id {
                let _ = self
                    .event_tx
                    .send(ConnectionEvent::MachineDisconnected {
                        machine_id: machine_id.clone(),
                    })
                    .await;
            }
        }

        Ok(())
    }

    /// Handle channel EOF
    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        tracing::debug!("Channel EOF: {:?}", channel);
        Ok(())
    }
}

/// Configuration for the SSH server
#[derive(Clone)]
pub struct ServerConfig {
    /// russh server configuration
    pub ssh_config: Arc<russh::server::Config>,
}

impl ServerConfig {
    /// Create a new server configuration with the given host key
    pub fn new(host_key: russh_keys::key::KeyPair) -> Self {
        let mut config = russh::server::Config::default();
        config.keys.push(host_key);
        config.auth_rejection_time = std::time::Duration::from_secs(1);
        config.auth_rejection_time_initial = Some(std::time::Duration::from_secs(0));

        Self {
            ssh_config: Arc::new(config),
        }
    }
}
