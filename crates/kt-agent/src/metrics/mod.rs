//! System metrics collection

use sysinfo::System;

/// System metrics for a machine
#[derive(Debug, Clone)]
pub struct SystemMetrics {
    /// CPU usage percentage (0-100)
    pub cpu_percent: f32,
    /// Memory usage percentage (0-100)
    pub memory_percent: f32,
    /// Total memory in bytes
    pub memory_total: u64,
    /// Used memory in bytes
    pub memory_used: u64,
    /// Available disk space in bytes
    pub disk_available: u64,
    /// Total disk space in bytes
    pub disk_total: u64,
    /// System load average (1 minute) - Unix only
    pub load_avg_1m: f32,
}

impl SystemMetrics {
    /// Collect current system metrics
    pub fn collect() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        // Calculate CPU usage (average across all cores)
        let cpu_percent = sys.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>()
            / sys.cpus().len().max(1) as f32;

        // Memory metrics
        let memory_total = sys.total_memory();
        let memory_used = sys.used_memory();
        let memory_percent = if memory_total > 0 {
            (memory_used as f32 / memory_total as f32) * 100.0
        } else {
            0.0
        };

        // Disk metrics (sum all disks)
        let disks = sysinfo::Disks::new_with_refreshed_list();
        let (disk_total, disk_available) =
            disks.iter().fold((0u64, 0u64), |(total, avail), disk| {
                (total + disk.total_space(), avail + disk.available_space())
            });

        // Load average (Unix only)
        let load_avg_1m = System::load_average().one as f32;

        Self {
            cpu_percent,
            memory_percent,
            memory_total,
            memory_used,
            disk_available,
            disk_total,
            load_avg_1m,
        }
    }

    /// Get a human-readable summary of the metrics
    pub fn summary(&self) -> String {
        format!(
            "CPU: {:.1}%, Memory: {:.1}% ({}/{}), Disk: {}/{} available",
            self.cpu_percent,
            self.memory_percent,
            human_bytes(self.memory_used),
            human_bytes(self.memory_total),
            human_bytes(self.disk_available),
            human_bytes(self.disk_total),
        )
    }
}

/// Convert bytes to human-readable format
fn human_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.1}TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1}GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}KB", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_metrics() {
        let metrics = SystemMetrics::collect();
        // Basic sanity checks
        assert!(metrics.cpu_percent >= 0.0 && metrics.cpu_percent <= 100.0);
        assert!(metrics.memory_percent >= 0.0 && metrics.memory_percent <= 100.0);
        assert!(metrics.memory_total > 0);
    }

    #[test]
    fn test_human_bytes() {
        assert_eq!(human_bytes(0), "0B");
        assert_eq!(human_bytes(512), "512B");
        assert_eq!(human_bytes(1024), "1.0KB");
        assert_eq!(human_bytes(1024 * 1024), "1.0MB");
        assert_eq!(human_bytes(1024 * 1024 * 1024), "1.0GB");
        assert_eq!(human_bytes(1024 * 1024 * 1024 * 1024), "1.0TB");
    }
}
