use crate::error::BotError;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
#[serde(from = "RawConfig")]
pub struct Config {
    pub telegram_bot_token: String,
    pub allowed_chat_ids: HashSet<i64>,
    pub timeout_seconds: u64,
    pub socket_path: Option<PathBuf>,
}

/// Intermediate type for deserialization (Vec → `HashSet` conversion).
#[derive(Deserialize)]
struct RawConfig {
    telegram_bot_token: String,
    allowed_chat_ids: Vec<i64>,
    #[serde(default = "default_timeout")]
    timeout_seconds: u64,
    socket_path: Option<PathBuf>,
}

impl From<RawConfig> for Config {
    fn from(raw: RawConfig) -> Self {
        Self {
            telegram_bot_token: raw.telegram_bot_token,
            allowed_chat_ids: raw.allowed_chat_ids.into_iter().collect(),
            timeout_seconds: raw.timeout_seconds,
            socket_path: raw.socket_path,
        }
    }
}

const fn default_timeout() -> u64 {
    300
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let config_path = config_file_path()?;
        Self::load_from_path(&config_path)
    }

    pub fn load_from_path(config_path: &std::path::Path) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(config_path).map_err(|e| {
            BotError::ConfigInvalid(format!(
                "Cannot read config at {}: {}",
                config_path.display(),
                e
            ))
        })?;
        let config: Self = toml::from_str(&contents).map_err(|e| {
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
        if let Some(ref path) = self.socket_path
            && let Some(parent) = path.parent()
            && !parent.exists()
        {
            anyhow::bail!(
                "socket_path parent directory does not exist: {}",
                parent.display()
            );
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
    let home = dirs::home_dir()
        .ok_or_else(|| BotError::ConfigInvalid("Cannot determine home directory".to_string()))?;
    Ok(home
        .join(".config")
        .join("vibe-reachout")
        .join("config.toml"))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn write_config(dir: &std::path::Path, content: &str) -> PathBuf {
        let path = dir.join("config.toml");
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn load_valid_config() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_config(
            tmp.path(),
            r#"
            telegram_bot_token = "123:ABC"
            allowed_chat_ids = [12345]
            timeout_seconds = 120
            "#,
        );
        let config = Config::load_from_path(&path).unwrap();
        assert_eq!(config.telegram_bot_token, "123:ABC");
        assert_eq!(config.allowed_chat_ids, HashSet::from([12345]));
        assert_eq!(config.timeout_seconds, 120);
        assert!(config.socket_path.is_none());
    }

    #[test]
    fn default_timeout_is_300() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_config(
            tmp.path(),
            r#"
            telegram_bot_token = "tok"
            allowed_chat_ids = [1]
            "#,
        );
        let config = Config::load_from_path(&path).unwrap();
        assert_eq!(config.timeout_seconds, 300);
    }

    #[test]
    fn empty_token_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_config(
            tmp.path(),
            r#"
            telegram_bot_token = ""
            allowed_chat_ids = [1]
            "#,
        );
        let err = Config::load_from_path(&path).unwrap_err();
        assert!(err.to_string().contains("telegram_bot_token"));
    }

    #[test]
    fn empty_chat_ids_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_config(
            tmp.path(),
            r#"
            telegram_bot_token = "tok"
            allowed_chat_ids = []
            "#,
        );
        let err = Config::load_from_path(&path).unwrap_err();
        assert!(err.to_string().contains("allowed_chat_ids"));
    }

    #[test]
    fn timeout_zero_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_config(
            tmp.path(),
            r#"
            telegram_bot_token = "tok"
            allowed_chat_ids = [1]
            timeout_seconds = 0
            "#,
        );
        assert!(Config::load_from_path(&path).is_err());
    }

    #[test]
    fn timeout_3601_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_config(
            tmp.path(),
            r#"
            telegram_bot_token = "tok"
            allowed_chat_ids = [1]
            timeout_seconds = 3601
            "#,
        );
        assert!(Config::load_from_path(&path).is_err());
    }

    #[test]
    fn timeout_boundary_values_accepted() {
        let tmp = tempfile::tempdir().unwrap();
        for timeout in [1, 3600] {
            let path = write_config(
                tmp.path(),
                &format!(
                    r#"
                    telegram_bot_token = "tok"
                    allowed_chat_ids = [1]
                    timeout_seconds = {timeout}
                    "#
                ),
            );
            assert!(Config::load_from_path(&path).is_ok());
        }
    }

    #[test]
    fn effective_socket_path_uses_custom_if_set() {
        let tmp = tempfile::tempdir().unwrap();
        let sock = tmp.path().join("custom.sock");
        let path = write_config(
            tmp.path(),
            &format!(
                "telegram_bot_token = \"tok\"\nallowed_chat_ids = [1]\nsocket_path = \"{}\"",
                sock.display()
            ),
        );
        let config = Config::load_from_path(&path).unwrap();
        assert_eq!(config.effective_socket_path(), sock);
    }

    #[test]
    fn effective_socket_path_falls_back_to_default() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_config(
            tmp.path(),
            r#"
            telegram_bot_token = "tok"
            allowed_chat_ids = [1]
            "#,
        );
        let config = Config::load_from_path(&path).unwrap();
        assert_eq!(config.effective_socket_path(), default_socket_path());
    }

    #[test]
    fn socket_path_with_nonexistent_parent_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_config(
            tmp.path(),
            r#"
            telegram_bot_token = "tok"
            allowed_chat_ids = [1]
            socket_path = "/nonexistent/dir/test.sock"
            "#,
        );
        let err = Config::load_from_path(&path).unwrap_err();
        assert!(err.to_string().contains("socket_path parent directory"));
    }

    #[test]
    fn default_socket_path_contains_expected_name() {
        let path = default_socket_path();
        let path_str = path.to_string_lossy();
        assert!(path_str.contains("vibe-reachout"));
        assert!(path_str.ends_with(".sock"));
    }
}
