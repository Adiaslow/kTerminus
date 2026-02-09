//! End-to-end system tests
//!
//! These tests run the actual orchestrator and agent to verify
//! the full system works correctly.
//!
//! These tests require:
//! - ssh-keygen available in PATH
//! - Available network ports
//! - No other orchestrator running on the test ports
//!
//! Tests that only use the orchestrator run in CI.
//! Tests that require Tailscale (agent connection) are marked with #[ignore].

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::PathBuf;
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
                config
                    .path
                    .to_str()
                    .expect("Config path contains invalid UTF-8 characters"),
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
            .expect("Could not determine home directory - check HOME environment variable")
            .join(".config/k-terminus/agent_key");

        eprintln!("Starting agent connecting to {}", orchestrator_addr);
        eprintln!("Using key: {:?} (exists: {})", home_key, home_key.exists());

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_k-terminus"));
        cmd.args(["join", orchestrator_addr, "--foreground"]);

        if home_key.exists() {
            cmd.args([
                "--key",
                home_key
                    .to_str()
                    .expect("Home key path contains invalid UTF-8 characters"),
            ]);
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

/// Read the IPC authentication token from the default location
fn read_auth_token() -> Result<String, std::io::Error> {
    let token_path = get_default_token_path()?;
    std::fs::read_to_string(token_path).map(|s| s.trim().to_string())
}

/// Get the default token path (matches kt_core::ipc_auth::default_token_path)
fn get_default_token_path() -> Result<PathBuf, std::io::Error> {
    let config_dir = dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "Could not find config directory"))?;
    Ok(config_dir.join("k-terminus").join("ipc_auth_token"))
}

/// Send an IPC request and get response (without authentication)
fn ipc_request_raw(port: u16, request: &str) -> Result<String, std::io::Error> {
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

/// Send an IPC request with authentication
fn ipc_request(port: u16, request: &str) -> Result<String, std::io::Error> {
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let mut reader = BufReader::new(stream.try_clone()?);

    // First, authenticate
    let token = read_auth_token()?;
    let auth_request = format!(r#"{{"type":"authenticate","token":"{}"}}"#, token);
    writeln!(stream, "{}", auth_request)?;
    stream.flush()?;

    let mut auth_response = String::new();
    reader.read_line(&mut auth_response)?;

    if !auth_response.contains("authenticated") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("Authentication failed: {}", auth_response),
        ));
    }

    // Now send the actual request
    writeln!(stream, "{}", request)?;
    stream.flush()?;

    let mut response = String::new();
    reader.read_line(&mut response)?;

    Ok(response)
}

/// Persistent IPC client that maintains a connection across multiple requests.
/// Sessions are tied to the owning client, so we need to keep the connection open
/// for the full session lifecycle.
struct PersistentIpcClient {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
}

/// Response types that indicate an actual response (not an event)
const RESPONSE_TYPES: &[&str] = &[
    "\"type\":\"pong\"",
    "\"type\":\"authenticated\"",
    "\"type\":\"authentication_required\"",
    "\"type\":\"status\"",
    "\"type\":\"machines\"",
    "\"type\":\"machine\"",
    "\"type\":\"sessions\"",
    "\"type\":\"session_created\"",  // Note: also used as event, but response has specific format
    "\"type\":\"ok\"",
    "\"type\":\"error\"",
    "\"type\":\"pairing_code\"",
    "\"type\":\"pairing_code_valid\"",
];

impl PersistentIpcClient {
    fn connect(port: u16) -> Result<Self, std::io::Error> {
        let stream = TcpStream::connect(format!("127.0.0.1:{}", port))?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;

        let reader = BufReader::new(stream.try_clone()?);

        let mut client = Self { stream, reader };

        // Authenticate
        let token = read_auth_token()?;
        let auth_request = format!(r#"{{"type":"authenticate","token":"{}"}}"#, token);
        let auth_response = client.send_raw(&auth_request)?;

        if !auth_response.contains("authenticated") {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("Authentication failed: {}", auth_response),
            ));
        }

