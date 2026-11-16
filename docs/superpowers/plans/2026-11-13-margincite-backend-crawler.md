# Margincite Backend Foundation and Crawler Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Rust service where a signed-in workspace can add a website, have it crawled politely in the background, and read back every page it found.

**Architecture:** One Rust crate in `backend/`, organised by feature. An axum HTTP API and a background worker run in the same process and share one Postgres database, which also holds the job queue. The crawler is a set of small pure pieces (address rules, robots rules, page extraction) around one guarded HTTP client.

**Tech Stack:** Rust (pinned toolchain), axum, tokio, sqlx with Postgres, reqwest with rustls, scraper, texting_robots, quick-xml, chromiumoxide, wiremock for tests.

**Spec:** `docs/superpowers/specs/2026-11-13-margincite-design.md`, sections 3, 4, 6.1 to 6.4, 6.9 to 6.11.

## Where this plan sits

Phase 1 is now six plans. The backend was one plan and is split in two, so each ships something that works on its own.

| Plan | Piece | Status |
|---|---|---|
| 1 | Landing page | built, on `feat/landing-page` |
| 2 | Backend foundation and crawler | built, on `feat/backend-crawler` |
| 3 | Checks and suggestions | not written |
| 4 | Sign-in and the Claude connector | not written |
| 5 | Google Search Console | not written |

The Electron app was dropped by the owner on 2026-11-13. The Claude connector is the only way in.

Not in this plan: checks, findings, suggestions, real sign-in, the connector, Search Console. Sign-in is stood in for by fixed development tokens behind an interface that plan 4 replaces.

## Global Constraints

- Every shell command starts with `export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"`. The default PATH has neither cargo nor psql.
- The Rust version is pinned in `backend/rust-toolchain.toml` to `1.100.0`. Never run `rustup update` or change the machine's default toolchain; other projects on this machine use it.
- The machine has 16 GB of RAM shared by many sessions. Build and test with `CARGO_BUILD_JOBS=4`, run tests with `--test-threads=4`, and always pass a test name filter while working on a task.
- Postgres 14 is already running locally and holds other projects' databases. Use only `margincite_dev` and the temporary databases `sqlx::test` creates. Never drop, alter or read any other database.
- Work on the branch `feat/backend-crawler`, created from `feat/landing-page`. Never commit to `main`.
- Commit messages use `<type>: <description>` with types feat, fix, refactor, docs, test, chore, perf, ci. No attribution lines.
- Tests first. Line coverage of `backend/src` is at least 80%, measured with `cargo llvm-cov`.
- Tests never contact a real website. Every crawl in a test targets a `wiremock` server on localhost.
- Every table except `workspaces` has a `workspace_id`. Every repository function takes a `workspace_id` and filters by it. A resource in another workspace is reported as not found, never as forbidden.
- Every API response uses one envelope: `{ "success": bool, "data": object | null, "error": string | null, "meta": object | null }`.
- The crawler identifies as `MarginciteBot`, obeys robots.txt, makes at most 2 requests at a time per site, and stops at the site's page cap, which defaults to 500.
- The crawler refuses non-HTTP schemes and private or internal addresses, and checks again after every redirect.
- Crawled content is untrusted data. It is stored and returned as text and never executed or interpreted.
- No secrets in code. Settings come from the environment and are checked at start-up. Error messages shown to callers never include internals; the detail goes to the log.
- Files stay under 800 lines and functions under 50. Prefer returning new values to mutating shared ones.
- Latest stable crate versions are added with `cargo add`; `Cargo.lock` is committed.

## Review Focus

1. **A public page redirects to an internal address** such as `http://169.254.169.254/`. The hop is refused, that page is stored with a fetch error, and the crawl carries on. Test in Task 4 and Task 8.
2. **robots.txt disallows everything.** The crawl finishes as completed with zero pages and a plain reason, not as a failure. Test in Task 8.
3. **A page never stops sending, or is 50 MB.** It is cut off at 20 seconds or 5 MB, stored with a fetch error, and the crawl carries on. Test in Task 4.
4. **The worker dies in the middle of a crawl.** The job is picked up again after 15 minutes and the crawl finishes without duplicate pages. Tests in Task 7 and Task 8.
5. **Two workspaces add the same website.** Each gets its own site, crawls and pages, and each gets "not found" for the other's ids. Tests in Task 3 and Task 9.

---

## File structure

