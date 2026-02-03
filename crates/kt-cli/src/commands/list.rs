//! List command implementation

use anyhow::Result;

use crate::ipc::OrchestratorClient;
use crate::output::{format_machines, format_sessions, print_error};

/// Execute the list command
pub async fn list_command(
    client: &mut OrchestratorClient,
    machine: Option<&str>,
    tag: Option<&[String]>,
    long: bool,
) -> Result<()> {
    // List machines
    let machines = match client.list_machines().await {
        Ok(m) => m,
        Err(e) => {
            print_error(&format!("Failed to list machines: {}", e));
            return Err(e);
        }
    };

    // Filter by machine name if specified
    let machines: Vec<_> = if let Some(filter) = machine {
        machines
            .into_iter()
            .filter(|m| {
                m.id.contains(filter)
                    || m.alias
                        .as_ref()
                        .map(|a| a.contains(filter))
                        .unwrap_or(false)
                    || m.hostname.contains(filter)
            })
            .collect()
    } else {
        machines
    };

    // Filter by tag if specified
    let machines: Vec<_> = if let Some(_tags) = tag {
        // Tags would be stored in MachineInfo in a full implementation
        // For now, we just pass through
        machines
    } else {
        machines
    };

    // Print machine table
    println!("Connected Machines:");
    println!("{}", format_machines(&machines, long));

    // List sessions if specific machine requested
    if machine.is_some() || machines.len() == 1 {
        let machine_id = machine.or_else(|| machines.first().map(|m| m.id.as_str()));

        if let Some(mid) = machine_id {
            let sessions = match client.list_sessions(Some(mid)).await {
                Ok(s) => s,
                Err(e) => {
                    print_error(&format!("Failed to list sessions: {}", e));
                    return Err(e);
                }
            };

            println!("\nActive Sessions:");
            println!("{}", format_sessions(&sessions));
        }
    }

    Ok(())
}
