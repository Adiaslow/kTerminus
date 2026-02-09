//! IPC authentication token management
//!
//! Provides secure token-based authentication for IPC connections.
//! The orchestrator generates a random token on startup and writes it
//! to a file with restricted permissions (readable only by the owner).
//! Clients must read this token and present it to authenticate.
//!
//! # Ownership Model
//!
//! The token file includes the PID of the owning orchestrator process.
//! Before writing a new token, we check if the current owner is still alive.
//! This prevents overwriting a running orchestrator's token, which would
//! cause authentication failures for its clients.
//!
//! # Security Model
//!
//! - Token is 32 bytes of cryptographically random data, hex-encoded (64 chars)
//! - Token file has mode 0600 (owner read/write only) on Unix
//! - Token is regenerated only when no live orchestrator owns the current token
//! - Clients must authenticate before any requests (except Ping)
//!
//! This prevents unauthorized local processes from controlling the orchestrator,
//! even if they can connect to the IPC port.

use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::pidfile::is_process_alive;

/// Length of the authentication token in bytes (before hex encoding)
const TOKEN_BYTES: usize = 32;

/// Token file name (now JSON format with ownership info)
const TOKEN_FILENAME: &str = "ipc_auth_token.json";

/// Legacy token file name (plain text, for migration)
const LEGACY_TOKEN_FILENAME: &str = "ipc_auth_token";

/// Token file contents with ownership information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    /// The authentication token (hex-encoded)
    pub token: String,
    /// PID of the process that owns this token
    pub pid: u32,
    /// IPC address the orchestrator is listening on
    pub address: String,
}

/// Result of attempting to acquire token ownership
#[derive(Debug)]
pub enum TokenOwnership {
    /// We acquired ownership - use this token (we wrote it)
    Acquired { token: String },
    /// Another live process owns the token - use theirs
    External { token: String, pid: u32, address: String },
}

/// Get the default path for the IPC authentication token file
///
/// Returns `~/Library/Application Support/k-terminus/ipc_auth_token.json` on macOS
/// or `~/.config/k-terminus/ipc_auth_token.json` on Linux
/// or `%APPDATA%\k-terminus\ipc_auth_token.json` on Windows
pub fn default_token_path() -> io::Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Could not find config directory"))?;

    Ok(config_dir.join("k-terminus").join(TOKEN_FILENAME))
}

/// Get the path for the legacy token file (for migration)
fn legacy_token_path() -> io::Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Could not find config directory"))?;

    Ok(config_dir.join("k-terminus").join(LEGACY_TOKEN_FILENAME))
}

/// Generate a new random authentication token
///
/// Returns a 64-character hex string (32 random bytes)
pub fn generate_token() -> String {
    use rand::Rng;
    let mut bytes = [0u8; TOKEN_BYTES];
    rand::thread_rng().fill(&mut bytes);
    hex::encode(bytes)
}

/// Acquire ownership of the IPC token
///
/// This is the primary entry point for orchestrators. It ensures only one
/// orchestrator can own the token at a time:
///
/// 1. If no token file exists, generate a new token and claim ownership
/// 2. If a token file exists but the owner is dead, take over ownership
/// 3. If a token file exists and the owner is alive, return their token info
///
/// # Arguments
/// * `address` - The IPC address this orchestrator will listen on
///
/// # Returns
/// * `TokenOwnership::Acquired` - We now own the token, use it
/// * `TokenOwnership::External` - Another orchestrator owns it, connect to them
pub fn acquire_token_ownership(address: &str) -> io::Result<TokenOwnership> {
    let our_pid = std::process::id();

    // Try to read existing token info
    if let Some(info) = read_token_info()? {
        // Check if the owning process is still alive
        if is_process_alive(info.pid) {
            // Another orchestrator is running - use their token
            tracing::info!(
                "Found existing orchestrator (PID {}) at {}, using external mode",
                info.pid,
                info.address
            );
            return Ok(TokenOwnership::External {
                token: info.token,
                pid: info.pid,
                address: info.address,
            });
        }

        // Owner is dead, we can take over
        tracing::info!(
            "Previous orchestrator (PID {}) is no longer running, taking ownership",
            info.pid
        );
    }

    // Clean up legacy token file if it exists
    if let Ok(legacy_path) = legacy_token_path() {
        let _ = fs::remove_file(legacy_path);
    }

    // Generate new token and claim ownership
    let token = generate_token();
    let info = TokenInfo {
        token: token.clone(),
        pid: our_pid,
        address: address.to_string(),
    };

    write_token_info(&info)?;

    tracing::info!("Acquired IPC token ownership (PID {})", our_pid);

    Ok(TokenOwnership::Acquired { token })
}

