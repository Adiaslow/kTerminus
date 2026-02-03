//! Tailscale-based peer verification
//!
//! Verifies that connecting clients are members of the same Tailscale network.

use std::net::IpAddr;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use kt_core::tailscale::{self, TailscalePeer};

/// Cache duration for peer list (avoids calling `tailscale status` on every connection)
const CACHE_DURATION: Duration = Duration::from_secs(30);

/// Verifies that connecting peers are members of the same Tailscale network
pub struct TailscaleVerifier {
    /// Cached peer list
    cache: RwLock<PeerCache>,
}

struct PeerCache {
    peers: Vec<TailscalePeer>,
    last_refresh: Instant,
}

impl TailscaleVerifier {
    /// Create a new Tailscale verifier
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(PeerCache {
                peers: Vec::new(),
                last_refresh: Instant::now() - CACHE_DURATION, // Force initial refresh
            }),
        }
    }

    /// Verify that an IP address belongs to a peer in our tailnet
    ///
    /// Returns `Some(TailscalePeer)` if the IP is from a valid tailnet peer,
    /// `None` otherwise.
    pub fn verify_peer(&self, ip: IpAddr) -> Option<TailscalePeer> {
        // Check if we need to refresh the cache
        {
            let cache = self.cache.read().ok()?;
            if cache.last_refresh.elapsed() < CACHE_DURATION {
                // Use cached data
                let ip_str = ip.to_string();
                if let Some(peer) = cache.peers.iter().find(|p| p.ips.contains(&ip_str)) {
                    return Some(peer.clone());
                }
                return None;
            }
        }

        // Refresh cache
        self.refresh_cache();

        // Try again with fresh data
        let cache = self.cache.read().ok()?;
        let ip_str = ip.to_string();
        cache
            .peers
            .iter()
            .find(|p| p.ips.contains(&ip_str))
            .cloned()
    }

    /// Force refresh of the peer cache
    fn refresh_cache(&self) {
        match tailscale::get_tailscale_peers() {
            Ok(peers) => {
                if let Ok(mut cache) = self.cache.write() {
                    cache.peers = peers;
                    cache.last_refresh = Instant::now();
                    tracing::debug!(
                        "Refreshed Tailscale peer cache: {} peers",
                        cache.peers.len()
                    );
                }
            }
            Err(e) => {
                tracing::warn!("Failed to refresh Tailscale peer cache: {}", e);
            }
        }
    }

    /// Check if Tailscale is available and we're logged in
    pub fn is_available() -> bool {
        tailscale::get_tailscale_info()
            .ok()
            .flatten()
            .map(|info| info.logged_in)
            .unwrap_or(false)
    }
}

impl Default for TailscaleVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verifier_creation() {
        let verifier = TailscaleVerifier::new();
        // Just verify it can be created without panicking
        assert!(true);
        drop(verifier);
    }
}