```
backend/
  Cargo.toml  Cargo.lock  rust-toolchain.toml  .env.example  README.md
  migrations/                 one SQL file per task that adds tables
  src/
    main.rs                   reads settings, starts the API and the worker
    lib.rs                    builds the app from its parts, so tests can too
    config.rs                 settings from the environment
    error.rs                  AppError and how it becomes a response
    envelope.rs               the response envelope and paging
    auth.rs                   Principal, Authenticator, the dev-token implementation, the request extractor
    http.rs                   router, rate limit, health
    workspaces.rs             workspace and user records
    sites/      mod.rs scope.rs repo.rs routes.rs
    crawl/      mod.rs guard.rs client.rs robots.rs sitemap.rs urls.rs extract.rs
                render.rs runner.rs repo.rs routes.rs
    jobs/       mod.rs queue.rs worker.rs
  tests/
    common/mod.rs             builds a test app, a fixture site and tokens
    api_*.rs  crawl_*.rs  jobs_*.rs
```

---

### Task 1: Crate, settings, envelope and health check

**Files:** Create `backend/Cargo.toml`, `rust-toolchain.toml`, `.env.example`, `src/{main,lib,config,error,envelope,http}.rs`, `tests/common/mod.rs`, `tests/api_health.rs`. Modify root `.gitignore` (add `backend/target`, `backend/.env`).

**Interfaces — produces:**
- `Config { database_url: String, bind_addr: SocketAddr, dev_tokens: Vec<(String, String)>, crawler_contact: String, chrome_path: Option<PathBuf> }` and `Config::from_env(vars: impl Fn(&str) -> Option<String>) -> Result<Config, ConfigError>`.
- Environment names: `DATABASE_URL`, `BIND_ADDR` (default `127.0.0.1:8787`), `DEV_API_TOKENS` (`token=email` pairs separated by commas), `CRAWLER_CONTACT` (an email or URL shown in the user agent), `CHROME_PATH` (optional).
- `Envelope<T>` with `Envelope::ok(data)`, `Envelope::page(data, Meta { total, page, limit })`, and `AppError` variants `NotFound`, `Invalid(String)`, `Unauthorized`, `Conflict(String)`, `RateLimited`, `Internal(anyhow::Error)`, mapped to 404, 422, 401, 409, 429, 500.
- `pub fn app(state: AppState) -> axum::Router` and `AppState { pool: PgPool, config: Arc<Config>, .. }`.
- `tests/common`: `async fn spawn_app(pool: PgPool) -> TestApp` with `TestApp { addr, client }`.

- [ ] **Step 1: Branch and database**

```bash
git checkout -b feat/backend-crawler
createdb margincite_dev
```

- [ ] **Step 2: Scaffold the crate** with `cargo init backend --name margincite`, pin the toolchain, add crates, and install the two cargo tools the plan uses (`sqlx-cli` with `--no-default-features --features rustls,postgres`, and `cargo-llvm-cov`).

- [ ] **Step 3: Write the failing tests**

```rust
// src/config.rs
#[test] fn reads_every_setting()            // all five set -> Ok, bind_addr parsed
#[test] fn bind_addr_defaults_to_local_8787()
#[test] fn missing_database_url_names_the_variable() // Err message contains "DATABASE_URL"
#[test] fn missing_crawler_contact_names_the_variable()
#[test] fn malformed_dev_token_pair_is_rejected()    // "abc" without "=" -> Err

// tests/api_health.rs
#[sqlx::test] async fn health_answers_with_the_envelope(pool)
// GET /healthz -> 200, body == {"success":true,"data":{"status":"ok"},"error":null,"meta":null}
#[sqlx::test] async fn unknown_route_is_a_404_envelope(pool)
// GET /nope -> 404, success false, error "Not found", data null
```

- [ ] **Step 4: Run and see them fail.** `cargo test --test api_health -- --test-threads=4` → fails to compile, `spawn_app` not found.
- [ ] **Step 5: Implement.** `Internal` errors log the cause with `tracing::error!` and answer `"Something went wrong on our side."`. `main.rs` fails fast with the `ConfigError` text if a setting is missing.
- [ ] **Step 6: Run and see them pass.** `cargo test config:: && cargo test --test api_health` → 7 pass. `cargo clippy --all-targets -- -D warnings` → clean.
- [ ] **Step 7: Commit** `chore: scaffold the Margincite backend with settings and health check`.

---

### Task 2: Workspaces, development tokens and rate limiting

**Files:** Create `migrations/0001_workspaces.sql`, `src/workspaces.rs`, `src/auth.rs`, `tests/api_auth.rs`. Modify `src/http.rs`, `src/lib.rs`.

