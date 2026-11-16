//! AI visibility over the API: settings, questions, runs, answers and what
//! they feed. Every engine is the mock server.

mod common;

use common::ai::{claude_body, openai_body, KEYS, Q1, Q2};
use common::google::GoogleWorld;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

const HOME: &str = "https://example.com/";

async fn put_settings(w: &GoogleWorld, token: &str, body: Value) -> (u16, Value) {
    w.send(
        token,
        reqwest::Method::PUT,
        &w.ai_path("/settings"),
        Some(body),
    )
    .await
}

// ----------------------------------------------------------------- settings

#[sqlx::test]
async fn settings_start_empty_and_are_saved_tidied(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let (status, body) = w.get(&w.ai_path("/settings")).await;
    assert_eq!(status, 200);
    assert_eq!(
        body["data"],
        json!({"brand_name": "", "aliases": [], "competitors": []})
    );
    let (status, body) = put_settings(
        &w,
        "test-token",
        json!({
            "brand_name": "  Acme   Widgets ",
            "aliases": ["The Widget Co", "the widget co", "  "],
            "competitors": ["https://www.Rival.com/pricing?x=1", "rival.com", "other.io"]
        }),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let expected = json!({
        "brand_name": "Acme Widgets",
        "aliases": ["The Widget Co"],
        "competitors": ["rival.com", "other.io"]
    });
    assert_eq!(body["data"], expected);
    assert_eq!(w.get(&w.ai_path("/settings")).await.1["data"], expected);
}

#[sqlx::test]
async fn settings_over_their_limits_or_with_bad_domains_are_422(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let many = |n: usize, f: &dyn Fn(usize) -> String| -> Vec<String> { (0..n).map(f).collect() };
    let cases = [
        json!({"brand_name": "x".repeat(101)}),
        json!({"aliases": many(11, &|n| format!("alias{n}"))}),
        json!({"aliases": ["y".repeat(101)]}),
        json!({"competitors": many(11, &|n| format!("rival{n}.com"))}),
        json!({"competitors": ["not a domain"]}),
        json!({"competitors": ["nodot"]}),
    ];
    for case in cases {
        let (status, body) = put_settings(&w, "test-token", case.clone()).await;
        assert_eq!(status, 422, "{case}: {body}");
    }
    let ten = many(10, &|n| format!("rival{n}.com"));
    let (status, _) = put_settings(&w, "test-token", json!({"competitors": ten})).await;
    assert_eq!(status, 200, "ten is allowed");
}

// ------------------------------------------------------------------ prompts

#[sqlx::test]
async fn questions_are_added_listed_and_deleted(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let (status, body) = w.add_prompt(&format!("  {Q1}  ")).await;
    assert_eq!(status, 201, "{body}");
    assert_eq!(body["data"]["text"], Q1);
    let id = body["data"]["id"].as_str().unwrap().to_string();
    assert!(body["data"]["created_at"].as_str().unwrap().contains('T'));
    w.add_prompt(Q2).await;
    let (_, listed) = w.get(&w.ai_path("/prompts")).await;
    let texts: Vec<&str> = listed["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["text"].as_str().unwrap())
        .collect();
    assert_eq!(texts, [Q1, Q2]);
    let (status, _) = w
        .send(
            "test-token",
            reqwest::Method::DELETE,
            &format!("/api/v1/ai/prompts/{id}"),
            None,
        )
        .await;
    assert_eq!(status, 200);
    let (_, listed) = w.get(&w.ai_path("/prompts")).await;
    assert_eq!(listed["data"].as_array().unwrap().len(), 1);
    let (status, _) = w
        .send(
            "test-token",
            reqwest::Method::DELETE,
            &format!("/api/v1/ai/prompts/{id}"),
            None,
        )
        .await;
    assert_eq!(status, 404);
}

#[sqlx::test]
async fn a_question_must_be_ten_to_three_hundred_characters_and_new(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    for text in ["too short", "         nine    ", &"x".repeat(301), ""] {
        assert_eq!(w.add_prompt(text).await.0, 422, "{text:?}");
    }
    assert_eq!(w.add_prompt("exactly 10").await.0, 201);
    assert_eq!(
        w.add_prompt(&"é".repeat(300)).await.0,
        201,
        "length counts characters"
    );
    let (status, body) = w.add_prompt("EXACTLY  10").await;
    assert_eq!(status, 409, "{body}");
    assert_eq!(body["error"], "This question is already on the list.");
}

#[sqlx::test]
async fn a_site_holds_at_most_twenty_five_questions(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    for n in 0..25 {
        let (status, body) = w
            .add_prompt(&format!("Question number {n} for widgets"))
            .await;
        assert_eq!(status, 201, "{body}");
    }
    let (status, body) = w.add_prompt("One question too many here").await;
    assert_eq!(status, 409);
    assert_eq!(body["error"], "A site can have at most 25 questions.");
}

#[sqlx::test]
async fn adding_a_question_schedules_the_weekly_run(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    w.add_prompt(Q1).await;
    w.add_prompt(Q2).await;
    let jobs = w.queued_ai_jobs().await;
    assert_eq!(jobs.len(), 1, "one queued job per site");
    assert!(jobs[0].get("run_id").is_none());
    let run_at: time::OffsetDateTime =
        sqlx::query_scalar("SELECT run_at FROM jobs WHERE kind = 'ai_run'")
            .fetch_one(&w.pool)
            .await
            .unwrap();
    assert!(run_at > time::OffsetDateTime::now_utc() + time::Duration::days(6));
}

#[sqlx::test]
async fn without_an_engine_adding_a_question_schedules_nothing(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    w.add_prompt(Q1).await;
    assert!(w.queued_ai_jobs().await.is_empty());
}

// -------------------------------------------------------------- suggestions

fn suggestions_reply() -> Value {
    let mut body = claude_body("x", &[]);
    body["content"] = json!([{ "type": "text", "text": json!({"questions": [
        "Which widget shop ships to Canada quickly?",
        "Is Acme worth the price for widgets?",
        "How do I choose widgets for a small bakery?"
    ]}).to_string() }]);
    body
}

#[sqlx::test]
async fn suggestions_need_a_claude_key(pool: PgPool) {
    let w = GoogleWorld::new(pool).await;
    let (status, body) = w.post(&w.ai_path("/prompts/suggest")).await;
    assert_eq!(status, 422);
    assert!(body["error"].as_str().unwrap().contains("Settings"));
}

#[sqlx::test]
async fn suggestions_come_back_unsaved_without_the_brand(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    w.put_ai_settings("Acme", &[], &[]).await;
    w.add_keyword("blue widgets").await;
    w.mount("/claude/v1/messages", 200, suggestions_reply(), None)
        .await;
    let (status, body) = w.post(&w.ai_path("/prompts/suggest")).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        body["data"]["suggestions"],
        json!([
            "Which widget shop ships to Canada quickly?",
            "How do I choose widgets for a small bakery?"
        ])
    );
    let (_, listed) = w.get(&w.ai_path("/prompts")).await;
    assert!(listed["data"].as_array().unwrap().is_empty(), "not saved");
    let sent: Value =
        serde_json::from_slice(&w.requests_with("/claude/v1/messages").await[0].body).unwrap();
    let prompt = sent["messages"][0]["content"].as_str().unwrap();
    assert!(
        prompt.contains("blue widgets"),
        "keywords are given to Claude"
    );
}

#[sqlx::test]
async fn a_failing_claude_is_a_502_that_names_no_key(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    w.mount("/claude/v1/messages", 500, json!({"error": "boom"}), None)
        .await;
    let (status, body) = w.post(&w.ai_path("/prompts/suggest")).await;
    assert_eq!(status, 502);
    assert!(!body.to_string().contains(KEYS[0]));
}

// --------------------------------------------------------------------- runs

#[sqlx::test]
async fn a_run_needs_a_question(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    let (status, body) = w.start_run().await;
    assert_eq!(status, 422);
    assert!(body["error"].as_str().unwrap().contains("question"));
}

#[sqlx::test]
async fn a_run_needs_an_engine(pool: PgPool) {
    let none = GoogleWorld::new(pool).await;
    none.add_prompt(Q1).await;
    let (status, body) = none.start_run().await;
    assert_eq!(status, 422);
    assert!(body["error"].as_str().unwrap().contains("API key"));
}

#[sqlx::test]
async fn a_run_is_queued_once_with_its_id_in_the_job(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    w.add_prompt(Q1).await;
    w.add_prompt(Q2).await;
    let (status, body) = w.start_run().await;
    assert_eq!(status, 202, "{body}");
    let run = &body["data"];
    assert_eq!(run["status"], "queued");
    assert_eq!(run["trigger"], "manual");
    assert_eq!(
        run["engines"],
        json!(["claude", "chatgpt", "grok", "gemini", "perplexity"])
    );
    assert_eq!(run["answers_total"], 10);
    assert_eq!(run["answers_done"], 0);
    assert!(run["error"].is_null() && run["started_at"].is_null() && run["finished_at"].is_null());
    let queued: Vec<Value> = w
        .queued_ai_jobs()
        .await
        .into_iter()
        .filter(|p| p.get("run_id").is_some())
        .collect();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0]["run_id"], run["id"]);

    let (status, body) = w.start_run().await;
    assert_eq!(status, 409, "{body}");
}

