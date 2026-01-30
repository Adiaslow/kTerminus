//! kt-core: Core abstractions and configuration for k-Terminus
//!
//! This crate provides shared types, traits, and configuration structures
//! used by the orchestrator, agent, and CLI components.

pub mod config;
pub mod error;
pub mod traits;
pub mod types;

pub use error::KtError;
pub use types::{MachineId, Capability};
