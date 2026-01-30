//! kt-protocol: Wire protocol for k-Terminus session multiplexing
//!
//! This crate defines the binary protocol used for communication between
//! the orchestrator and client agents over SSH tunnels.

pub mod error;
pub mod frame;
pub mod message;
pub mod codec;
pub mod session;

pub use error::ProtocolError;
pub use frame::{FrameHeader, HEADER_SIZE, MAX_PAYLOAD_SIZE};
pub use message::{Message, MessageType, TerminalSize};
pub use codec::{Frame, FrameCodec};
pub use session::SessionId;