#[sqlx::test]
async fn two_starts_at_once_leave_one_run(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    w.add_prompt(Q1).await;
    let (a, b) = tokio::join!(w.start_run(), w.start_run());
    let mut codes = [a.0, b.0];
    codes.sort_unstable();
    assert_eq!(codes, [202, 409]);
    let active: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM ai_runs WHERE status IN ('queued', 'running')")
            .fetch_one(&w.pool)
            .await
            .unwrap();
    assert_eq!(active, 1);
}

#[sqlx::test]
async fn the_database_refuses_a_second_active_run_for_a_site(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    let insert = |status: &'static str| {
        let pool = w.pool.clone();
        let (ws, site) = (w.workspace_id, w.site_id);
        async move {
            sqlx::query(
                "INSERT INTO ai_runs (id, workspace_id, site_id, status, trigger) VALUES ($1, $2, $3, $4, 'manual')",
            )
            .bind(Uuid::new_v4())
            .bind(ws)
            .bind(site)
            .bind(status)
            .execute(&pool)
            .await
        }
    };
    assert!(insert("running").await.is_ok());
    assert!(insert("queued").await.is_err());
    assert!(insert("completed").await.is_ok());
}

#[sqlx::test]
async fn the_fourth_manual_run_of_a_day_is_429(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    w.add_prompt(Q1).await;
    w.mock_engines("Some answer.", &[], None).await;
    for _ in 0..3 {
        let id = w.run_to_end().await;
        assert_eq!(w.ai_run(id).await["status"], "completed");
    }
    let (status, body) = w.start_run().await;
    assert_eq!(status, 429, "{body}");
    assert!(body["error"].as_str().unwrap().contains("3"));
    // A scheduled run does not count, and a run yesterday does not either.
    sqlx::query("UPDATE ai_runs SET created_at = now() - interval '2 days'")
        .execute(&w.pool)
        .await
        .unwrap();
    assert_eq!(w.start_run().await.0, 202);
}

