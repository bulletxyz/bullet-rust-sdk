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
            let context = mentions_message_id_source(&event.key)
                || mentions_message_id_source(&event.type_)
                || mentions_message_id_source(&event.module.name);
            find_message_id_in_map(&event.value, context)
        })
    }
}

fn find_message_id_in_map(map: &serde_json::Map<String, Value>, context: bool) -> Option<String> {
    for (key, value) in map {
        let key_is_message_id = is_message_id_key(key) || (context && normalize_key(key) == "id");
        if key_is_message_id && let Some(message_id) = message_id_from_value(value, true) {
            return Some(message_id);
        }
    }

    for (key, value) in map {
        let child_context = context || mentions_message_id_source(key);
        if let Some(message_id) = message_id_from_value(value, child_context) {
            return Some(message_id);
        }
    }

    None
}

fn message_id_from_value(value: &Value, context: bool) -> Option<String> {
    match value {
        Value::String(value) if context => normalize_message_id(value),
        Value::Array(values) => {
            values.iter().find_map(|value| message_id_from_value(value, context))
        }
        Value::Object(map) => find_message_id_in_map(map, context),
        _ => None,
    }
}

fn normalize_message_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let raw = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")).unwrap_or(trimmed);
    if raw.len() == 64 && raw.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(format!("0x{raw}"))
    } else {
        None
    }
}

fn is_message_id_key(key: &str) -> bool {
    let key = normalize_key(key);
    matches!(key.as_str(), "messageid" | "msgid" | "hyperlanemessageid")
        || (key.contains("message") && key.ends_with("id"))
}

fn mentions_message_id_source(key: &str) -> bool {
    let key = normalize_key(key);
    key.contains("message")
        || key.contains("hyperlane")
        || key.contains("dispatch")
        || key.contains("mailbox")
        || key == "msg"
}

fn normalize_key(key: &str) -> String {
    key.chars().filter(|c| c.is_ascii_alphanumeric()).flat_map(char::to_lowercase).collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::generated::types::{LedgerEvent, ModuleRef, SubmitTxResponse, TxStatus};

    fn response_with_value(value: serde_json::Value) -> SubmitTxResponse {
        let value = value.as_object().expect("event value must be an object").clone();

        SubmitTxResponse {
            events: vec![LedgerEvent {
                key: "dispatch".to_string(),
                module: ModuleRef { name: "warp".to_string() },
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
    fn message_id_ignores_non_bytes32_ids() {
        let response = response_with_value(json!({ "id": "0xtx" }));

        assert_eq!(response.message_id(), None);
    }
}
