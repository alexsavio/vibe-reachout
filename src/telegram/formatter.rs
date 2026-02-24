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

    let context_section = request
        .assistant_context
        .as_deref()
        .map(|ctx| format!("\n\n\u{1f4ac} {}", escape_html(ctx)))
        .unwrap_or_default();

    let message = format!(
        "<b>\u{1f4cb} {project_name}</b>{context}\n\n<b>\u{1f527} {tool}</b>\n{details}\n\n\u{1f4c1} {cwd}\n\u{1f194} Session: <code>{session}</code>",
        project_name = escape_html(project_name),
        context = context_section,
        tool = escape_html(&request.tool_name),
        details = tool_details,
        cwd = escape_html(&request.cwd),
        session = escape_html(session_short),
    );

    truncate(&message, MAX_TOTAL_CHARS)
}

fn format_tool_details(tool_name: &str, tool_input: &serde_json::Value) -> String {
    match tool_name {
        "Bash" => {
            let command = tool_input
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("<no command>");
            let truncated = truncate(command, MAX_FIELD_CHARS);
            format!("<pre>{}</pre>", escape_html(&truncated))
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
            format!(
                "\u{1f4c4} <code>{}</code> ({})",
                escape_html(file_path),
                escape_html(&size)
            )
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
            let old_truncated = truncate(old, MAX_FIELD_CHARS / 2);
            let new_truncated = truncate(new, MAX_FIELD_CHARS / 2);
            format!(
                "\u{1f4c4} <code>{}</code>\n<pre>- {}\n+ {}</pre>",
                escape_html(file_path),
                escape_html(&old_truncated),
                escape_html(&new_truncated),
            )
        }
        _ => {
            // Generic: show JSON excerpt
            let json_str = serde_json::to_string_pretty(tool_input).unwrap_or_default();
            let truncated = truncate(&json_str, MAX_FIELD_CHARS);
            format!("<pre>{}</pre>", escape_html(&truncated))
        }
    }
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let boundary = s.floor_char_boundary(max);
        format!("{}... (truncated)", &s[..boundary])
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
            assistant_context: None,
        }
    }

    #[test]
    fn format_bash_tool() {
        let req = make_request("Bash", serde_json::json!({"command": "ls -la"}));
        let msg = format_permission_message(&req);
        assert!(msg.contains("<b>\u{1f527} Bash</b>"));
        assert!(msg.contains("<pre>ls -la</pre>"));
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
        assert!(msg.contains("<b>\u{1f527} Write</b>"));
        assert!(msg.contains("<code>/tmp/test.rs</code>"));
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
        assert!(msg.contains("<b>\u{1f527} Edit</b>"));
        assert!(msg.contains("<code>/tmp/test.rs</code>"));
        assert!(msg.contains("- fn old()"));
        assert!(msg.contains("+ fn new()"));
        assert!(msg.contains("<pre>"));
    }

    #[test]
    fn format_unknown_tool_shows_json() {
        let req = make_request("CustomTool", serde_json::json!({"key": "value"}));
        let msg = format_permission_message(&req);
        assert!(msg.contains("<b>\u{1f527} CustomTool</b>"));
        assert!(msg.contains("<pre>"));
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
        assert!(msg.contains("<code>abcdef12</code>"));
        // Should not contain full session ID
        assert!(!msg.contains("abcdef1234567890"));
    }

    #[test]
    fn truncate_on_multibyte_utf8() {
        // Emoji (4 bytes each): cutting at byte 5 would land inside the second emoji
        let input = "\u{1f600}\u{1f601}\u{1f602}\u{1f603}"; // 4 emoji = 16 bytes
        let result = truncate(input, 5);
        // Should truncate at char boundary (after first emoji, byte 4)
        assert!(result.starts_with("\u{1f600}"));
        assert!(result.ends_with("... (truncated)"));
        // Must not contain the second emoji fully
        assert!(!result.contains("\u{1f601}\u{1f602}\u{1f603}"));

        // CJK characters (3 bytes each): cutting at byte 4 lands inside second char
        let cjk = "\u{4e16}\u{754c}\u{4f60}\u{597d}"; // 4 CJK = 12 bytes
        let result = truncate(cjk, 4);
        assert!(result.starts_with("\u{4e16}"));
        assert!(result.ends_with("... (truncated)"));
    }

    #[test]
    fn truncate_long_message_on_multibyte_utf8() {
        let input = "Hello \u{1f30d}\u{1f30d}\u{1f30d}".to_string() + &"x".repeat(4000);
        let result = truncate(&input, 10);
        // Byte 10 lands inside the second emoji (6 ASCII + 4 bytes of first emoji = 10)
        // floor_char_boundary(10) = 10 (end of first emoji)
        assert!(result.ends_with("... (truncated)"));
        // Should not panic and should be valid UTF-8
        assert!(result.is_char_boundary(0));
    }

    #[test]
    fn format_with_assistant_context() {
        let mut req = make_request("Bash", serde_json::json!({"command": "cargo test"}));
        req.assistant_context = Some("I will run the tests now.".to_string());
        let msg = format_permission_message(&req);
        assert!(msg.contains("\u{1f4ac} I will run the tests now."));
        // Project name should appear before context
        let project_pos = msg.find("\u{1f4cb}").unwrap();
        let context_pos = msg.find("\u{1f4ac}").unwrap();
        assert!(project_pos < context_pos);
    }

    #[test]
    fn format_without_assistant_context() {
        let req = make_request("Bash", serde_json::json!({"command": "cargo test"}));
        let msg = format_permission_message(&req);
        assert!(!msg.contains("\u{1f4ac}"));
    }

    #[test]
    fn escape_html_special_chars() {
        assert_eq!(escape_html("a < b & c > d"), "a &lt; b &amp; c &gt; d");
        assert_eq!(
            escape_html("<script>alert('xss')</script>"),
            "&lt;script&gt;alert('xss')&lt;/script&gt;"
        );
        assert_eq!(escape_html("no special chars"), "no special chars");
        assert_eq!(escape_html(""), "");
    }

    #[test]
    fn html_special_chars_in_command_are_escaped() {
        let req = make_request(
            "Bash",
            serde_json::json!({"command": "echo '<hello>' && true"}),
        );
        let msg = format_permission_message(&req);
        assert!(msg.contains("&lt;hello&gt;"));
        assert!(msg.contains("&amp;&amp;"));
        // Raw < and > should not appear in the command area
        assert!(!msg.contains("<hello>"));
    }

    #[test]
    fn html_special_chars_in_file_path_are_escaped() {
        let req = make_request(
            "Write",
            serde_json::json!({"file_path": "/tmp/<test>.rs", "content": "x"}),
        );
        let msg = format_permission_message(&req);
        assert!(msg.contains("&lt;test&gt;"));
    }

    #[test]
    fn html_special_chars_in_assistant_context_are_escaped() {
        let mut req = make_request("Bash", serde_json::json!({"command": "ls"}));
        req.assistant_context = Some("Use <pre> tags & stuff".to_string());
        let msg = format_permission_message(&req);
        assert!(msg.contains("&lt;pre&gt; tags &amp; stuff"));
    }

    #[test]
    fn project_name_bold_html() {
        let req = make_request("Bash", serde_json::json!({"command": "ls"}));
        let msg = format_permission_message(&req);
        assert!(msg.contains("<b>\u{1f4cb} my-project</b>"));
    }
}
