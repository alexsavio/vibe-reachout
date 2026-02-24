use crate::config::Config;
use crate::ipc::server::PendingMap;
use crate::models::IpcResponse;
use crate::telegram::callback_data::{CallbackAction, CallbackData};
use dashmap::DashMap;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{ChatId, MessageId};
use uuid::Uuid;

pub type ReplyState = Arc<DashMap<ChatId, (Uuid, MessageId)>>;

pub async fn handle_callback(
    bot: Bot,
    query: CallbackQuery,
    config: Arc<Config>,
    pending_map: PendingMap,
    reply_state: ReplyState,
) -> Result<(), teloxide::RequestError> {
    let Some(msg) = query.message.as_ref() else {
        tracing::warn!("Callback query with no associated message");
        return Ok(());
    };
    let chat_id = msg.chat().id;
    let query_id = query.id;

    // Authorization check
    if !config.allowed_chat_ids.contains(&chat_id.0) {
        tracing::warn!(chat_id = chat_id.0, "Unauthorized callback attempt");
        bot.answer_callback_query(query_id.clone())
            .text("Unauthorized")
            .show_alert(true)
            .await?;
        return Ok(());
    }

    let data = match query.data {
        Some(ref d) => d.clone(),
        None => return Ok(()),
    };

    // Parse callback data: "{uuid}:{action}"
    let Some(callback) = CallbackData::parse(&data) else {
        tracing::warn!(data = %data, "Failed to parse callback data");
        return Ok(());
    };

    let request_id = callback.request_id;

    // Handle reply action specially — don't resolve yet
    if callback.action == CallbackAction::Reply {
        bot.answer_callback_query(query_id.clone()).await?;

        if pending_map.contains_key(&request_id) {
            // Send ForceReply prompt
            let msg = bot
                .send_message(chat_id, "Type your reply:")
                .reply_markup(teloxide::types::ForceReply::new())
                .await?;

            reply_state.insert(chat_id, (request_id, msg.id));
        } else {
            bot.send_message(chat_id, "This request has already been handled.")
                .await?;
        }

        return Ok(());
    }

    // For allow/deny/always — resolve the pending request
    let Some((_, pending)) = pending_map.remove(&request_id) else {
        // Already handled
        bot.answer_callback_query(query_id.clone())
            .text("This request has already been handled")
            .show_alert(true)
            .await?;
        return Ok(());
    };

    bot.answer_callback_query(query_id.clone()).await?;

    let (response, status_text) =
        build_callback_response(callback.action, request_id, &pending.permission_suggestions);

    // Edit ALL sent messages to show status
    crate::bot::edit_messages_status(
        &bot,
        &pending.sent_messages,
        &pending.original_text,
        status_text,
    )
    .await;

    // Send response via oneshot channel
    let _ = pending.sender.send(response);

    Ok(())
}

/// Build the IPC response and status text for a callback action.
/// Extracted as a pure function for testability.
fn build_callback_response(
    action: CallbackAction,
    request_id: Uuid,
    permission_suggestions: &[serde_json::Value],
) -> (IpcResponse, &'static str) {
    match action {
        CallbackAction::Allow => (IpcResponse::allow(request_id), "\u{2705} Approved"),
        CallbackAction::Deny => (
            IpcResponse::deny(request_id, "Denied by user via Telegram".to_string()),
            "\u{274c} Denied",
        ),
        CallbackAction::Always => {
            let suggestion = permission_suggestions.first().cloned();
            (
                IpcResponse::always_allow(request_id, suggestion),
                "\u{1f513} Always Allowed",
            )
        }
        CallbackAction::Reply => unreachable!("Reply should be handled before calling this"),
    }
}

pub async fn handle_message(
    bot: Bot,
    msg: Message,
    config: Arc<Config>,
    pending_map: PendingMap,
    reply_state: ReplyState,
) -> Result<(), teloxide::RequestError> {
    let chat_id = msg.chat.id;

    // Authorization check
    if !config.allowed_chat_ids.contains(&chat_id.0) {
        tracing::warn!(chat_id = chat_id.0, "Unauthorized message attempt");
        return Ok(());
    }

    // Check if this is a reply to a ForceReply prompt
    let Some((_, (request_id, prompt_message_id))) = reply_state.remove(&chat_id) else {
        return Ok(()); // Not a reply we're tracking
    };

    let text = msg.text().unwrap_or("").trim().to_string();

    if text.is_empty() {
        // Re-prompt
        let new_msg = bot
            .send_message(chat_id, "Reply cannot be empty. Type your reply:")
            .reply_markup(teloxide::types::ForceReply::new())
            .await?;
        reply_state.insert(chat_id, (request_id, new_msg.id));
        return Ok(());
    }

    // Resolve the pending request
    let Some((_, pending)) = pending_map.remove(&request_id) else {
        bot.send_message(chat_id, "This request has already been handled.")
            .await?;
        return Ok(());
    };

    let response = IpcResponse::reply(request_id, text);

    // Edit ALL sent messages
    crate::bot::edit_messages_status(
        &bot,
        &pending.sent_messages,
        &pending.original_text,
        "\u{1f4ac} Replied",
    )
    .await;

    // Delete the ForceReply prompt message (best-effort)
    let _ = bot.delete_message(chat_id, prompt_message_id).await;

    // Send response via oneshot channel
    let _ = pending.sender.send(response);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Decision;

    #[test]
    fn build_response_allow() {
        let id = Uuid::new_v4();
        let (resp, status) = build_callback_response(CallbackAction::Allow, id, &[]);
        assert_eq!(resp.decision, Decision::Allow);
        assert_eq!(resp.request_id, id);
        assert!(status.contains("Approved"));
    }

    #[test]
    fn build_response_deny() {
        let id = Uuid::new_v4();
        let (resp, status) = build_callback_response(CallbackAction::Deny, id, &[]);
        assert_eq!(resp.decision, Decision::Deny);
        assert_eq!(resp.message.as_deref(), Some("Denied by user via Telegram"));
        assert!(status.contains("Denied"));
    }

    #[test]
    fn build_response_always_with_suggestion() {
        let id = Uuid::new_v4();
        let suggestions = vec![serde_json::json!({"tool": "Bash", "command": "ls"})];
        let (resp, status) = build_callback_response(CallbackAction::Always, id, &suggestions);
        assert_eq!(resp.decision, Decision::AlwaysAllow);
        assert_eq!(
            resp.always_allow_suggestion,
            Some(serde_json::json!({"tool": "Bash", "command": "ls"}))
        );
        assert!(status.contains("Always Allowed"));
    }

    #[test]
    fn build_response_always_without_suggestion() {
        let id = Uuid::new_v4();
        let (resp, _status) = build_callback_response(CallbackAction::Always, id, &[]);
        assert_eq!(resp.decision, Decision::AlwaysAllow);
        assert!(resp.always_allow_suggestion.is_none());
    }

    #[test]
    #[should_panic(expected = "Reply should be handled")]
    fn build_response_reply_panics() {
        let id = Uuid::new_v4();
        build_callback_response(CallbackAction::Reply, id, &[]);
    }
}
