//! kt-agent: Remote client agent for k-Terminus
//!
//! The agent runs on remote machines and establishes an outbound
//! reverse SSH tunnel to the orchestrator. It manages local PTY
//! sessions and streams I/O over the multiplexed tunnel.

pub mod metrics;
pub mod pty;
pub mod state;
pub mod tunnel;

pub use state::AgentState;
