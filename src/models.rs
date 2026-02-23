use serde::{Deserialize, Serialize};
use teloxide::types::{ChatId, MessageId};
use tokio::sync::oneshot;
use tokio::time::Instant;
use uuid::Uuid;

/// Claude Code's JSON sent to hook via stdin.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct HookInput {
    pub session_id: String,
    pub transcript_path: String,
    pub cwd: String,
    pub permission_mode: String,
    pub hook_event_name: String,
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    #[serde(default)]
    pub permission_suggestions: Vec<serde_json::Value>,
}

/// JSON written to stdout for Claude Code.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookOutput {
    pub hook_specific_output: HookSpecificOutput,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookSpecificOutput {
    pub hook_event_name: String,
    pub decision: HookDecision,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookDecision {
    pub behavior: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_permissions: Option<Vec<serde_json::Value>>,
}

impl HookOutput {
    pub fn allow() -> Self {
        Self {
            hook_specific_output: HookSpecificOutput {
                hook_event_name: "PermissionRequest".to_string(),
                decision: HookDecision {
                    behavior: "allow".to_string(),
                    message: None,
                    updated_permissions: None,
                },
            },
        }
    }

    pub fn deny(message: String) -> Self {
        Self {
            hook_specific_output: HookSpecificOutput {
                hook_event_name: "PermissionRequest".to_string(),
                decision: HookDecision {
                    behavior: "deny".to_string(),
                    message: Some(message),
                    updated_permissions: None,
                },
            },
        }
    }

    pub fn allow_always(permissions: Vec<serde_json::Value>) -> Self {
        Self {
            hook_specific_output: HookSpecificOutput {
                hook_event_name: "PermissionRequest".to_string(),
                decision: HookDecision {
                    behavior: "allow".to_string(),
                    message: None,
                    updated_permissions: Some(permissions),
                },
            },
        }
    }
}

/// Permission details sent from hook to bot over Unix socket (NDJSON).
#[derive(Debug, Serialize, Deserialize)]
pub struct IpcRequest {
    pub request_id: Uuid,
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    pub cwd: String,
    pub session_id: String,
    #[serde(default)]
    pub permission_suggestions: Vec<serde_json::Value>,
}

/// Decision sent from bot to hook over Unix socket (NDJSON).
#[derive(Debug, Serialize, Deserialize)]
pub struct IpcResponse {
    pub request_id: Uuid,
    pub decision: Decision,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub always_allow_suggestion: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Deny,
    AlwaysAllow,
    Reply,
    Timeout,
}

/// Tracks a Telegram message sent to one chat for a pending request.
#[derive(Debug, Clone)]
pub struct SentMessage {
    pub chat_id: ChatId,
    pub message_id: MessageId,
}

/// In-memory state for a request awaiting Telegram response.
#[allow(dead_code)]
pub struct PendingRequest {
    pub request_id: Uuid,
    pub sender: oneshot::Sender<IpcResponse>,
    pub sent_messages: Vec<SentMessage>,
    pub original_text: String,
    pub permission_suggestions: Vec<serde_json::Value>,
    pub created_at: Instant,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_output_allow_produces_correct_json() {
        let output = HookOutput::allow();
        let json: serde_json::Value = serde_json::to_value(&output).unwrap();

        assert_eq!(
            json["hookSpecificOutput"]["hookEventName"],
            "PermissionRequest"
        );
        assert_eq!(json["hookSpecificOutput"]["decision"]["behavior"], "allow");
        assert!(json["hookSpecificOutput"]["decision"].get("message").is_none());
        assert!(json["hookSpecificOutput"]["decision"]
            .get("updatedPermissions")
            .is_none());
    }

    #[test]
    fn hook_output_deny_produces_correct_json() {
        let output = HookOutput::deny("not allowed".to_string());
        let json: serde_json::Value = serde_json::to_value(&output).unwrap();

        assert_eq!(json["hookSpecificOutput"]["decision"]["behavior"], "deny");
        assert_eq!(
            json["hookSpecificOutput"]["decision"]["message"],
            "not allowed"
        );
    }

    #[test]
    fn hook_output_allow_always_includes_updated_permissions() {
        let perms = vec![serde_json::json!({"tool": "Bash", "command": "ls"})];
        let output = HookOutput::allow_always(perms);
        let json: serde_json::Value = serde_json::to_value(&output).unwrap();

        assert_eq!(json["hookSpecificOutput"]["decision"]["behavior"], "allow");
        assert_eq!(
            json["hookSpecificOutput"]["decision"]["updatedPermissions"],
            serde_json::json!([{"tool": "Bash", "command": "ls"}])
        );
    }

    #[test]
    fn decision_serde_roundtrip() {
        for decision in [
            Decision::Allow,
            Decision::Deny,
            Decision::AlwaysAllow,
            Decision::Reply,
            Decision::Timeout,
        ] {
            let json = serde_json::to_string(&decision).unwrap();
            let back: Decision = serde_json::from_str(&json).unwrap();
            assert_eq!(back, decision);
        }
    }

    #[test]
    fn hook_input_deserialization_from_realistic_json() {
        let json = r#"{
            "session_id": "abc123-def456-ghi789",
            "transcript_path": "/tmp/transcript.jsonl",
            "cwd": "/home/user/project",
            "permission_mode": "default",
            "hook_event_name": "PermissionRequest",
            "tool_name": "Bash",
            "tool_input": {"command": "ls -la"},
            "permission_suggestions": [{"tool": "Bash", "command": "ls -la"}]
        }"#;

        let input: HookInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.session_id, "abc123-def456-ghi789");
        assert_eq!(input.tool_name, "Bash");
        assert_eq!(input.permission_suggestions.len(), 1);
    }

    #[test]
    fn hook_input_deserialization_without_optional_fields() {
        let json = r#"{
            "session_id": "abc",
            "transcript_path": "/tmp/t.jsonl",
            "cwd": "/home",
            "permission_mode": "default",
            "hook_event_name": "PermissionRequest",
            "tool_name": "Read",
            "tool_input": {"file_path": "/etc/hosts"}
        }"#;

        let input: HookInput = serde_json::from_str(json).unwrap();
        assert!(input.permission_suggestions.is_empty());
    }

    #[test]
    fn ipc_request_response_serde_roundtrip() {
        let id = Uuid::new_v4();
        let request = IpcRequest {
            request_id: id,
            tool_name: "Bash".to_string(),
            tool_input: serde_json::json!({"command": "echo hello"}),
            cwd: "/home/user".to_string(),
            session_id: "session-123".to_string(),
            permission_suggestions: vec![],
        };

        let json = serde_json::to_string(&request).unwrap();
        let back: IpcRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.request_id, id);
        assert_eq!(back.tool_name, "Bash");

        let response = IpcResponse {
            request_id: id,
            decision: Decision::Allow,
            message: None,
            user_message: Some("approved by user".to_string()),
            always_allow_suggestion: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        let back: IpcResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.request_id, id);
        assert_eq!(back.decision, Decision::Allow);
        assert_eq!(back.user_message.as_deref(), Some("approved by user"));
    }
}