**Interfaces — produces:**
- Tables `workspaces(id uuid pk, name text, created_at timestamptz)` and `users(id uuid pk, workspace_id uuid fk, email text unique, created_at timestamptz)`.
- `Principal { user_id: Uuid, workspace_id: Uuid }`, usable as an axum extractor in any handler.
- `#[async_trait] trait Authenticator: Send + Sync { async fn authenticate(&self, bearer: &str) -> Result<Principal, AppError>; }`
- `DevTokenAuthenticator::new(pool, tokens)`: a known token finds or creates the user for its email and that user's own workspace. Plan 4 swaps this type for one that verifies real tokens; nothing else changes.
- `GET /api/v1/me` → `{ user_id, workspace_id, email }`.
- Rate limit: 120 requests per minute per bearer token, or per IP address when there is no token. Over the limit is a 429 envelope.

- [ ] **Step 1: Write the failing tests** in `tests/api_auth.rs`

```rust
#[sqlx::test] async fn no_token_is_401(pool)            // GET /api/v1/me -> 401, error "Sign in to continue."
#[sqlx::test] async fn unknown_token_is_401(pool)
#[sqlx::test] async fn known_token_returns_its_user(pool) // email matches the pair in DEV_API_TOKENS
#[sqlx::test] async fn the_same_token_keeps_the_same_workspace(pool) // two calls, equal workspace_id
#[sqlx::test] async fn two_tokens_get_two_workspaces(pool)
#[sqlx::test] async fn the_121st_request_in_a_minute_is_429(pool)
#[sqlx::test] async fn health_needs_no_token(pool)
```

- [ ] **Step 2: Run and see them fail.** `cargo test --test api_auth` → 401 tests fail with 404.
- [ ] **Step 3: Implement.** Tokens are compared in constant time. The bearer value is never logged.
- [ ] **Step 4: Run and see them pass**, then `cargo sqlx prepare` and commit `.sqlx/`.
- [ ] **Step 5: Commit** `feat: add workspaces, development tokens and rate limiting`.

---

### Task 3: Sites

**Files:** Create `migrations/0002_sites.sql`, `src/sites/{mod,scope,repo,routes}.rs`, `tests/api_sites.rs`.

**Interfaces — produces:**
- Table `sites(id, workspace_id, root_url text, host text, path_prefix text, display_name text, page_cap int default 500, created_at)`, unique on `(workspace_id, host, path_prefix)`.
- `SiteScope { root: Url, host: String, path_prefix: String }` with:
  - `SiteScope::parse(input: &str) -> Result<SiteScope, ScopeError>`: accepts an address with or without a scheme (default `https`), lowercases the host, strips a leading `www.`, drops query and fragment, and keeps the path as the prefix (`/` when empty).
  - `fn contains(&self, url: &Url) -> bool`: same host ignoring `www.`, and a path under the prefix.
- `sites::repo`: `create`, `list`, `get`, `delete`, each `(pool, workspace_id, ..)`.
- Routes: `GET /api/v1/sites`, `POST /api/v1/sites` with `{ "url": "...", "name": "..."? }`, `GET /api/v1/sites/{id}`, `DELETE /api/v1/sites/{id}`.

- [ ] **Step 1: Write the failing tests**

```rust
// src/sites/scope.rs
#[test] fn adds_https_when_no_scheme()                 // "example.com" -> root "https://example.com/"
#[test] fn www_and_bare_host_are_the_same_site()       // host == "example.com" for both
#[test] fn keeps_a_sub_path_as_the_prefix()            // "example.com/docs" -> path_prefix "/docs"
#[test] fn contains_pages_under_the_prefix_only()      // /docs/a yes, /blog no, /docsx no
#[test] fn other_subdomains_are_outside()              // blog.example.com -> false
#[test] fn rejects_non_http_schemes()                  // "ftp://x", "javascript:alert(1)", "file:///etc/passwd"
#[test] fn rejects_addresses_with_credentials()        // "https://user:pw@example.com"
#[test] fn rejects_ip_literals_and_localhost()         // "http://127.0.0.1", "http://[::1]", "localhost"
#[test] fn rejects_empty_and_overlong_input()          // "" and 2049 chars

// tests/api_sites.rs
#[sqlx::test] async fn adding_a_site_returns_it(pool)            // 201, host "example.com", page_cap 500
#[sqlx::test] async fn an_invalid_address_is_422_with_a_reason(pool)
#[sqlx::test] async fn adding_the_same_site_twice_is_409(pool)
#[sqlx::test] async fn the_list_shows_only_my_sites(pool)
#[sqlx::test] async fn another_workspaces_site_is_404_to_read_and_delete(pool)
#[sqlx::test] async fn two_workspaces_can_add_the_same_site(pool) // both 201, different ids
#[sqlx::test] async fn deleting_a_site_removes_it(pool)
```

- [ ] **Step 2: Run and see them fail.** `cargo test sites::scope && cargo test --test api_sites`.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run and see them pass**, `cargo sqlx prepare`.
- [ ] **Step 5: Commit** `feat: add sites scoped to a workspace`.

---

### Task 4: The guarded HTTP client

