//! Question suggestions: one Claude request without web search, answering
//! with structured JSON.

use serde_json::{json, Value};

use super::detect;
use super::engines::{ClaudeEngine, EngineError};
use crate::crawl::clean::strip_nul;

pub const SUGGESTION_COUNT: usize = 10;
pub const MIN_PROMPT_CHARS: usize = 10;
pub const MAX_PROMPT_CHARS: usize = 300;

fn schema() -> Value {
    json!({
        "type": "object",
        "properties": { "questions": { "type": "array", "items": { "type": "string" } } },
        "required": ["questions"],
        "additionalProperties": false,
    })
}

fn request_text(site_title: &str, description: &str, keywords: &[String], brand: &str) -> String {
    let keywords = if keywords.is_empty() {
        "none given".to_string()
    } else {
        keywords.join(", ")
    };
    let brand_rule = if brand.trim().is_empty() {
        String::new()
    } else {
        format!(" Never use the name \"{}\".", brand.trim())
    };
    format!(
        "Write {SUGGESTION_COUNT} questions that a potential customer would ask an AI assistant \
         when looking for what this website offers. Write them the way people talk to an \
         assistant, one question each, between {MIN_PROMPT_CHARS} and {MAX_PROMPT_CHARS} \
         characters, without naming the company or brand.{brand_rule}\n\n\
         Website: {site_title}\nDescription: {description}\nTarget keywords: {keywords}"
    )
}

/// The questions that can be added as they are: trimmed, a sane length, each
/// once, none naming the brand, at most ten.
pub fn clean_questions(raw: &[String], brand: &str) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();
    for question in raw {
        let question = strip_nul(question)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let length = question.chars().count();
        let fits = (MIN_PROMPT_CHARS..=MAX_PROMPT_CHARS).contains(&length);
        let seen = kept
            .iter()
            .any(|k| k.to_lowercase() == question.to_lowercase());
        if fits && !seen && !detect::mentioned(&question, brand, &[], "") {
            kept.push(question);
        }
        if kept.len() == SUGGESTION_COUNT {
            break;
        }
    }
    kept
}

pub async fn suggest_prompts(
    claude: &ClaudeEngine,
    site_title: &str,
    description: &str,
    keywords: &[String],
    brand: &str,
) -> Result<Vec<String>, EngineError> {
    let text = claude
        .complete_json(
            &request_text(site_title, description, keywords, brand),
            &schema(),
        )
        .await?;
    let parsed: Value = serde_json::from_str(&text).map_err(|_| EngineError::Unreadable)?;
    let questions: Vec<String> = parsed
        .get("questions")
        .and_then(Value::as_array)
        .ok_or(EngineError::Unreadable)?
        .iter()
        .filter_map(|q| q.as_str().map(str::to_string))
        .collect();
    Ok(clean_questions(&questions, brand))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn questions_are_trimmed_deduped_and_bounded() {
        let raw: Vec<String> = [
            "  What is the best   invoicing tool for freelancers? ",
            "what is the best invoicing tool for freelancers?",
            "Too short",
            &"x".repeat(301),
            "How do I send invoices from my phone?",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(
            clean_questions(&raw, ""),
            vec![
                "What is the best invoicing tool for freelancers?",
                "How do I send invoices from my phone?"
            ]
        );
    }

    #[test]
    fn questions_naming_the_brand_are_dropped_but_substrings_are_kept() {
        let raw: Vec<String> = [
            "Is Acme any good for small teams?",
            "Which pineapple growers deliver nearby?",
            "What does acme charge per month?",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(
            clean_questions(&raw, "Acme"),
            vec!["Which pineapple growers deliver nearby?"]
        );
        assert_eq!(clean_questions(&raw, "apple").len(), 3);
    }

    #[test]
    fn at_most_ten_are_kept() {
        let raw: Vec<String> = (0..20)
            .map(|n| format!("Question number {n} here?"))
            .collect();
        assert_eq!(clean_questions(&raw, "").len(), SUGGESTION_COUNT);
    }

    #[test]
    fn the_request_asks_without_naming_the_brand() {
        let text = request_text("Site", "Desc", &["a".into(), "b".into()], "Acme");
        assert!(text.contains("Target keywords: a, b"));
        assert!(text.contains("Never use the name \"Acme\""));
        assert!(!request_text("S", "D", &[], "").contains("Never use"));
    }
}
