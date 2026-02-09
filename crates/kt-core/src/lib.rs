//! kt-core: Core abstractions and configuration for k-Terminus
//!
//! This crate provides shared types, traits, and configuration structures
//! used by the orchestrator, agent, and CLI components.

pub mod config;
pub mod error;
pub mod ipc;
pub mod ipc_auth;
pub mod pidfile;
pub mod setup;
pub mod tailscale;
pub mod time;
pub mod traits;
pub mod types;

pub use error::KtError;
pub use ipc::{
    default_ipc_address, is_orchestrator_running, try_ipc_ping, try_ipc_ping_with_timeout,
    IpcEvent, IpcMessage, IpcRequest, IpcResponse, MachineInfo, MachineStatus, OrchestratorStatus,
    SessionInfo, TerminalSize, DEFAULT_IPC_PORT,
};
pub use ipc_auth::{
    acquire_token_ownership, default_token_path, generate_token as generate_ipc_token,
    read_token as read_ipc_token, read_token_info, remove_token as remove_ipc_token,
    token_exists as ipc_token_exists, validate_token as validate_ipc_token,
    write_token as write_ipc_token, TokenInfo, TokenOwnership,
};
pub use pidfile::{
    default_pid_path, is_process_alive, read_pid_file, remove_pid_file, write_pid_file,
    PidFileGuard,
};
pub use setup::{auto_setup, is_initialized, SetupResult};
pub use tailscale::TailscaleInfo;
pub use types::{Capability, MachineId};
