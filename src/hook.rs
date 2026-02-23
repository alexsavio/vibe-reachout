use crate::config::Config;
use crate::models::{Decision, HookInput, HookOutput, IpcRequest, IpcResponse};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

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
            Some(HookOutput::deny(format!("User replied: {user_msg}")))
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

    let request_id = Uuid::new_v4();

    let ipc_request = IpcRequest {
        request_id,
        tool_name: hook_input.tool_name,
        tool_input: hook_input.tool_input,
        cwd: hook_input.cwd,
        session_id: hook_input.session_id,
        permission_suggestions: hook_input.permission_suggestions,
    };

    let socket_path = config.effective_socket_path();

    // Send to bot and wait for response (timeout handled by bot side)
    // Hook-side timeout is config.timeout_seconds + 30s buffer
    let ipc_timeout = config.timeout_seconds + 30;
    let response =
        crate::ipc::client::send_request(&socket_path, &ipc_request, ipc_timeout).await?;

    // Map IpcResponse to HookOutput
    let Some(output) = map_decision_to_output(&response) else {
        // Timeout — exit code 1, fall back to terminal
        std::process::exit(1);
    };

    // Write JSON to stdout
    let json = serde_json::to_string(&output)?;
    println!("{json}");

    Ok(())
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
            "User replied: please use pytest"
        );
    }

    #[test]
    fn map_reply_without_message() {
        let resp = make_response(Decision::Reply);
        let output = map_decision_to_output(&resp).unwrap();
        let json: serde_json::Value = serde_json::to_value(&output).unwrap();
        assert_eq!(
            json["hookSpecificOutput"]["decision"]["message"],
            "User replied: (no message)"
        );
    }

    #[test]
    fn map_timeout_returns_none() {
        let resp = make_response(Decision::Timeout);
        assert!(map_decision_to_output(&resp).is_none());
    }
}
