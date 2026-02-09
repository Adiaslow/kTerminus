//! Automatic setup and initialization for k-Terminus
//!
//! Handles first-run configuration and key generation.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::config::default_config_dir;
use crate::tailscale::{self, TailscaleInfo};

/// Setup result containing paths to generated files
#[derive(Debug)]
pub struct SetupResult {
    pub config_dir: PathBuf,
    pub host_key_path: PathBuf,
    pub agent_key_path: PathBuf,
    pub agent_key_pub_path: PathBuf,
    pub config_path: PathBuf,
    pub was_initialized: bool,
    /// Tailscale information (if available)
    pub tailscale: Option<TailscaleInfo>,
}

/// Check if k-Terminus has been initialized
pub fn is_initialized() -> bool {
    let config_dir = default_config_dir();
    config_dir.join("initialized").exists()
}

/// Run automatic first-time setup
///
/// This creates all necessary directories, generates SSH keys,
/// and creates default configuration files.
pub fn auto_setup() -> Result<SetupResult> {
    let config_dir = default_config_dir();

    // Check if already initialized
    if is_initialized() {
        // Still try to get Tailscale info
        let tailscale = tailscale::get_tailscale_info().ok().flatten();
        return Ok(SetupResult {
            config_dir: config_dir.clone(),
            host_key_path: config_dir.join("host_key"),
            agent_key_path: config_dir.join("agent_key"),
            agent_key_pub_path: config_dir.join("agent_key.pub"),
            config_path: config_dir.join("config.toml"),
            was_initialized: false,
            tailscale,
        });
    }

    tracing::info!("First-time setup: initializing k-Terminus...");

    // Step 1: Check and setup Tailscale
    let tailscale_info = setup_tailscale()?;

    // Create config directory
    fs::create_dir_all(&config_dir)
        .with_context(|| format!("Failed to create config directory: {:?}", config_dir))?;

    // Generate host key (for orchestrator identity)
    let host_key_path = config_dir.join("host_key");
    if !host_key_path.exists() {
        generate_ed25519_key(&host_key_path, "k-terminus orchestrator host key")?;
        tracing::info!("Generated orchestrator host key");
    }

    // Generate agent key (for agents to authenticate)
    let agent_key_path = config_dir.join("agent_key");
    let agent_key_pub_path = config_dir.join("agent_key.pub");
    if !agent_key_path.exists() {
        generate_ed25519_key(&agent_key_path, "k-terminus agent key")?;
        tracing::info!("Generated agent authentication key");
    }

    // Create default configuration
    let config_path = config_dir.join("config.toml");
    if !config_path.exists() {
        let default_config = generate_default_config(&config_dir, tailscale_info.as_ref());
        fs::write(&config_path, default_config).with_context(|| "Failed to write config file")?;
        tracing::info!("Created default configuration");
    }

    // Mark as initialized
    fs::write(config_dir.join("initialized"), "")
        .with_context(|| "Failed to create initialized marker")?;

    tracing::info!("k-Terminus initialization complete!");
    tracing::info!("Config directory: {:?}", config_dir);

    if let Some(ref ts) = tailscale_info {
        tracing::info!("Tailscale device: {}", ts.hostname);
    }

    Ok(SetupResult {
        config_dir: config_dir.clone(),
        host_key_path,
        agent_key_path,
        agent_key_pub_path,
        config_path,
        was_initialized: true,
        tailscale: tailscale_info,
    })
}