This is the only code that fetches addresses a user typed in, so it carries the protection against being pointed at internal systems.

**Files:** Create `src/crawl/{mod,guard,client}.rs`, `tests/crawl_client.rs`.

**Interfaces — produces:**
- `guard::check_ip(ip: IpAddr) -> Result<(), Blocked>`: refuses loopback, private ranges, link-local, `100.64.0.0/10`, unspecified, multicast, broadcast, documentation ranges, IPv6 unique-local and link-local, and IPv4-mapped forms of any of these.
- `guard::check_url(url: &Url) -> Result<(), Blocked>`: http or https only, no credentials, and `check_ip` when the host is an address.
- `Network::Public` or `Network::AllowPrivate`. `main.rs` always passes `Public`; only tests pass `AllowPrivate`. It is not readable from the environment.
- `CrawlClient::new(contact: &str, network: Network) -> CrawlClient`, with user agent `MarginciteBot/0.1 (+<contact>)`.
- `CrawlClient::with_limits(contact, network, Limits { timeout: Duration, max_bytes: usize, max_redirects: usize })`, which `new` calls with the defaults below. Tests use it to shorten the timeout.
- `async fn fetch(&self, url: &Url) -> Fetched`, where
  `Fetched { final_url: Url, status: Option<u16>, headers: HeaderMap, body: Option<String>, content_type: Option<String>, redirects: Vec<Url>, elapsed_ms: u32, bytes: u32, error: Option<FetchError> }` and
  `FetchError` is one of `Blocked`, `Timeout`, `TooLarge`, `TooManyRedirects`, `Network(String)`.
- Limits: 20 seconds per request, 5 MB per body, 10 redirects. Redirects are followed by hand and each hop goes through `check_url`. Name lookups go through a resolver that drops refused addresses, so a name that points inward cannot be reached. Only `text/html` and `application/xhtml+xml` bodies are read; other types return status and headers with no body.

- [ ] **Step 1: Write the failing tests**

```rust
// src/crawl/guard.rs
#[test] fn refuses_internal_v4()   // 127.0.0.1, 10.0.0.1, 172.16.0.1, 192.168.1.1, 169.254.169.254, 100.64.0.1, 0.0.0.0
#[test] fn refuses_internal_v6()   // ::1, fc00::1, fe80::1, ::ffff:127.0.0.1, ::ffff:10.0.0.1
#[test] fn allows_public()         // 93.184.216.34, 2606:2800:220:1::1
#[test] fn refuses_bad_urls()      // ftp://, file://, http://user:pw@x.test, http://127.0.0.1/

// tests/crawl_client.rs  (wiremock, Network::AllowPrivate unless noted)
#[tokio::test] async fn fetches_html_and_times_it()            // status 200, body present, elapsed_ms > 0
#[tokio::test] async fn sends_the_bot_user_agent()             // header starts "MarginciteBot/"
#[tokio::test] async fn follows_redirects_and_records_them()   // 2 hops -> redirects.len() == 2
#[tokio::test] async fn stops_after_ten_redirects()            // error TooManyRedirects
#[tokio::test] async fn cuts_off_a_body_over_5_mb()            // error TooLarge, body None
#[tokio::test] async fn times_out_a_slow_response()            // timeout shortened via a test constructor, error Timeout
#[tokio::test] async fn does_not_read_a_pdf_body()             // content_type "application/pdf", body None, no error
#[tokio::test] async fn public_mode_refuses_localhost()        // Network::Public -> error Blocked, no request received
#[tokio::test] async fn public_mode_refuses_a_redirect_to_an_internal_address()
// a Network::Public client given a Location of http://169.254.169.254/ -> error Blocked
```

- [ ] **Step 2: Run and see them fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run and see them pass.** `cargo test crawl::guard && cargo test --test crawl_client`.
- [ ] **Step 5: Commit** `feat: add the guarded crawler HTTP client`.

---

### Task 5: Robots rules, sitemaps and address rules

**Files:** Create `src/crawl/{robots,sitemap,urls}.rs`.

**Interfaces — produces:**
- `urls::normalize(url: &Url) -> Url`: drops the fragment, lowercases the host, removes default ports, turns an empty path into `/`, and removes the tracking parameters `utm_*`, `gclid`, `fbclid`.
- `Robots::parse(text: &str) -> Robots`, `Robots::allow_all()`, `fn allows(&self, url: &Url) -> bool` for the agent `MarginciteBot`, `fn crawl_delay(&self) -> Option<Duration>`, `fn sitemaps(&self) -> Vec<Url>`.
- `async fn load_robots(client: &CrawlClient, scope: &SiteScope) -> RobotsOutcome` with `RobotsOutcome { rules: Robots, raw: Option<String>, unreachable: bool }`. A 4xx answer means allow everything. A 5xx answer or a network failure sets `unreachable` and allows nothing.
- `sitemap::parse(xml: &str) -> SitemapDoc`, either `Urls(Vec<Url>)` or `Index(Vec<Url>)`.
- `async fn load_sitemap_urls(client, scope, robots) -> SitemapOutcome { found: bool, urls: Vec<Url> }`: reads the sitemaps named in robots.txt plus `/sitemap.xml`, follows an index one level, keeps only addresses inside the scope, and stops at 50,000.

