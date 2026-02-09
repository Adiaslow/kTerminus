//! Global orchestrator state

use std::sync::Arc;

use kt_core::config::OrchestratorConfig;
use kt_core::ipc::StateEpoch;
use rand::Rng;

use crate::auth::TailscaleVerifier;
use crate::coordinator::StateCoordinator;

/// Pairing code length.
///
/// 8 characters from a 32-character alphabet provides ~40 bits of entropy
/// (32^8 = ~1.1 trillion combinations). This is sufficient for a discovery
/// mechanism that:
/// - Is only used for initial pairing (not ongoing authentication)
/// - Expires after the orchestrator restarts
/// - Is rate-limited by the IPC server
///
/// The previous 6-character code (~2.67M combinations) was increased to provide
/// a better security margin against brute-force discovery attempts.
const PAIRING_CODE_LENGTH: usize = 8;

/// Generate a random pairing code
fn generate_pairing_code() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789"; // No I, O, 0, 1 to avoid confusion
    let mut rng = rand::thread_rng();
    (0..PAIRING_CODE_LENGTH)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Global state for the orchestrator daemon
pub struct OrchestratorState {
    /// Configuration
    pub config: OrchestratorConfig,
    /// State coordinator for centralized connection/session management
    pub coordinator: Arc<StateCoordinator>,
    /// Tailscale peer verifier
    pub tailscale: Arc<TailscaleVerifier>,
    /// Pairing code for easy agent connection
    pub pairing_code: String,
    /// Global state epoch for event sequencing
    pub epoch: Arc<StateEpoch>,
}

impl OrchestratorState {
    /// Create new orchestrator state
    pub fn new(config: OrchestratorConfig) -> Self {
        let pairing_code = generate_pairing_code();
        // Log at debug level to avoid exposing pairing code in production logs
        // The code is displayed in the desktop app UI for secure access
        tracing::debug!("Generated pairing code (view in desktop app)");

        let coordinator = Arc::new(StateCoordinator::new());

        Self {
            config,
            coordinator,
            tailscale: Arc::new(TailscaleVerifier::new()),
            pairing_code,
            epoch: Arc::new(StateEpoch::new()),
        }
    }

    /// Get the pairing code
    pub fn pairing_code(&self) -> &str {
        &self.pairing_code
    }

    /// Verify a pairing code matches
    pub fn verify_pairing_code(&self, code: &str) -> bool {
        self.pairing_code.eq_ignore_ascii_case(code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pairing_code_length() {
        let code = generate_pairing_code();
        assert_eq!(
            code.len(),
            PAIRING_CODE_LENGTH,
            "Pairing code should be {} characters",
            PAIRING_CODE_LENGTH
        );
    }

    #[test]
    fn test_pairing_code_charset() {
        // Generate multiple codes and verify they only contain valid characters
        for _ in 0..100 {
            let code = generate_pairing_code();
            for c in code.chars() {
                assert!(
                    c.is_ascii_uppercase() || c.is_ascii_digit(),
                    "Character '{}' should be uppercase letter or digit",
                    c
                );
                // Should not contain confusing characters
                assert!(
                    c != 'I' && c != 'O' && c != '0' && c != '1',
                    "Should not contain confusing character '{}'",
                    c
                );
            }
        }
    }

    #[test]
    fn test_pairing_code_uniqueness() {
        // Generate many codes and verify reasonable uniqueness
        let mut codes = std::collections::HashSet::new();
        for _ in 0..1000 {
            let code = generate_pairing_code();
            codes.insert(code);
        }
        // With 8 chars from 32-char alphabet, collision is extremely unlikely
        assert!(
            codes.len() >= 990,
            "Should have at least 990 unique codes out of 1000 (got {})",
            codes.len()
        );
    }

    #[test]
    fn test_pairing_code_entropy_constant() {
        // Verify the constant is set to 8 for sufficient entropy
        assert_eq!(
            PAIRING_CODE_LENGTH, 8,
            "Pairing code should be 8 characters for ~40 bits of entropy"
        );
    }
}
