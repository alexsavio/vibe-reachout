use crate::config::Config;
use crate::models::{Decision, HookInput, HookOutput, IpcRequest};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

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
    let output = match response.decision {
        Decision::Allow => HookOutput::allow(),
        Decision::Deny => {
            let msg = response
                .message
                .unwrap_or_else(|| "Denied via Telegram".to_string());
            HookOutput::deny(msg)
        }
        Decision::AlwaysAllow => {
            let permissions = response
                .always_allow_suggestion
                .map(|s| vec![s])
                .unwrap_or_default();
            HookOutput::allow_always(permissions)
        }
        Decision::Reply => {
            let user_msg = response
                .user_message
                .unwrap_or_else(|| "(no message)".to_string());
            HookOutput::deny(format!("User replied: {user_msg}"))
        }
        Decision::Timeout => {
            // Exit code 1 — fall back to terminal
            std::process::exit(1);
        }
    };

    // Write JSON to stdout
    let json = serde_json::to_string(&output)?;
    println!("{json}");

    Ok(())
}
