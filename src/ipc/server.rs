use crate::error::BotError;
use crate::models::{IpcRequest, IpcResponse, PendingRequest};
use dashmap::DashMap;
use std::path::Path;
use std::sync::Arc;
use teloxide::prelude::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::{Semaphore, oneshot};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::config::Config;

pub type PendingMap = Arc<DashMap<Uuid, PendingRequest>>;

pub fn detect_and_clean_stale_socket(socket_path: &Path) -> Result<(), BotError> {
    if !socket_path.exists() {
        return Ok(());
    }

    // Try a synchronous connection to see if a bot is actively listening
    match std::os::unix::net::UnixStream::connect(socket_path) {
        Ok(_) => {
            // Connection succeeded — another bot is running
            Err(BotError::AlreadyRunning(socket_path.display().to_string()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
            // Stale socket — remove it
            tracing::info!("Removing stale socket at {}", socket_path.display());
            std::fs::remove_file(socket_path).map_err(BotError::SocketBind)?;
            Ok(())
        }
        Err(_) => {
            // Other error — try to remove and rebind
            tracing::warn!(
                "Unknown socket state at {}, attempting cleanup",
                socket_path.display()
            );
            std::fs::remove_file(socket_path).map_err(BotError::SocketBind)?;
            Ok(())
        }
    }
}

pub async fn run_server(
    socket_path: &Path,
    cancel_token: CancellationToken,
    bot: Bot,
    config: Arc<Config>,
    pending_map: PendingMap,
) -> Result<(), BotError> {
    let listener = UnixListener::bind(socket_path).map_err(BotError::SocketBind)?;
    let semaphore = Arc::new(Semaphore::new(50));
    tracing::info!("Socket server listening on {}", socket_path.display());

    loop {
        tokio::select! {
            () = cancel_token.cancelled() => {
                tracing::info!("Socket server shutting down");
                break;
            }
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, _addr)) => {
                        let Ok(permit) = semaphore.clone().try_acquire_owned() else {
                            tracing::warn!("Max concurrent connections reached, dropping connection");
                            continue;
                        };
                        let bot = bot.clone();
                        let config = config.clone();
                        let pending_map = pending_map.clone();
                        let cancel = cancel_token.clone();
                        tokio::spawn(async move {
                            let _permit = permit;
                            if let Err(e) = handle_connection(stream, bot, config, pending_map, cancel).await {
                                tracing::error!("Connection handler error: {e}");
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("Failed to accept connection: {e}");
                    }
                }
            }
        }
    }

    // Cleanup socket file
    if socket_path.exists() {
        let _ = std::fs::remove_file(socket_path);
    }

    Ok(())
}

async fn handle_connection(
    stream: tokio::net::UnixStream,
    bot: Bot,
    config: Arc<Config>,
    pending_map: PendingMap,
    cancel_token: CancellationToken,
) -> anyhow::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();
    buf_reader.read_line(&mut line).await?;

    if line.trim().is_empty() {
        tracing::warn!("Empty IPC request received");
        return Ok(());
    }

    let ipc_request: IpcRequest = serde_json::from_str(line.trim())?;
    tracing::info!(
        request_id = %ipc_request.request_id,
        tool = %ipc_request.tool_name,
        "Received permission request"
    );

    let (tx, rx) = oneshot::channel::<IpcResponse>();

    let request_id = ipc_request.request_id;

    // Send to Telegram and store pending request
    let sent_messages =
        crate::bot::send_permission_to_telegram(&bot, &config, &ipc_request).await?;

    let original_text = crate::telegram::formatter::format_permission_message(&ipc_request);

    pending_map.insert(
        request_id,
        PendingRequest {
            request_id,
            sender: tx,
            sent_messages,
            original_text,
            permission_suggestions: ipc_request.permission_suggestions,
            created_at: Instant::now(),
        },
    );

    // Wait for response with timeout
    let timeout_duration = std::time::Duration::from_secs(config.timeout_seconds);
    let response = tokio::select! {
        () = cancel_token.cancelled() => {
            pending_map.remove(&request_id);
            IpcResponse::timeout(request_id)
        }
        result = tokio::time::timeout(timeout_duration, rx) => {
            match result {
                Ok(Ok(response)) => response,
                Ok(Err(_)) => {
                    // Sender dropped (shouldn't happen normally)
                    pending_map.remove(&request_id);
                    IpcResponse::timeout(request_id)
                }
                Err(_) => {
                    // Timeout
                    tracing::warn!(request_id = %request_id, "Request timed out");
                    if let Some((_, pending)) = pending_map.remove(&request_id) {
                        crate::bot::edit_messages_status(&bot, &pending.sent_messages, &pending.original_text, "\u{23f1}\u{fe0f} Timed out").await;
                    }
                    IpcResponse::timeout(request_id)
                }
            }
        }
    };

    // Write NDJSON response back
    let mut json = serde_json::to_string(&response)?;
    json.push('\n');
    writer.write_all(json.as_bytes()).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_socket_returns_ok() {
        let tmp = tempfile::tempdir().unwrap();
        let sock = tmp.path().join("nonexistent.sock");
        assert!(detect_and_clean_stale_socket(&sock).is_ok());
    }

    #[test]
    fn stale_socket_is_removed() {
        let tmp = tempfile::tempdir().unwrap();
        let sock = tmp.path().join("stale.sock");
        // Create a listener then drop it to leave a stale socket file
        {
            let _listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();
        }
        assert!(sock.exists());
        assert!(detect_and_clean_stale_socket(&sock).is_ok());
        assert!(!sock.exists());
    }

    #[test]
    fn active_socket_returns_already_running() {
        let tmp = tempfile::tempdir().unwrap();
        let sock = tmp.path().join("active.sock");
        let _listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();
        let result = detect_and_clean_stale_socket(&sock);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, BotError::AlreadyRunning(_)));
    }

    fn make_test_config() -> Config {
        Config {
            telegram_bot_token: "fake-token".to_string(),
            allowed_chat_ids: std::collections::HashSet::from([12345]),
            timeout_seconds: 5,
            socket_path: None,
        }
    }

    #[tokio::test]
    async fn run_server_stops_on_cancel() {
        let tmp = tempfile::tempdir().unwrap();
        let sock = tmp.path().join("test.sock");
        let cancel = CancellationToken::new();
        let bot = Bot::new("fake-token");
        let config = Arc::new(make_test_config());
        let pending: PendingMap = Arc::new(DashMap::new());

        let cancel2 = cancel.clone();
        let sock2 = sock.clone();
        let handle =
            tokio::spawn(async move { run_server(&sock2, cancel2, bot, config, pending).await });

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert!(sock.exists());

        cancel.cancel();
        let result = handle.await.unwrap();
        assert!(result.is_ok());
        assert!(!sock.exists()); // Socket cleaned up
    }

    #[tokio::test]
    async fn server_handles_empty_request() {
        use tokio::io::AsyncWriteExt;

        let tmp = tempfile::tempdir().unwrap();
        let sock = tmp.path().join("empty.sock");
        let cancel = CancellationToken::new();
        let bot = Bot::new("fake-token");
        let config = Arc::new(make_test_config());
        let pending: PendingMap = Arc::new(DashMap::new());

        let cancel2 = cancel.clone();
        let sock2 = sock.clone();
        let handle =
            tokio::spawn(async move { run_server(&sock2, cancel2, bot, config, pending).await });

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Connect and send empty line
        let mut stream = tokio::net::UnixStream::connect(&sock).await.unwrap();
        stream.write_all(b"\n").await.unwrap();
        drop(stream);

        // Give server time to process
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        cancel.cancel();
        handle.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn server_handles_invalid_json() {
        use tokio::io::AsyncWriteExt;

        let tmp = tempfile::tempdir().unwrap();
        let sock = tmp.path().join("invalid.sock");
        let cancel = CancellationToken::new();
        let bot = Bot::new("fake-token");
        let config = Arc::new(make_test_config());
        let pending: PendingMap = Arc::new(DashMap::new());

        let cancel2 = cancel.clone();
        let sock2 = sock.clone();
        let handle =
            tokio::spawn(async move { run_server(&sock2, cancel2, bot, config, pending).await });

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Connect and send invalid JSON
        let mut stream = tokio::net::UnixStream::connect(&sock).await.unwrap();
        stream.write_all(b"not valid json\n").await.unwrap();
        drop(stream);

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        cancel.cancel();
        handle.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn server_handles_valid_request_with_fake_bot() {
        use tokio::io::AsyncWriteExt;

        let tmp = tempfile::tempdir().unwrap();
        let sock = tmp.path().join("valid.sock");
        let cancel = CancellationToken::new();
        let bot = Bot::new("fake-token");
        let config = Arc::new(make_test_config());
        let pending: PendingMap = Arc::new(DashMap::new());

        let cancel2 = cancel.clone();
        let sock2 = sock.clone();
        let handle =
            tokio::spawn(async move { run_server(&sock2, cancel2, bot, config, pending).await });

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Connect and send a valid IPC request (will fail at Telegram API call)
        let request = crate::models::IpcRequest {
            request_id: Uuid::new_v4(),
            tool_name: "Bash".to_string(),
            tool_input: serde_json::json!({"command": "echo hello"}),
            cwd: "/tmp".to_string(),
            session_id: "test-session".to_string(),
            permission_suggestions: vec![],
            assistant_context: None,
        };
        let mut json = serde_json::to_string(&request).unwrap();
        json.push('\n');

        let mut stream = tokio::net::UnixStream::connect(&sock).await.unwrap();
        stream.write_all(json.as_bytes()).await.unwrap();
        drop(stream);

        // Give server time to process (Telegram API call will fail quickly)
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        cancel.cancel();
        handle.await.unwrap().unwrap();
    }
}
