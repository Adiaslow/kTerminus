//! End-to-end system tests
//!
//! These tests run the actual orchestrator and agent to verify
//! the full system works correctly.
//!
//! **These tests are ignored by default** because they require:
//! - ssh-keygen available in PATH
//! - Available network ports
//! - No other orchestrator running
//!
//! Run with: `cargo test --test e2e_test -- --ignored`

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

/// Base port for test servers - use large gaps to avoid conflicts in parallel tests
static PORT_COUNTER: AtomicU16 = AtomicU16::new(0);

fn get_test_ports() -> (u16, u16) {
    let offset = PORT_COUNTER.fetch_add(1, Ordering::SeqCst) * 100;
    let ssh_port = 24000 + offset;
    let ipc_port = 24001 + offset;
    (ssh_port, ipc_port)
}

struct TestConfig {
    #[allow(dead_code)] // Keeps temp dir alive
    dir: tempfile::TempDir,
    path: std::path::PathBuf,
    ssh_port: u16,
    #[allow(dead_code)] // Stored for potential future use
    ipc_port: u16,
}

impl TestConfig {
    fn new(ssh_port: u16, ipc_port: u16) -> Self {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let config_path = dir.path().join("config.toml");
        let host_key_path = dir.path().join("host_key");

        let config = format!(
            r#"
[orchestrator]
bind_address = "127.0.0.1:{}"
ipc_port = {}
host_key_path = "{}"
heartbeat_interval = 5
heartbeat_timeout = 15
"#,
            ssh_port,
            ipc_port,
            host_key_path.display()
        );

        eprintln!("Test config:\n{}", config);
        std::fs::write(&config_path, config).expect("Failed to write config");

        Self {
            dir,
            path: config_path,
            ssh_port,
            ipc_port,
        }
    }
}

struct TestOrchestrator {
    process: Child,
    ipc_port: u16,
    #[allow(dead_code)]
    config: TestConfig,
}

