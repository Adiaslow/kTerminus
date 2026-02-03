//! Tailscale integration for k-Terminus
//!
//! Provides detection, installation, and configuration of Tailscale
//! for seamless networking across NAT/firewalls.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::process::Command;

/// Information about the local Tailscale installation
#[derive(Debug, Clone)]
pub struct TailscaleInfo {
    /// Device name (e.g., "adams-macbook")
    pub device_name: String,
    /// Tailnet domain (e.g., "tailnet-abc.ts.net" or "tail1234.ts.net")
    pub tailnet: String,
    /// Tailscale IP address (e.g., "100.64.1.50")
    pub ip: String,
    /// Full hostname (e.g., "adams-macbook.tailnet-abc.ts.net")
    pub hostname: String,
    /// Whether logged in to Tailscale
    pub logged_in: bool,
}

/// Status response from `tailscale status --json`
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TailscaleStatus {
    backend_state: String,
    #[serde(rename = "Self")]
    self_node: Option<SelfNode>,
    current_tailnet: Option<CurrentTailnet>,
    #[serde(default, deserialize_with = "deserialize_null_as_empty_map")]
    peer: std::collections::HashMap<String, PeerNode>,
}

/// Deserialize null as an empty HashMap
fn deserialize_null_as_empty_map<'de, D, K, V>(
    deserializer: D,
) -> Result<std::collections::HashMap<K, V>, D::Error>
where
    D: serde::Deserializer<'de>,
    K: std::cmp::Eq + std::hash::Hash + Deserialize<'de>,
    V: Deserialize<'de>,
{
    Option::<std::collections::HashMap<K, V>>::deserialize(deserializer)
        .map(|opt| opt.unwrap_or_default())
}

