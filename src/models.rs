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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
