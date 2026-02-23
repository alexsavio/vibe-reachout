use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};
use uuid::Uuid;

pub fn make_keyboard(request_id: Uuid, has_permission_suggestions: bool) -> InlineKeyboardMarkup {
    let id = request_id.to_string();

    let mut buttons = vec![
        InlineKeyboardButton::callback("\u{2705} Allow", format!("{id}:allow")),
        InlineKeyboardButton::callback("\u{274c} Deny", format!("{id}:deny")),
        InlineKeyboardButton::callback("\u{1f4ac} Reply", format!("{id}:reply")),
    ];

    if has_permission_suggestions {
        buttons.push(InlineKeyboardButton::callback(
            "\u{1f513} Always Allow",
            format!("{id}:always"),
        ));
    }

    InlineKeyboardMarkup::new(vec![buttons])
}
