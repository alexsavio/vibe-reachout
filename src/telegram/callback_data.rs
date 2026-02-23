use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallbackAction {
    Allow,
    Deny,
    Reply,
    Always,
}

#[derive(Debug)]
pub struct CallbackData {
    pub request_id: Uuid,
    pub action: CallbackAction,
}

impl CallbackData {
    pub fn parse(data: &str) -> Option<Self> {
        let (id_str, action_str) = data.split_once(':')?;
        let request_id = Uuid::parse_str(id_str).ok()?;
        let action = match action_str {
            "allow" => CallbackAction::Allow,
            "deny" => CallbackAction::Deny,
            "reply" => CallbackAction::Reply,
            "always" => CallbackAction::Always,
            _ => return None,
        };
        Some(Self { request_id, action })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_allow() {
        let id = Uuid::new_v4();
        let data = format!("{id}:allow");
        let parsed = CallbackData::parse(&data).unwrap();
        assert_eq!(parsed.request_id, id);
        assert_eq!(parsed.action, CallbackAction::Allow);
    }

    #[test]
    fn parse_valid_deny() {
        let id = Uuid::new_v4();
        let data = format!("{id}:deny");
        let parsed = CallbackData::parse(&data).unwrap();
        assert_eq!(parsed.action, CallbackAction::Deny);
    }

    #[test]
    fn parse_valid_reply() {
        let id = Uuid::new_v4();
        let data = format!("{id}:reply");
        let parsed = CallbackData::parse(&data).unwrap();
        assert_eq!(parsed.action, CallbackAction::Reply);
    }

    #[test]
    fn parse_valid_always() {
        let id = Uuid::new_v4();
        let data = format!("{id}:always");
        let parsed = CallbackData::parse(&data).unwrap();
        assert_eq!(parsed.action, CallbackAction::Always);
    }

    #[test]
    fn parse_unknown_action_returns_none() {
        let id = Uuid::new_v4();
        let data = format!("{id}:unknown");
        assert!(CallbackData::parse(&data).is_none());
    }

    #[test]
    fn parse_invalid_uuid_returns_none() {
        assert!(CallbackData::parse("not-a-uuid:allow").is_none());
    }

    #[test]
    fn parse_no_colon_returns_none() {
        assert!(CallbackData::parse("justadata").is_none());
    }

    #[test]
    fn parse_empty_returns_none() {
        assert!(CallbackData::parse("").is_none());
    }
}
