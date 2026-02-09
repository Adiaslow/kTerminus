//! IPC client for communicating with orchestrator
//!
//! Uses TCP on localhost for cross-platform compatibility.
//!
//! ## Event Sequencing
//!
//! Events from the orchestrator are wrapped in `IpcEventEnvelope` with monotonic
//! sequence numbers. Use `epoch_id` to detect orchestrator restarts and `seq`
//! numbers for gap detection.

mod client;

pub use client::{OrchestratorClient, TerminalSession};

// Re-export constants and types from kt_core
pub use kt_core::ipc::{
    default_ipc_address, IpcEventEnvelope, MachineInfo, MachineStatus, OrchestratorStatus,
    SessionInfo, DEFAULT_IPC_PORT,
};
