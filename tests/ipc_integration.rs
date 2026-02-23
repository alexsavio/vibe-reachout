use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use uuid::Uuid;
use vibe_reachout::ipc::client::send_request;
use vibe_reachout::models::{Decision, IpcRequest, IpcResponse};

fn make_request() -> IpcRequest {
    IpcRequest {
        request_id: Uuid::new_v4(),
        tool_name: "Bash".to_string(),
        tool_input: serde_json::json!({"command": "echo hello"}),
        cwd: "/home/user".to_string(),
        session_id: "test-session".to_string(),
        permission_suggestions: vec![],
    }
}

/// Spin up a mock server that reads one NDJSON request and writes back an `IpcResponse`.
fn mock_server(socket_path: &Path) -> tokio::task::JoinHandle<IpcRequest> {
    let listener = UnixListener::bind(socket_path).unwrap();

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let (reader, mut writer) = stream.into_split();

        let mut buf_reader = BufReader::new(reader);
        let mut line = String::new();
        buf_reader.read_line(&mut line).await.unwrap();

        let request: IpcRequest = serde_json::from_str(line.trim()).unwrap();

        let response = IpcResponse {
            request_id: request.request_id,
            decision: Decision::Allow,
            message: None,
            user_message: Some("approved".to_string()),
            always_allow_suggestion: None,
        };

        let mut json = serde_json::to_string(&response).unwrap();
        json.push('\n');
        writer.write_all(json.as_bytes()).await.unwrap();

        request
    })
}

#[tokio::test]
async fn client_server_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let socket_path = tmp.path().join("test.sock");

    let server_handle = mock_server(&socket_path);

    let request = make_request();
    let request_id = request.request_id;

    let response = send_request(&socket_path, &request, 5).await.unwrap();

    assert_eq!(response.request_id, request_id);
    assert_eq!(response.decision, Decision::Allow);
    assert_eq!(response.user_message.as_deref(), Some("approved"));

    // Verify server received the correct request
    let received = server_handle.await.unwrap();
    assert_eq!(received.request_id, request_id);
    assert_eq!(received.tool_name, "Bash");
}

#[tokio::test]
async fn client_returns_error_when_socket_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let socket_path = tmp.path().join("nonexistent.sock");

    let request = make_request();
    let result = send_request(&socket_path, &request, 5).await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("socket not found") || err_msg.contains("Socket not found"));
}

#[tokio::test]
async fn client_times_out_when_server_does_not_respond() {
    let tmp = tempfile::tempdir().unwrap();
    let socket_path = tmp.path().join("timeout.sock");

    // Server that accepts but never responds
    let listener = UnixListener::bind(&socket_path).unwrap();
    let _server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        // Hold the connection open but never write a response
        let (_reader, _writer) = stream.into_split();
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    });

    let request = make_request();
    let result = send_request(&socket_path, &request, 1).await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("timed out"));
}
