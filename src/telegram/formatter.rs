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
                .map(|s| s.len())
                .unwrap_or(0);
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
            format!(
                "\u{1f4c4} {file_path}\n- {old_truncated}\n+ {new_truncated}"
            )
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
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
