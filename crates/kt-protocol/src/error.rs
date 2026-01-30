//! Protocol error types

use thiserror::Error;

/// Errors that can occur during protocol operations
#[derive(Error, Debug)]
pub enum ProtocolError {
    /// Invalid frame header
    #[error("Invalid frame header")]
    InvalidHeader,

    /// Unknown message type
    #[error("Unknown message type: {0}")]
    UnknownMessageType(u8),

    /// Payload exceeds maximum size
    #[error("Payload too large: {size} bytes exceeds maximum of {max} bytes")]
    PayloadTooLarge { size: usize, max: usize },

    /// Incomplete frame received
    #[error("Incomplete frame: expected {expected} bytes, got {actual}")]
    IncompleteFrame { expected: usize, actual: usize },

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
