//! The check that reads what AI assistants answered. It runs only when a
//! visibility run has completed.

use std::collections::BTreeMap;

use serde_json::json;

use super::context::{AiAnswerView, CrawlContext, Finding};

/// The most domains named in a finding's evidence.
const EVIDENCE_DOMAINS: usize = 10;

/// One finding per question where at least one engine answered and none of
/// the answers mentioned or cited the site. A question every engine failed on
/// says nothing, so it gets no finding. The target is the question in quotes.
pub fn run(ctx: &CrawlContext) -> Vec<Finding> {
    let Some(data) = ctx.ai.as_ref() else {
        return Vec::new();
    };
    let mut by_prompt: BTreeMap<&str, Vec<&AiAnswerView>> = BTreeMap::new();
    for answer in &data.answers {
        by_prompt
            .entry(answer.prompt.as_str())
            .or_default()
            .push(answer);
    }
    by_prompt
        .into_iter()
        .filter_map(|(prompt, answers)| unseen(prompt, &answers, &ctx.site_host))
        .collect()
}

fn unseen(prompt: &str, answers: &[&AiAnswerView], site_host: &str) -> Option<Finding> {
    let answered: Vec<&&AiAnswerView> = answers.iter().filter(|a| !a.failed).collect();
    if answered.is_empty() || answered.iter().any(|a| a.mentioned || a.cited) {
        return None;
    }
    let mut engines: Vec<&str> = answered.iter().map(|a| a.engine.as_str()).collect();
    engines.sort_unstable();
    engines.dedup();
    let mut instead: Vec<String> = Vec::new();
    let mut rivals: Vec<String> = Vec::new();
    for answer in &answered {
        for domain in &answer.cited_domains {
            let own = domain == site_host || domain.ends_with(&format!(".{site_host}"));
            if !own && !instead.contains(domain) {
                instead.push(domain.clone());
            }
        }
        for rival in &answer.competitors {
            if !rivals.contains(rival) {
                rivals.push(rival.clone());
            }
        }
    }
    instead.truncate(EVIDENCE_DOMAINS);
    Some(Finding::site(
        "ai_not_mentioned",
        &format!("\"{prompt}\""),
        json!({ "engines": engines, "cited_instead": instead, "competitors": rivals }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::context::fixtures::{context, healthy};
    use crate::checks::context::AiData;

    fn answer(prompt: &str, engine: &str, mentioned: bool, failed: bool) -> AiAnswerView {
        AiAnswerView {
            prompt: prompt.into(),
            engine: engine.into(),
            mentioned,
            cited: false,
            failed,
            cited_domains: vec!["rival.com".into(), "example.com".into()],
            competitors: vec!["rival.com".into()],
        }
    }

    fn with(answers: Vec<AiAnswerView>) -> CrawlContext {
        CrawlContext {
            ai: Some(AiData { answers }),
            ..context(vec![healthy("https://example.com/")])
        }
    }

    #[test]
    fn nothing_runs_without_a_completed_run() {
        assert!(run(&context(vec![healthy("https://example.com/")])).is_empty());
    }

    #[test]
    fn a_question_nobody_mentioned_you_for_is_a_site_finding_with_the_evidence() {
        let found = run(&with(vec![
            answer("Best invoicing tool?", "claude", false, false),
            answer("Best invoicing tool?", "gemini", false, false),
        ]));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].target, "\"Best invoicing tool?\"");
        assert_eq!(found[0].target_kind.as_str(), "site");
        assert_eq!(
            found[0].evidence,
            json!({"engines": ["claude", "gemini"], "cited_instead": ["rival.com"], "competitors": ["rival.com"]})
        );
    }

    #[test]
    fn one_engine_mentioning_you_clears_the_question() {
        let mut cited = answer("q one here", "gemini", false, false);
        cited.cited = true;
        assert!(run(&with(vec![
            answer("q one here", "claude", false, false),
            cited
        ]))
        .is_empty());
        assert!(run(&with(vec![answer("q one here", "claude", true, false)])).is_empty());
    }

    #[test]
    fn failed_answers_are_ignored_and_a_question_with_only_failures_is_skipped() {
        let all_failed = with(vec![answer("q one here", "claude", false, true)]);
        assert!(run(&all_failed).is_empty());
        let some = run(&with(vec![
            answer("q one here", "claude", false, true),
            answer("q one here", "gemini", false, false),
        ]));
        assert_eq!(some[0].evidence["engines"], json!(["gemini"]));
    }
}