        Ok(client)
    }

    fn send(&mut self, request: &str) -> Result<String, std::io::Error> {
        writeln!(self.stream, "{}", request)?;
        self.stream.flush()?;

        // Read responses, skipping any broadcast events
        // Events can arrive at any time (e.g., session_started with PID)
        // We need to skip them and return the actual response to our request
        loop {
            let mut response = String::new();
            self.reader.read_line(&mut response)?;

            // Check if this is an actual response by looking for response type markers
            // Events like "session_started", "machine_connected" etc. should be skipped
            let is_response = RESPONSE_TYPES.iter().any(|t| response.contains(t));

            // Special case: session_created can be both a response AND an event
            // The response has "createdAt" field, event version is for broadcasting
            // We need to check if this is the response to our create_session request
            if response.contains("\"type\":\"session_created\"") {
                // If we just sent create_session, this is our response
                if request.contains("create_session") {
                    return Ok(response);
                }
                // Otherwise it's a broadcast event, skip it
                eprintln!("Skipping broadcast event: {}", response.trim());
                continue;
            }

            if is_response {
                return Ok(response);
            }

            // Skip this event and try again
            eprintln!("Skipping broadcast event: {}", response.trim());
        }
    }

    fn send_raw(&mut self, request: &str) -> Result<String, std::io::Error> {
        writeln!(self.stream, "{}", request)?;
        self.stream.flush()?;

        let mut response = String::new();
        self.reader.read_line(&mut response)?;
        Ok(response)
    }
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
fn test_e2e_orchestrator_starts_and_responds_to_ping() {
    let mut orchestrator = TestOrchestrator::start();

    // Verify orchestrator is running
    assert!(orchestrator.is_running(), "Orchestrator should be running");

    // Wait for IPC to be ready
    if !wait_for_ipc(orchestrator.ipc_port, Duration::from_secs(5)) {
        panic!("IPC server did not start within timeout");
    }

    // Send ping via IPC (ping doesn't require authentication)
    let response =
        ipc_request_raw(orchestrator.ipc_port, r#"{"type":"ping"}"#).expect("Failed to send ping");

    assert!(
        response.contains("pong"),
        "Expected pong response, got: {}",
        response
    );
}

#[test]
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
fn test_e2e_agent_connects_to_orchestrator() {
    // This test connects via localhost (127.0.0.1), which bypasses Tailscale verification.
    // The orchestrator accepts loopback connections without checking Tailscale membership.
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
fn test_e2e_full_session_flow() {
    // This test connects via localhost (127.0.0.1), which bypasses Tailscale verification.
    // The orchestrator accepts loopback connections without checking Tailscale membership.
    let mut orchestrator = TestOrchestrator::start();
    assert!(orchestrator.is_running(), "Orchestrator should be running");

    if !wait_for_ipc(orchestrator.ipc_port, Duration::from_secs(5)) {
        panic!("IPC server did not start within timeout");
    }

    let _agent = TestAgent::start(&format!("127.0.0.1:{}", orchestrator.config.ssh_port));
    std::thread::sleep(Duration::from_secs(3));

    // Use a persistent client for the full session flow.
    // Sessions are tied to their owning client and cleaned up when the client disconnects,
    // so we need to maintain the same connection throughout the test.
    let mut client = PersistentIpcClient::connect(orchestrator.ipc_port)
        .expect("Failed to connect persistent client");

    // List machines to verify agent connected
    let machines_resp = client
        .send(r#"{"type":"list_machines"}"#)
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
    let create_resp = client.send(&create_req).expect("Failed to create session");
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
    let sessions_resp = client
        .send(r#"{"type":"list_sessions"}"#)
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
    let close_resp = client.send(&close_req).expect("Failed to close session");
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
    let sessions_after = client
        .send(r#"{"type":"list_sessions"}"#)
        .expect("Failed to list sessions after close");
    println!("Sessions after close: {}", sessions_after);

    assert!(
        !sessions_after.contains(session_id),
        "Session should be removed after close, got: {}",
        sessions_after
    );
}
