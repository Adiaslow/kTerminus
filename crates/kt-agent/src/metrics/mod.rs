//! System metrics collection

/// System metrics for a machine
#[derive(Debug, Clone)]
pub struct SystemMetrics {
    /// CPU usage percentage (0-100)
    pub cpu_percent: f32,
    /// Memory usage percentage (0-100)
    pub memory_percent: f32,
    /// Available disk space in bytes
    pub disk_available: u64,
    /// System load average (1 minute)
    pub load_avg_1m: f32,
}

impl SystemMetrics {
    /// Collect current system metrics
    pub fn collect() -> Self {
        // TODO: Actually collect metrics using sysinfo or similar
        Self {
            cpu_percent: 0.0,
            memory_percent: 0.0,
            disk_available: 0,
            load_avg_1m: 0.0,
        }
    }
}
