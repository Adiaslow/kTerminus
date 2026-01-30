//! Message types for the k-Terminus protocol

use bytes::Bytes;
use serde::{Deserialize, Serialize};

/// Terminal dimensions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalSize {
    /// Number of rows
    pub rows: u16,
    /// Number of columns
    pub cols: u16,
}

impl TerminalSize {
    /// Create a new terminal size
    pub fn new(rows: u16, cols: u16) -> Self {
        Self { rows, cols }
    }

    /// Default terminal size (24x80)
    pub fn default_size() -> Self {
        Self { rows: 24, cols: 80 }
    }
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self::default_size()
    }
}

/// Message type identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    /// Request to create a new PTY session
    SessionCreate = 0x01,
    /// Acknowledgment that session is ready
    SessionReady = 0x02,
    /// Terminal data (stdin/stdout/stderr)
    Data = 0x03,
    /// Terminal resize event
    Resize = 0x04,
    /// Request to close a session
    SessionClose = 0x05,
    /// Heartbeat ping
    Heartbeat = 0x06,
    /// Heartbeat acknowledgment
    HeartbeatAck = 0x07,
    /// Registration message (agent → orchestrator)
    Register = 0x08,
    /// Registration acknowledgment
    RegisterAck = 0x09,
    /// Error response
    Error = 0xFF,
}

impl MessageType {
    /// Convert to u8
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    /// Convert from u8
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x01 => Some(Self::SessionCreate),
            0x02 => Some(Self::SessionReady),
            0x03 => Some(Self::Data),
            0x04 => Some(Self::Resize),
            0x05 => Some(Self::SessionClose),
            0x06 => Some(Self::Heartbeat),
            0x07 => Some(Self::HeartbeatAck),
            0x08 => Some(Self::Register),
            0x09 => Some(Self::RegisterAck),
            0xFF => Some(Self::Error),
            _ => None,
        }
    }
}

/// Error codes for error messages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum ErrorCode {
    /// Unknown error
    Unknown = 0,
    /// Session not found
    SessionNotFound = 1,
    /// PTY allocation failed
    PtyAllocationFailed = 2,
    /// Authentication failed
    AuthenticationFailed = 3,
    /// Session limit exceeded
    SessionLimitExceeded = 4,
    /// Invalid message
    InvalidMessage = 5,
}

/// Protocol messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Request to create a new session
    SessionCreate {
        /// Shell to spawn (None = default shell)
        shell: Option<String>,
        /// Environment variables to set
        env: Vec<(String, String)>,
        /// Initial terminal size
        initial_size: TerminalSize,
    },

    /// Session is ready
    SessionReady {
        /// Process ID of the spawned shell
        pid: u32,
    },

    /// Terminal data
    Data(Bytes),

    /// Terminal resize
    Resize(TerminalSize),

    /// Close session
    SessionClose {
        /// Exit code if process exited normally
        exit_code: Option<i32>,
    },

    /// Heartbeat ping
    Heartbeat {
        /// Timestamp for latency measurement
        timestamp: u64,
    },

    /// Heartbeat acknowledgment
    HeartbeatAck {
        /// Echo of the original timestamp
        timestamp: u64,
    },

    /// Agent registration
    Register {
        /// Machine ID (derived from public key fingerprint)
        machine_id: String,
        /// Hostname
        hostname: String,
        /// Operating system
        os: String,
        /// Architecture
        arch: String,
    },

    /// Registration acknowledgment
    RegisterAck {
        /// Whether registration was accepted
        accepted: bool,
        /// Reason if not accepted
        reason: Option<String>,
    },

    /// Error response
    Error {
        /// Error code
        code: ErrorCode,
        /// Human-readable message
        message: String,
    },
}

impl Message {
    /// Get the message type for this message
    pub fn message_type(&self) -> MessageType {
        match self {
            Message::SessionCreate { .. } => MessageType::SessionCreate,
            Message::SessionReady { .. } => MessageType::SessionReady,
            Message::Data(_) => MessageType::Data,
            Message::Resize(_) => MessageType::Resize,
            Message::SessionClose { .. } => MessageType::SessionClose,
            Message::Heartbeat { .. } => MessageType::Heartbeat,
            Message::HeartbeatAck { .. } => MessageType::HeartbeatAck,
            Message::Register { .. } => MessageType::Register,
            Message::RegisterAck { .. } => MessageType::RegisterAck,
            Message::Error { .. } => MessageType::Error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_type_roundtrip() {
        for msg_type in [
            MessageType::SessionCreate,
            MessageType::SessionReady,
            MessageType::Data,
            MessageType::Resize,
            MessageType::SessionClose,
            MessageType::Heartbeat,
            MessageType::HeartbeatAck,
            MessageType::Register,
            MessageType::RegisterAck,
            MessageType::Error,
        ] {
            let byte = msg_type.as_u8();
            let recovered = MessageType::from_u8(byte).unwrap();
            assert_eq!(recovered, msg_type);
        }
    }

    #[test]
    fn test_terminal_size_default() {
        let size = TerminalSize::default();
        assert_eq!(size.rows, 24);
        assert_eq!(size.cols, 80);
    }
}
