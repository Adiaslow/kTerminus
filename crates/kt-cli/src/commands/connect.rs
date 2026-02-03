//! Connect command implementation

use anyhow::Result;

use crate::ipc::{OrchestratorClient, TerminalSession};
use crate::output::{print_error, print_info, print_success};

/// Execute the connect command - create new session and attach
pub async fn connect_command(
    client: OrchestratorClient,
    machine: &str,
    shell: Option<&str>,
) -> Result<()> {
    // Need a mutable client for the initial request
    let mut client = client;

    print_info(&format!("Creating session on '{}'...", machine));

    // Create session
    let session = match client.create_session(machine, shell).await {
        Ok(s) => s,
        Err(e) => {
            print_error(&format!("Failed to create session: {}", e));
            return Err(e);
        }
    };

    print_success(&format!(
        "Session created: {} (PID: {})",
        session.id,
        session
            .pid
            .map(|p| p.to_string())
            .unwrap_or_else(|| "pending".to_string())
    ));

    // Attach to the session
    print_info("Attaching to session... (Press Ctrl+] to detach)");

    // Create terminal session and run it
    let terminal = TerminalSession::new(client, session.id.clone()).await?;
    terminal.run().await?;

    print_success("Detached from session");
    Ok(())
}

/// Attach to an existing session
pub async fn attach_command(client: OrchestratorClient, session_id: &str) -> Result<()> {
    print_info(&format!("Attaching to session {}...", session_id));
    print_info("Press Ctrl+] to detach");

    // Create terminal session and run it
    let terminal = TerminalSession::new(client, session_id.to_string()).await?;
    terminal.run().await?;

    print_success("Detached from session");
    Ok(())
}
