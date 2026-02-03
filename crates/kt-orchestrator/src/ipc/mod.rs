//! IPC server for CLI/GUI communication
//!
//! Provides a Unix socket server that the desktop app and CLI
//! use to communicate with the running orchestrator daemon.

mod server;

pub use server::IpcServer;
