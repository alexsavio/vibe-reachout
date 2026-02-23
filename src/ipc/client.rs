use crate::error::HookError;
use crate::models::{IpcRequest, IpcResponse};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::time::{timeout, Duration};

pub async fn send_request(
    socket_path: &Path,
    request: &IpcRequest,
    timeout_secs: u64,
) -> Result<IpcResponse, HookError> {
    if !socket_path.exists() {
        return Err(HookError::SocketNotFound(
            socket_path.display().to_string(),
        ));
    }

    let stream = UnixStream::connect(socket_path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::ConnectionRefused {
            HookError::ConnectionRefused
        } else {
            HookError::StdinRead(e)
        }
    })?;

    let (reader, mut writer) = stream.into_split();

    // Write NDJSON request
    let mut json = serde_json::to_string(request).map_err(|e| {
        HookError::InvalidResponse(format!("Failed to serialize request: {e}"))
    })?;
    json.push('\n');
    writer.write_all(json.as_bytes()).await?;
    writer.shutdown().await?;

    // Read NDJSON response with timeout
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    let read_result = timeout(Duration::from_secs(timeout_secs), buf_reader.read_line(&mut line))
        .await
        .map_err(|_| HookError::Timeout(timeout_secs))?;

    read_result?;

    if line.trim().is_empty() {
        return Err(HookError::InvalidResponse(
            "Empty response from bot".to_string(),
        ));
    }

    let response: IpcResponse = serde_json::from_str(line.trim())?;
    Ok(response)
}
