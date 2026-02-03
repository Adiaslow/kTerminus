//! IPC integration tests
//!
//! Tests the IPC server and client communication.

use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use kt_core::config::OrchestratorConfig;
use kt_core::ipc::{IpcRequest, IpcResponse};
use kt_orchestrator::ipc::IpcServer;
use kt_orchestrator::OrchestratorState;

/// Base port for test servers - each test gets a unique offset
static PORT_COUNTER: AtomicU16 = AtomicU16::new(0);

/// Get a unique port for this test
fn get_test_port() -> u16 {
    // Use a range of ports starting from 39000
    let offset = PORT_COUNTER.fetch_add(1, Ordering::SeqCst);
    39000 + offset
}

/// Create test state with default config
fn create_test_state() -> Arc<OrchestratorState> {
    let config = OrchestratorConfig::default();
    Arc::new(OrchestratorState::new(config))
}

/// IPC test client wrapper
struct TestClient {
    reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
    writer: BufWriter<tokio::net::tcp::OwnedWriteHalf>,
}

impl TestClient {
    async fn connect(address: &str) -> Self {
        // Retry connection a few times in case server isn't ready
        let mut last_err = None;
        for _ in 0..10 {
            match TcpStream::connect(address).await {
                Ok(stream) => {
                    let (reader, writer) = stream.into_split();
                    return Self {
                        reader: BufReader::new(reader),
                        writer: BufWriter::new(writer),
                    };
                }
                Err(e) => {
                    last_err = Some(e);
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
        }
        panic!(
            "Failed to connect to IPC server at {}: {:?}",
            address, last_err
        );
    }

    async fn send_request(&mut self, request: IpcRequest) -> IpcResponse {
        // Send request
        let mut request_json =
            serde_json::to_string(&request).expect("Failed to serialize request");
        request_json.push('\n');
        self.writer
            .write_all(request_json.as_bytes())
            .await
            .expect("Failed to write request");
        self.writer.flush().await.expect("Failed to flush");

        // Read response
        let mut response_line = String::new();
        self.reader
            .read_line(&mut response_line)
            .await
            .expect("Failed to read response");

        if response_line.is_empty() {
            panic!("Server sent empty response (connection closed?)");
        }

        serde_json::from_str(&response_line).expect("Failed to parse response")
    }
}

#[tokio::test]
async fn test_ipc_ping_pong() {
    let port = get_test_port();
    let address = format!("127.0.0.1:{}", port);
    let state = create_test_state();

    let server = Arc::new(IpcServer::new(address.clone(), state));
    let server_clone = Arc::clone(&server);

    // Start server in background
    let server_handle = tokio::spawn(async move {
        let _ = server_clone.run().await;
    });

    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Connect client
    let mut client = TestClient::connect(&address).await;

    // Send ping
    let response = client.send_request(IpcRequest::Ping).await;
    assert!(matches!(response, IpcResponse::Pong));

    // Clean up
    server_handle.abort();
}

#[tokio::test]
async fn test_ipc_get_status() {
    let port = get_test_port();
    let address = format!("127.0.0.1:{}", port);
    let state = create_test_state();

    let server = Arc::new(IpcServer::new(address.clone(), state));
    let server_clone = Arc::clone(&server);

    let server_handle = tokio::spawn(async move {
        let _ = server_clone.run().await;
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut client = TestClient::connect(&address).await;

    let response = client.send_request(IpcRequest::GetStatus).await;

    match response {
        IpcResponse::Status(status) => {
            assert!(status.running);
            assert_eq!(status.machine_count, 0);
            assert_eq!(status.session_count, 0);
        }
        other => panic!("Expected Status response, got {:?}", other),
    }

    server_handle.abort();
}

#[tokio::test]
async fn test_ipc_list_machines_empty() {
    let port = get_test_port();
    let address = format!("127.0.0.1:{}", port);
    let state = create_test_state();

    let server = Arc::new(IpcServer::new(address.clone(), state));
    let server_clone = Arc::clone(&server);

    let server_handle = tokio::spawn(async move {
        let _ = server_clone.run().await;
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut client = TestClient::connect(&address).await;

    let response = client.send_request(IpcRequest::ListMachines).await;

    match response {
        IpcResponse::Machines { machines } => {
            assert!(machines.is_empty());
        }
        other => panic!("Expected Machines response, got {:?}", other),
    }

    server_handle.abort();
}

#[tokio::test]
async fn test_ipc_list_sessions_empty() {
    let port = get_test_port();
    let address = format!("127.0.0.1:{}", port);
    let state = create_test_state();

    let server = Arc::new(IpcServer::new(address.clone(), state));
    let server_clone = Arc::clone(&server);

    let server_handle = tokio::spawn(async move {
        let _ = server_clone.run().await;
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut client = TestClient::connect(&address).await;

    let response = client
        .send_request(IpcRequest::ListSessions { machine_id: None })
        .await;

    match response {
        IpcResponse::Sessions { sessions } => {
            assert!(sessions.is_empty());
        }
        other => panic!("Expected Sessions response, got {:?}", other),
    }

    server_handle.abort();
}

#[tokio::test]
async fn test_ipc_get_machine_not_found() {
    let port = get_test_port();
    let address = format!("127.0.0.1:{}", port);
    let state = create_test_state();

    let server = Arc::new(IpcServer::new(address.clone(), state));
    let server_clone = Arc::clone(&server);

    let server_handle = tokio::spawn(async move {
        let _ = server_clone.run().await;
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut client = TestClient::connect(&address).await;

    let response = client
        .send_request(IpcRequest::GetMachine {
            machine_id: "nonexistent".to_string(),
        })
        .await;

    match response {
        IpcResponse::Error { message } => {
            assert!(message.contains("not found"));
        }
        other => panic!("Expected Error response, got {:?}", other),
    }

    server_handle.abort();
}

#[tokio::test]
async fn test_ipc_create_session_machine_not_found() {
    let port = get_test_port();
    let address = format!("127.0.0.1:{}", port);
    let state = create_test_state();

    let server = Arc::new(IpcServer::new(address.clone(), state));
    let server_clone = Arc::clone(&server);

    let server_handle = tokio::spawn(async move {
        let _ = server_clone.run().await;
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut client = TestClient::connect(&address).await;

    let response = client
        .send_request(IpcRequest::CreateSession {
            machine_id: "nonexistent".to_string(),
            shell: None,
        })
        .await;

    match response {
        IpcResponse::Error { message } => {
            assert!(message.contains("not found"));
        }
        other => panic!("Expected Error response, got {:?}", other),
    }

    server_handle.abort();
}

#[tokio::test]
async fn test_ipc_shutdown() {
    let port = get_test_port();
    let address = format!("127.0.0.1:{}", port);
    let state = create_test_state();
    let cancel = CancellationToken::new();

    let server =
        Arc::new(IpcServer::new(address.clone(), state).with_shutdown_token(cancel.clone()));
    let server_clone = Arc::clone(&server);

    let server_handle = tokio::spawn(async move {
        let _ = server_clone.run().await;
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut client = TestClient::connect(&address).await;

    let response = client.send_request(IpcRequest::Shutdown).await;
    assert!(matches!(response, IpcResponse::Ok));

    // Verify cancellation token was triggered
    assert!(cancel.is_cancelled());

    server_handle.abort();
}

#[tokio::test]
async fn test_ipc_subscribe_unsubscribe() {
    let port = get_test_port();
    let address = format!("127.0.0.1:{}", port);
    let state = create_test_state();

    let server = Arc::new(IpcServer::new(address.clone(), state));
    let server_clone = Arc::clone(&server);

    let server_handle = tokio::spawn(async move {
        let _ = server_clone.run().await;
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut client = TestClient::connect(&address).await;

    // Subscribe
    let response = client
        .send_request(IpcRequest::Subscribe {
            session_id: "session-1".to_string(),
        })
        .await;
    assert!(matches!(response, IpcResponse::Ok));

    // Unsubscribe
    let response = client
        .send_request(IpcRequest::Unsubscribe {
            session_id: "session-1".to_string(),
        })
        .await;
    assert!(matches!(response, IpcResponse::Ok));

    server_handle.abort();
}

#[tokio::test]
async fn test_ipc_multiple_requests() {
    let port = get_test_port();
    let address = format!("127.0.0.1:{}", port);
    let state = create_test_state();

    let server = Arc::new(IpcServer::new(address.clone(), state));
    let server_clone = Arc::clone(&server);

    let server_handle = tokio::spawn(async move {
        let _ = server_clone.run().await;
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut client = TestClient::connect(&address).await;

    // Send multiple requests on same connection
    for _ in 0..5 {
        let response = client.send_request(IpcRequest::Ping).await;
        assert!(matches!(response, IpcResponse::Pong));
    }

    // Mixed requests
    let response = client.send_request(IpcRequest::GetStatus).await;
    assert!(matches!(response, IpcResponse::Status(_)));

    let response = client.send_request(IpcRequest::ListMachines).await;
    assert!(matches!(response, IpcResponse::Machines { .. }));

    let response = client.send_request(IpcRequest::Ping).await;
    assert!(matches!(response, IpcResponse::Pong));

    server_handle.abort();
}

#[tokio::test]
async fn test_ipc_concurrent_clients() {
    let port = get_test_port();
    let address = format!("127.0.0.1:{}", port);
    let state = create_test_state();

    let server = Arc::new(IpcServer::new(address.clone(), state));
    let server_clone = Arc::clone(&server);

    let server_handle = tokio::spawn(async move {
        let _ = server_clone.run().await;
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Spawn multiple concurrent clients
    let mut handles = vec![];
    for i in 0..5 {
        let addr = address.clone();
        handles.push(tokio::spawn(async move {
            let mut client = TestClient::connect(&addr).await;

            // Each client sends multiple pings
            for _ in 0..3 {
                let response = client.send_request(IpcRequest::Ping).await;
                assert!(
                    matches!(response, IpcResponse::Pong),
                    "Client {} expected Pong",
                    i
                );
            }
        }));
    }

    // Wait for all clients to complete
    let result = timeout(Duration::from_secs(5), async {
        for handle in handles {
            handle.await.expect("Client task failed");
        }
    })
    .await;

    assert!(result.is_ok(), "Concurrent client test timed out");

    server_handle.abort();
}
