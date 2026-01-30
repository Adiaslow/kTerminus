//! IPC client for communicating with orchestrator

mod client;

pub use client::{
    MachineInfo, OrchestratorClient, OrchestratorStatus, SessionInfo, DEFAULT_SOCKET_PATH,
};
