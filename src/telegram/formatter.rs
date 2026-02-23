use crate::models::IpcRequest;
use std::path::Path;

const MAX_FIELD_CHARS: usize = 500;
const MAX_TOTAL_CHARS: usize = 4000;

pub fn format_permission_message(request: &IpcRequest) -> String {
    let project_name = Path::new(&request.cwd)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let session_short = if request.session_id.len() > 8 {
        &request.session_id[..8]
    } else {
        &request.session_id
    };

    let tool_details = format_tool_details(&request.tool_name, &request.tool_input);

    let message = format!(
        "\u{1f4cb} {project_name}\n\n\u{1f527} {tool}\n{details}\n\n\u{1f4c1} {cwd}\n\u{1f194} Session: {session}",
        tool = request.tool_name,
        details = tool_details,
        cwd = request.cwd,
        session = session_short,
    );

    truncate_message(&message, MAX_TOTAL_CHARS)
}

fn format_tool_details(tool_name: &str, tool_input: &serde_json::Value) -> String {
    match tool_name {
        "Bash" => {
            let command = tool_input
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("<no command>");
            let truncated = truncate_field(command, MAX_FIELD_CHARS);
            format!("```\n{truncated}\n```")
        }
        "Write" => {
            let file_path = tool_input
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("<unknown file>");
            let content_len = tool_input
                .get("content")
                .and_then(|v| v.as_str())
                .map_or(0, str::len);
            let size = format_size(content_len);
            format!("\u{1f4c4} {file_path} ({size})")
        }
        "Edit" => {
            let file_path = tool_input
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("<unknown file>");
            let old = tool_input
                .get("old_string")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let new = tool_input
                .get("new_string")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let old_truncated = truncate_field(old, MAX_FIELD_CHARS / 2);
            let new_truncated = truncate_field(new, MAX_FIELD_CHARS / 2);
            format!("\u{1f4c4} {file_path}\n- {old_truncated}\n+ {new_truncated}")
        }
        _ => {
            // Generic: show JSON excerpt
            let json_str = serde_json::to_string_pretty(tool_input).unwrap_or_default();
            let truncated = truncate_field(&json_str, MAX_FIELD_CHARS);
            format!("```json\n{truncated}\n```")
        }
    }
}

fn truncate_field(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}... (truncated)", &s[..max])
    }
}

fn truncate_message(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}... (truncated)", &s[..max])
    }
}

fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        #[allow(clippy::cast_precision_loss)]
        let kb = bytes as f64 / 1024.0;
        format!("{kb:.1} KB")
    } else {
        #[allow(clippy::cast_precision_loss)]
        let mb = bytes as f64 / (1024.0 * 1024.0);
        format!("{mb:.1} MB")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_request(tool_name: &str, tool_input: serde_json::Value) -> IpcRequest {
        IpcRequest {
            request_id: Uuid::new_v4(),
            tool_name: tool_name.to_string(),
            tool_input,
            cwd: "/home/user/my-project".to_string(),
            session_id: "abcdef1234567890".to_string(),
            permission_suggestions: vec![],
        }
    }

    #[test]
    fn format_bash_tool() {
        let req = make_request("Bash", serde_json::json!({"command": "ls -la"}));
        let msg = format_permission_message(&req);
        assert!(msg.contains("Bash"));
        assert!(msg.contains("ls -la"));
        assert!(msg.contains("my-project"));
    }

    #[test]
    fn format_write_tool() {
        let content = "a".repeat(100);
        let req = make_request(
            "Write",
            serde_json::json!({"file_path": "/tmp/test.rs", "content": content}),
        );
        let msg = format_permission_message(&req);
        assert!(msg.contains("Write"));
        assert!(msg.contains("/tmp/test.rs"));
        assert!(msg.contains("100 B"));
    }

    #[test]
    fn format_edit_tool() {
        let req = make_request(
            "Edit",
            serde_json::json!({
                "file_path": "/tmp/test.rs",
                "old_string": "fn old()",
                "new_string": "fn new()"
            }),
        );
        let msg = format_permission_message(&req);
        assert!(msg.contains("Edit"));
        assert!(msg.contains("/tmp/test.rs"));
        assert!(msg.contains("fn old()"));
        assert!(msg.contains("fn new()"));
    }

    #[test]
    fn format_unknown_tool_shows_json() {
        let req = make_request(
            "CustomTool",
            serde_json::json!({"key": "value"}),
        );
        let msg = format_permission_message(&req);
        assert!(msg.contains("CustomTool"));
        assert!(msg.contains("key"));
        assert!(msg.contains("value"));
    }

    #[test]
    fn field_truncation_at_500_chars() {
        let long_command = "x".repeat(600);
        let req = make_request("Bash", serde_json::json!({"command": long_command}));
        let msg = format_permission_message(&req);
        assert!(msg.contains("... (truncated)"));
        // The full 600-char command should NOT appear
        assert!(!msg.contains(&long_command));
    }

    #[test]
    fn total_message_truncation_at_4000_chars() {
        // Create a very long command that will push total > 4000
        let long_command = "y".repeat(4500);
        let req = make_request("Bash", serde_json::json!({"command": long_command}));
        let msg = format_permission_message(&req);
        assert!(msg.len() <= 4000 + "... (truncated)".len());
    }

    #[test]
    fn format_size_bytes() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1023), "1023 B");
    }

    #[test]
    fn format_size_kilobytes() {
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1024 * 100), "100.0 KB");
    }

    #[test]
    fn format_size_megabytes() {
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(1024 * 1024 * 5), "5.0 MB");
    }

    #[test]
    fn session_id_truncated_to_8_chars() {
        let req = make_request("Bash", serde_json::json!({"command": "ls"}));
        let msg = format_permission_message(&req);
        assert!(msg.contains("abcdef12"));
        // Should not contain full session ID
        assert!(!msg.contains("abcdef1234567890"));
    }
}