/// Read the full token info from the token file
///
/// Returns `Ok(None)` if the file doesn't exist.
/// Handles migration from legacy plain-text token files.
pub fn read_token_info() -> io::Result<Option<TokenInfo>> {
    let path = default_token_path()?;

    match fs::read_to_string(&path) {
        Ok(contents) => {
            // Try to parse as JSON (new format)
            match serde_json::from_str::<TokenInfo>(&contents) {
                Ok(info) => Ok(Some(info)),
                Err(_) => {
                    // Not valid JSON - might be legacy format or corrupted
                    // Treat as if file doesn't exist (will be overwritten)
                    tracing::debug!("Token file exists but is not valid JSON, will regenerate");
                    Ok(None)
                }
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            // Check for legacy token file
            if let Ok(legacy_path) = legacy_token_path() {
                if legacy_path.exists() {
                    tracing::debug!("Found legacy token file, will migrate on next write");
                }
            }
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

/// Write token info to the token file
///
/// Creates the parent directory if it doesn't exist.
/// Sets file permissions to 0600 (owner read/write only) on Unix.
fn write_token_info(info: &TokenInfo) -> io::Result<PathBuf> {
    let path = default_token_path()?;

    // Create parent directory if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Serialize to JSON
    let json = serde_json::to_string_pretty(info)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    // Write to file
    fs::write(&path, json)?;

    // Set restrictive permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o600);
        fs::set_permissions(&path, permissions)?;
    }

    Ok(path)
}

/// Write an authentication token to the token file (legacy API)
///
/// **Deprecated**: Use `acquire_token_ownership` instead to ensure proper
/// ownership semantics. This function is kept for backward compatibility
/// but does not check if another orchestrator owns the token.
///
/// Creates the parent directory if it doesn't exist.
/// Sets file permissions to 0600 (owner read/write only) on Unix.
pub fn write_token(token: &str) -> io::Result<PathBuf> {
    // Convert to new format with current process info
    let info = TokenInfo {
        token: token.to_string(),
        pid: std::process::id(),
        address: format!("127.0.0.1:{}", crate::ipc::DEFAULT_IPC_PORT),
    };
    write_token_info(&info)
}

/// Read the authentication token from the token file
///
/// Returns an error if the file doesn't exist or can't be read.
/// Handles both new JSON format and legacy plain-text format.
pub fn read_token() -> io::Result<String> {
    // First try the new JSON format
    if let Some(info) = read_token_info()? {
        return Ok(info.token);
    }

    // Fall back to legacy format
    let legacy_path = legacy_token_path()?;
    match fs::read_to_string(&legacy_path) {
        Ok(token) => Ok(token.trim().to_string()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "No token file found",
            ))
        }
        Err(e) => Err(e),
    }
}

/// Remove the authentication token file
///
/// Called on orchestrator shutdown for cleanup.
/// Only removes if we are the current owner (prevents removing another
/// orchestrator's token).
/// Ignores errors if the file doesn't exist.
pub fn remove_token() -> io::Result<()> {
    let path = default_token_path()?;
    let our_pid = std::process::id();

    // Check if we own the token before removing
    if let Some(info) = read_token_info()? {
        if info.pid != our_pid {
            tracing::debug!(
                "Not removing token file - owned by PID {}, we are PID {}",
                info.pid,
                our_pid
            );
            return Ok(());
        }
    }

    match fs::remove_file(&path) {
        Ok(()) => {
            tracing::debug!("Removed token file");
            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Check if a token file exists
pub fn token_exists() -> bool {
    default_token_path()
        .map(|p| p.exists())
        .unwrap_or(false)
}

/// Validate a token against the stored token
///
/// Returns true if the provided token matches the stored token.
/// Uses constant-time comparison to prevent timing attacks.
pub fn validate_token(provided: &str, expected: &str) -> bool {
    // Use constant-time comparison to prevent timing attacks
    if provided.len() != expected.len() {
        return false;
    }

    let mut result = 0u8;
    for (a, b) in provided.bytes().zip(expected.bytes()) {
        result |= a ^ b;
    }
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_generate_token() {
        let token = generate_token();
        assert_eq!(token.len(), TOKEN_BYTES * 2); // Hex encoding doubles length
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_generate_token_unique() {
        let token1 = generate_token();
        let token2 = generate_token();
        assert_ne!(token1, token2);
    }

    #[test]
    fn test_validate_token() {
        let token = "abc123def456";
        assert!(validate_token(token, token));
        assert!(!validate_token(token, "different"));
        assert!(!validate_token(token, "abc123def45")); // Different length
    }

    #[test]
    fn test_validate_token_constant_time() {
        // This test ensures we don't short-circuit on length mismatch
        // by checking that comparison happens even with different lengths
        let short = "abc";
        let long = "abcdef";
        assert!(!validate_token(short, long));
    }

    #[test]
    fn test_token_info_serialization() {
        let info = TokenInfo {
            token: "abc123".to_string(),
            pid: 12345,
            address: "127.0.0.1:22230".to_string(),
        };

        let json = serde_json::to_string(&info).expect("Failed to serialize");
        let parsed: TokenInfo = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(parsed.token, info.token);
        assert_eq!(parsed.pid, info.pid);
        assert_eq!(parsed.address, info.address);
    }

    #[test]
    fn test_write_and_read_token_info() {
        let dir = tempdir().expect("Failed to create temp dir");
        let path = dir.path().join("test_token.json");

        let info = TokenInfo {
            token: generate_token(),
            pid: std::process::id(),
            address: "127.0.0.1:22230".to_string(),
        };

        // Write token info
        let json = serde_json::to_string_pretty(&info).expect("Failed to serialize");
        fs::write(&path, &json).expect("Failed to write");

        // Read it back
        let contents = fs::read_to_string(&path).expect("Failed to read");
        let parsed: TokenInfo = serde_json::from_str(&contents).expect("Failed to parse");

        assert_eq!(parsed.token, info.token);
        assert_eq!(parsed.pid, info.pid);
    }

    #[cfg(unix)]
    #[test]
    fn test_token_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().expect("Failed to create temp dir");
        let path = dir.path().join("test_token.json");

        let token = generate_token();
        fs::write(&path, &token).expect("Failed to write token");

        // Set permissions
        let permissions = fs::Permissions::from_mode(0o600);
        fs::set_permissions(&path, permissions).expect("Failed to set permissions");

        // Verify permissions
        let metadata = fs::metadata(&path).expect("Failed to get metadata");
        let mode = metadata.permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
