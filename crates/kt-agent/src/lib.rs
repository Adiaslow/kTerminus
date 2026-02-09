//! kt-agent: Remote client agent for k-Terminus
//!
//! The agent runs on remote machines and establishes an outbound
//! reverse SSH tunnel to the orchestrator. It manages local PTY
//! sessions and streams I/O over the multiplexed tunnel.

pub mod metrics;
pub mod pairing;
pub mod pty;
pub mod state;
pub mod tunnel;

pub use pairing::{discover_orchestrator, prompt_pairing_code, DiscoveredOrchestrator};
pub use state::AgentState;
