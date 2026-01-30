//! SSH client handler implementation
//!
//! Implements the russh server handler for accepting reverse tunnel connections
//! from remote agents.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::BytesMut;
use russh::server::{Auth, Handler, Msg, Session};
use russh::{Channel, ChannelId, CryptoVec};
use russh_keys::key::PublicKey;
use tokio::sync::mpsc;
use tokio_util::codec::Decoder;

use kt_core::types::MachineId;
use kt_protocol::{Frame, FrameCodec, Message, SessionId};

use crate::state::OrchestratorState;

/// Handler for a single SSH client connection
pub struct ClientHandler {
    /// Shared orchestrator state
    state: Arc<OrchestratorState>,
    /// Machine ID derived from public key (set after auth)
    machine_id: Option<MachineId>,
    /// Machine alias (set after registration)
    alias: Option<String>,
    /// Codec for decoding frames
    codec: FrameCodec,
    /// Buffer for incoming data
    buffer: BytesMut,
    /// Channel senders for each SSH channel
    channels: HashMap<ChannelId, ChannelState>,
    /// Sender to notify orchestrator of events
    event_tx: mpsc::Sender<ConnectionEvent>,
}

/// State for a single SSH channel
struct ChannelState {
    /// Channel ID
    id: ChannelId,
    /// Whether this channel has been registered
    registered: bool,
}

/// Events emitted by connection handlers
#[derive(Debug)]
pub enum ConnectionEvent {
    /// A new machine has connected and registered
    MachineConnected {
        machine_id: MachineId,
        alias: String,
        hostname: String,
    },
    /// A machine has disconnected
    MachineDisconnected { machine_id: MachineId },
    /// A new session was created
    SessionCreated {
        machine_id: MachineId,
        session_id: SessionId,
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

impl ClientHandler {
    /// Create a new client handler
    pub fn new(state: Arc<OrchestratorState>, event_tx: mpsc::Sender<ConnectionEvent>) -> Self {
        Self {
            state,
            machine_id: None,
            alias: None,
            codec: FrameCodec::new(),
            buffer: BytesMut::with_capacity(8192),
            channels: HashMap::new(),
            event_tx,
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
                self.send_message(session, SessionId::CONTROL, ack).await;

                // Notify orchestrator
                let _ = self
                    .event_tx
                    .send(ConnectionEvent::MachineConnected {
                        machine_id,
                        alias: reported_id,
                        hostname,
                    })
                    .await;
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
            }

            _ => {
                tracing::warn!("Unexpected message type from {}: {:?}", machine_id, frame.message);
            }
        }
    }

    /// Send a message to the client
    async fn send_message(&self, session: &mut Session, session_id: SessionId, message: Message) {
        let frame = Frame::new(session_id, message);
        let mut buf = BytesMut::new();

        // Use the codec to encode
        let mut codec = FrameCodec::new();
        if let Err(e) = tokio_util::codec::Encoder::encode(&mut codec, frame, &mut buf) {
            tracing::error!("Failed to encode message: {}", e);
            return;
        }

        // Send on the first channel (we use a single channel for multiplexing)
        if let Some((channel_id, _)) = self.channels.iter().next() {
            session.data(*channel_id, CryptoVec::from_slice(&buf));
        }
    }
}

#[async_trait]
impl Handler for ClientHandler {
    type Error = anyhow::Error;

    /// Handle public key authentication
    async fn auth_publickey(
        &mut self,
        user: &str,
        public_key: &PublicKey,
    ) -> Result<Auth, Self::Error> {
        // Get the key fingerprint
        let fingerprint = public_key.fingerprint();
        tracing::info!("Auth attempt from user '{}', key: {}", user, fingerprint);

        // Check if key is authorized
        if self.state.auth.is_authorized(&fingerprint) {
            // Derive machine ID from fingerprint
            self.machine_id = Some(MachineId::from_fingerprint(&fingerprint));
            tracing::info!("Authentication successful for {}", fingerprint);
            Ok(Auth::Accept)
        } else {
            tracing::warn!("Authentication rejected for {}", fingerprint);
            Ok(Auth::Reject {
                proceed_with_methods: None,
            })
        }
    }

    /// Handle channel open request
    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        session: &mut Session,
    ) -> Result<bool, Self::Error> {
        let channel_id = channel.id();
        tracing::debug!("Channel opened: {:?}", channel_id);

        self.channels.insert(
            channel_id,
            ChannelState {
                id: channel_id,
                registered: false,
            },
        );

        Ok(true)
    }

    /// Handle incoming data on a channel
    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
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
        session: &mut Session,
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
