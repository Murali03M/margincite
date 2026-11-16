//! Which worker loop takes which kind of job: the long ones (AI runs, rank
//! runs, page speed runs, search volume refreshes) never sit in the lane that
//! crawls and syncs use.

use margincite::jobs::lanes::lanes;
use margincite::jobs::queue;
use serde_json::json;
use sqlx::PgPool;
use time::OffsetDateTime;

const KINDS: [&str; 7] = [
    "crawl",
    "google_sync",
    "bing_sync",
    "ai_run",
    "rank_run",
    "speed_run",
    "keyword_volumes",
];

async fn lane_takes(pool: &PgPool, lane_index: usize) -> Vec<String> {
    sqlx::query("DELETE FROM jobs").execute(pool).await.unwrap();
    for kind in KINDS {
        queue::enqueue(pool, kind, &json!({}), OffsetDateTime::now_utc())
            .await
            .unwrap();
    }
    let mut took = Vec::new();
    while let Some(job) = queue::claim_in(pool, "t", lanes()[lane_index])
        .await
        .unwrap()
    {
        took.push(job.kind);
    }
    took.sort();
    took
}

#[sqlx::test]
async fn crawls_and_syncs_have_a_lane_without_the_long_jobs(pool: PgPool) {
    let general = lane_takes(&pool, lanes().len() - 1).await;
    assert_eq!(general, ["bing_sync", "crawl", "google_sync"]);
}

#[sqlx::test]
async fn every_kind_of_job_is_taken_by_exactly_one_lane(pool: PgPool) {
    let mut all = Vec::new();
    for index in 0..lanes().len() {
        all.extend(lane_takes(&pool, index).await);
    }
    all.sort();
    let mut expected: Vec<String> = KINDS.iter().map(|kind| kind.to_string()).collect();
    expected.sort();
    assert_eq!(all, expected);
}

#[sqlx::test]
async fn page_speed_and_search_volume_do_not_wait_behind_rank_runs(pool: PgPool) {
    let mut together = Vec::new();
    for index in 0..lanes().len() - 1 {
        let took = lane_takes(&pool, index).await;
        if took.contains(&"rank_run".to_string()) {
            together = took;
        }
    }
    assert_eq!(together, ["rank_run"]);
}