// --------------------------------------------------------------- running it

/// Settings that make Acme the brand and Rival a competitor.
async fn acme(w: &GoogleWorld) {
    w.put_ai_settings("Acme", &["The Widget Co"], &["rival.com"])
        .await;
}

#[sqlx::test]
async fn a_run_asks_every_engine_every_question_and_judges_each_answer(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    acme(&w).await;
    w.add_prompt(Q1).await;
    w.add_prompt(Q2).await;
    // Q1: Claude names the brand and cites the site's blog; the rest cite rivals.
    w.mount(
        "/claude/v1/messages",
        200,
        claude_body(
            "Acme is a good shop.",
            &[
                ("https://other.com/", "Other"),
                ("https://blog.example.com/post", "Blog"),
            ],
        ),
        Some(Q1),
    )
    .await;
    w.mock_engines(
        "Rival is the usual pick.",
        &[("https://rival.com/w", "Rival")],
        None,
    )
    .await;
    let id = w.run_to_end().await;
    let run = w.ai_run(id).await;
    assert_eq!(run["status"], "completed");
    assert_eq!(
        (run["answers_total"].as_i64(), run["answers_done"].as_i64()),
        (Some(10), Some(10))
    );
    assert!(run["started_at"].is_string() && run["finished_at"].is_string());

    let answers = w.ai_answers(id).await;
    assert_eq!(answers.len(), 10);
    let pick = |prompt: &str, engine: &str| {
        answers
            .iter()
            .find(|a| a["prompt"] == prompt && a["engine"] == engine)
            .unwrap()
            .clone()
    };
    let claude = pick(Q1, "claude");
    assert_eq!(claude["mentioned"], true);
    assert_eq!(claude["cited"], true);
    assert_eq!(claude["citation_rank"], 2);
    assert_eq!(
        claude["cited_domains"],
        json!(["other.com", "blog.example.com"])
    );
    assert_eq!(claude["competitor_mentions"], json!([]));
    assert_eq!(claude["model"], "claude-opus-5-5");
    assert!(claude["error"].is_null());
    let grok = pick(Q1, "grok");
    assert!(grok["error"].is_null(), "{grok}");
    assert_eq!(grok["model"], "grok-4.7");
    assert_eq!(grok["mentioned"], false);
    assert_eq!(grok["competitor_mentions"], json!(["rival.com"]));
    let gemini = pick(Q1, "gemini");
    assert_eq!(
        (gemini["mentioned"].clone(), gemini["cited"].clone()),
        (json!(false), json!(false))
    );
    assert!(gemini["citation_rank"].is_null());
    assert_eq!(gemini["competitor_mentions"], json!(["rival.com"]));
    assert_eq!(
        pick(Q2, "claude")["mentioned"],
        false,
        "the Q1-only mock does not apply to Q2"
    );

    let (status, detail) = w
        .get(&format!(
            "/api/v1/ai/answers/{}",
            claude["id"].as_str().unwrap()
        ))
        .await;
    assert_eq!(status, 200);
    let detail = &detail["data"];
    assert!(detail["answer_text"]
        .as_str()
        .unwrap()
        .contains("Acme is a good shop."));
    assert_eq!(
        detail["citations"],
        json!([
            {"url": "https://other.com/", "title": "Other", "domain": "other.com"},
            {"url": "https://blog.example.com/post", "title": "Blog", "domain": "blog.example.com"}
        ])
    );
    assert_eq!(detail["prompt"], Q1);

    let row: (i32, i32, i32) = sqlx::query_as(
        "SELECT input_tokens, output_tokens, searches FROM ai_answers WHERE id = $1",
    )
    .bind(claude["id"].as_str().unwrap().parse::<Uuid>().unwrap())
    .fetch_one(&w.pool)
    .await
    .unwrap();
    assert_eq!(row, (1200, 340, 2));
}

