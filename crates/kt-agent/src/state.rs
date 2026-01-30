//! Agent state management

use kt_core::config::AgentConfig;

use crate::pty::PtyManager;

/// Global state for the agent daemon
pub struct AgentState {
    /// Configuration
    pub config: AgentConfig,
    /// PTY manager
    pub pty_manager: PtyManager,
}

impl AgentState {
    /// Create new agent state
    pub fn new(config: AgentConfig) -> Self {
        Self {
            config,
            pty_manager: PtyManager::new(),
        }
    }
}
