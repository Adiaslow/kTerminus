//! Core error types for k-Terminus

use kt_protocol::ProtocolError;
use std::path::PathBuf;
use thiserror::Error;

/// Top-level error type for the k-terminus ecosystem
#[derive(Error, Debug)]
pub enum KtError {
    /// Protocol error
    #[error("Protocol error: {0}")]
    Protocol(#[from] ProtocolError),

    /// Connection error
    #[error("Connection error: {0}")]
    Connection(#[from] ConnectionError),

    /// Session error
    #[error("Session error: {0}")]
    Session(#[from] SessionError),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Connection-related errors
#[derive(Error, Debug)]
pub enum ConnectionError {
    /// Authentication failed
    #[error("Authentication failed")]
    AuthenticationFailed,

    /// Connection refused
    #[error("Connection refused: {0}")]
    ConnectionRefused(String),

    /// Connection lost
    #[error("Connection lost: {0}")]
    ConnectionLost(String),

    /// Machine not found
    #[error("Machine not found: {0}")]
    MachineNotFound(String),

    /// Tunnel error
    #[error("Tunnel error: {0}")]
    TunnelError(String),

    /// Host key verification failed
    #[error("Host key verification failed")]
    HostKeyVerificationFailed,
}

/// Session-related errors
#[derive(Error, Debug)]
pub enum SessionError {
    /// Session not found
    #[error("Session not found: {0}")]
    NotFound(String),

    /// Session already exists
    #[error("Session already exists: {0}")]
    AlreadyExists(String),

    /// PTY allocation failed
    #[error("PTY allocation failed: {0}")]
    PtyAllocation(String),

    /// Session closed unexpectedly
    #[error("Session closed unexpectedly")]
    UnexpectedClose,

    /// Session limit exceeded
    #[error("Session limit exceeded")]
    LimitExceeded,
}

/// Configuration-related errors
#[derive(Error, Debug)]
pub enum ConfigError {
    /// Config file not found
    #[error("Config file not found: {0}")]
    NotFound(PathBuf),

    /// Invalid configuration
    #[error("Invalid config: {0}")]
    Invalid(String),

    /// TOML parse error
    #[error("TOML parse error: {0}")]
    Parse(#[from] toml::de::Error),

    /// TOML serialize error
    #[error("TOML serialize error: {0}")]
    Serialize(#[from] toml::ser::Error),

    /// Missing required field
    #[error("Missing required field: {0}")]
    MissingField(String),
}