#[sqlx::test]
async fn domains_rank_the_sites_cited_across_the_latest_run(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    acme(&w).await;
    w.add_prompt(Q1).await;
    let (status, body) = w.get(&w.ai_path("/domains")).await;
    assert_eq!((status, body["data"].clone()), (200, json!([])));
    w.mock_engines(
        "Answer.",
        &[
            ("https://rival.com/a", "R"),
            ("https://www.example.com/", "Me"),
            ("https://news.org/", "N"),
        ],
        None,
    )
    .await;
    w.run_to_end().await;
    let (_, body) = w.get(&w.ai_path("/domains")).await;
    let domains = body["data"].as_array().unwrap();
    assert_eq!(domains.len(), 3);
    let by = |name: &str| {
        domains
            .iter()
            .find(|d| d["domain"] == name)
            .unwrap()
            .clone()
    };
    assert_eq!(by("rival.com")["citations"], 5);
    assert_eq!(by("rival.com")["answers"], 5);
    assert_eq!(by("rival.com")["is_competitor"], true);
    assert_eq!(by("rival.com")["is_you"], false);
    assert_eq!(by("example.com")["is_you"], true);
    assert_eq!(by("news.org")["is_competitor"], false);
}

#[sqlx::test]
async fn one_engine_failing_never_fails_the_run_and_is_left_out_of_the_numbers(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    acme(&w).await;
    w.add_prompt(Q1).await;
    w.mount(
        "/openai/v1/responses",
        500,
        json!({"error": {"message": "overloaded"}}),
        None,
    )
    .await;
    w.mount(
        "/claude/v1/messages",
        200,
        claude_body("Acme is great.", &[]),
        None,
    )
    .await;
    w.mount(
        "/perplexity/chat/completions",
        200,
        common::ai::perplexity_body("Nothing relevant.", &[]),
        None,
    )
    .await;
    w.mount(
        "/gemini/v1beta/models/gemini-3.5-flash:generateContent",
        200,
        common::ai::gemini_body("Acme works.", &[]),
        None,
    )
    .await;
    let id = w.run_to_end().await;
    let run = w.ai_run(id).await;
    assert_eq!(run["status"], "completed");
    assert_eq!(run["answers_done"], 5);
    let answers = w.ai_answers(id).await;
    let chatgpt = answers.iter().find(|a| a["engine"] == "chatgpt").unwrap();
    assert_eq!(chatgpt["error"], "The engine answered with HTTP 500.");
    assert_eq!(chatgpt["mentioned"], false);
    let metrics = w.ai_metrics().await;
    assert_eq!(metrics["answers"], 3, "the failed answer is not counted");
    assert_eq!(metrics["mentions"], 2);
    assert_eq!(metrics["percent"], 66.7);
    assert!(metrics["reason"].is_null());
    let engines = metrics["engines"].as_array().unwrap();
    let names: Vec<&str> = engines
        .iter()
        .map(|e| e["engine"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["claude", "gemini", "perplexity"],
        "fixed order, failed engine absent"
    );
    assert_eq!(
        engines[1],
        json!({"engine": "gemini", "percent": 100.0, "mentions": 1, "answers": 1})
    );
    assert_eq!(
        engines[2],
        json!({"engine": "perplexity", "percent": 0.0, "mentions": 0, "answers": 1})
    );
    assert_eq!(engines[0]["percent"], 100.0);
}

#[sqlx::test]
async fn a_run_where_every_engine_fails_is_failed_and_says_so(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    w.add_prompt(Q1).await;
    // No engine is mocked: every call gets the mock server's 404.
    let id = w.run_to_end().await;
    let run = w.ai_run(id).await;
    assert_eq!(run["status"], "failed");
    assert_eq!(run["answers_done"], 5);
    assert!(run["error"]
        .as_str()
        .unwrap()
        .contains("No engine could answer"));
    let metrics = w.ai_metrics().await;
    assert!(metrics["percent"].is_null());
    assert!(metrics["reason"]
        .as_str()
        .unwrap()
        .contains("could not get an answer"));
    // A failed run never counts as having data for the check, and the site can try again.
    let (_, checks) = w.get(&format!("/api/v1/sites/{}/checks", w.site_id)).await;
    let entry = checks["data"]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["check_id"] == "ai_not_mentioned")
        .unwrap()
        .clone();
    assert_eq!(entry["status"], "not_run");
    assert_eq!(w.start_run().await.0, 202);
}

