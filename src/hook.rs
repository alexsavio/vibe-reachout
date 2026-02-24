use crate::config::Config;
use crate::models::{Decision, HookInput, HookOutput, IpcRequest, IpcResponse};
use std::path::Path;
use tokio::io::AsyncReadExt;
use uuid::Uuid;

const MAX_ASSISTANT_CONTEXT_CHARS: usize = 500;

/// Maps an `IpcResponse` to the corresponding `HookOutput`.
/// Returns `None` for `Decision::Timeout` (caller handles process exit).
pub fn map_decision_to_output(response: &IpcResponse) -> Option<HookOutput> {
    match response.decision {
        Decision::Allow => Some(HookOutput::allow()),
        Decision::Deny => {
            let msg = response
                .message
                .clone()
                .unwrap_or_else(|| "Denied via Telegram".to_string());
            Some(HookOutput::deny(msg))
        }
        Decision::AlwaysAllow => {
            let permissions = response
                .always_allow_suggestion
                .clone()
                .map(|s| vec![s])
                .unwrap_or_default();
            Some(HookOutput::allow_always(permissions))
        }
        Decision::Reply => {
            let user_msg = response
                .user_message
                .clone()
                .unwrap_or_else(|| "(no message)".to_string());
            Some(HookOutput::deny(format!(
                "The user wants you to modify your approach: {user_msg}"
            )))
        }
        Decision::Timeout => None,
    }
}

pub async fn run_hook(config: &Config) -> anyhow::Result<()> {
    // Read all stdin
    let mut input = String::new();
    tokio::io::stdin().read_to_string(&mut input).await?;

    if input.trim().is_empty() {
        anyhow::bail!("Empty stdin — no hook input received");
    }

    let hook_input: HookInput = serde_json::from_str(&input)?;

    let assistant_context = extract_last_assistant_text(&hook_input.transcript_path);

    let request_id = Uuid::new_v4();

    let ipc_request = IpcRequest {
        request_id,
        tool_name: hook_input.tool_name,
        tool_input: hook_input.tool_input,
        cwd: hook_input.cwd,
        session_id: hook_input.session_id,
        permission_suggestions: hook_input.permission_suggestions,
        assistant_context,
    };

    let socket_path = config.effective_socket_path();

    // Send to bot and wait for response (timeout handled by bot side)
    // Hook-side timeout is config.timeout_seconds + 30s buffer
    let ipc_timeout = config.timeout_seconds + 30;
    let response =
        crate::ipc::client::send_request(&socket_path, &ipc_request, ipc_timeout).await?;

    // Map IpcResponse to HookOutput
    let output = map_decision_to_output(&response)
        .ok_or_else(|| anyhow::anyhow!("Request timed out — falling back to terminal"))?;

    // Write JSON to stdout
    let json = serde_json::to_string(&output)?;
    println!("{json}");

    Ok(())
}

/// Reads the transcript JSONL file and extracts the last assistant text message.
///
/// The transcript is a JSONL file where each line is a JSON object.
/// We look for the last entry where `type == "assistant"` and extract text
/// blocks from `message.content`.
///
/// Returns `None` if the file is unreadable or no assistant text is found.
fn extract_last_assistant_text(transcript_path: &str) -> Option<String> {
    let path = Path::new(transcript_path);
    let content = std::fs::read_to_string(path).ok()?;

    // Iterate lines from the end to find the last assistant message with text content
    for line in content.lines().rev() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };

        if entry.get("type").and_then(|t| t.as_str()) != Some("assistant") {
            continue;
        }

        // Extract text blocks from message.content
        let Some(content_arr) = entry
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array())
        else {
            continue;
        };

        let mut texts = Vec::new();
        for block in content_arr {
            if block.get("type").and_then(|t| t.as_str()) == Some("text")
                && let Some(text) = block.get("text").and_then(|t| t.as_str())
            {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    texts.push(trimmed.to_string());
                }
            }
        }

        if !texts.is_empty() {
            let joined = texts.join("\n");
            return Some(truncate_assistant_context(&joined));
        }
    }

    None
}

