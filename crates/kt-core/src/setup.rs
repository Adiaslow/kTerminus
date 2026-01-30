//! Automatic setup and initialization for k-Terminus
//!
//! Handles first-run configuration, key generation, and agent pairing.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::config::default_config_dir;

/// Setup result containing paths to generated files
#[derive(Debug)]
pub struct SetupResult {
    pub config_dir: PathBuf,
    pub host_key_path: PathBuf,
    pub agent_key_path: PathBuf,
    pub agent_key_pub_path: PathBuf,
    pub authorized_keys_path: PathBuf,
    pub config_path: PathBuf,
    pub was_initialized: bool,
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
        return Ok(SetupResult {
            config_dir: config_dir.clone(),
            host_key_path: config_dir.join("host_key"),
            agent_key_path: config_dir.join("agent_key"),
            agent_key_pub_path: config_dir.join("agent_key.pub"),
            authorized_keys_path: config_dir.join("authorized_keys"),
            config_path: config_dir.join("config.toml"),
            was_initialized: false,
        });
    }

    tracing::info!("First-time setup: initializing k-Terminus...");

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

    // Create authorized_keys from agent public key
    let authorized_keys_path = config_dir.join("authorized_keys");
    if !authorized_keys_path.exists() && agent_key_pub_path.exists() {
        fs::copy(&agent_key_pub_path, &authorized_keys_path)
            .with_context(|| "Failed to create authorized_keys")?;
        tracing::info!("Created authorized_keys file");
    }

    // Create default configuration
    let config_path = config_dir.join("config.toml");
    if !config_path.exists() {
        let default_config = generate_default_config(&config_dir);
        fs::write(&config_path, default_config)
            .with_context(|| "Failed to write config file")?;
        tracing::info!("Created default configuration");
    }

    // Mark as initialized
    fs::write(config_dir.join("initialized"), "")
        .with_context(|| "Failed to create initialized marker")?;

    tracing::info!("k-Terminus initialization complete!");
    tracing::info!("Config directory: {:?}", config_dir);

    Ok(SetupResult {
        config_dir: config_dir.clone(),
        host_key_path,
        agent_key_path,
        agent_key_pub_path,
        authorized_keys_path,
        config_path,
        was_initialized: true,
    })
}

/// Generate an ED25519 SSH key pair
fn generate_ed25519_key(path: &Path, comment: &str) -> Result<()> {
    let status = Command::new("ssh-keygen")
        .args([
            "-t", "ed25519",
            "-f", path.to_str().unwrap(),
            "-N", "",  // No passphrase
            "-C", comment,
            "-q",  // Quiet
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
fn generate_default_config(config_dir: &Path) -> String {
    format!(r#"# k-Terminus Configuration
# Auto-generated on first run

[orchestrator]
# Address to bind SSH server (agents connect here)
bind_address = "0.0.0.0:2222"

# Authorized public keys for agent authentication
auth_keys = ["{}/authorized_keys"]

# Host key for orchestrator identity
host_key_path = "{}/host_key"

# Heartbeat interval in seconds
heartbeat_interval = 30

# Connection timeout in seconds
connect_timeout = 10

[orchestrator.backoff]
initial_secs = 1
max_secs = 60
multiplier = 2.0

[agent]
# Default orchestrator address (can be overridden per-agent)
# orchestrator_address = "your-server:2222"

# Default shell for new sessions
# default_shell = "/bin/bash"
"#,
        config_dir.display(),
        config_dir.display()
    )
}

/// Generate a pairing command for a remote machine
pub fn generate_pairing_command(orchestrator_address: &str) -> Result<String> {
    let config_dir = default_config_dir();
    let agent_key_path = config_dir.join("agent_key");

    // Read the private key
    let key_content = fs::read_to_string(&agent_key_path)
        .with_context(|| "Agent key not found. Run setup first.")?;

    // Base64 encode the key for easy transfer
    let key_b64 = base64_encode(&key_content);

    // Generate the install/pair command
    let command = format!(
        r#"curl -sSL https://k-terminus.dev/install.sh | sh -s -- --pair "{}" --key "{}""#,
        orchestrator_address,
        key_b64
    );

    Ok(command)
}

/// Generate a simple local pairing command (for when curl isn't available)
pub fn generate_local_pairing_info(orchestrator_address: &str) -> Result<PairingInfo> {
    let config_dir = default_config_dir();
    let agent_key_path = config_dir.join("agent_key");
    let host_key_pub_path = config_dir.join("host_key.pub");

    let agent_key = fs::read_to_string(&agent_key_path)
        .with_context(|| "Agent key not found")?;

    let host_key_pub = fs::read_to_string(&host_key_pub_path)
        .with_context(|| "Host key not found")?;

    // Get host key fingerprint
    let fingerprint = get_key_fingerprint(&host_key_pub_path)?;

    Ok(PairingInfo {
        orchestrator_address: orchestrator_address.to_string(),
        agent_key,
        host_fingerprint: fingerprint,
    })
}

/// Pairing information for remote agents
#[derive(Debug, Clone)]
pub struct PairingInfo {
    pub orchestrator_address: String,
    pub agent_key: String,
    pub host_fingerprint: String,
}

impl PairingInfo {
    /// Generate the kt-agent command to run on remote machine
    pub fn agent_command(&self) -> String {
        format!(
            "kt-agent --orchestrator {} --foreground",
            self.orchestrator_address
        )
    }
}

/// Get SSH key fingerprint
fn get_key_fingerprint(pub_key_path: &Path) -> Result<String> {
    let output = Command::new("ssh-keygen")
        .args(["-lf", pub_key_path.to_str().unwrap()])
        .output()
        .with_context(|| "Failed to get key fingerprint")?;

    if !output.status.success() {
        anyhow::bail!("ssh-keygen fingerprint failed");
    }

    let fingerprint = String::from_utf8_lossy(&output.stdout);
    // Extract just the fingerprint part (second field)
    Ok(fingerprint
        .split_whitespace()
        .nth(1)
        .unwrap_or("unknown")
        .to_string())
}

/// Simple base64 encoding
fn base64_encode(input: &str) -> String {
    use std::io::Write;
    let mut buf = Vec::new();
    {
        let mut encoder = base64_writer(&mut buf);
        encoder.write_all(input.as_bytes()).unwrap();
    }
    String::from_utf8(buf).unwrap()
}

fn base64_writer(writer: &mut Vec<u8>) -> impl Write + '_ {
    struct Base64Writer<'a>(&'a mut Vec<u8>);

    impl Write for Base64Writer<'_> {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

            for chunk in buf.chunks(3) {
                let b0 = chunk[0] as usize;
                let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
                let b2 = chunk.get(2).copied().unwrap_or(0) as usize;

                self.0.push(ALPHABET[b0 >> 2]);
                self.0.push(ALPHABET[((b0 & 0x03) << 4) | (b1 >> 4)]);

                if chunk.len() > 1 {
                    self.0.push(ALPHABET[((b1 & 0x0f) << 2) | (b2 >> 6)]);
                } else {
                    self.0.push(b'=');
                }

                if chunk.len() > 2 {
                    self.0.push(ALPHABET[b2 & 0x3f]);
                } else {
                    self.0.push(b'=');
                }
            }
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    Base64Writer(writer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_encode() {
        assert_eq!(base64_encode("hello"), "aGVsbG8=");
        assert_eq!(base64_encode("hi"), "aGk=");
        assert_eq!(base64_encode("a"), "YQ==");
    }
}
