use crate::config::Config;
use crate::ipc::server::PendingMap;
use crate::models::{Decision, IpcResponse};
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
    let chat_id = query
        .message
        .as_ref()
        .map(|m| m.chat().id)
        .unwrap_or(ChatId(0));

    // Authorization check
    if !config.allowed_chat_ids.contains(&chat_id.0) {
        tracing::warn!(chat_id = chat_id.0, "Unauthorized callback attempt");
        bot.answer_callback_query(&query.id)
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
    let parts: Vec<&str> = data.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Ok(());
    }

    let request_id = match Uuid::parse_str(parts[0]) {
        Ok(id) => id,
        Err(_) => return Ok(()),
    };
    let action = parts[1];

    // Handle reply action specially — don't resolve yet
    if action == "reply" {
        bot.answer_callback_query(&query.id).await?;

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
    let pending = match pending_map.remove(&request_id) {
        Some((_, p)) => p,
        None => {
            // Already handled
            bot.answer_callback_query(&query.id)
                .text("This request has already been handled")
                .show_alert(true)
                .await?;
            return Ok(());
        }
    };

    bot.answer_callback_query(&query.id).await?;

    let (response, status_text) = match action {
        "allow" => (
            IpcResponse {
                request_id,
                decision: Decision::Allow,
                message: None,
                user_message: None,
                always_allow_suggestion: None,
            },
            "\u{2705} Approved",
        ),
        "deny" => (
            IpcResponse {
                request_id,
                decision: Decision::Deny,
                message: Some("Denied by user via Telegram".to_string()),
                user_message: None,
                always_allow_suggestion: None,
            },
            "\u{274c} Denied",
        ),
        "always" => {
            let suggestion = pending
                .permission_suggestions
                .first()
                .cloned();
            (
                IpcResponse {
                    request_id,
                    decision: Decision::AlwaysAllow,
                    message: None,
                    user_message: None,
                    always_allow_suggestion: suggestion,
                },
                "\u{1f513} Always Allowed",
            )
        }
        _ => return Ok(()),
    };

    // Edit ALL sent messages to show status
    crate::bot::edit_messages_status(&bot, &pending.sent_messages, &pending.original_text, status_text).await;

    // Send response via oneshot channel
    let _ = pending.sender.send(response);

    Ok(())
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
    let (request_id, prompt_message_id) = match reply_state.remove(&chat_id) {
        Some((_, entry)) => entry,
        None => return Ok(()), // Not a reply we're tracking
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
    let pending = match pending_map.remove(&request_id) {
        Some((_, p)) => p,
        None => {
            bot.send_message(chat_id, "This request has already been handled.")
                .await?;
            return Ok(());
        }
    };

    let response = IpcResponse {
        request_id,
        decision: Decision::Reply,
        message: None,
        user_message: Some(text),
        always_allow_suggestion: None,
    };

    // Edit ALL sent messages
    crate::bot::edit_messages_status(&bot, &pending.sent_messages, &pending.original_text, "\u{1f4ac} Replied").await;

    // Delete the ForceReply prompt message (best-effort)
    let _ = bot.delete_message(chat_id, prompt_message_id).await;

    // Send response via oneshot channel
    let _ = pending.sender.send(response);

    Ok(())
}