/// Truncates with a short `"..."` suffix (vs `formatter::truncate` which uses
/// `"... (truncated)"`) since this text is displayed inline in the Telegram message.
fn truncate_assistant_context(s: &str) -> String {
    if s.len() <= MAX_ASSISTANT_CONTEXT_CHARS {
        s.to_string()
    } else {
        let boundary = s.floor_char_boundary(MAX_ASSISTANT_CONTEXT_CHARS);
        format!("{}...", &s[..boundary])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_response(decision: Decision) -> IpcResponse {
        IpcResponse {
            request_id: Uuid::new_v4(),
            decision,
            message: None,
            user_message: None,
            always_allow_suggestion: None,
        }
    }

    #[test]
    fn map_allow() {
        let resp = make_response(Decision::Allow);
        let output = map_decision_to_output(&resp).unwrap();
        let json: serde_json::Value = serde_json::to_value(&output).unwrap();
        assert_eq!(json["hookSpecificOutput"]["decision"]["behavior"], "allow");
        assert!(
            json["hookSpecificOutput"]["decision"]
                .get("message")
                .is_none()
        );
    }

    #[test]
    fn map_deny_with_message() {
        let mut resp = make_response(Decision::Deny);
        resp.message = Some("custom denial".to_string());
        let output = map_decision_to_output(&resp).unwrap();
        let json: serde_json::Value = serde_json::to_value(&output).unwrap();
        assert_eq!(json["hookSpecificOutput"]["decision"]["behavior"], "deny");
        assert_eq!(
            json["hookSpecificOutput"]["decision"]["message"],
            "custom denial"
        );
    }

    #[test]
    fn map_deny_without_message() {
        let resp = make_response(Decision::Deny);
        let output = map_decision_to_output(&resp).unwrap();
        let json: serde_json::Value = serde_json::to_value(&output).unwrap();
        assert_eq!(
            json["hookSpecificOutput"]["decision"]["message"],
            "Denied via Telegram"
        );
    }

    #[test]
    fn map_always_allow_with_suggestion() {
        let mut resp = make_response(Decision::AlwaysAllow);
        resp.always_allow_suggestion = Some(serde_json::json!({"tool": "Bash", "command": "ls"}));
        let output = map_decision_to_output(&resp).unwrap();
        let json: serde_json::Value = serde_json::to_value(&output).unwrap();
        assert_eq!(json["hookSpecificOutput"]["decision"]["behavior"], "allow");
        let perms = &json["hookSpecificOutput"]["decision"]["updatedPermissions"];
        assert_eq!(
            perms,
            &serde_json::json!([{"tool": "Bash", "command": "ls"}])
        );
    }

    #[test]
    fn map_always_allow_without_suggestion() {
        let resp = make_response(Decision::AlwaysAllow);
        let output = map_decision_to_output(&resp).unwrap();
        let json: serde_json::Value = serde_json::to_value(&output).unwrap();
        let perms = &json["hookSpecificOutput"]["decision"]["updatedPermissions"];
        assert_eq!(perms, &serde_json::json!([]));
    }

    #[test]
    fn map_reply_with_message() {
        let mut resp = make_response(Decision::Reply);
        resp.user_message = Some("please use pytest".to_string());
        let output = map_decision_to_output(&resp).unwrap();
        let json: serde_json::Value = serde_json::to_value(&output).unwrap();
        assert_eq!(json["hookSpecificOutput"]["decision"]["behavior"], "deny");
        assert_eq!(
            json["hookSpecificOutput"]["decision"]["message"],
            "The user wants you to modify your approach: please use pytest"
        );
    }

    #[test]
    fn map_reply_without_message() {
        let resp = make_response(Decision::Reply);
        let output = map_decision_to_output(&resp).unwrap();
        let json: serde_json::Value = serde_json::to_value(&output).unwrap();
        assert_eq!(
            json["hookSpecificOutput"]["decision"]["message"],
            "The user wants you to modify your approach: (no message)"
        );
    }

    #[test]
    fn map_timeout_returns_none() {
        let resp = make_response(Decision::Timeout);
        assert!(map_decision_to_output(&resp).is_none());
    }

    #[test]
    fn extract_assistant_text_from_transcript() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("transcript.jsonl");
        let lines = [
            r#"{"type":"user","message":{"content":[{"type":"text","text":"hello"}]}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"I will run the tests now."}]}}"#,
            r#"{"type":"tool_use","message":{"content":[]}}"#,
        ];
        std::fs::write(&path, lines.join("\n")).unwrap();
        let result = extract_last_assistant_text(path.to_str().unwrap());
        assert_eq!(result.as_deref(), Some("I will run the tests now."));
    }

    #[test]
    fn extract_assistant_text_skips_non_text_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("transcript.jsonl");
        let lines = [
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"123"}]}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Final message"}]}}"#,
        ];
        std::fs::write(&path, lines.join("\n")).unwrap();
        let result = extract_last_assistant_text(path.to_str().unwrap());
        assert_eq!(result.as_deref(), Some("Final message"));
    }

    #[test]
    fn extract_assistant_text_concatenates_multiple_text_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("transcript.jsonl");
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Part 1"},{"type":"text","text":"Part 2"}]}}"#;
        std::fs::write(&path, line).unwrap();
        let result = extract_last_assistant_text(path.to_str().unwrap());
        assert_eq!(result.as_deref(), Some("Part 1\nPart 2"));
    }

    #[test]
    fn extract_assistant_text_returns_none_for_missing_file() {
        let result = extract_last_assistant_text("/nonexistent/path/transcript.jsonl");
        assert!(result.is_none());
    }

    #[test]
    fn extract_assistant_text_returns_none_for_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.jsonl");
        std::fs::write(&path, "").unwrap();
        let result = extract_last_assistant_text(path.to_str().unwrap());
        assert!(result.is_none());
    }

    #[test]
    fn extract_assistant_text_truncates_long_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("transcript.jsonl");
        let long_text = "x".repeat(600);
        let line = format!(
            r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"{long_text}"}}]}}}}"#,
        );
        std::fs::write(&path, line).unwrap();
        let result = extract_last_assistant_text(path.to_str().unwrap()).unwrap();
        assert!(result.len() <= MAX_ASSISTANT_CONTEXT_CHARS + 3); // +3 for "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn truncate_assistant_context_short_text() {
        let result = truncate_assistant_context("short text");
        assert_eq!(result, "short text");
    }

    #[test]
    fn truncate_assistant_context_long_text() {
        let long = "a".repeat(600);
        let result = truncate_assistant_context(&long);
        assert!(result.ends_with("..."));
        assert!(result.len() <= MAX_ASSISTANT_CONTEXT_CHARS + 3);
    }
}
