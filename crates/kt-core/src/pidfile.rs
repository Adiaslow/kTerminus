//! PID file utilities for single-instance management
//!
//! Provides utilities for managing a PID file to detect and prevent
//! multiple orchestrator instances from running simultaneously.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use crate::config;

/// Default PID file name
const PID_FILE_NAME: &str = "orchestrator.pid";

/// Get the default PID file path
pub fn default_pid_path() -> PathBuf {
    config::default_config_dir().join(PID_FILE_NAME)
}

/// Read the PID from the PID file
///
/// Returns `Ok(Some(pid))` if the file exists and contains a valid PID,
/// `Ok(None)` if the file doesn't exist, or an error if the file is malformed.
pub fn read_pid_file(path: &Path) -> io::Result<Option<u32>> {
    match fs::File::open(path) {
        Ok(mut file) => {
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            let pid = contents
                .trim()
                .parse::<u32>()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            Ok(Some(pid))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Write the current process PID to the PID file
///
/// Creates parent directories if they don't exist.
pub fn write_pid_file(path: &Path, pid: u32) -> io::Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = fs::File::create(path)?;
    writeln!(file, "{}", pid)?;
    Ok(())
}

/// Remove the PID file
///
/// Returns `Ok(())` even if the file doesn't exist.
pub fn remove_pid_file(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Check if a process with the given PID is still alive
///
/// On Unix, uses kill(pid, 0) to check if the process exists.
/// On Windows, uses OpenProcess to check if the process exists.
#[cfg(unix)]
pub fn is_process_alive(pid: u32) -> bool {
    // kill(pid, 0) returns 0 if the process exists and we have permission to signal it
    // ESRCH (No such process) indicates the process doesn't exist
    // EPERM (Operation not permitted) indicates the process exists but we can't signal it
    unsafe {
        let result = libc::kill(pid as libc::pid_t, 0);
        if result == 0 {
            return true;
        }
        // Check if error is EPERM (process exists but we can't signal it)
        // Use std::io::Error to get errno in a cross-platform way
        let err = std::io::Error::last_os_error();
        err.raw_os_error() == Some(libc::EPERM)
    }
}

#[cfg(windows)]
pub fn is_process_alive(pid: u32) -> bool {
    use std::ptr;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle == INVALID_HANDLE_VALUE || handle == ptr::null_mut() {
            return false;
        }
        CloseHandle(handle);
        true
    }
}

/// Guard that removes the PID file when dropped
///
/// Useful for ensuring the PID file is cleaned up even on panic.
pub struct PidFileGuard {
    path: PathBuf,
}

impl PidFileGuard {
    /// Create a new guard and write the PID file
    pub fn new(path: PathBuf, pid: u32) -> io::Result<Self> {
        write_pid_file(&path, pid)?;
        Ok(Self { path })
    }

    /// Create a guard for the default path
    pub fn default(pid: u32) -> io::Result<Self> {
        Self::new(default_pid_path(), pid)
    }
}

impl Drop for PidFileGuard {
    fn drop(&mut self) {
        if let Err(e) = remove_pid_file(&self.path) {
            tracing::warn!("Failed to remove PID file {:?}: {}", self.path, e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_read_nonexistent_pid_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.pid");
        assert!(read_pid_file(&path).unwrap().is_none());
    }

    #[test]
    fn test_write_and_read_pid_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.pid");

        write_pid_file(&path, 12345).unwrap();
        assert_eq!(read_pid_file(&path).unwrap(), Some(12345));
    }

    #[test]
    fn test_remove_pid_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.pid");

        write_pid_file(&path, 12345).unwrap();
        remove_pid_file(&path).unwrap();
        assert!(read_pid_file(&path).unwrap().is_none());
    }

    #[test]
    fn test_remove_nonexistent_pid_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.pid");
        // Should not error
        remove_pid_file(&path).unwrap();
    }

    #[test]
    fn test_current_process_is_alive() {
        let pid = std::process::id();
        assert!(is_process_alive(pid));
    }

    #[test]
    fn test_invalid_pid_not_alive() {
        // PID 0 is special (kernel), very high PIDs likely don't exist
        // Use a very high PID that's unlikely to be a real process
        assert!(!is_process_alive(999999999));
    }

    #[test]
    fn test_pid_file_guard() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("guard.pid");

        {
            let _guard = PidFileGuard::new(path.clone(), 12345).unwrap();
            assert!(path.exists());
        }

        // Guard dropped, file should be removed
        assert!(!path.exists());
    }
}