- [ ] **Step 1: Write the failing tests** (unit tests in each file, plus wiremock tests for the two loaders)

```rust
#[test] fn normalize_drops_fragment_and_tracking()    // /a?utm_source=x&id=2#top -> /a?id=2
#[test] fn normalize_keeps_meaningful_query()
#[test] fn robots_disallow_applies_to_our_agent()
#[test] fn robots_specific_group_beats_wildcard()     // "User-agent: MarginciteBot" group wins over "*"
#[test] fn robots_reads_crawl_delay_and_sitemaps()
#[test] fn sitemap_reads_a_urlset()
#[test] fn sitemap_reads_an_index()
#[test] fn sitemap_survives_broken_xml()              // returns Urls(vec![]), no panic
#[tokio::test] async fn missing_robots_allows_everything()      // 404 -> allows, unreachable false
#[tokio::test] async fn a_500_on_robots_allows_nothing()        // unreachable true
#[tokio::test] async fn sitemap_urls_outside_the_site_are_dropped()
#[tokio::test] async fn an_index_is_followed_one_level()
```

- [ ] **Step 2: Run and see them fail.**
- [ ] **Step 3: Implement.** Compressed sitemaps are skipped in this plan and logged.
- [ ] **Step 4: Run and see them pass.** `cargo test crawl::robots crawl::sitemap crawl::urls`.
- [ ] **Step 5: Commit** `feat: read robots rules and sitemaps`.

---

### Task 6: Pulling the facts out of a page

**Files:** Create `src/crawl/extract.rs`, `tests/fixtures/pages/*.html`.

**Interfaces — produces:**
- `fn extract(html: &str, page_url: &Url) -> PageFacts`, a pure function.
- `PageFacts { title: Option<String>, meta_description: Option<String>, h1s: Vec<String>, headings: Vec<Heading { level: u8, text: String }>, canonical: Option<Url>, robots_meta: Option<String>, has_viewport: bool, word_count: u32, text: String, structured_data: Vec<serde_json::Value>, structured_data_errors: u32, links: Vec<Link { url: Url, text: String, nofollow: bool }>, images: Vec<Image { src: String, alt: Option<String> }>, has_scripts: bool, published: Option<String>, modified: Option<String>, author: Option<String> }`
- `fn looks_javascript_only(facts: &PageFacts) -> bool`: fewer than 50 words and at least one script.
- Text is the visible words of `body` without `script`, `style`, `noscript`, `template` or `svg`, with whitespace collapsed, cut at 200,000 characters. Links are resolved against the page address and its `<base>` tag, and `javascript:`, `mailto:` and `tel:` links are dropped.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn reads_title_description_and_canonical()
#[test] fn collects_headings_in_order_with_levels()
#[test] fn counts_visible_words_only()                 // script and style text not counted
#[test] fn resolves_relative_links_and_honours_base()
#[test] fn drops_javascript_mailto_and_tel_links()
#[test] fn notes_nofollow_links()
#[test] fn an_image_without_alt_has_none_and_an_empty_alt_has_some_empty()
#[test] fn parses_json_ld_and_counts_broken_blocks()   // one valid, one broken -> len 1, errors 1
#[test] fn reads_robots_meta_and_viewport()
#[test] fn reads_dates_and_author_from_meta_and_json_ld()
#[test] fn an_app_shell_looks_javascript_only()        // <div id="root"></div><script src=..> -> true
#[test] fn a_normal_article_does_not()
#[test] fn survives_malformed_and_empty_html()         // "", "<<<>>>", unclosed tags: no panic
#[test] fn cuts_very_long_text_at_the_limit()
```

- [ ] **Step 2: Run and see them fail.**
- [ ] **Step 3: Implement** with the `scraper` crate.
- [ ] **Step 4: Run and see them pass.** `cargo test crawl::extract`.
- [ ] **Step 5: Commit** `feat: extract page facts from HTML`.

---

### Task 7: The job queue and worker

**Files:** Create `migrations/0003_jobs.sql`, `src/jobs/{mod,queue,worker}.rs`, `tests/jobs_queue.rs`.

**Interfaces — produces:**
- Table `jobs(id, kind text, payload jsonb, status text check in ('queued','running','done','failed'), run_at timestamptz, attempts int, locked_at timestamptz, locked_by text, last_error text, created_at)`.
- `queue::enqueue(pool, kind: &str, payload: &Value, run_at: OffsetDateTime) -> Result<Uuid>`
- `queue::claim(pool, worker: &str) -> Result<Option<Job>>`: takes the oldest due queued job with `FOR UPDATE SKIP LOCKED`, marks it running and adds one to `attempts`.
- `queue::complete(pool, id)`, and `queue::fail(pool, id, error: &str)`: a failed job is queued again after 1, 5 and then 25 minutes, and marked failed after the third attempt.
- `queue::release_stale(pool, older_than: Duration) -> Result<u64>`: running jobs locked for longer than that go back to queued.
- `#[async_trait] trait JobHandler { fn kind(&self) -> &'static str; async fn run(&self, payload: Value) -> anyhow::Result<()>; }`
- `worker::run(pool, handlers: Vec<Arc<dyn JobHandler>>, shutdown: CancellationToken)`: polls every 2 seconds, releases jobs stale for 15 minutes, and finishes the current job before stopping.

