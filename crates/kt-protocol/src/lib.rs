//! kt-protocol: Wire protocol for k-Terminus session multiplexing
//!
//! This crate defines the binary protocol used for communication between
//! the orchestrator and client agents over SSH tunnels.

pub mod codec;
pub mod error;
pub mod frame;
pub mod message;
pub mod session;

pub use codec::{Frame, FrameCodec};
pub use error::ProtocolError;
pub use frame::{FrameHeader, HEADER_SIZE, MAX_PAYLOAD_SIZE};
pub use message::{ErrorCode, Message, MessageType, TerminalSize, PROTOCOL_VERSION};
pub use session::SessionId;
