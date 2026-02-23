use crate::error::BotError;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub telegram_bot_token: String,
    pub allowed_chat_ids: Vec<i64>,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    pub socket_path: Option<PathBuf>,
}

fn default_timeout() -> u64 {
    300
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let config_path = config_file_path()?;
        let contents = std::fs::read_to_string(&config_path).map_err(|e| {
            BotError::ConfigInvalid(format!("Cannot read config at {}: {}", config_path.display(), e))
        })?;
        let config: Config = toml::from_str(&contents).map_err(|e| {
            BotError::ConfigInvalid(format!("Invalid TOML in {}: {}", config_path.display(), e))
        })?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> anyhow::Result<()> {
        if self.telegram_bot_token.is_empty() {
            anyhow::bail!("telegram_bot_token must not be empty");
        }
        if self.allowed_chat_ids.is_empty() {
            anyhow::bail!("allowed_chat_ids must have at least one entry");
        }
        if self.timeout_seconds == 0 || self.timeout_seconds > 3600 {
            anyhow::bail!("timeout_seconds must be between 1 and 3600");
        }
        if let Some(ref path) = self.socket_path {
            if let Some(parent) = path.parent() {
                if !parent.exists() {
                    anyhow::bail!(
                        "socket_path parent directory does not exist: {}",
                        parent.display()
                    );
                }
            }
        }
        Ok(())
    }

    pub fn effective_socket_path(&self) -> PathBuf {
        if let Some(ref path) = self.socket_path {
            return path.clone();
        }
        default_socket_path()
    }
}

fn config_file_path() -> anyhow::Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| BotError::ConfigInvalid("Cannot determine config directory".to_string()))?;
    Ok(config_dir.join("vibe-reachout").join("config.toml"))
}

pub fn default_socket_path() -> PathBuf {
    // 1. XDG_RUNTIME_DIR (Linux)
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime_dir).join("vibe-reachout.sock");
    }

    // 2. /tmp/vibe-reachout-{uid}.sock (macOS / fallback)
    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/tmp/vibe-reachout-{uid}.sock"))
}
