//! Postgres refuses a NUL character in `text` and `\u0000` in `jsonb`, and a
//! hostile page can contain either. Everything crawled passes through here on
//! its way into the database, so one bad page cannot fail a whole crawl.

use std::borrow::Cow;

use serde_json::Value;

pub fn strip_nul(text: &str) -> Cow<'_, str> {
    if text.contains('\0') {
        Cow::Owned(text.replace('\0', ""))
    } else {
        Cow::Borrowed(text)
    }
}

pub fn strip_nul_owned(text: String) -> String {
    match strip_nul(&text) {
        Cow::Borrowed(_) => text,
        Cow::Owned(clean) => clean,
    }
}

pub fn strip_nul_json(value: &Value) -> Value {
    match value {
        Value::String(text) => Value::String(strip_nul(text).into_owned()),
        Value::Array(items) => Value::Array(items.iter().map(strip_nul_json).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, item)| (strip_nul(key).into_owned(), strip_nul_json(item)))
                .collect(),
        ),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn removes_nul_from_text_and_keeps_the_rest() {
        assert_eq!(strip_nul("a\0b\0"), "ab");
        assert!(matches!(strip_nul("clean"), Cow::Borrowed(_)));
    }

    #[test]
    fn removes_nul_from_every_json_string_and_key() {
        let dirty = json!({ "k\u{0}": ["a\u{0}", 1, null, { "x": "y\u{0}z" }] });
        assert_eq!(
            strip_nul_json(&dirty),
            json!({ "k": ["a", 1, null, { "x": "yz" }] })
        );
    }
}
