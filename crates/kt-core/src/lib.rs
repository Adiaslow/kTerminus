//! kt-core: Core abstractions and configuration for k-Terminus
//!
//! This crate provides shared types, traits, and configuration structures
//! used by the orchestrator, agent, and CLI components.

pub mod config;
pub mod error;
pub mod ipc;
pub mod setup;
pub mod tailscale;
pub mod traits;
pub mod types;

pub use error::KtError;
pub use ipc::{
    IpcEvent, IpcMessage, IpcRequest, IpcResponse, MachineInfo, MachineStatus, OrchestratorStatus,
    SessionInfo, TerminalSize,
};
pub use setup::{auto_setup, is_initialized, SetupResult};
pub use tailscale::TailscaleInfo;
pub use types::{Capability, MachineId};
