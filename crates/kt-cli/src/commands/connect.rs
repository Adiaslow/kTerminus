//! Connect command implementation

use anyhow::Result;

use crate::ipc::OrchestratorClient;
use crate::output::{print_error, print_info, print_success};

/// Execute the connect command - create new session and attach
pub async fn connect_command(
    client: &mut OrchestratorClient,
    machine: &str,
    shell: Option<&str>,
) -> Result<()> {
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
        session.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string())
    ));

    // Attach to the session
    attach_to_session(client, &session.id).await
}

/// Attach to an existing session
pub async fn attach_to_session(
    _client: &mut OrchestratorClient,
    session_id: &str,
) -> Result<()> {
    use crossterm::{
        event::{self, Event, KeyCode, KeyModifiers},
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
        ExecutableCommand,
    };
    use std::io::{stdout, Write};

    print_info(&format!("Attaching to session {}...", session_id));
    print_info("Press Ctrl+] to detach");

    // Enter raw mode for terminal passthrough
    enable_raw_mode()?;
    let mut stdout = stdout();
    stdout.execute(EnterAlternateScreen)?;

    // Main event loop for terminal interaction
    // In a full implementation, this would:
    // 1. Open a streaming connection to the orchestrator
    // 2. Forward keyboard input to the session
    // 3. Display output from the session
    // 4. Handle resize events

    loop {
        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key_event) => {
                    // Ctrl+] to detach
                    if key_event.modifiers.contains(KeyModifiers::CONTROL)
                        && key_event.code == KeyCode::Char(']')
                    {
                        break;
                    }

                    // In a full implementation, send key to session
                    // For now, just echo what we receive for testing
                    match key_event.code {
                        KeyCode::Char(c) => {
                            // Would send to session
                            print!("{}", c);
                            stdout.flush()?;
                        }
                        KeyCode::Enter => {
                            println!();
                        }
                        _ => {}
                    }
                }
                Event::Resize(cols, rows) => {
                    // In full implementation, send resize to session
                    tracing::debug!("Terminal resize: {}x{}", cols, rows);
                }
                _ => {}
            }
        }
    }

    // Restore terminal
    stdout.execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;

    print_success("Detached from session");
    Ok(())
}
