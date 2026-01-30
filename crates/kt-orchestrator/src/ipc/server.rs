//! IPC server implementation

use std::path::PathBuf;

/// IPC server for CLI/GUI communication
pub struct IpcServer {
    /// Socket path
    pub socket_path: PathBuf,
}

impl IpcServer {
    /// Create a new IPC server
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }

    /// Start the IPC server
    pub async fn run(&self) -> anyhow::Result<()> {
        tracing::info!("IPC server starting on {:?}", self.socket_path);
        // TODO: Implement IPC server
        Ok(())
    }
}