impl TestOrchestrator {
    fn start() -> Self {
        let (ssh_port, ipc_port) = get_test_ports();
        let config = TestConfig::new(ssh_port, ipc_port);

        // Start orchestrator in foreground mode with test config
        let process = Command::new(env!("CARGO_BIN_EXE_k-terminus"))
            .args([
                "--config",
                config.path.to_str().unwrap(),
                "serve",
                "--foreground",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start orchestrator");

        let orchestrator = Self {
            process,
            ipc_port,
            config,
        };

        // Wait for orchestrator to be ready
        std::thread::sleep(Duration::from_millis(500));

        orchestrator
    }

    fn is_running(&mut self) -> bool {
        match self.process.try_wait() {
            Ok(None) => true,     // Still running
            Ok(Some(_)) => false, // Exited
            Err(_) => false,
        }
    }
}

impl Drop for TestOrchestrator {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

struct TestAgent {
    pub process: Child,
}

impl TestAgent {
    fn start(orchestrator_addr: &str) -> Self {
        // Use the default agent key from ~/.config/k-terminus/
        let home_key = dirs::home_dir()
            .unwrap()
            .join(".config/k-terminus/agent_key");

        eprintln!("Starting agent connecting to {}", orchestrator_addr);
        eprintln!("Using key: {:?} (exists: {})", home_key, home_key.exists());

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_k-terminus"));
        cmd.args(["join", orchestrator_addr, "--foreground"]);

        if home_key.exists() {
            cmd.args(["--key", home_key.to_str().unwrap()]);
        }

        let process = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start agent");

        let agent = Self { process };

        // Wait for agent to connect
        std::thread::sleep(Duration::from_secs(2));

        agent
    }
}

impl Drop for TestAgent {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

/// Send an IPC request and get response
fn ipc_request(port: u16, request: &str) -> Result<String, std::io::Error> {
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    // Send request
    writeln!(stream, "{}", request)?;
    stream.flush()?;

    // Read response
    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader.read_line(&mut response)?;

    Ok(response)
}

/// Wait for IPC server to be ready
fn wait_for_ipc(port: u16, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[test]
#[ignore] // Requires full environment - run with: cargo test -- --ignored
fn test_e2e_orchestrator_starts_and_responds_to_ping() {
    let mut orchestrator = TestOrchestrator::start();

    // Verify orchestrator is running
    assert!(orchestrator.is_running(), "Orchestrator should be running");

    // Wait for IPC to be ready
    if !wait_for_ipc(orchestrator.ipc_port, Duration::from_secs(5)) {
        panic!("IPC server did not start within timeout");
    }

    // Send ping via IPC
    let response =
        ipc_request(orchestrator.ipc_port, r#"{"type":"ping"}"#).expect("Failed to send ping");

    assert!(
        response.contains("pong"),
        "Expected pong response, got: {}",
        response
    );
}

#[test]
#[ignore]
fn test_e2e_orchestrator_status() {
    let mut orchestrator = TestOrchestrator::start();
    assert!(orchestrator.is_running(), "Orchestrator should be running");

    if !wait_for_ipc(orchestrator.ipc_port, Duration::from_secs(5)) {
        panic!("IPC server did not start within timeout");
    }

    let response = ipc_request(orchestrator.ipc_port, r#"{"type":"get_status"}"#)
        .expect("Failed to get status");

    assert!(
        response.contains("status"),
        "Expected status response, got: {}",
        response
    );
    assert!(
        response.contains("\"running\":true"),
        "Status should show running"
    );
}

#[test]
#[ignore]
fn test_e2e_orchestrator_list_machines_empty() {
    let mut orchestrator = TestOrchestrator::start();
    assert!(orchestrator.is_running(), "Orchestrator should be running");

    if !wait_for_ipc(orchestrator.ipc_port, Duration::from_secs(5)) {
        panic!("IPC server did not start within timeout");
    }

    let response = ipc_request(orchestrator.ipc_port, r#"{"type":"list_machines"}"#)
        .expect("Failed to list machines");

    assert!(
        response.contains("machines"),
        "Expected machines response, got: {}",
        response
    );
    // Should be empty list
    assert!(
        response.contains("\"machines\":[]"),
        "Expected empty machines list, got: {}",
        response
    );
}

#[test]
#[ignore]
fn test_e2e_orchestrator_list_sessions_empty() {
    let mut orchestrator = TestOrchestrator::start();
    assert!(orchestrator.is_running(), "Orchestrator should be running");

    if !wait_for_ipc(orchestrator.ipc_port, Duration::from_secs(5)) {
        panic!("IPC server did not start within timeout");
    }

    let response = ipc_request(
        orchestrator.ipc_port,
        r#"{"type":"list_sessions","machine_id":null}"#,
    )
    .expect("Failed to list sessions");

    assert!(
        response.contains("sessions"),
        "Expected sessions response, got: {}",
        response
    );
    assert!(
        response.contains("\"sessions\":[]"),
        "Expected empty sessions list, got: {}",
        response
    );
}

#[test]
#[ignore]
fn test_e2e_orchestrator_shutdown() {
    let mut orchestrator = TestOrchestrator::start();
    assert!(orchestrator.is_running(), "Orchestrator should be running");

    if !wait_for_ipc(orchestrator.ipc_port, Duration::from_secs(5)) {
        panic!("IPC server did not start within timeout");
    }

    let response = ipc_request(orchestrator.ipc_port, r#"{"type":"shutdown"}"#)
        .expect("Failed to send shutdown");

    assert!(
        response.contains("\"type\":\"ok\""),
        "Expected ok response, got: {}",
        response
    );

    // Wait for process to exit (may take a moment to clean up)
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(100));
        if !orchestrator.is_running() {
            return; // Success - orchestrator stopped
        }
    }
    panic!("Orchestrator should have stopped after shutdown (waited 2s)");
}

#[test]
#[ignore]
fn test_e2e_agent_connects_to_orchestrator() {
    let mut orchestrator = TestOrchestrator::start();
    assert!(orchestrator.is_running(), "Orchestrator should be running");

    if !wait_for_ipc(orchestrator.ipc_port, Duration::from_secs(5)) {
        panic!("IPC server did not start within timeout");
    }

    eprintln!(
        "Orchestrator started on SSH port {} and IPC port {}",
        orchestrator.config.ssh_port, orchestrator.ipc_port
    );

    // Start agent connecting to orchestrator's SSH port
    let mut agent = TestAgent::start(&format!("127.0.0.1:{}", orchestrator.config.ssh_port));

    // Give agent more time to connect
    std::thread::sleep(Duration::from_secs(5));

    // Check if machine appears in list
    let response = ipc_request(orchestrator.ipc_port, r#"{"type":"list_machines"}"#)
        .expect("Failed to list machines");

    eprintln!("Machines response: {}", response);

    // If agent connected successfully, we should see a machine
    if !response.contains("\"id\":") {
        // Agent failed to connect - check if process exited with error
        match agent.process.try_wait() {
            Ok(Some(status)) => {
                panic!(
                    "Agent exited with status {}, response: {}",
                    status, response
                );
            }
            Ok(None) => {
                panic!("Agent running but not connected, response: {}", response);
            }
            Err(e) => {
                panic!(
                    "Failed to check agent status: {}, response: {}",
                    e, response
                );
            }
        }
    }
}

#[test]
#[ignore]
fn test_e2e_full_session_flow() {
    let mut orchestrator = TestOrchestrator::start();
    assert!(orchestrator.is_running(), "Orchestrator should be running");

    if !wait_for_ipc(orchestrator.ipc_port, Duration::from_secs(5)) {
        panic!("IPC server did not start within timeout");
    }

    let _agent = TestAgent::start(&format!("127.0.0.1:{}", orchestrator.config.ssh_port));
    std::thread::sleep(Duration::from_secs(3));

    // List machines to verify agent connected
    let machines_resp = ipc_request(orchestrator.ipc_port, r#"{"type":"list_machines"}"#)
        .expect("Failed to list machines");
    println!("Machines: {}", machines_resp);

    assert!(
        machines_resp.contains("\"id\":"),
        "Agent should have connected"
    );

    // Parse the machine ID from the response
    let machines_json: serde_json::Value =
        serde_json::from_str(&machines_resp).expect("Failed to parse machines response");
    let machine_id = machines_json["machines"][0]["id"]
        .as_str()
        .expect("Failed to get machine ID");
    println!("Machine ID: {}", machine_id);

    // Create a session on the machine
    let create_req = format!(
        r#"{{"type":"create_session","machine_id":"{}","shell":null}}"#,
        machine_id
    );
    let create_resp =
        ipc_request(orchestrator.ipc_port, &create_req).expect("Failed to create session");
    println!("Create session response: {}", create_resp);

    assert!(
        create_resp.contains("\"type\":\"session_created\""),
        "Session should be created, got: {}",
        create_resp
    );

    // Parse session ID
    let create_json: serde_json::Value =
        serde_json::from_str(&create_resp).expect("Failed to parse create session response");
    let session_id = create_json["id"]
        .as_str()
        .expect("Failed to get session ID");
    println!("Session ID: {}", session_id);

    // List sessions to verify it exists
    let sessions_resp = ipc_request(orchestrator.ipc_port, r#"{"type":"list_sessions"}"#)
        .expect("Failed to list sessions");
    println!("Sessions: {}", sessions_resp);

    assert!(
        sessions_resp.contains(session_id),
        "Session should be in list, got: {}",
        sessions_resp
    );

    // Close the session
    let close_req = format!(
        r#"{{"type":"close_session","session_id":"{}","force":false}}"#,
        session_id
    );
    let close_resp =
        ipc_request(orchestrator.ipc_port, &close_req).expect("Failed to close session");
    println!("Close session response: {}", close_resp);

    assert!(
        close_resp.contains("\"type\":\"ok\"")
            || close_resp.contains("\"type\":\"session_closed\""),
        "Session close should succeed, got: {}",
        close_resp
    );

    // Give time for session to close
    std::thread::sleep(Duration::from_millis(500));

    // Verify session is gone
    let sessions_after = ipc_request(orchestrator.ipc_port, r#"{"type":"list_sessions"}"#)
        .expect("Failed to list sessions after close");
    println!("Sessions after close: {}", sessions_after);

    assert!(
        !sessions_after.contains(session_id),
        "Session should be removed after close, got: {}",
        sessions_after
    );
}