#[sqlx::test]
async fn a_slow_engine_becomes_an_answer_with_an_error_and_the_run_completes(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    w.add_prompt(Q1).await;
    w.mount("/claude/v1/messages", 200, claude_body("Fine.", &[]), None)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/openai/v1/responses"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_json(openai_body("late", &[]))
                .set_delay(std::time::Duration::from_secs(3)),
        )
        .mount(&w.mock)
        .await;
    let (_, body) = w.start_run().await;
    let id: Uuid = body["data"]["id"].as_str().unwrap().parse().unwrap();
    let handler = margincite::ai::run::AiRunJob::new(
        w.pool.clone(),
        std::sync::Arc::new(common::test_config(&common::ai::ai_settings())),
        w.ai_engines(std::time::Duration::from_millis(400)),
    );
    use margincite::jobs::worker::JobHandler;
    handler.run(&w.ai_job(Some(id))).await.unwrap();
    let answers = w.ai_answers(id).await;
    let chatgpt = answers.iter().find(|a| a["engine"] == "chatgpt").unwrap();
    assert_eq!(chatgpt["error"], "The engine did not answer in time.");
    assert_eq!(w.ai_run(id).await["status"], "completed");
}

#[sqlx::test]
async fn a_retried_job_does_not_ask_again_what_it_already_has(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    w.add_prompt(Q1).await;
    w.add_prompt(Q2).await;
    w.mock_engines("An answer.", &[], None).await;
    let (_, body) = w.start_run().await;
    let id: Uuid = body["data"]["id"].as_str().unwrap().parse().unwrap();
    let prompt_id: Uuid = sqlx::query_scalar("SELECT id FROM ai_prompts WHERE text = $1")
        .bind(Q1)
        .fetch_one(&w.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE ai_runs SET status = 'running' WHERE id = $1")
        .bind(id)
        .execute(&w.pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO ai_answers (id, workspace_id, run_id, site_id, prompt_id, prompt_text, engine, model, answer_text)
         VALUES ($1, $2, $3, $4, $5, $6, 'claude', 'm', 'kept')",
    )
    .bind(Uuid::new_v4())
    .bind(w.workspace_id)
    .bind(id)
    .bind(w.site_id)
    .bind(prompt_id)
    .bind(Q1)
    .execute(&w.pool)
    .await
    .unwrap();
    w.run_ai_job(Some(id)).await.unwrap();
    assert_eq!(
        w.requests_with("/claude/v1/messages").await.len(),
        1,
        "only Q2 was asked of Claude"
    );
    assert_eq!(w.requests_with("/openai/v1/responses").await.len(), 2);
    let run = w.ai_run(id).await;
    assert_eq!(
        (run["status"].clone(), run["answers_done"].clone()),
        (json!("completed"), json!(10))
    );
    // Running the finished job again does nothing.
    w.run_ai_job(Some(id)).await.unwrap();
    assert_eq!(w.requests_with("/openai/v1/responses").await.len(), 2);
}

#[sqlx::test]
async fn a_job_that_failed_for_good_frees_the_site_for_the_next_run(pool: PgPool) {
    use margincite::jobs::worker::JobHandler;
    let w = GoogleWorld::with_ai(pool).await;
    w.add_prompt(Q1).await;
    let (_, body) = w.start_run().await;
    let id: Uuid = body["data"]["id"].as_str().unwrap().parse().unwrap();
    w.ai_handler().on_dead(&w.ai_job(Some(id)), "boom").await;
    let run = w.ai_run(id).await;
    assert_eq!(run["status"], "failed");
    assert!(!run["error"].as_str().unwrap().contains("boom"));
    assert_eq!(w.start_run().await.0, 202);
}

#[sqlx::test]
async fn a_scheduled_job_makes_its_own_run_and_the_next_is_queued_after_it(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    // Nothing to ask yet: the job does nothing.
    w.run_ai_job(None).await.unwrap();
    let runs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ai_runs")
        .fetch_one(&w.pool)
        .await
        .unwrap();
    assert_eq!(runs, 0);
    w.add_prompt(Q1).await;
    w.mock_engines("Acme is great.", &[], None).await;
    w.put_ai_settings("Acme", &[], &[]).await;
    w.run_ai_job(None).await.unwrap();
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT trigger, status FROM ai_runs")
        .fetch_all(&w.pool)
        .await
        .unwrap();
    assert_eq!(rows, [("scheduled".to_string(), "completed".to_string())]);
    assert_eq!(
        w.queued_ai_jobs().await.len(),
        1,
        "the next weekly run is queued, once"
    );
    // A scheduled run does not use up a manual allowance.
    assert_eq!(w.start_run().await.0, 202);
}

#[sqlx::test]
async fn a_scheduled_job_leaves_a_run_already_under_way_alone(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    w.add_prompt(Q1).await;
    assert_eq!(w.start_run().await.0, 202);
    w.run_ai_job(None).await.unwrap();
    let runs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ai_runs")
        .fetch_one(&w.pool)
        .await
        .unwrap();
    assert_eq!(runs, 1);
    assert!(w.requests_with("/claude/v1/messages").await.is_empty());
}

#[sqlx::test]
async fn stored_text_is_cut_and_cleaned_and_no_key_reaches_a_row_or_a_response(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    w.add_prompt(Q1).await;
    let long = format!("Acme {}\u{0}end", "a".repeat(30_000));
    w.mount(
        "/claude/v1/messages",
        200,
        claude_body(&long, &[("https://x.com/\u{0}p", "T\u{0}")]),
        None,
    )
    .await;
    // The other engines refuse the key and echo it back.
    for (path, key) in [
        ("/openai/v1/responses", KEYS[1]),
        ("/perplexity/chat/completions", KEYS[2]),
        (
            "/gemini/v1beta/models/gemini-3.5-flash:generateContent",
            KEYS[3],
        ),
    ] {
        w.mount(
            path,
            401,
            json!({"error": {"message": format!("bad key {key}")}}),
            None,
        )
        .await;
    }
    let id = w.run_to_end().await;
    let answers = w.ai_answers(id).await;
    let claude = answers.iter().find(|a| a["engine"] == "claude").unwrap();
    let (_, detail) = w
        .get(&format!(
            "/api/v1/ai/answers/{}",
            claude["id"].as_str().unwrap()
        ))
        .await;
    let text = detail["data"]["answer_text"].as_str().unwrap();
    assert_eq!(text.chars().count(), 20_000);
    assert!(!text.contains('\u{0}'));
    assert_eq!(detail["data"]["citations"][0]["url"], "https://x.com/p");
    let rejected = answers.iter().find(|a| a["engine"] == "chatgpt").unwrap();
    assert_eq!(
        rejected["error"],
        "The engine rejected the API key (HTTP 401)."
    );

    let mut everything = serde_json::to_string(&answers).unwrap();
    everything.push_str(&detail.to_string());
    everything.push_str(&w.ai_run(id).await.to_string());
    let rows: Vec<(String, Option<String>, Value, Value)> =
        sqlx::query_as("SELECT answer_text, error, citations, notes FROM ai_answers")
            .fetch_all(&w.pool)
            .await
            .unwrap();
    everything.push_str(&format!("{rows:?}"));
    for key in KEYS {
        assert!(!everything.contains(key), "{key} leaked");
    }
    let (_, integrations) = w.get("/api/v1/integrations").await;
    for key in KEYS {
        assert!(!integrations.to_string().contains(key));
    }
}

// ---------------------------------------------------------- the check itself

async fn item_statuses(w: &GoogleWorld) -> Vec<(String, String)> {
    let (_, list) = w
        .get(&format!(
            "/api/v1/sites/{}/suggestions?status=all",
            w.site_id
        ))
        .await;
    let Some(found) = list["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["check_id"] == "ai_not_mentioned")
    else {
        return Vec::new();
    };
    let id = found["id"].as_str().unwrap();
    let (_, detail) = w.get(&format!("/api/v1/suggestions/{id}?status=all")).await;
    detail["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            (
                i["target"].as_str().unwrap().to_string(),
                i["status"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

async fn check_entry(w: &GoogleWorld) -> Value {
    let (_, checks) = w.get(&format!("/api/v1/sites/{}/checks", w.site_id)).await;
    checks["data"]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["check_id"] == "ai_not_mentioned")
        .unwrap()
        .clone()
}

#[sqlx::test]
async fn ai_not_mentioned_is_not_run_before_a_run_then_fires_per_question_and_resolves(
    pool: PgPool,
) {
    let w = GoogleWorld::with_ai(pool).await;
    w.crawl(&[(HOME, true)]).await;
    acme(&w).await;
    w.add_prompt(Q1).await;
    w.add_prompt(Q2).await;
    let before = check_entry(&w).await;
    assert_eq!(before["status"], "not_run");
    assert_eq!(before["reason"], "Run an AI visibility check to see this.");
    assert!(item_statuses(&w).await.is_empty());

    // Nobody mentions the site for Q1; only Perplexity does for Q2.
    w.mount(
        "/perplexity/chat/completions",
        200,
        common::ai::perplexity_body("Try Acme.", &[]),
        Some(Q2),
    )
    .await;
    w.mock_engines(
        "Rival is the usual pick.",
        &[("https://rival.com/w", "Rival")],
        None,
    )
    .await;
    w.run_to_end().await;

    let entry = check_entry(&w).await;
    assert_eq!(entry["status"], "failed", "{entry}");
    assert_eq!(entry["open"], 1);
    let items = item_statuses(&w).await;
    assert_eq!(items, [(format!("\"{Q1}\""), "open".to_string())]);

    // Later every engine names the site for Q1: the item resolves by itself.
    w.mock.reset().await;
    w.mock_engines("Acme is the best.", &[], None).await;
    w.run_to_end().await;
    assert_eq!(check_entry(&w).await["status"], "passed");
    assert_eq!(item_statuses(&w).await[0].1, "done");
}

#[sqlx::test]
async fn a_deleted_question_keeps_its_answers_but_its_item_resolves(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    w.crawl(&[(HOME, true)]).await;
    w.add_prompt(Q1).await;
    w.mock_engines("Rival is the usual pick.", &[], None).await;
    let id = w.run_to_end().await;
    assert_eq!(check_entry(&w).await["open"], 1);
    let (_, listed) = w.get(&w.ai_path("/prompts")).await;
    let prompt_id = listed["data"][0]["id"].as_str().unwrap().to_string();
    let (status, _) = w
        .send(
            "test-token",
            reqwest::Method::DELETE,
            &format!("/api/v1/ai/prompts/{prompt_id}"),
            None,
        )
        .await;
    assert_eq!(status, 200);
    assert_eq!(w.ai_answers(id).await.len(), 5, "history stays");
    assert_eq!(check_entry(&w).await["open"], 0);
}

// -------------------------------------------------------------------- metrics

#[sqlx::test]
async fn the_ai_block_explains_itself_until_there_is_a_run_and_then_has_every_number(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    acme(&w).await;
    let empty = w.ai_metrics().await;
    assert_eq!(
        empty,
        json!({"percent": null, "mentions": null, "citations": null, "answers": 0,
               "last_run_at": null, "engines": [], "reason": "Run an AI visibility check to see this."})
    );
    w.add_prompt(Q1).await;
    w.add_prompt(Q2).await;
    w.mount(
        "/claude/v1/messages",
        200,
        claude_body("Acme is it.", &[("https://example.com/p", "Me")]),
        Some(Q1),
    )
    .await;
    w.mock_engines(
        "Rival is the usual pick.",
        &[("https://rival.com/w", "Rival")],
        None,
    )
    .await;
    w.run_to_end().await;
    let metrics = w.ai_metrics().await;
    assert_eq!(metrics["answers"], 10);
    assert_eq!(metrics["mentions"], 1);
    assert_eq!(metrics["citations"], 1);
    assert_eq!(metrics["percent"], 10.0);
    assert!(metrics["last_run_at"].as_str().unwrap().contains('T'));
    assert!(metrics["reason"].is_null());
    assert_eq!(
        metrics["engines"][0],
        json!({"engine": "claude", "percent": 50.0, "mentions": 1, "answers": 2})
    );
    assert_eq!(metrics["engines"].as_array().unwrap().len(), 5);
    println!("AI_METRICS_JSON {metrics}");
}

// ----------------------------------------------------------- other workspaces

#[sqlx::test]
async fn another_workspace_can_read_nothing_of_this_one(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    w.add_prompt(Q1).await;
    w.mock_engines("Answer.", &[], None).await;
    let run_id = w.run_to_end().await;
    let answer_id = w.ai_answers(run_id).await[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let (_, listed) = w.get(&w.ai_path("/prompts")).await;
    let prompt_id = listed["data"][0]["id"].as_str().unwrap().to_string();

    let get = reqwest::Method::GET;
    let other = |method: &reqwest::Method, path: String, body: Option<Value>| {
        let (w, method) = (&w, method.clone());
        async move { w.send("other-token", method, &path, body).await }
    };
    let site = w.site_id;
    for path in [
        format!("/api/v1/sites/{site}/ai/settings"),
        format!("/api/v1/sites/{site}/ai/prompts"),
        format!("/api/v1/sites/{site}/ai/runs"),
        format!("/api/v1/sites/{site}/ai/domains"),
        format!("/api/v1/ai/runs/{run_id}"),
        format!("/api/v1/ai/runs/{run_id}/answers"),
        format!("/api/v1/ai/answers/{answer_id}"),
    ] {
        let (status, body) = other(&get, path.clone(), None).await;
        assert_eq!(status, 404, "{path}: {body}");
    }
    let post = reqwest::Method::POST;
    for path in [
        format!("/api/v1/sites/{site}/ai/runs"),
        format!("/api/v1/sites/{site}/ai/prompts/suggest"),
    ] {
        assert_eq!(other(&post, path, None).await.0, 404);
    }
    let (status, _) = other(
        &post,
        format!("/api/v1/sites/{site}/ai/prompts"),
        Some(json!({"text": Q2})),
    )
    .await;
    assert_eq!(status, 404);
    let (status, _) = other(
        &reqwest::Method::PUT,
        format!("/api/v1/sites/{site}/ai/settings"),
        Some(json!({"brand_name": "Hijack"})),
    )
    .await;
    assert_eq!(status, 404);
    let (status, _) = other(
        &reqwest::Method::DELETE,
        format!("/api/v1/ai/prompts/{prompt_id}"),
        None,
    )
    .await;
    assert_eq!(status, 404);
    let (_, still) = w.get(&w.ai_path("/prompts")).await;
    assert_eq!(still["data"].as_array().unwrap().len(), 1);
    assert_eq!(
        w.get(&w.ai_path("/settings")).await.1["data"]["brand_name"],
        ""
    );
}

#[sqlx::test]
async fn deleting_a_site_removes_its_ai_data(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    w.add_prompt(Q1).await;
    w.mock_engines("Answer.", &[], None).await;
    w.put_ai_settings("Acme", &[], &[]).await;
    w.run_to_end().await;
    let (status, _) = w
        .send(
            "test-token",
            reqwest::Method::DELETE,
            &format!("/api/v1/sites/{}", w.site_id),
            None,
        )
        .await;
    assert_eq!(status, 200);
    for table in ["ai_settings", "ai_prompts", "ai_runs", "ai_answers"] {
        let sql = format!("SELECT COUNT(*) FROM {table}");
        let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .fetch_one(&w.pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "{table}");
    }
}

#[sqlx::test]
async fn the_runs_list_pages_newest_first(pool: PgPool) {
    let w = GoogleWorld::with_ai(pool).await;
    w.add_prompt(Q1).await;
    w.mock_engines("Answer.", &[], None).await;
    let first = w.run_to_end().await;
    let second = w.run_to_end().await;
    let (status, body) = w.get(&w.ai_path("/runs?limit=1")).await;
    assert_eq!(status, 200);
    assert_eq!(body["data"][0]["id"], second.to_string());
    assert_eq!(body["meta"]["total"], 2);
    let (_, body) = w.get(&w.ai_path("/runs?limit=1&page=2")).await;
    assert_eq!(body["data"][0]["id"], first.to_string());
}
