use serde::{Deserialize, Serialize};
use teloxide::types::{ChatId, MessageId};
use tokio::sync::oneshot;
use tokio::time::Instant;
use uuid::Uuid;

/// Claude Code's JSON sent to hook via stdin.
#[derive(Debug, Deserialize)]
pub struct HookInput {
    pub session_id: String,
    /// Deserialized for protocol completeness; not read after parsing.
    #[allow(dead_code)]
    pub transcript_path: String,
    pub cwd: String,
    /// Deserialized for protocol completeness; not read after parsing.
    #[allow(dead_code)]
    pub permission_mode: String,
    /// Deserialized for protocol completeness; not read after parsing.
    #[allow(dead_code)]
    pub hook_event_name: String,
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    #[serde(default)]
    pub permission_suggestions: Vec<serde_json::Value>,
}

/// The behavior a hook decision can take.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum HookBehavior {
    Allow,
    Deny,
}

/// Known hook event names in the Claude Code protocol.
#[derive(Debug, Serialize)]
pub enum HookEventName {
    PermissionRequest,
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
    pub hook_event_name: HookEventName,
    pub decision: HookDecision,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookDecision {
    pub behavior: HookBehavior,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_permissions: Option<Vec<serde_json::Value>>,
}

impl HookOutput {
    #[must_use]
    pub const fn allow() -> Self {
        Self {
            hook_specific_output: HookSpecificOutput {
                hook_event_name: HookEventName::PermissionRequest,
                decision: HookDecision {
                    behavior: HookBehavior::Allow,
                    message: None,
                    updated_permissions: None,
                },
            },
        }
    }

    #[must_use]
    pub const fn deny(message: String) -> Self {
        Self {
            hook_specific_output: HookSpecificOutput {
                hook_event_name: HookEventName::PermissionRequest,
                decision: HookDecision {
                    behavior: HookBehavior::Deny,
                    message: Some(message),
                    updated_permissions: None,
                },
            },
        }
    }

    #[must_use]
    pub const fn allow_always(permissions: Vec<serde_json::Value>) -> Self {
        Self {
            hook_specific_output: HookSpecificOutput {
                hook_event_name: HookEventName::PermissionRequest,
                decision: HookDecision {
                    behavior: HookBehavior::Allow,
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

impl IpcResponse {
    #[must_use]
    pub const fn timeout(request_id: Uuid) -> Self {
        Self {
            request_id,
            decision: Decision::Timeout,
            message: None,
            user_message: None,
            always_allow_suggestion: None,
        }
    }

    #[must_use]
    pub const fn allow(request_id: Uuid) -> Self {
        Self {
            request_id,
            decision: Decision::Allow,
            message: None,
            user_message: None,
            always_allow_suggestion: None,
        }
    }

    #[must_use]
    pub const fn deny(request_id: Uuid, message: String) -> Self {
        Self {
            request_id,
            decision: Decision::Deny,
            message: Some(message),
            user_message: None,
            always_allow_suggestion: None,
        }
    }

    #[must_use]
    pub const fn always_allow(request_id: Uuid, suggestion: Option<serde_json::Value>) -> Self {
        Self {
            request_id,
            decision: Decision::AlwaysAllow,
            message: None,
            user_message: None,
            always_allow_suggestion: suggestion,
        }
    }

    #[must_use]
    pub const fn reply(request_id: Uuid, user_message: String) -> Self {
        Self {
            request_id,
            decision: Decision::Reply,
            message: None,
            user_message: Some(user_message),
            always_allow_suggestion: None,
        }
    }
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
pub struct PendingRequest {
    /// Redundant with the `DashMap` key; kept for logging convenience.
    #[allow(dead_code)]
    pub request_id: Uuid,
    pub sender: oneshot::Sender<IpcResponse>,
    pub sent_messages: Vec<SentMessage>,
    pub original_text: String,
    pub permission_suggestions: Vec<serde_json::Value>,
    /// Stored for future timeout diagnostics.
    #[allow(dead_code)]
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
        assert!(
            json["hookSpecificOutput"]["decision"]
                .get("message")
                .is_none()
        );
        assert!(
            json["hookSpecificOutput"]["decision"]
                .get("updatedPermissions")
                .is_none()
        );
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

    #[test]
    fn ipc_response_timeout_constructor() {
        let id = Uuid::new_v4();
        let resp = IpcResponse::timeout(id);
        assert_eq!(resp.request_id, id);
        assert_eq!(resp.decision, Decision::Timeout);
        assert!(resp.message.is_none());
        assert!(resp.user_message.is_none());
        assert!(resp.always_allow_suggestion.is_none());
    }

    #[test]
    fn ipc_response_constructors_produce_correct_decisions() {
        let id = Uuid::new_v4();

        let allow = IpcResponse::allow(id);
        assert_eq!(allow.decision, Decision::Allow);

        let deny = IpcResponse::deny(id, "nope".to_string());
        assert_eq!(deny.decision, Decision::Deny);
        assert_eq!(deny.message.as_deref(), Some("nope"));

        let suggestion = serde_json::json!({"tool": "Bash"});
        let always = IpcResponse::always_allow(id, Some(suggestion.clone()));
        assert_eq!(always.decision, Decision::AlwaysAllow);
        assert_eq!(always.always_allow_suggestion, Some(suggestion));

        let reply = IpcResponse::reply(id, "use pytest".to_string());
        assert_eq!(reply.decision, Decision::Reply);
        assert_eq!(reply.user_message.as_deref(), Some("use pytest"));
    }
}
