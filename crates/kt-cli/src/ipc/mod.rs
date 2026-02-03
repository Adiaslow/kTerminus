//! IPC client for communicating with orchestrator
//!
//! Uses TCP on localhost for cross-platform compatibility.

mod client;

pub use client::{default_ipc_address, OrchestratorClient, TerminalSession, DEFAULT_IPC_PORT};

// Re-export types from kt_core
pub use kt_core::ipc::{MachineInfo, MachineStatus, OrchestratorStatus, SessionInfo};
