//! What the engines answered: storing, reading, and the numbers drawn from it.

use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use super::detect::on_domain;
use super::engines::Citation;
use crate::checks::context::{AiAnswerView, AiData};
use crate::error::AppError;

fn internal(err: sqlx::Error) -> AppError {
    AppError::Internal(err.into())
}

/// An answer, or the reason there is none, ready to store.
pub struct NewAnswer {
    pub workspace_id: Uuid,
    pub run_id: Uuid,
    pub site_id: Uuid,
    pub prompt_id: Uuid,
    pub prompt_text: String,
    pub engine: String,
    pub model: String,
    pub answer_text: String,
    pub citations: Vec<Citation>,
    pub mentioned: bool,
    pub cited: bool,
    pub citation_rank: Option<i32>,
    pub competitor_mentions: Vec<String>,
    pub notes: Vec<String>,
    pub error: Option<String>,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub searches: i32,
}

fn citations_json(citations: &[Citation]) -> Value {
    Value::Array(
        citations
            .iter()
            .map(|c| serde_json::json!({ "url": c.url, "title": c.title, "domain": c.domain }))
            .collect(),
    )
}

/// Stores one answer. A pair already stored for the run is left as it is, so
/// a retried run can ask again without doubling anything.
pub async fn store(pool: &PgPool, answer: &NewAnswer) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO ai_answers
            (id, workspace_id, run_id, site_id, prompt_id, prompt_text, engine, model,
             answer_text, citations, mentioned, cited, citation_rank, competitor_mentions,
             notes, error, input_tokens, output_tokens, searches)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
         ON CONFLICT (run_id, prompt_id, engine) DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(answer.workspace_id)
    .bind(answer.run_id)
    .bind(answer.site_id)
    .bind(answer.prompt_id)
    .bind(&answer.prompt_text)
    .bind(&answer.engine)
    .bind(&answer.model)
    .bind(&answer.answer_text)
    .bind(citations_json(&answer.citations))
    .bind(answer.mentioned)
    .bind(answer.cited)
    .bind(answer.citation_rank)
    .bind(serde_json::json!(answer.competitor_mentions))
    .bind(serde_json::json!(answer.notes))
    .bind(&answer.error)
    .bind(answer.input_tokens)
    .bind(answer.output_tokens)
    .bind(answer.searches)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

// ----------------------------------------------------------------- reading

#[derive(Debug, Serialize)]
pub struct AnswerOut {
    pub id: Uuid,
    pub prompt_id: Uuid,
    pub prompt: String,
    pub engine: String,
    pub model: String,
    pub mentioned: bool,
    pub cited: bool,
    pub citation_rank: Option<i32>,
    pub cited_domains: Vec<String>,
    pub competitor_mentions: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CitationOut {
    pub url: String,
    pub title: Option<String>,
    pub domain: String,
}

#[derive(Debug, Serialize)]
pub struct AnswerDetail {
    #[serde(flatten)]
    pub answer: AnswerOut,
    pub answer_text: String,
    pub citations: Vec<CitationOut>,
    pub notes: Vec<String>,
}

type AnswerRow = (
    Uuid,
    Uuid,
    String,
    String,
    String,
    bool,
    bool,
    Option<i32>,
    Value,
    Value,
    Option<String>,
    String,
    Value,
);

macro_rules! answer_columns {
    () => {
        "id, prompt_id, prompt_text, engine, model, mentioned, cited, citation_rank, \
         citations, competitor_mentions, error, answer_text, notes"
    };
}

fn strings(value: Value) -> Vec<String> {
    serde_json::from_value(value).unwrap_or_default()
}

fn citations_of(value: Value) -> Vec<CitationOut> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| {
            Some(CitationOut {
                url: c.get("url")?.as_str()?.to_string(),
                title: c.get("title").and_then(Value::as_str).map(str::to_string),
                domain: c
                    .get("domain")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            })
        })
        .collect()
}

/// The domains of the citations, each once, in citation order.
fn domains_of(citations: &[CitationOut]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for citation in citations {
        if !citation.domain.is_empty() && !seen.contains(&citation.domain) {
            seen.push(citation.domain.clone());
        }
    }
    seen
}

fn detail_of(row: AnswerRow) -> AnswerDetail {
    let citations = citations_of(row.8);
    AnswerDetail {
        answer: AnswerOut {
            id: row.0,
            prompt_id: row.1,
            prompt: row.2,
            engine: row.3,
            model: row.4,
            mentioned: row.5,
            cited: row.6,
            citation_rank: row.7,
            cited_domains: domains_of(&citations),
            competitor_mentions: strings(row.9),
            error: row.10,
        },
        answer_text: row.11,
        citations,
        notes: strings(row.12),
    }
}

