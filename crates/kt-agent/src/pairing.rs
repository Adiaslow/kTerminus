//! Pairing code discovery for easy orchestrator connection
//!
//! This module enables agents to discover orchestrators using a 6-character
//! pairing code instead of requiring manual hostname/IP configuration.
//!
//! The flow:
//! 1. User runs `kt-agent --code ABC123` or is prompted for a code
//! 2. Agent queries Tailscale for all online peers
//! 3. For each peer, probes the IPC port (22230) and verifies the pairing code
//! 4. Connects to the peer that validates the code

use std::time::Duration;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use kt_core::ipc::{IpcRequest, IpcResponse, DEFAULT_IPC_PORT};
use kt_core::tailscale::{get_tailscale_peers, TailscalePeer};

/// Result of pairing discovery
#[derive(Debug)]
pub struct DiscoveredOrchestrator {
    /// The Tailscale peer that was discovered
    pub peer: TailscalePeer,
    /// The address to connect to (IP:port)
    pub ssh_address: String,
}

/// Discover an orchestrator using a pairing code
///
/// This probes all online Tailscale peers to find one running an orchestrator
/// with the given pairing code. Also checks localhost for same-machine testing.
pub async fn discover_orchestrator(pairing_code: &str) -> Result<DiscoveredOrchestrator> {
    tracing::info!("Discovering orchestrator with pairing code...");

    // First, check localhost (for same-machine testing)
    tracing::debug!("Checking localhost for orchestrator...");
    if let Ok(true) = probe_address_for_code("127.0.0.1", pairing_code).await {
        tracing::info!("Found orchestrator on localhost");
        return Ok(DiscoveredOrchestrator {
            peer: TailscalePeer {
                device_name: "localhost".to_string(),
                dns_name: "localhost".to_string(),
                ips: vec!["127.0.0.1".to_string()],
                online: true,
            },
            ssh_address: "127.0.0.1:2222".to_string(),
        });
    }

    // Get Tailscale peers
    let peers = get_tailscale_peers().context("Failed to get Tailscale peers")?;
    let online_peers: Vec<_> = peers.into_iter().filter(|p| p.online).collect();

    if online_peers.is_empty() {
        anyhow::bail!(
            "No orchestrator found with pairing code '{}'\n\n\
             Checked localhost and found no Tailscale peers.\n\
             Make sure the orchestrator is running.",
            pairing_code.to_uppercase()
        );
    }

    tracing::debug!(
        "Probing {} online peers for pairing code...",
        online_peers.len()
    );

    // Probe peers concurrently
    let mut tasks = Vec::new();
    for peer in online_peers {
        let code = pairing_code.to_string();
        tasks.push(tokio::spawn(async move {
            match probe_peer_for_code(&peer, &code).await {
                Ok(true) => Some(peer),
                Ok(false) => None,
                Err(e) => {
                    tracing::debug!("Probe failed for {}: {}", peer.device_name, e);
                    None
                }
            }
        }));
    }

    // Wait for all probes to complete
    let mut found_peer: Option<TailscalePeer> = None;
    for task in tasks {
        if let Ok(Some(peer)) = task.await {
            found_peer = Some(peer);
            break;
        }
    }

    let peer = found_peer.ok_or_else(|| {
        anyhow::anyhow!(
            "No orchestrator found with pairing code '{}'\n\n\
             Make sure:\n\
             - The orchestrator is running\n\
             - You entered the correct pairing code\n\
             - Both machines are on the same Tailscale network",
            pairing_code.to_uppercase()
        )
    })?;

    // Get IPv4 address for SSH connection
    let ip = peer
        .ips
        .iter()
        .find(|ip| !ip.contains(':'))
        .or(peer.ips.first())
        .cloned()
        .context("Peer has no IP addresses")?;

    let ssh_address = format!("{}:2222", ip);

    tracing::info!(
        "Found orchestrator '{}' at {}",
        peer.device_name,
        ssh_address
    );

    Ok(DiscoveredOrchestrator { peer, ssh_address })
}

