//! Authentication module for the orchestrator
//!
//! Authentication is handled via Tailscale network membership.
//! Loopback connections (127.0.0.1) are always accepted.

mod tailscale;

pub use tailscale::TailscaleVerifier;
