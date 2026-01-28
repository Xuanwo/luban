use serde_json::Value;

pub(super) fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

pub(super) fn extract_content_array(value: &Value) -> Option<&Vec<Value>> {
    value
        .pointer("/message/content")
        .and_then(|v| v.as_array())
        .or_else(|| value.get("content").and_then(|v| v.as_array()))
}

pub(super) fn parse_tool_result_content(content: &Value) -> Value {
    if let Some(s) = content.as_str() {
        return Value::String(s.to_owned());
    }
    content.clone()
}

pub(super) fn tool_name_key(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

pub(super) fn extract_string_field(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(v) = value.get(*key)
            && let Some(s) = v.as_str()
        {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_owned());
            }
        }
    }
    None
}