/// Probe a specific IP address to check if it's running an orchestrator with the given code
///
/// Connects to the IPC port on the given IP address and sends a pairing code
/// verification request. Uses a short 500ms timeout to avoid blocking on
/// unreachable hosts.
///
/// # Arguments
/// * `ip` - The IP address to probe (without port)
/// * `code` - The pairing code to verify
///
/// # Returns
/// * `Ok(true)` - The orchestrator at this address has the matching pairing code
/// * `Ok(false)` - The orchestrator responded but the code doesn't match
/// * `Err(_)` - Connection failed or the host is not running an orchestrator
async fn probe_address_for_code(ip: &str, code: &str) -> Result<bool> {
    let address = format!("{}:{}", ip, DEFAULT_IPC_PORT);

    // Connect with timeout
    let stream = tokio::time::timeout(Duration::from_millis(500), TcpStream::connect(&address))
        .await
        .map_err(|_| anyhow::anyhow!("Connection timed out"))?
        .context("Failed to connect")?;

    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    // Send VerifyPairingCode request
    let request = IpcRequest::VerifyPairingCode {
        code: code.to_string(),
    };
    let mut request_json =
        serde_json::to_string(&request).context("Failed to serialize request")?;
    request_json.push('\n');

    writer
        .write_all(request_json.as_bytes())
        .await
        .context("Failed to send request")?;

    // Read response with timeout
    let mut line = String::new();
    tokio::time::timeout(Duration::from_millis(500), reader.read_line(&mut line))
        .await
        .map_err(|_| anyhow::anyhow!("Response timed out"))?
        .context("Failed to read response")?;

    // Parse response
    let response: IpcResponse =
        serde_json::from_str(line.trim()).context("Failed to parse response")?;

    match response {
        IpcResponse::PairingCodeValid { valid } => Ok(valid),
        _ => Ok(false),
    }
}

/// Probe a Tailscale peer to check if it's running an orchestrator with the given code
///
/// Similar to `probe_address_for_code`, but takes a `TailscalePeer` and
/// automatically extracts the IPv4 address (preferred over IPv6). Uses a
/// longer 2-second timeout as Tailscale peers may have higher latency.
///
/// # Arguments
/// * `peer` - The Tailscale peer to probe
/// * `code` - The pairing code to verify
///
/// # Returns
/// * `Ok(true)` - The orchestrator on this peer has the matching pairing code
/// * `Ok(false)` - The peer responded but the code doesn't match
/// * `Err(_)` - Connection failed or the peer is not running an orchestrator
async fn probe_peer_for_code(peer: &TailscalePeer, code: &str) -> Result<bool> {
    // Get IPv4 address (prefer over IPv6)
    let ip = peer
        .ips
        .iter()
        .find(|ip| !ip.contains(':'))
        .or(peer.ips.first())
        .context("Peer has no IP addresses")?;

    let address = format!("{}:{}", ip, DEFAULT_IPC_PORT);

    tracing::debug!("Probing {} at {}", peer.device_name, address);

    // Connect with timeout
    let stream = tokio::time::timeout(Duration::from_secs(2), TcpStream::connect(&address))
        .await
        .map_err(|_| anyhow::anyhow!("Connection timed out"))?
        .context("Failed to connect")?;

    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    // Send VerifyPairingCode request
    let request = IpcRequest::VerifyPairingCode {
        code: code.to_string(),
    };
    let mut request_json =
        serde_json::to_string(&request).context("Failed to serialize request")?;
    request_json.push('\n');

    writer
        .write_all(request_json.as_bytes())
        .await
        .context("Failed to send request")?;

    // Read response with timeout
    let mut line = String::new();
    tokio::time::timeout(Duration::from_secs(2), reader.read_line(&mut line))
        .await
        .map_err(|_| anyhow::anyhow!("Response timed out"))?
        .context("Failed to read response")?;

    // Parse response
    let response: IpcResponse =
        serde_json::from_str(line.trim()).context("Failed to parse response")?;

    match response {
        IpcResponse::PairingCodeValid { valid } => Ok(valid),
        _ => Ok(false), // Unexpected response - not our orchestrator
    }
}

/// Prompt the user to enter a pairing code from stdin
///
/// Displays a prompt and reads a 6-character pairing code from the user.
/// The input is converted to uppercase and validated to ensure it's exactly
/// 6 characters.
///
/// # Returns
/// The validated pairing code in uppercase, or an error if:
/// - The input is empty
/// - The input is not exactly 6 characters
/// - An I/O error occurs
pub fn prompt_pairing_code() -> Result<String> {
    use std::io::{self, Write};

    print!("Enter pairing code: ");
    io::stdout().flush()?;

    let mut code = String::new();
    io::stdin().read_line(&mut code)?;

    let code = code.trim().to_uppercase();

    if code.is_empty() {
        anyhow::bail!("Pairing code cannot be empty");
    }

    if code.len() != 6 {
        anyhow::bail!("Pairing code should be 6 characters (got {})", code.len());
    }

    Ok(code)
}
