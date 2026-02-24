use crate::error::InstallError;
use std::path::{Path, PathBuf};

pub fn run_install() -> anyhow::Result<()> {
    let settings_path = settings_file_path()?;
    install_hook(&settings_path)
}

pub fn install_hook(settings_path: &Path) -> anyhow::Result<()> {
    // Read existing settings or create empty object
    let mut settings: serde_json::Value = if settings_path.exists() {
        let contents = std::fs::read_to_string(settings_path).map_err(InstallError::WriteError)?;
        serde_json::from_str(&contents)?
    } else {
        // Create directory if needed
        if let Some(parent) = settings_path.parent() {
            std::fs::create_dir_all(parent).map_err(InstallError::WriteError)?;
        }
        serde_json::json!({})
    };

    let hooks = settings
        .as_object_mut()
        .expect("settings must be a JSON object")
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .expect("hooks must be a JSON object");

    // The hook entry we want to install
    let hook_entry = serde_json::json!({
        "type": "command",
        "command": "vibe-reachout",
        "timeout": 600
    });

    let matcher_entry = serde_json::json!({
        "hooks": [hook_entry]
    });

    // Check if PermissionRequest array exists
    if let Some(perm_hooks) = hooks.get_mut("PermissionRequest") {
        if let Some(arr) = perm_hooks.as_array_mut() {
            // Look for existing vibe-reachout hook
            let mut found = false;
            for entry in arr.iter_mut() {
                if let Some(inner_hooks) = entry.get("hooks").and_then(|h| h.as_array()) {
                    for h in inner_hooks {
                        if h.get("command").and_then(|c| c.as_str()) == Some("vibe-reachout") {
                            // Update in place
                            *entry = matcher_entry.clone();
                            found = true;
                            break;
                        }
                    }
                }
                if found {
                    break;
                }
            }
            if !found {
                arr.push(matcher_entry);
            }
        }
    } else {
        hooks.insert(
            "PermissionRequest".to_string(),
            serde_json::json!([matcher_entry]),
        );
    }

    // Write back
    let json_str = serde_json::to_string_pretty(&settings)?;
    std::fs::write(settings_path, json_str).map_err(InstallError::WriteError)?;

    println!("Hook installed at {}", settings_path.display());
    Ok(())
}

fn settings_file_path() -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| {
        InstallError::SettingsNotFound("Cannot determine home directory".to_string())
    })?;
    Ok(home.join(".claude").join("settings.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_settings(path: &Path) -> serde_json::Value {
        let content = std::fs::read_to_string(path).unwrap();
        serde_json::from_str(&content).unwrap()
    }

    #[test]
    fn install_into_empty_file_creates_structure() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("settings.json");

        install_hook(&path).unwrap();

        let settings = read_settings(&path);
        let hooks = &settings["hooks"]["PermissionRequest"];
        assert!(hooks.is_array());
        let arr = hooks.as_array().unwrap();
        assert_eq!(arr.len(), 1);

        let inner_hook = &arr[0]["hooks"][0];
        assert_eq!(inner_hook["command"], "vibe-reachout");
        assert_eq!(inner_hook["type"], "command");
        assert_eq!(inner_hook["timeout"], 600);
    }

    #[test]
    fn install_preserves_existing_hooks() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("settings.json");

        let existing = serde_json::json!({
            "hooks": {
                "PreToolUse": [{"hooks": [{"type": "command", "command": "other-tool"}]}]
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&existing).unwrap()).unwrap();

        install_hook(&path).unwrap();

        let settings = read_settings(&path);
        // Existing hook preserved
        assert!(settings["hooks"]["PreToolUse"].is_array());
        // New hook added
        assert!(settings["hooks"]["PermissionRequest"].is_array());
    }

    #[test]
    fn install_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("settings.json");

        install_hook(&path).unwrap();
        install_hook(&path).unwrap();

        let settings = read_settings(&path);
        let arr = settings["hooks"]["PermissionRequest"].as_array().unwrap();
        // Should still be exactly 1 entry, not duplicated
        assert_eq!(arr.len(), 1);
    }

    #[test]
    fn install_adds_alongside_existing_permission_hooks() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("settings.json");

        let existing = serde_json::json!({
            "hooks": {
                "PermissionRequest": [
                    {"hooks": [{"type": "command", "command": "other-tool"}]}
                ]
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&existing).unwrap()).unwrap();

        install_hook(&path).unwrap();

        let settings = read_settings(&path);
        let arr = settings["hooks"]["PermissionRequest"].as_array().unwrap();
        assert_eq!(arr.len(), 2); // original + vibe-reachout
    }

    #[test]
    fn install_creates_parent_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sub").join("dir").join("settings.json");

        install_hook(&path).unwrap();

        assert!(path.exists());
    }
}
