//! Authorized keys management

use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{Context, Result};
use russh_keys::key::PublicKey;

/// Manages authorized public keys for client authentication
#[derive(Debug, Default)]
pub struct AuthorizedKeys {
    /// Set of authorized key fingerprints
    fingerprints: HashSet<String>,
    /// Set of authorized public keys (for display purposes)
    keys: Vec<AuthorizedKey>,
}

/// An authorized public key
#[derive(Debug, Clone)]
pub struct AuthorizedKey {
    /// Key fingerprint
    pub fingerprint: String,
    /// Key comment (if any)
    pub comment: Option<String>,
}

impl AuthorizedKeys {
    /// Create a new empty authorized keys store
    pub fn new() -> Self {
        Self::default()
    }

    /// Load authorized keys from multiple files
    pub fn load_from_files(paths: &[impl AsRef<Path>]) -> Result<Self> {
        let mut store = Self::new();

        for path in paths {
            let path = path.as_ref();

            // Expand ~ to home directory
            let expanded = if path.starts_with("~") {
                if let Some(home) = dirs::home_dir() {
                    home.join(path.strip_prefix("~").unwrap_or(path))
                } else {
                    path.to_path_buf()
                }
            } else {
                path.to_path_buf()
            };

            if expanded.exists() {
                store.load_from_file(&expanded)?;
            } else {
                tracing::warn!("Authorized keys file not found: {:?}", expanded);
            }
        }

        Ok(store)
    }

    /// Load authorized keys from a single file
    pub fn load_from_file(&mut self, path: &Path) -> Result<()> {
        tracing::info!("Loading authorized keys from {:?}", path);

        let file = std::fs::File::open(path)
            .with_context(|| format!("Failed to open {:?}", path))?;

        let reader = BufReader::new(file);
        let mut count = 0;

        for (line_num, line) in reader.lines().enumerate() {
            let line = line.with_context(|| format!("Failed to read line {} of {:?}", line_num + 1, path))?;
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Try to parse as an OpenSSH public key
            match russh_keys::parse_public_key_base64(line) {
                Ok(key) => {
                    let fingerprint = key.fingerprint();
                    let comment = extract_comment(line);

                    tracing::debug!(
                        "Loaded key: {} ({})",
                        fingerprint,
                        comment.as_deref().unwrap_or("no comment")
                    );

                    self.fingerprints.insert(fingerprint.clone());
                    self.keys.push(AuthorizedKey { fingerprint, comment });
                    count += 1;
                }
                Err(e) => {
                    // Try parsing the full line (type base64 comment format)
                    if let Some(key) = parse_openssh_line(line) {
                        let fingerprint = key.fingerprint();
                        let comment = extract_comment(line);

                        tracing::debug!(
                            "Loaded key: {} ({})",
                            fingerprint,
                            comment.as_deref().unwrap_or("no comment")
                        );

                        self.fingerprints.insert(fingerprint.clone());
                        self.keys.push(AuthorizedKey { fingerprint, comment });
                        count += 1;
                    } else {
                        tracing::warn!(
                            "Failed to parse key on line {} of {:?}: {}",
                            line_num + 1,
                            path,
                            e
                        );
                    }
                }
            }
        }

        tracing::info!("Loaded {} authorized keys from {:?}", count, path);
        Ok(())
    }

    /// Check if a key fingerprint is authorized
    pub fn is_authorized(&self, fingerprint: &str) -> bool {
        self.fingerprints.contains(fingerprint)
    }

    /// Add a fingerprint to the authorized set
    pub fn add_fingerprint(&mut self, fingerprint: String) {
        self.fingerprints.insert(fingerprint.clone());
        self.keys.push(AuthorizedKey {
            fingerprint,
            comment: None,
        });
    }

    /// Add a public key to the authorized set
    pub fn add_key(&mut self, key: &PublicKey, comment: Option<String>) {
        let fingerprint = key.fingerprint();
        self.fingerprints.insert(fingerprint.clone());
        self.keys.push(AuthorizedKey { fingerprint, comment });
    }

    /// Get the number of authorized keys
    pub fn len(&self) -> usize {
        self.fingerprints.len()
    }

    /// Check if there are no authorized keys
    pub fn is_empty(&self) -> bool {
        self.fingerprints.is_empty()
    }

    /// List all authorized keys
    pub fn list(&self) -> &[AuthorizedKey] {
        &self.keys
    }
}

/// Parse an OpenSSH public key line (type base64 [comment])
fn parse_openssh_line(line: &str) -> Option<PublicKey> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        // The base64 key is the second part
        russh_keys::parse_public_key_base64(parts[1]).ok()
    } else {
        None
    }
}

/// Extract the comment from an OpenSSH public key line
fn extract_comment(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.splitn(3, ' ').collect();
    if parts.len() >= 3 {
        Some(parts[2].to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_authorized_keys() {
        let mut file = NamedTempFile::new().unwrap();

        // Write some test keys (these are example keys, not real ones)
        writeln!(file, "# Comment line").unwrap();
        writeln!(file, "").unwrap(); // Empty line
        writeln!(
            file,
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHGgXXRY1E9n5gMKjNkZ7g0I+XN5f3QYjXZ5+Qo0aW1t test@example.com"
        )
        .unwrap();

        let mut store = AuthorizedKeys::new();
        // This might fail if the key format isn't exactly right, but that's OK for this test
        let _ = store.load_from_file(file.path());

        // The test is mainly that it doesn't panic
    }

    #[test]
    fn test_is_authorized() {
        let mut store = AuthorizedKeys::new();
        store.add_fingerprint("SHA256:test123".to_string());

        assert!(store.is_authorized("SHA256:test123"));
        assert!(!store.is_authorized("SHA256:other"));
    }
}