/// Setup Tailscale for networking
/// Returns TailscaleInfo if successful, None if user needs to set up manually
fn setup_tailscale() -> Result<Option<TailscaleInfo>> {
    // Check if Tailscale is installed
    if !tailscale::is_tailscale_installed() {
        tracing::info!("Tailscale not found - attempting to install...");

        // Try auto-install
        match tailscale::auto_install_tailscale() {
            Ok(true) => {
                tracing::info!("Tailscale installed successfully");
            }
            Ok(false) | Err(_) => {
                // Auto-install failed or not supported, show instructions
                tracing::warn!("Could not auto-install Tailscale");
                tracing::info!(
                    "Installation instructions:\n{}\nAfter installing Tailscale, run 'k-terminus setup' again.",
                    tailscale::get_install_instructions()
                );
                return Ok(None);
            }
        }
    }

    // Check Tailscale status
    match tailscale::get_tailscale_info()? {
        Some(info) if info.logged_in => {
            tracing::info!("Tailscale authenticated: {}", info.hostname);
            Ok(Some(info))
        }
        Some(_) | None => {
            // Not logged in, start authentication
            tracing::info!("Starting Tailscale authentication...");

            match tailscale::start_tailscale_auth() {
                Ok(Some(url)) => {
                    tracing::info!("Tailscale authentication required");
                    tracing::info!("Open this URL to authenticate: {}", url);

                    // Try to open browser
                    #[cfg(target_os = "macos")]
                    let _ = Command::new("open").arg(&url).spawn();
                    #[cfg(target_os = "linux")]
                    let _ = Command::new("xdg-open").arg(&url).spawn();
                    #[cfg(target_os = "windows")]
                    let _ = Command::new("cmd").args(["/c", "start", &url]).spawn();

                    tracing::info!("Waiting for authentication...");

                    // Poll for authentication (blocking, up to 5 minutes)
                    let start = std::time::Instant::now();
                    let timeout = std::time::Duration::from_secs(300);

                    while start.elapsed() < timeout {
                        std::thread::sleep(std::time::Duration::from_secs(3));

                        if let Ok(Some(info)) = tailscale::get_tailscale_info() {
                            if info.logged_in {
                                tracing::info!("Tailscale authenticated: {}", info.hostname);
                                return Ok(Some(info));
                            }
                        }
                    }

                    tracing::warn!("Timeout waiting for Tailscale authentication. Run setup again after authenticating.");
                    Ok(None)
                }
                Ok(None) => {
                    // Already authenticated, get info
                    if let Some(info) = tailscale::get_tailscale_info()? {
                        if info.logged_in {
                            return Ok(Some(info));
                        }
                    }
                    Ok(None)
                }
                Err(e) => {
                    tracing::warn!(
                        "Tailscale authentication error: {}. Run 'sudo tailscale up' manually, then run 'k-terminus setup' again.",
                        e
                    );
                    Ok(None)
                }
            }
        }
    }
}

/// Generate an ED25519 SSH key pair
fn generate_ed25519_key(path: &Path, comment: &str) -> Result<()> {
    // Convert path to string, handling non-UTF8 paths gracefully
    let path_str = path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Key path contains invalid UTF-8: {:?}", path))?;

    let status = Command::new("ssh-keygen")
        .args([
            "-t",
            "ed25519",
            "-f",
            path_str,
            "-N",
            "", // No passphrase
            "-C",
            comment,
            "-q", // Quiet
        ])
        .status()
        .with_context(|| "Failed to run ssh-keygen")?;

    if !status.success() {
        anyhow::bail!("ssh-keygen failed with status: {}", status);
    }

    // Set restrictive permissions on private key
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(path, perms)?;
    }

    Ok(())
}

/// Generate default configuration content
fn generate_default_config(config_dir: &Path, tailscale: Option<&TailscaleInfo>) -> String {
    let tailscale_section = if let Some(ts) = tailscale {
        format!(
            r#"
# Tailscale hostname (auto-detected)
tailscale_hostname = "{}"
"#,
            ts.hostname
        )
    } else {
        String::new()
    };

    format!(
        r#"# k-Terminus Configuration
# Auto-generated on first run

[orchestrator]
# Address to bind SSH server (agents connect here)
bind_address = "0.0.0.0:2222"

# Host key for orchestrator identity
host_key_path = "{}/host_key"
{tailscale_section}
# Heartbeat interval in seconds
heartbeat_interval = 30

# Connection timeout in seconds
heartbeat_timeout = 90

[orchestrator.backoff]
initial = 1
max = 60
multiplier = 2.0
jitter = 0.25

[agent]
# Default orchestrator address (can be overridden per-agent)
# orchestrator_address = "your-server:2222"

# Default shell for new sessions
# default_shell = "/bin/bash"
"#,
        config_dir.display()
    )
}
