use crate::error::InstallError;
use std::path::PathBuf;

pub fn run_install() -> anyhow::Result<()> {
    let settings_path = settings_file_path()?;

    // Read existing settings or create empty object
    let mut settings: serde_json::Value = if settings_path.exists() {
        let contents = std::fs::read_to_string(&settings_path)
            .map_err(InstallError::WriteError)?;
        serde_json::from_str(&contents)?
    } else {
        // Create directory if needed
        if let Some(parent) = settings_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(InstallError::WriteError)?;
        }
        serde_json::json!({})
    };

    // Ensure hooks object exists
    if settings.get("hooks").is_none() {
        settings["hooks"] = serde_json::json!({});
    }

    let hooks = settings["hooks"].as_object_mut().unwrap();

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
    std::fs::write(&settings_path, json_str)
        .map_err(InstallError::WriteError)?;

    println!("Hook installed at {}", settings_path.display());
    Ok(())
}

fn settings_file_path() -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| {
        InstallError::SettingsNotFound("Cannot determine home directory".to_string())
    })?;
    Ok(home.join(".claude").join("settings.json"))
}