- [ ] **Step 1: Write the failing tests** in `tests/jobs_queue.rs`

```rust
#[sqlx::test] async fn a_due_job_is_claimed_once(pool)             // second claim -> None
#[sqlx::test] async fn a_future_job_is_not_claimed(pool)
#[sqlx::test] async fn two_workers_never_get_the_same_job(pool)    // 20 jobs, 4 concurrent claimers, 20 distinct ids
#[sqlx::test] async fn a_failed_job_is_retried_later(pool)         // run_at about 1 minute ahead, attempts 1
#[sqlx::test] async fn a_job_is_failed_for_good_after_three_tries(pool)
#[sqlx::test] async fn a_stale_running_job_is_released(pool)       // locked_at 16 minutes ago -> queued again
#[sqlx::test] async fn the_worker_runs_a_handler_and_marks_it_done(pool)
#[sqlx::test] async fn a_handler_error_is_recorded_without_stopping_the_worker(pool)
#[sqlx::test] async fn a_job_of_unknown_kind_is_failed_with_a_reason(pool)
```

- [ ] **Step 2: Run and see them fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run and see them pass**, `cargo sqlx prepare`.
- [ ] **Step 5: Commit** `feat: add the Postgres job queue and worker`.

---

### Task 8: The crawl runner

**Files:** Create `migrations/0004_crawls.sql`, `src/crawl/{runner,repo,render}.rs`, `tests/crawl_runner.rs`, and `tests/common` gains `FixtureSite`.

**Interfaces — produces:**
- Table `crawls(id, workspace_id, site_id, status text check in ('queued','running','completed','failed'), trigger text check in ('manual','scheduled','first'), stop_reason text, robots_txt text, sitemap_found bool, llms_txt_found bool, pages_crawled int, pages_failed int, error text, started_at, finished_at, created_at)`.
- Table `pages(id, workspace_id, crawl_id, url text, depth int, in_sitemap bool, status_code int, content_type text, fetch_error text, redirects jsonb, response_ms int, bytes int, is_https bool, x_robots_tag text, needed_javascript bool, facts jsonb, text text, created_at)`, unique on `(crawl_id, url)`. `facts` is `PageFacts` without its `text`.
- `stop_reason` is one of `finished`, `page_cap`, `robots_blocked`, `site_unreachable`.
- `#[async_trait] trait Renderer: Send + Sync { async fn render(&self, url: &Url) -> Result<String, String>; }` and `NoRenderer`, which always returns an error. Task 10 adds the real one.
- `CrawlSettings { concurrency: usize, delay: Duration }`, default 2 and 500 ms. A robots crawl-delay longer than the default is honoured up to 5 seconds.
- `async fn run_crawl(deps: &CrawlDeps, workspace_id: Uuid, crawl_id: Uuid) -> anyhow::Result<()>` with `CrawlDeps { pool, client: CrawlClient, renderer: Arc<dyn Renderer>, settings: CrawlSettings }`.
- `CrawlJob`, a `JobHandler` of kind `crawl` whose payload is `{ workspace_id, crawl_id }`.
- `crawl::repo`: `create_crawl`, `get_crawl`, `list_crawls`, `upsert_page`, `list_pages`, `get_page`, `crawled_urls`, each taking `workspace_id`.

