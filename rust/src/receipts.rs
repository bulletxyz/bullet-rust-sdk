use serde_json::Value;

use crate::generated::types::SubmitTxResponse;

impl SubmitTxResponse {
    /// Return the Hyperlane message id emitted by a bridge withdrawal, when the
    /// trading API includes it in the transaction events.
    ///
    /// The OpenAPI response shape carries arbitrary event JSON rather than a
    /// dedicated message-id field, so this scans event values for common
    /// `messageId` / `message_id` spellings and returns a normalized `0x`-
    /// prefixed bytes32 hex string.
    pub fn message_id(&self) -> Option<String> {
        self.events.iter().find_map(|event| {
            find_message_id_in_map(&event.value, MessageIdSearchContext::default())
        })
    }
}

#[derive(Clone, Copy, Default)]
struct MessageIdSearchContext {
    allow_id_key: bool,
    allow_direct_string: bool,
}

impl MessageIdSearchContext {
    fn exact_message_id_value() -> Self {
        Self {
            allow_id_key: true,
            allow_direct_string: true,
        }
    }
}

fn find_message_id_in_map(
    map: &serde_json::Map<String, Value>,
    context: MessageIdSearchContext,
) -> Option<String> {
    for (key, value) in map {
        let key = normalize_key(key);
        let key_is_message_id = is_message_id_key(&key) || (context.allow_id_key && key == "id");
        if key_is_message_id
            && let Some(message_id) =
                message_id_from_value(value, MessageIdSearchContext::exact_message_id_value())
        {
            return Some(message_id);
        }
    }

    for (key, value) in map {
        let child_context = child_message_id_context(&normalize_key(key), context);
        if let Some(message_id) = message_id_from_value(value, child_context) {
            return Some(message_id);
        }
    }

    None
}

fn message_id_from_value(value: &Value, context: MessageIdSearchContext) -> Option<String> {
    match value {
        Value::String(value) if context.allow_direct_string => normalize_message_id(value),
        Value::Array(values) => values
            .iter()
            .find_map(|value| message_id_from_value(value, context)),
        Value::Object(map) => find_message_id_in_map(map, context),
        _ => None,
    }
}

fn normalize_message_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let raw = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    if raw.len() == 64 && raw.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(format!("0x{raw}"))
    } else {
        None
    }
}

fn is_message_id_key(key: &str) -> bool {
    matches!(
        key,
        "messageid"
            | "msgid"
            | "hyperlanemessageid"
            | "dispatchid"
            | "dispatchmessageid"
            | "mailboxmessageid"
    )
}

fn child_message_id_context(
    key: &str,
    parent_context: MessageIdSearchContext,
) -> MessageIdSearchContext {
    if matches!(key, "message" | "msg" | "hyperlanemessage") {
        return MessageIdSearchContext {
            allow_id_key: true,
            allow_direct_string: true,
        };
    }

    if matches!(key, "mailbox" | "hyperlane" | "hyperlanemailbox")
        || (parent_context.allow_id_key && key == "dispatch")
    {
        return MessageIdSearchContext {
            allow_id_key: true,
            allow_direct_string: false,
        };
    }

    MessageIdSearchContext::default()
}

fn normalize_key(key: &str) -> String {
    key.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::generated::types::{LedgerEvent, ModuleRef, SubmitTxResponse, TxStatus};

    fn response_with_value(value: serde_json::Value) -> SubmitTxResponse {
        let value = value
            .as_object()
            .expect("event value must be an object")
            .clone();

        SubmitTxResponse {
            events: vec![LedgerEvent {
                key: "dispatch".to_string(),
                module: ModuleRef {
                    name: "warp".to_string(),
                },
                number: 1,
                tx_hash: None,
                type_: "dispatch".to_string(),
                value,
            }],
            id: "0xtx".to_string(),
            receipt: None,
            status: TxStatus::Processed,
            tx_number: Some(7),
        }
    }

    #[test]
    fn message_id_extracts_direct_event_field() {
        let message_id = format!("0x{}", "ab".repeat(32));
        let response = response_with_value(json!({ "message_id": message_id }));

        assert_eq!(response.message_id(), Some(message_id));
    }

    #[test]
    fn message_id_extracts_nested_message_id() {
        let message_id = format!("0x{}", "cd".repeat(32));
        let response = response_with_value(json!({ "message": { "id": message_id } }));

        assert_eq!(response.message_id(), Some(message_id));
    }

    #[test]
    fn message_id_extracts_nested_mailbox_dispatch_id() {
        let message_id = format!("0x{}", "34".repeat(32));
        let response = response_with_value(json!({
            "mailbox": {
                "dispatch": {
                    "id": message_id,
                },
            },
        }));

        assert_eq!(response.message_id(), Some(message_id));
    }

    #[test]
    fn message_id_ignores_non_bytes32_ids() {
        let response = response_with_value(json!({ "id": "0xtx" }));

        assert_eq!(response.message_id(), None);
    }

    #[test]
    fn message_id_ignores_unrelated_message_suffixed_keys() {
        let unrelated_id = format!("0x{}", "ef".repeat(32));
        let response = response_with_value(json!({ "account_message_id": unrelated_id }));

        assert_eq!(response.message_id(), None);
    }

    #[test]
    fn message_id_ignores_bare_event_ids() {
        let unrelated_id = format!("0x{}", "12".repeat(32));
        let response = response_with_value(json!({ "id": unrelated_id }));

        assert_eq!(response.message_id(), None);
    }

    #[test]
    fn message_id_ignores_standalone_dispatch_ids() {
        let unrelated_id = format!("0x{}", "56".repeat(32));
        let response = response_with_value(json!({ "dispatch": { "id": unrelated_id } }));

        assert_eq!(response.message_id(), None);
    }
}
