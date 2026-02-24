use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};
use uuid::Uuid;

pub fn make_keyboard(request_id: Uuid, has_permission_suggestions: bool) -> InlineKeyboardMarkup {
    let id = request_id.to_string();
    // Telegram limits callback_data to 64 bytes. UUID (36) + ":" (1) + "always" (6) = 43.
    debug_assert!(
        id.len() + ":always".len() <= 64,
        "callback data exceeds Telegram 64-byte limit"
    );

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_without_suggestions_has_3_buttons() {
        let id = Uuid::new_v4();
        let kb = make_keyboard(id, false);
        let buttons = &kb.inline_keyboard[0];
        assert_eq!(buttons.len(), 3);
    }

    #[test]
    fn keyboard_with_suggestions_has_4_buttons() {
        let id = Uuid::new_v4();
        let kb = make_keyboard(id, true);
        let buttons = &kb.inline_keyboard[0];
        assert_eq!(buttons.len(), 4);
    }

    #[test]
    fn button_callback_data_format() {
        let id = Uuid::new_v4();
        let kb = make_keyboard(id, true);
        let buttons = &kb.inline_keyboard[0];

        let id_str = id.to_string();
        for (button, expected_action) in buttons.iter().zip(["allow", "deny", "reply", "always"]) {
            let expected_data = format!("{id_str}:{expected_action}");
            // InlineKeyboardButton callback_data is in the kind field
            match &button.kind {
                teloxide::types::InlineKeyboardButtonKind::CallbackData(data) => {
                    assert_eq!(data, &expected_data);
                }
                _ => panic!("Expected CallbackData button kind"),
            }
        }
    }
}
