//! Kill command implementation

use anyhow::Result;

use crate::ipc::OrchestratorClient;
use crate::output::{print_error, print_success, print_warning};

/// Execute the kill command
pub async fn kill_command(
    client: &mut OrchestratorClient,
    sessions: &[String],
    force: bool,
) -> Result<()> {
    if sessions.is_empty() {
        print_error("No sessions specified");
        return Ok(());
    }

    if !force && sessions.len() > 1 {
        print_warning(&format!(
            "About to kill {} sessions. Use --force to skip confirmation.",
            sessions.len()
        ));

        // In a full implementation, prompt for confirmation
        print!("Continue? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            print_warning("Aborted");
            return Ok(());
        }
    }

    let mut errors = Vec::new();

    for session_id in sessions {
        match client.kill_session(session_id, force).await {
            Ok(()) => {
                print_success(&format!("Killed session: {}", session_id));
            }
            Err(e) => {
                print_error(&format!("Failed to kill session {}: {}", session_id, e));
                errors.push((session_id.clone(), e));
            }
        }
    }

    if !errors.is_empty() {
        anyhow::bail!("Failed to kill {} session(s)", errors.len());
    }

    Ok(())
}