pub async fn for_run(
    pool: &PgPool,
    workspace_id: Uuid,
    run_id: Uuid,
) -> Result<Vec<AnswerOut>, AppError> {
    let rows = sqlx::query_as::<_, AnswerRow>(concat!(
        "SELECT ",
        answer_columns!(),
        " FROM ai_answers
         WHERE workspace_id = $1 AND run_id = $2
         ORDER BY prompt_text, CASE engine WHEN 'claude' THEN 1 WHEN 'chatgpt' THEN 2
                  WHEN 'grok' THEN 3 WHEN 'gemini' THEN 4 WHEN 'perplexity' THEN 5
                  ELSE 6 END, id"
    ))
    .bind(workspace_id)
    .bind(run_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(rows.into_iter().map(|row| detail_of(row).answer).collect())
}

pub async fn get(pool: &PgPool, workspace_id: Uuid, id: Uuid) -> Result<AnswerDetail, AppError> {
    sqlx::query_as::<_, AnswerRow>(concat!(
        "SELECT ",
        answer_columns!(),
        " FROM ai_answers WHERE id = $1 AND workspace_id = $2"
    ))
    .bind(id)
    .bind(workspace_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .map(detail_of)
    .ok_or(AppError::NotFound)
}

// ----------------------------------------------------------------- domains

#[derive(Debug, Serialize)]
pub struct DomainOut {
    pub domain: String,
    pub citations: i64,
    pub answers: i64,
    pub is_you: bool,
    pub is_competitor: bool,
}

/// The most cited domains across a run's answers, most cited first.
pub async fn top_domains(
    pool: &PgPool,
    workspace_id: Uuid,
    run_id: Uuid,
    site_host: &str,
    competitors: &[String],
) -> Result<Vec<DomainOut>, AppError> {
    let rows: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT c->>'domain' AS domain, COUNT(*), COUNT(DISTINCT a.id)
         FROM ai_answers a, jsonb_array_elements(a.citations) c
         WHERE a.workspace_id = $1 AND a.run_id = $2 AND a.error IS NULL
           AND COALESCE(c->>'domain', '') <> ''
         GROUP BY 1 ORDER BY 2 DESC, 3 DESC, 1 LIMIT 20",
    )
    .bind(workspace_id)
    .bind(run_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(rows
        .into_iter()
        .map(|(domain, citations, answers)| DomainOut {
            is_you: on_domain(&domain, site_host),
            is_competitor: competitors.iter().any(|c| on_domain(&domain, c)),
            domain,
            citations,
            answers,
        })
        .collect())
}

// ----------------------------------------------------------------- metrics

/// One engine's answers without an error in a run: how many, and how many
/// mentioned or cited the site.
#[derive(Debug)]
pub struct EngineTally {
    pub engine: String,
    pub answers: i64,
    pub mentions: i64,
    pub citations: i64,
    pub visible: i64,
}

pub async fn tally(
    pool: &PgPool,
    workspace_id: Uuid,
    run_id: Uuid,
) -> Result<Vec<EngineTally>, AppError> {
    let rows: Vec<(String, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT engine, COUNT(*),
                COUNT(*) FILTER (WHERE mentioned),
                COUNT(*) FILTER (WHERE cited),
                COUNT(*) FILTER (WHERE mentioned OR cited)
         FROM ai_answers
         WHERE workspace_id = $1 AND run_id = $2 AND error IS NULL
         GROUP BY engine
         ORDER BY MIN(CASE engine WHEN 'claude' THEN 1 WHEN 'chatgpt' THEN 2
                      WHEN 'grok' THEN 3 WHEN 'gemini' THEN 4 WHEN 'perplexity' THEN 5
                      ELSE 6 END)",
    )
    .bind(workspace_id)
    .bind(run_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(rows
        .into_iter()
        .map(
            |(engine, answers, mentions, citations, visible)| EngineTally {
                engine,
                answers,
                mentions,
                citations,
                visible,
            },
        )
        .collect())
}

// ------------------------------------------------------------------ checks

/// The answers of the latest completed run, for the questions still on the
/// list. `None` when no run has completed.
pub async fn load_ai_data(
    pool: &PgPool,
    workspace_id: Uuid,
    site_id: Uuid,
) -> Result<Option<AiData>, AppError> {
    let Some(run) = super::repo::latest_completed(pool, workspace_id, site_id).await? else {
        return Ok(None);
    };
    let rows: Vec<(String, String, bool, bool, bool, Value, Value)> = sqlx::query_as(
        "SELECT a.prompt_text, a.engine, a.mentioned, a.cited, a.error IS NOT NULL,
                a.citations, a.competitor_mentions
         FROM ai_answers a
         WHERE a.workspace_id = $1 AND a.run_id = $2
           AND a.prompt_id IN (SELECT id FROM ai_prompts WHERE site_id = $3)
         ORDER BY a.prompt_text, a.engine",
    )
    .bind(workspace_id)
    .bind(run.id)
    .bind(site_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    let answers = rows
        .into_iter()
        .map(
            |(prompt, engine, mentioned, cited, failed, citations, competitors)| AiAnswerView {
                prompt,
                engine,
                mentioned,
                cited,
                failed,
                cited_domains: domains_of(&citations_of(citations)),
                competitors: strings(competitors),
            },
        )
        .collect();
    Ok(Some(AiData { answers }))
}
