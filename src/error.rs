use thiserror::Error;

#[derive(Error, Debug)]
pub enum HookError {
    #[error("Bot not running (socket not found at {0})")]
    SocketNotFound(String),

    #[error("Connection refused (socket exists but bot not responding)")]
    ConnectionRefused,

    #[error("Invalid response from bot: {0}")]
    InvalidResponse(String),

    #[error("Request timed out after {0}s")]
    Timeout(u64),

    #[error("IPC connection failed: {0}")]
    ConnectionFailed(#[source] std::io::Error),

    #[error("Failed to parse hook input: {0}")]
    InvalidInput(#[from] serde_json::Error),
}

#[derive(Error, Debug)]
pub enum BotError {
    #[error("Bot already running (socket is active at {0})")]
    AlreadyRunning(String),

    #[error("Invalid configuration: {0}")]
    ConfigInvalid(String),

    #[error("Socket bind error: {0}")]
    SocketBind(#[source] std::io::Error),
}

#[derive(Error, Debug)]
pub enum InstallError {
    #[error("Settings file not found at {0}")]
    SettingsNotFound(String),

    #[error("Failed to parse settings: {0}")]
    ParseError(#[from] serde_json::Error),

    #[error("Failed to write settings: {0}")]
    WriteError(#[source] std::io::Error),
}