/// Information about a peer in the tailnet
#[derive(Debug, Clone)]
pub struct TailscalePeer {
    /// Device name (e.g., "lab-server")
    pub device_name: String,
    /// Full DNS name (e.g., "lab-server.tailnet-abc.ts.net")
    pub dns_name: String,
    /// Tailscale IP addresses
    pub ips: Vec<String>,
    /// Whether the peer is currently online
    pub online: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PeerNode {
    #[serde(rename = "DNSName")]
    dns_name: String,
    #[serde(rename = "TailscaleIPs")]
    tailscale_ips: Vec<String>,
    online: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SelfNode {
    #[serde(rename = "DNSName")]
    dns_name: String,
    #[serde(rename = "TailscaleIPs")]
    tailscale_ips: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CurrentTailnet {
    /// Tailnet name (required for deserialization but not used)
    #[serde(rename = "Name")]
    _name: String,
    #[serde(rename = "MagicDNSSuffix")]
    magic_dns_suffix: String,
}

/// Check if Tailscale is installed
pub fn is_tailscale_installed() -> bool {
    Command::new("tailscale")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get Tailscale status and information
pub fn get_tailscale_info() -> Result<Option<TailscaleInfo>> {
    if !is_tailscale_installed() {
        return Ok(None);
    }

    let output = Command::new("tailscale")
        .args(["status", "--json"])
        .output()
        .context("Failed to run tailscale status")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Not logged in or not running
        if stderr.contains("not logged in") || stderr.contains("stopped") {
            return Ok(Some(TailscaleInfo {
                device_name: String::new(),
                tailnet: String::new(),
                ip: String::new(),
                hostname: String::new(),
                logged_in: false,
            }));
        }
        anyhow::bail!("Tailscale status failed: {}", stderr);
    }

    let status: TailscaleStatus =
        serde_json::from_slice(&output.stdout).context("Failed to parse tailscale status JSON")?;

    // Check if logged in
    if status.backend_state != "Running" {
        return Ok(Some(TailscaleInfo {
            device_name: String::new(),
            tailnet: String::new(),
            ip: String::new(),
            hostname: String::new(),
            logged_in: false,
        }));
    }

    let self_node = status
        .self_node
        .context("No self node in tailscale status")?;
    let tailnet = status.current_tailnet.context("No current tailnet")?;

    // Parse DNS name to get device name (e.g., "adams-macbook.tailnet-abc.ts.net." -> "adams-macbook")
    let dns_name = self_node.dns_name.trim_end_matches('.');
    let device_name = dns_name.split('.').next().unwrap_or(dns_name).to_string();

    // Get IPv4 address (prefer over IPv6)
    let ip = self_node
        .tailscale_ips
        .iter()
        .find(|ip: &&String| !ip.contains(':'))
        .or(self_node.tailscale_ips.first())
        .cloned()
        .unwrap_or_default();

    Ok(Some(TailscaleInfo {
        device_name: device_name.clone(),
        tailnet: tailnet.magic_dns_suffix.clone(),
        ip,
        hostname: format!("{}.{}", device_name, tailnet.magic_dns_suffix),
        logged_in: true,
    }))
}

/// Get platform-specific Tailscale installation instructions
pub fn get_install_instructions() -> String {
    #[cfg(target_os = "macos")]
    {
        r#"Tailscale is not installed. Install it with:

    brew install tailscale

Or download from: https://tailscale.com/download/mac

After installing, run:
    sudo tailscale up"#
            .to_string()
    }

    #[cfg(target_os = "linux")]
    {
        r#"Tailscale is not installed. Install it with:

    curl -fsSL https://tailscale.com/install.sh | sh

After installing, run:
    sudo tailscale up"#
            .to_string()
    }

    #[cfg(target_os = "windows")]
    {
        r#"Tailscale is not installed. Download it from:

    https://tailscale.com/download/windows

After installing, run Tailscale from the Start menu."#
            .to_string()
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        r#"Tailscale is not installed. Visit https://tailscale.com/download for installation instructions."#.to_string()
    }
}

/// Attempt to auto-install Tailscale
/// Returns Ok(true) if installation was attempted, Ok(false) if user needs to install manually
pub fn auto_install_tailscale() -> Result<bool> {
    #[cfg(target_os = "macos")]
    {
        // Check if Homebrew is available
        if Command::new("brew")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            println!("Installing Tailscale via Homebrew...");
            let status = Command::new("brew")
                .args(["install", "tailscale"])
                .status()
                .context("Failed to run brew install")?;

            if status.success() {
                println!("Tailscale installed successfully!");
                return Ok(true);
            }
        }
        // Fall back to manual installation
        Ok(false)
    }

    #[cfg(target_os = "linux")]
    {
        println!("Installing Tailscale...");
        let status = Command::new("sh")
            .args(["-c", "curl -fsSL https://tailscale.com/install.sh | sh"])
            .status()
            .context("Failed to run Tailscale install script")?;

        if status.success() {
            println!("Tailscale installed successfully!");
            return Ok(true);
        }
        Ok(false)
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows, we can't easily auto-install, prompt user
        Ok(false)
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Ok(false)
    }
}

/// Start Tailscale authentication
/// Returns the auth URL if authentication is needed
pub fn start_tailscale_auth() -> Result<Option<String>> {
    // Run `tailscale up` which will print an auth URL if needed
    let output = Command::new("tailscale")
        .arg("up")
        .output()
        .context("Failed to run tailscale up")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    // Look for auth URL in output
    for line in combined.lines() {
        if line.contains("https://login.tailscale.com/") || line.contains("https://tailscale.com/")
        {
            // Extract URL
            if let Some(url_start) = line.find("https://") {
                let url = line[url_start..]
                    .split_whitespace()
                    .next()
                    .unwrap_or(&line[url_start..]);
                return Ok(Some(url.to_string()));
            }
        }
    }

    // If no URL found, either already authenticated or there was an error
    if output.status.success() {
        Ok(None) // Already authenticated
    } else {
        anyhow::bail!("Tailscale authentication failed: {}", stderr)
    }
}

/// Wait for Tailscale to be authenticated and return info
/// This polls until Tailscale is in a running state
pub async fn wait_for_tailscale_auth(timeout_secs: u64) -> Result<TailscaleInfo> {
    use std::time::{Duration, Instant};
    use tokio::time::sleep;

    let start = Instant::now();
    let timeout = Duration::from_secs(timeout_secs);

    loop {
        if start.elapsed() > timeout {
            anyhow::bail!("Timeout waiting for Tailscale authentication");
        }

        if let Some(info) = get_tailscale_info()? {
            if info.logged_in {
                return Ok(info);
            }
        }

        sleep(Duration::from_secs(2)).await;
    }
}

/// Resolve a device name to its full Tailscale hostname
/// If the name already contains a dot, assumes it's already a full hostname
pub fn resolve_device_name(name: &str, own_tailnet: &str) -> String {
    if name.contains('.') {
        name.to_string()
    } else {
        format!("{}.{}", name, own_tailnet)
    }
}

/// Get all peers in the current tailnet
pub fn get_tailscale_peers() -> Result<Vec<TailscalePeer>> {
    if !is_tailscale_installed() {
        anyhow::bail!("Tailscale is not installed");
    }

    let output = Command::new("tailscale")
        .args(["status", "--json"])
        .output()
        .context("Failed to run tailscale status")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Tailscale status failed: {}", stderr);
    }

    let status: TailscaleStatus =
        serde_json::from_slice(&output.stdout).context("Failed to parse tailscale status JSON")?;

    let peers = status
        .peer
        .into_values()
        .map(|node| {
            let dns_name = node.dns_name.trim_end_matches('.').to_string();
            let device_name = dns_name.split('.').next().unwrap_or(&dns_name).to_string();

            TailscalePeer {
                device_name,
                dns_name,
                ips: node.tailscale_ips,
                online: node.online,
            }
        })
        .collect();

    Ok(peers)
}

/// Look up a peer by their IP address
/// Returns the peer info if the IP belongs to a device in our tailnet
pub fn lookup_peer_by_ip(ip: &std::net::IpAddr) -> Result<Option<TailscalePeer>> {
    let ip_str = ip.to_string();
    let peers = get_tailscale_peers()?;

    Ok(peers.into_iter().find(|peer| peer.ips.contains(&ip_str)))
}

/// Check if an IP address belongs to a peer in our tailnet
pub fn is_tailscale_peer(ip: &std::net::IpAddr) -> bool {
    lookup_peer_by_ip(ip).ok().flatten().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_device_name() {
        let tailnet = "tailnet-abc.ts.net";

        // Short name should get tailnet appended
        assert_eq!(
            resolve_device_name("my-laptop", tailnet),
            "my-laptop.tailnet-abc.ts.net"
        );

        // Full hostname should be returned as-is
        assert_eq!(
            resolve_device_name("other-device.different-tailnet.ts.net", tailnet),
            "other-device.different-tailnet.ts.net"
        );
    }
}
