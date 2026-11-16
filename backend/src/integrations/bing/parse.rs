//! Reads Bing's JSON defensively. The documented shapes are old third-party
//! write-ups (`{"d": ...}` wrappers, PascalCase fields), so nothing here
//! assumes a field exists, and field names match in any case.

use serde_json::Value;

/// Keys that may hold the list inside an object-shaped `d`.
const LIST_KEYS: [&str; 5] = ["links", "details", "items", "results", "value"];

/// A field of a JSON object, matched without regard to case.
pub fn field<'a>(object: &'a Value, name: &str) -> Option<&'a Value> {
    object
        .as_object()?
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value)
}

/// The text of the first of these fields that is a non-empty string.
pub fn text(object: &Value, names: &[&str]) -> Option<String> {
    names
        .iter()
        .filter_map(|name| field(object, name)?.as_str())
        .map(str::trim)
        .find(|text| !text.is_empty())
        .map(str::to_string)
}

/// A non-negative whole number from a field that may be a number or text.
pub fn count(object: &Value, name: &str) -> Option<i64> {
    let value = field(object, name)?;
    let number = match value {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f.round() as i64)),
        Value::String(s) => s
            .trim()
            .parse::<i64>()
            .ok()
            .or_else(|| s.trim().parse::<f64>().ok().map(|f| f.round() as i64)),
        _ => None,
    }?;
    Some(number.max(0))
}

/// What `rows` found in a response.
#[derive(Debug, PartialEq)]
pub struct Rows<'a> {
    pub rows: Vec<&'a Value>,
    /// False when the body had no list anywhere we know to look.
    pub recognised: bool,
}

/// The list of rows in an unwrapped response: the value itself as an array,
/// or an array held by one of the usual fields of an object.
pub fn rows(body: &Value) -> Rows<'_> {
    let list = match body {
        Value::Array(rows) => Some(rows),
        Value::Object(map) => LIST_KEYS
            .iter()
            .find_map(|name| field(body, name).and_then(Value::as_array))
            .or_else(|| map.values().find_map(Value::as_array)),
        _ => None,
    };
    match list {
        Some(list) => Rows {
            rows: list.iter().collect(),
            recognised: true,
        },
        None => Rows {
            rows: Vec::new(),
            recognised: false,
        },
    }
}

/// Removes the `{"d": ...}` wrapper when there is one, and reads `d` as JSON
/// when it was sent as a string.
pub fn unwrap_body(body: Value) -> Value {
    let inner = match field(&body, "d") {
        Some(inner) => inner.clone(),
        None => body,
    };
    match inner {
        Value::String(text) => serde_json::from_str(&text).unwrap_or(Value::String(text)),
        other => other,
    }
}

/// The names of an object's fields, for the log. Never their values.
pub fn field_names(body: &Value) -> Vec<String> {
    body.as_object()
        .map(|map| map.keys().take(10).cloned().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fields_match_in_any_case() {
        let row = json!({ "AnchorText": "hi", "URL": " https://x.example/ " });
        assert_eq!(text(&row, &["anchortext"]).as_deref(), Some("hi"));
        assert_eq!(text(&row, &["url"]).as_deref(), Some("https://x.example/"));
        assert_eq!(text(&row, &["missing"]), None);
        assert_eq!(text(&json!("not an object"), &["url"]), None);
    }

    #[test]
    fn counts_read_numbers_and_numeric_text_and_never_go_negative() {
        let row = json!({ "a": 5, "b": "7", "c": 2.6, "d": "x", "e": -4, "f": " 3 ", "g": "1.5" });
        assert_eq!(count(&row, "A"), Some(5));
        assert_eq!(count(&row, "b"), Some(7));
        assert_eq!(count(&row, "c"), Some(3));
        assert_eq!(count(&row, "d"), None);
        assert_eq!(count(&row, "e"), Some(0));
        assert_eq!(count(&row, "f"), Some(3));
        assert_eq!(count(&row, "g"), Some(2));
        assert_eq!(count(&row, "none"), None);
    }

    #[test]
    fn the_wrapper_is_removed_and_a_string_d_is_read_as_json() {
        assert_eq!(unwrap_body(json!({ "d": [1] })), json!([1]));
        assert_eq!(unwrap_body(json!({ "D": { "x": 1 } })), json!({ "x": 1 }));
        assert_eq!(unwrap_body(json!([1])), json!([1]));
        assert_eq!(unwrap_body(json!({ "d": "[2]" })), json!([2]));
        assert_eq!(unwrap_body(json!({ "d": "plain" })), json!("plain"));
    }

    #[test]
    fn rows_come_from_an_array_or_a_known_or_any_array_field() {
        let list = json!([{ "a": 1 }]);
        assert_eq!(rows(&list).rows.len(), 1);
        let links = json!({ "Links": [1, 2], "TotalPages": 3 });
        assert_eq!(rows(&links).rows.len(), 2);
        let details = json!({ "TotalPages": 1, "Details": [1] });
        assert_eq!(rows(&details).rows.len(), 1);
        let odd = json!({ "Whatever": [1, 2, 3] });
        assert_eq!(rows(&odd).rows.len(), 3);
        for none in [json!("x"), json!(null), json!({ "Links": "no" }), json!(7)] {
            let found = rows(&none);
            assert!(found.rows.is_empty() && !found.recognised, "{none}");
        }
        assert!(rows(&json!([])).recognised);
    }

    #[test]
    fn field_names_never_include_values() {
        let names = field_names(&json!({ "Secret": "value", "Other": 1 }));
        assert!(names.contains(&"Secret".to_string()));
        assert!(!names.iter().any(|n| n == "value"));
        assert!(field_names(&json!([1])).is_empty());
    }
}