How a crawl runs:
1. Mark it running. Load robots.txt and store its text. Check `/llms.txt` and store only whether it exists.
2. If robots is unreachable or the first page cannot be fetched at all, finish as `failed` with `site_unreachable`.
3. Start from the site's root plus the sitemap addresses, breadth first, tracking depth from the root. Skip addresses outside the scope, refused by robots, or already seen after `normalize`.
4. For each page: fetch, extract, and if it looks JavaScript-only ask the renderer. If the rendered page has more words, store the rendered facts and set `needed_javascript`. If rendering fails, keep the raw facts and still set `needed_javascript`.
5. Store every page as it completes with `upsert_page`, so a restarted crawl skips what `crawled_urls` already holds.
6. Stop at the page cap with `page_cap`, or when nothing is left with `finished`. A robots file that refuses the root finishes as `completed` with `robots_blocked` and zero pages. One page failing never fails the crawl.

- [ ] **Step 1: Write the failing tests** in `tests/crawl_runner.rs`, each against a `FixtureSite` on wiremock

```rust
#[sqlx::test] async fn crawls_every_linked_page_once(pool)           // 5 pages linked in a ring -> 5 rows
#[sqlx::test] async fn records_depth_from_the_home_page(pool)        // / = 0, /a = 1, /a/b = 2
#[sqlx::test] async fn finds_pages_only_the_sitemap_lists(pool)      // in_sitemap true, reached though unlinked
#[sqlx::test] async fn skips_what_robots_disallows(pool)             // /private never requested
#[sqlx::test] async fn stays_inside_the_site(pool)                   // a link to another host is never requested
#[sqlx::test] async fn stops_at_the_page_cap(pool)                   // cap 3 of 10 -> 3 rows, stop_reason page_cap
#[sqlx::test] async fn a_broken_page_is_recorded_and_the_crawl_continues(pool) // 500 page stored, status completed
#[sqlx::test] async fn a_redirect_to_an_internal_address_is_refused_and_recorded(pool)
#[sqlx::test] async fn robots_blocking_everything_completes_with_no_pages(pool) // stop_reason robots_blocked
#[sqlx::test] async fn an_unreachable_site_fails_with_a_plain_reason(pool)      // stop_reason site_unreachable
#[sqlx::test] async fn never_runs_more_than_two_requests_at_once(pool)          // fixture counts concurrent requests
#[sqlx::test] async fn a_javascript_only_page_is_marked_and_rendered(pool)      // fake renderer returns full HTML
#[sqlx::test] async fn a_failed_render_keeps_the_raw_page_and_the_mark(pool)
#[sqlx::test] async fn a_restarted_crawl_does_not_repeat_or_duplicate_pages(pool)
// run once with a cap, raise the cap, run again: each URL requested once in total, no duplicate rows
#[sqlx::test] async fn stores_robots_text_and_whether_llms_txt_exists(pool)
#[sqlx::test] async fn tracking_parameters_do_not_create_duplicate_pages(pool)
```

- [ ] **Step 2: Run and see them fail.**
- [ ] **Step 3: Implement.** Keep `runner.rs` to orchestration; the frontier (queue, seen set, depth) is its own small type in the same module with its own unit tests.
- [ ] **Step 4: Run and see them pass**, `cargo sqlx prepare`. `cargo test --test crawl_runner -- --test-threads=4`.
- [ ] **Step 5: Commit** `feat: add the crawl runner`.

---

### Task 9: Crawl endpoints, limits, schedule and retention

**Files:** Create `src/crawl/routes.rs`, `tests/api_crawls.rs`. Modify `src/sites/routes.rs`, `src/lib.rs`, `src/main.rs`, `src/crawl/runner.rs`.

**Interfaces — produces:**
- `POST /api/v1/sites` now also creates the site's first crawl (trigger `first`) and enqueues it. The response gains `first_crawl_id`.
- `POST /api/v1/sites/{id}/crawls` → 202 with the new crawl. Refused with 409 `"A crawl of this site is already running."` while one is queued or running, and with 429 `"You have used today's 10 crawls. Try again tomorrow."` after 10 manual crawls by the workspace in the current UTC day.
- `GET /api/v1/sites/{id}/crawls?page=&limit=`, `GET /api/v1/crawls/{id}`, `GET /api/v1/crawls/{id}/pages?page=&limit=` (limit at most 100, without `text`), `GET /api/v1/pages/{id}` (with `text`).
- When a crawl finishes, the runner enqueues the next one for the same site 7 days later with trigger `scheduled`, unless the site has been deleted. Scheduled crawls do not count toward the daily limit.
- When a crawl finishes, page `text` is set to null for all but the two most recent crawls of that site.
- `main.rs` starts the worker beside the API and stops both cleanly on Ctrl-C.

- [ ] **Step 1: Write the failing tests** in `tests/api_crawls.rs`

