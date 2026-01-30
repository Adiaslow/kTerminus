//! Status command implementation

use anyhow::Result;

use crate::ipc::OrchestratorClient;
use crate::output::{format_status, print_error};

/// Execute the status command
pub async fn status_command(
    client: &mut OrchestratorClient,
    detailed: bool,
) -> Result<()> {
    let status = match client.status().await {
        Ok(s) => s,
        Err(e) => {
            print_error(&format!("Failed to get orchestrator status: {}", e));
            print_error("Is the orchestrator running? Try: k-terminus start");
            return Err(e);
        }
    };

    println!("{}", format_status(&status, detailed));

    Ok(())
}
