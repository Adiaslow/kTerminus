//! kt-orchestrator: Local daemon accepting reverse SSH tunnels
//!
//! The orchestrator runs on the local machine and accepts incoming
//! reverse SSH connections from remote agents. It manages the connection
//! pool, multiplexes terminal sessions, and provides the IPC interface
//! for the CLI and GUI.

pub mod auth;
pub mod connection;
pub mod coordinator;
pub mod ipc;
pub mod server;
pub mod session;
pub mod state;

pub use coordinator::StateCoordinator;
pub use state::OrchestratorState;