```rust
#[sqlx::test] async fn adding_a_site_starts_its_first_crawl(pool)        // first_crawl_id set, a queued crawl job exists
#[sqlx::test] async fn starting_a_crawl_returns_202_and_queues_a_job(pool)
#[sqlx::test] async fn a_second_crawl_while_one_runs_is_409(pool)
#[sqlx::test] async fn the_eleventh_manual_crawl_in_a_day_is_429(pool)
#[sqlx::test] async fn scheduled_crawls_do_not_use_the_daily_allowance(pool)
#[sqlx::test] async fn pages_are_listed_in_pages_of_at_most_100(pool)    // limit=500 -> meta.limit 100
#[sqlx::test] async fn the_page_list_leaves_out_the_text_and_one_page_includes_it(pool)
#[sqlx::test] async fn another_workspaces_crawl_and_pages_are_404(pool)
#[sqlx::test] async fn finishing_a_crawl_schedules_the_next_in_seven_days(pool)
#[sqlx::test] async fn a_deleted_site_is_not_rescheduled(pool)
#[sqlx::test] async fn only_the_two_newest_crawls_keep_their_page_text(pool)
#[sqlx::test] async fn page_text_comes_back_as_plain_json_text(pool)
// a page containing <script>alert(1)</script> is returned inside a JSON string, content-type application/json
```

- [ ] **Step 2: Run and see them fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run and see them pass**, `cargo sqlx prepare`.
- [ ] **Step 5: Commit** `feat: add crawl endpoints, daily limit, weekly schedule and retention`.

---

### Task 10: Rendering JavaScript pages

**Files:** Modify `src/crawl/render.rs`, `src/main.rs`. Create `tests/crawl_render.rs`.

**Interfaces — produces:**
- `ChromeRenderer::launch(chrome_path: &Path, network: Network) -> anyhow::Result<ChromeRenderer>`, a `Renderer` using `chromiumoxide`. It opens one page at a time, waits for the network to go quiet for up to 15 seconds, returns the page's HTML, and closes the page.
- Requests the browser makes are checked with `guard::check_url`, so a page cannot make the browser fetch internal addresses.
- `main.rs` uses `ChromeRenderer` when `CHROME_PATH` is set and `NoRenderer` otherwise, and logs which one at start-up.

- [ ] **Step 1: Write the failing tests** in `tests/crawl_render.rs`. They are skipped with a printed reason when `CHROME_PATH` is not set.

```rust
#[tokio::test] async fn returns_html_that_javascript_wrote()        // fixture page writes "Hello from script" on load
#[tokio::test] async fn gives_up_on_a_page_that_never_settles()     // returns within 20 seconds with an error
#[tokio::test] async fn refuses_a_page_request_to_an_internal_address() // Network::Public, fixture page fetches 169.254.169.254
```

- [ ] **Step 2: Run and see them fail.** `CHROME_PATH="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" cargo test --test crawl_render`.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run and see them pass.**
- [ ] **Step 5: Commit** `feat: render JavaScript-only pages with headless Chrome`.

---

### Task 11: End to end, README and coverage

**Files:** Create `tests/e2e_crawl.rs`, `backend/README.md`.

- [ ] **Step 1: Write the test.** It starts the real app with its worker, a fixture site of six pages (one disallowed by robots, one returning 500, one only in the sitemap), and two tokens.

```rust
#[sqlx::test] async fn a_site_can_be_added_crawled_and_read_back(pool)
// POST /sites -> 201 with first_crawl_id
// poll GET /crawls/{id} until status completed (at most 20 seconds)
// stop_reason "finished", pages_crawled 5, pages_failed 1
// GET /crawls/{id}/pages -> 5 urls, none is the disallowed one, the sitemap-only page has in_sitemap true
// GET /pages/{id} for the home page -> title, word_count > 0, text present
// the second token gets 404 for the site, the crawl and the page
```

- [ ] **Step 2: Run it.** `cargo test --test e2e_crawl -- --test-threads=1` → passes. A failure here is a defect in an earlier task: fix the code and add a test for it in that task's test file.
- [ ] **Step 3: Write `backend/README.md`**: what the service does, the PATH line, the five settings with an example `.env`, how to create the database and run migrations, how to run the service and crawl a real site with `curl`, how to run tests and coverage, and the rule that tests never contact real websites.
- [ ] **Step 4: Run everything.**

```bash
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"
cd backend
cargo fmt --check && cargo clippy --all-targets -- -D warnings
CARGO_BUILD_JOBS=4 cargo llvm-cov --fail-under-lines 80 -- --test-threads=4
```

Expected: no formatting or lint findings, every test passes, line coverage 80 or higher.

- [ ] **Step 5: Commit** `test: add the end-to-end crawl test and README`.

---

## What the owner must supply

- `CRAWLER_CONTACT`: an email address or page that site owners can use to reach you about the bot. It is sent with every request the crawler makes.
- The address of the site to test on, when you are ready to crawl something real. The page cap is 500 until then.
