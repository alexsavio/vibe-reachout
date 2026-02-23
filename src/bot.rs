use crate::config::Config;
use crate::ipc::server::{self, PendingMap};
use crate::models::{IpcRequest, SentMessage};
use crate::telegram::handler::{self, ReplyState};
use dashmap::DashMap;
use std::sync::Arc;
use teloxide::dispatching::UpdateFilterExt;
use teloxide::dptree;
use teloxide::prelude::*;
use tokio_util::sync::CancellationToken;

pub async fn run_bot(config: Config) -> anyhow::Result<()> {
    let socket_path = config.effective_socket_path();

    // Check for existing bot
    server::detect_and_clean_stale_socket(&socket_path)?;

    let bot = Bot::new(&config.telegram_bot_token);
    let config = Arc::new(config);
    let pending_map: PendingMap = Arc::new(DashMap::new());
    let reply_state: ReplyState = Arc::new(DashMap::new());
    let cancel_token = CancellationToken::new();

    spawn_signal_handler(cancel_token.clone());

    let handler = {
        let callback_handler = Update::filter_callback_query().endpoint({
            let config = config.clone();
            let pending = pending_map.clone();
            let reply = reply_state.clone();
            move |bot: Bot, query: CallbackQuery| {
                let config = config.clone();
                let pending = pending.clone();
                let reply = reply.clone();
                async move { handler::handle_callback(bot, query, config, pending, reply).await }
            }
        });

        let message_handler = Update::filter_message().endpoint({
            let config = config.clone();
            let pending = pending_map.clone();
            let reply = reply_state;
            move |bot: Bot, msg: Message| {
                let config = config.clone();
                let pending = pending.clone();
                let reply = reply.clone();
                async move { handler::handle_message(bot, msg, config, pending, reply).await }
            }
        });

        dptree::entry()
            .branch(callback_handler)
            .branch(message_handler)
    };

    let cancel_for_dispatcher = cancel_token.clone();

    // Run socket server and Telegram dispatcher concurrently
    let socket_server = server::run_server(
        &socket_path,
        cancel_token.clone(),
        bot.clone(),
        config.clone(),
        pending_map.clone(),
    );

    let dispatcher = async {
        Box::pin(
            Dispatcher::builder(bot.clone(), handler)
                .enable_ctrlc_handler()
                .build()
                .dispatch_with_listener(
                    teloxide::update_listeners::polling_default(bot.clone()).await,
                    LoggingErrorHandler::with_custom_text("Dispatcher error"),
                ),
        )
        .await;
    };

    tracing::info!("Bot started. Listening for permissions...");

    tokio::select! {
        result = socket_server => {
            if let Err(e) = result {
                tracing::error!("Socket server error: {e}");
            }
        }
        () = dispatcher => {
            tracing::info!("Telegram dispatcher stopped");
        }
        () = cancel_for_dispatcher.cancelled() => {
            tracing::info!("Shutdown signal received");
        }
    }

    drain_pending_requests(&pending_map);

    Ok(())
}

fn spawn_signal_handler(cancel_token: CancellationToken) {
    tokio::spawn(async move {
        let ctrl_c = tokio::signal::ctrl_c();
        #[cfg(unix)]
        {
            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("Failed to register SIGTERM handler");
            tokio::select! {
                () = async { ctrl_c.await.expect("ctrl_c failed"); } => {
                    tracing::info!("Received SIGINT, shutting down...");
                }
                _ = sigterm.recv() => {
                    tracing::info!("Received SIGTERM, shutting down...");
                }
            }
        }
        #[cfg(not(unix))]
        {
            ctrl_c.await.expect("Failed to register Ctrl+C handler");
            tracing::info!("Received Ctrl+C, shutting down...");
        }
        cancel_token.cancel();
    });
}

fn drain_pending_requests(pending_map: &PendingMap) {
    let keys: Vec<_> = pending_map.iter().map(|e| *e.key()).collect();
    for key in keys {
        if let Some((_, pending)) = pending_map.remove(&key) {
            tracing::info!(request_id = %key, "Resolving pending request as timeout on shutdown");
            let _ = pending.sender.send(crate::models::IpcResponse {
                request_id: key,
                decision: crate::models::Decision::Timeout,
                message: None,
                user_message: None,
                always_allow_suggestion: None,
            });
        }
    }
}

pub async fn send_permission_to_telegram(
    bot: &Bot,
    config: &Config,
    request: &IpcRequest,
) -> anyhow::Result<Vec<SentMessage>> {
    let text = crate::telegram::formatter::format_permission_message(request);
    let keyboard = crate::telegram::keyboard::make_keyboard(
        request.request_id,
        !request.permission_suggestions.is_empty(),
    );

    let mut sent_messages = Vec::new();

    for &chat_id in &config.allowed_chat_ids {
        let chat = ChatId(chat_id);
        match bot
            .send_message(chat, &text)
            .reply_markup(keyboard.clone())
            .await
        {
            Ok(msg) => {
                sent_messages.push(SentMessage {
                    chat_id: chat,
                    message_id: msg.id,
                });
            }
            Err(e) => {
                tracing::warn!(chat_id = chat_id, "Failed to send message: {e}");
            }
        }
    }

    if sent_messages.is_empty() {
        anyhow::bail!("Failed to send permission message to any chat");
    }

    Ok(sent_messages)
}

pub async fn edit_messages_status(
    bot: &Bot,
    sent_messages: &[SentMessage],
    original_text: &str,
    status: &str,
) {
    let new_text = format!("{original_text}\n\n{status}");
    for msg in sent_messages {
        if let Err(e) = bot
            .edit_message_text(msg.chat_id, msg.message_id, &new_text)
            .await
        {
            tracing::warn!(
                chat_id = msg.chat_id.0,
                message_id = msg.message_id.0,
                "Failed to edit message: {e}"
            );
        }
    }
}
