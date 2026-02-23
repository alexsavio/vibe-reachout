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

    let (response, status_text) = match callback.action {
        CallbackAction::Allow => (IpcResponse::allow(request_id), "\u{2705} Approved"),
        CallbackAction::Deny => (
            IpcResponse::deny(request_id, "Denied by user via Telegram".to_string()),
            "\u{274c} Denied",
        ),
        CallbackAction::Always => {
            let suggestion = pending.permission_suggestions.first().cloned();
            (
                IpcResponse::always_allow(request_id, suggestion),
                "\u{1f513} Always Allowed",
            )
        }
        CallbackAction::Reply => unreachable!("Reply handled above"),
    };

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
