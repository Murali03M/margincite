# Margincite Checks, Suggestions and Web App Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** After a crawl, Margincite judges the pages and produces ranked suggestions, and a web app in the browser lets a person add a site, watch the crawl, and work through those suggestions.

**Architecture:** Checks are pure Rust functions that turn a crawl's stored pages into findings. A reconcile step folds findings into suggestions that persist across crawls. New API endpoints expose them. The web app is a separate Vite and React project in `web/` that talks only to the API.

**Tech Stack:** Rust, axum, sqlx, Postgres (existing backend). React 18, TypeScript, Vite 6, Tailwind 4, React Router, TanStack Query, Vitest, Testing Library, MSW, Playwright.

**Spec:** `docs/superpowers/specs/2026-11-13-margincite-design.md`: both amendments at the top, then sections 3, 4, 6.3, 6.5, 6.6, 6.7 and 6.9 to 6.11. Design system: `DESIGN.md`. Product truth: `PRODUCT.md`.

## Global Constraints

- Backend commands start with `export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"` and `export DATABASE_URL=postgres://localhost/margincite_dev`. Web commands start with `export PATH="/opt/homebrew/bin:$PATH"`.
- 16 GB of RAM shared by many sessions: `CARGO_BUILD_JOBS=4`, `--test-threads=4`, Vitest with `--maxWorkers=2`, and always a test filter while working.
- Never run `rustup update`. The toolchain is pinned in `backend/rust-toolchain.toml`.
- Use only the `margincite_dev` database and the temporary ones `sqlx::test` makes. Never touch any other database on this machine.
- Work on the branch `feat/web-app`. Never commit to `main`. Commit messages are `<type>: <description>` with no attribution or co-author lines.
- Tests first. Line coverage is at least 80% for `backend/src` (`cargo llvm-cov`) and for `web/src` (Vitest).
- Tests never contact a real website. Backend crawl tests use the existing wiremock fixtures. Web tests mock the API.
- Every table has `workspace_id`; every repository function takes it and filters by it; another workspace's resource is "not found".
- Every API response uses the existing envelope in `backend/src/envelope.rs`.
- Checks are rules. No AI model is called anywhere in this plan.
- Copy written for suggestions is plain and specific. It says what was found and how to fix it. It never promises a ranking or a citation. `llms.txt` is reported as information and is never a suggestion.
- Crawled content is untrusted. The web app shows it as text. A crawled address becomes a link only when it starts with `http://` or `https://`, and such links carry `rel="noopener noreferrer nofollow"`.
- The web app follows `DESIGN.md`: flat inks, 2px rules, 4px corners, League Gothic capitals for headings, Barlow Semi Condensed for everything else including tables, with tabular figures. No gradients, shadows or translucent fills. Buttons invert on hover. This is a tool people work in, so scanning and clear states come before decoration.
- Every screen has a loading state, an empty state and an error state with a plain message.
- Files stay under 800 lines and functions under 50. Prefer new values over mutation. No secrets in code.

## Standard task cycle

Every task follows the same five steps, so they are written once here:

1. Write the task's tests. They are listed by name with what each asserts.
2. Run them with the task's command and confirm they fail for the expected reason.
3. Implement the interfaces the task lists.
4. Run the task's command again and confirm they pass, then run the wider suite for that side (`cargo test -- --test-threads=4`, or `pnpm test`). For backend tasks also run `cargo clippy --all-targets -- -D warnings`.
5. Commit with the task's message.

## Review Focus

1. **A page marked done is still broken on the next crawl.** It reopens. A page the owner dismissed stays dismissed, and a different page newly failing the same check shows as a new open item. Tests in Task 4.
2. **A crawl that stopped at the page cap.** Items for pages that were not crawled this time keep their status; they are not marked fixed. Test in Task 4.
3. **A site with zero crawlable pages** (robots blocks everything). Checks run without dividing by zero, the score is shown as unavailable, and the web app explains why. Tests in Tasks 3, 4 and 8.
4. **Crawled text containing markup**, such as a title of `<img src=x onerror=alert(1)>`. The web app shows it as characters. Test in Task 10.
5. **The backend is down or the token has expired while the web app is open.** The screen says so and offers to retry or sign in again; it never shows a blank page. Tests in Tasks 6 and 7.

---

## File structure

```
backend/
  migrations/0006_findings_and_suggestions.sql
  src/checks/     mod.rs catalog.rs context.rs technical.rs ai.rs content.rs store.rs
  src/suggestions/ mod.rs reconcile.rs rank.rs repo.rs routes.rs
  tests/          checks_run.rs suggestions_reconcile.rs api_suggestions.rs
shared/
  tokens.css      the design tokens and base type, used by landing/ and web/
web/
  package.json index.html vite.config.ts vitest.config.ts playwright.config.ts tsconfig.json README.md
  src/
    main.tsx App.tsx index.css
    api/          client.ts types.ts hooks.ts
    auth/         session.ts SignIn.tsx RequireSession.tsx
    ui/           Button.tsx Field.tsx Table.tsx StatusTag.tsx States.tsx Shell.tsx safeLink.ts
    sites/        SitesPage.tsx AddSitePage.tsx SitePage.tsx CrawlStatus.tsx SettingsPage.tsx
    suggestions/  SuggestionsPage.tsx SuggestionPage.tsx
    pages/        PagesPage.tsx PageDetail.tsx
  tests/          unit/**  e2e/flow.spec.ts  mocks/handlers.ts
```

---

## Part A: backend

### Task 1: The check catalog and the technical checks

**Files:** create `src/checks/{mod,catalog,context,technical}.rs`.

**Interfaces — produces:**
- `enum Category { Technical, AiReadiness, Content }`, `enum TargetKind { Page, Site }`.
- `struct CheckDef { id: &'static str, category: Category, severity: u8, effort: u8, title: &'static str, why: &'static str, fix: &'static str }`. Severity and effort are 1 to 3.
- `catalog::all() -> &'static [CheckDef]` and `catalog::get(id: &str) -> Option<&'static CheckDef>`.
- `struct CrawlContext { root: Url, robots_txt: Option<String>, sitemap_found: bool, pages: Vec<PageView> }` and `struct PageView { url: String, depth: i32, in_sitemap: bool, status_code: Option<i32>, content_type: Option<String>, fetch_error: Option<String>, redirects: Vec<String>, response_ms: Option<i32>, bytes: Option<i32>, is_https: bool, x_robots_tag: Option<String>, needed_javascript: bool, facts: Option<PageFacts> }`.
- `impl PageView { fn is_html_ok(&self) -> bool }`: status 200 with facts present.
- `struct Finding { check_id: &'static str, target: String, target_kind: TargetKind, evidence: serde_json::Value }`.
- `technical::run(ctx: &CrawlContext) -> Vec<Finding>`.

Technical checks, with id, severity, effort and what triggers each:

| id | sev | effort | Triggers when |
|---|---|---|---|
| `broken_page` | 3 | 2 | status 400 or above, or a fetch error |
| `broken_internal_link` | 3 | 1 | a page links to a crawled page that is broken. Target is the linking page; evidence lists the broken addresses |
| `redirect_chain` | 1 | 1 | 2 or more redirects before the page |
| `title_missing` | 3 | 1 | no title, or an empty one |
| `title_duplicate` | 2 | 2 | the same title on 2 or more pages. Evidence lists the others |
| `title_length` | 1 | 1 | shorter than 10 or longer than 65 characters |
| `description_missing` | 2 | 1 | no meta description |
| `description_duplicate` | 1 | 2 | the same description on 2 or more pages |
| `description_length` | 1 | 1 | shorter than 50 or longer than 160 characters |
| `h1_missing` | 2 | 1 | no main heading |
| `h1_multiple` | 1 | 1 | more than one main heading |
| `canonical_broken` | 2 | 2 | the canonical address is a crawled page that is broken |
| `noindex_in_sitemap` | 3 | 1 | in the sitemap, and `noindex` in the robots meta tag or the `X-Robots-Tag` header |
| `sitemap_missing` | 2 | 2 | site-level: no sitemap was found |
| `not_in_sitemap` | 1 | 2 | a sitemap exists and this working page is not in it |
| `image_alt_missing` | 1 | 2 | at least one image with no `alt` attribute. Evidence gives the count and up to 5 sources |
| `viewport_missing` | 2 | 1 | no mobile viewport tag |
| `not_https` | 3 | 2 | the page was served over http |
| `slow_response` | 2 | 3 | the response took longer than 1500 ms |
| `heavy_page` | 1 | 3 | the HTML is larger than 2 MB |

Content-based checks apply only to pages where `is_html_ok()` is true.

**Tests** (unit tests beside each check, built from small `PageView` values):
- `catalog_ids_are_unique_and_every_field_is_filled`
- `catalog_copy_never_promises_results`: no `why` or `fix` matches `guarantee|will rank|will be cited|boost`
- One test per row above that triggers it and asserts target and evidence, and one that shows a healthy page produces nothing: `a_healthy_page_has_no_technical_findings`
- `duplicates_name_each_other_in_evidence`
- `checks_skip_pages_that_are_not_working_html`: a PDF and a 500 page produce no title or heading findings
- `an_empty_crawl_produces_only_site_level_findings`

**Command:** `cargo test --lib checks::` **Commit:** `feat: add the check catalog and technical checks`

### Task 2: AI readiness and content checks

**Files:** create `src/checks/{ai,content}.rs`. Modify `src/checks/{mod,catalog}.rs`.

**Interfaces — produces:** `ai::run(ctx) -> Vec<Finding>`, `content::run(ctx) -> Vec<Finding>`, and `checks::run_all(ctx: &CrawlContext) -> Vec<Finding>` which calls all three and returns findings sorted by check id then target.
- `ai::AI_CRAWLERS: [&str; 9]` = `GPTBot`, `OAI-SearchBot`, `ChatGPT-User`, `ClaudeBot`, `Claude-SearchBot`, `Claude-User`, `PerplexityBot`, `Google-Extended`, `Bingbot`.

| id | category | sev | effort | Triggers when |
|---|---|---|---|---|
| `ai_crawler_blocked` | AI | 3 | 1 | site-level, one finding per crawler: robots.txt refuses that crawler the site's home page. Target is the crawler's name |
| `needs_javascript` | AI | 3 | 3 | the page's content only appeared after JavaScript ran |
| `structured_data_missing` | AI | 1 | 2 | no JSON-LD on the page |
| `structured_data_invalid` | AI | 2 | 1 | at least one JSON-LD block that does not parse |
| `date_missing` | AI | 1 | 1 | 300 or more words, not the home page, and no published or updated date |
| `author_missing` | AI | 1 | 1 | 300 or more words, not the home page, and no author |
| `thin_content` | Content | 2 | 3 | fewer than 150 words, and not a JavaScript-only page |
| `duplicate_content` | Content | 2 | 2 | 150 or more words and identical text to another page |

Use the existing `crawl::robots::Robots` to decide whether a crawler is refused; extend it with `fn allows_agent(&self, agent: &str, url: &Url) -> bool` if it only answers for `MarginciteBot` today.

**Tests:** one per row triggering it, plus `a_crawler_named_in_its_own_group_is_judged_by_that_group`, `no_robots_file_blocks_no_crawler`, `a_wildcard_disallow_blocks_all_nine`, `duplicate_content_ignores_short_pages`, `run_all_is_sorted_and_stable`, and `llms_txt_never_produces_a_finding`.

**Command:** `cargo test --lib checks::` **Commit:** `feat: add AI readiness and content checks`

### Task 3: Storing findings and running checks after a crawl

**Files:** create `migrations/0006_findings_and_suggestions.sql`, `src/checks/store.rs`, `tests/checks_run.rs`. Modify `src/crawl/runner.rs`, `src/crawl/repo.rs`.

**Interfaces — produces:**
- Tables, all in this one migration:
  - `findings(id uuid pk, workspace_id uuid, crawl_id uuid fk on delete cascade, check_id text, target text, target_kind text check in ('page','site'), evidence jsonb, created_at)`, unique `(crawl_id, check_id, target)`.
  - `suggestions(id uuid pk, workspace_id uuid, site_id uuid fk on delete cascade, check_id text, first_seen timestamptz, last_seen timestamptz)`, unique `(site_id, check_id)`.
  - `suggestion_items(id uuid pk, workspace_id uuid, suggestion_id uuid fk on delete cascade, target text, target_kind text, status text check in ('open','done','dismissed'), evidence jsonb, first_seen, last_seen, resolved_at timestamptz)`, unique `(suggestion_id, target)`.
  - `crawls` gains `score int` (nullable) and `html_pages int not null default 0`.
- `crawl::repo::load_context(pool, workspace_id, crawl_id) -> Result<CrawlContext, AppError>`.
- `checks::store::replace_findings(pool, workspace_id, crawl_id, findings: &[Finding]) -> Result<(), AppError>`: deletes the crawl's findings and inserts these in one transaction.
- The runner, after a crawl ends as `completed`, loads the context, runs `checks::run_all`, stores the findings, and then calls Task 4's reconcile. A crawl that ended `failed` runs no checks. A failure inside the checks is logged and recorded in the crawl's `error`, and does not fail the crawl.

**Tests** in `tests/checks_run.rs`, using the existing `FixtureSite`:
- `a_finished_crawl_has_findings_for_its_broken_page`
- `findings_for_a_crawl_are_replaced_not_duplicated_when_checks_run_twice`
- `a_failed_crawl_has_no_findings`
- `a_crawl_with_no_pages_still_runs_and_stores_site_level_findings`: robots blocks everything, and the run does not panic
- `findings_carry_the_workspace_of_their_crawl`

**Command:** `cargo test --test checks_run` **Commit:** `feat: run checks when a crawl finishes`

### Task 4: Suggestions: reconcile, rank and score

**Files:** create `src/suggestions/{mod,reconcile,rank,repo}.rs`, `tests/suggestions_reconcile.rs`.

**Interfaces — produces:**
- `reconcile::apply(pool, workspace_id, site_id, crawl_id) -> Result<(), AppError>`, one transaction, with these rules:
  - a finding with no item yet creates an `open` item;
  - an `open` item found again updates `last_seen` and its evidence;
  - a `done` item found again goes back to `open`;
  - a `dismissed` item stays `dismissed`;
  - an `open` item not found this time becomes `done` with `resolved_at` set, **only if** its target was checked in this crawl: a page target must be among the crawl's pages, and a site target is always checked;
  - it then stores the crawl's `score` and `html_pages`.
- `rank::priority(severity: u8, effort: u8, reach: f64) -> f64` = `severity * reach / effort`.
- `rank::reach(open_items: i64, html_pages: i64, kind: TargetKind) -> f64`: 1.0 for a site-level check with any open item; otherwise open items divided by working HTML pages, capped at 1.0, and 0.0 when there are no pages.
- `rank::score(per_check: &[(u8 /*severity*/, f64 /*reach*/)], catalog_severity_total: u32) -> Option<u8>`: `round(100 * (1 - sum(severity * reach) / catalog_severity_total))`, clamped to 0 to 100. `None` when the crawl has no working HTML pages.
- `repo::list(pool, workspace_id, site_id, filter: ListFilter { status: Option<Status>, category: Option<Category> }, page, limit) -> (Vec<SuggestionRow>, total)` ordered by priority, highest first, then check id. `SuggestionRow { id, check_id, open, done, dismissed, priority, reach, first_seen, last_seen }`.
- `repo::get(pool, workspace_id, id)`, `repo::items(pool, workspace_id, suggestion_id, status: Option<Status>, page, limit)`.
- `repo::set_status(pool, workspace_id, suggestion_id, status: Status, targets: Option<&[String]>) -> Result<u64, AppError>`: all items when `targets` is `None`. Returns how many changed.
- `repo::counts(pool, workspace_id, site_id) -> Counts { open, done, dismissed }` over items.

**Tests** in `tests/suggestions_reconcile.rs` (each builds findings directly, without crawling) and unit tests in `rank.rs`:
- `a_new_finding_becomes_an_open_item`
- `a_done_item_that_is_still_found_reopens`
- `a_dismissed_item_stays_dismissed_when_found_again`
- `a_new_page_failing_a_dismissed_check_is_a_new_open_item`
- `an_open_item_no_longer_found_becomes_done`
- `an_item_for_a_page_not_crawled_this_time_keeps_its_status`
- `a_group_is_listed_under_done_only_when_no_item_is_open`
- `suggestions_are_ranked_by_severity_reach_and_effort`
- `marking_chosen_targets_changes_only_those`
- `another_workspace_cannot_read_or_change_my_suggestions`
- `score_is_100_with_no_open_items_and_lower_with_more`
- `score_is_none_when_there_are_no_working_pages`
- `reach_never_exceeds_one_and_never_divides_by_zero`

**Command:** `cargo test --lib suggestions:: && cargo test --test suggestions_reconcile` **Commit:** `feat: add suggestions with reconcile, ranking and score`

### Task 5: Suggestion endpoints

**Files:** create `src/suggestions/routes.rs`, `tests/api_suggestions.rs`. Modify `src/lib.rs`, `src/crawl/routes.rs`.

**Interfaces — produces** (all need the bearer token):
- `GET /api/v1/sites/{id}/summary` → `{ site, latest_crawl, score: number | null, counts: { open, done, dismissed }, by_category: [{ category, open }], top: [SuggestionOut; up to 5], info: { pages_crawled, html_pages, sitemap_found, llms_txt_found, reach_basis: "pages" } }`. `latest_crawl` is null before the first crawl exists.
- `GET /api/v1/sites/{id}/suggestions?status=open|done|dismissed|all&category=technical|ai_readiness|content&page=&limit=` → paged `SuggestionOut { id, check_id, category, severity, effort, title, open, done, dismissed, priority }`. `status` defaults to `open`: a group appears under `open` when it has at least one open item.
- `GET /api/v1/suggestions/{id}?status=&page=&limit=` → `{ suggestion: SuggestionOut, why, fix, items: [{ target, target_kind, status, evidence, first_seen, last_seen, resolved_at }] }` with paging meta for the items.
- `PATCH /api/v1/suggestions/{id}` with `{ "status": "open" | "done" | "dismissed", "targets": [string]? }` → `{ changed: number }`. An unknown status or an empty `targets` list is 422.
- `GET /api/v1/pages/{id}/findings` → `[{ check_id, category, severity, title, evidence }]` for that page in its crawl.
- Category names in JSON are `technical`, `ai_readiness`, `content`.

**Tests** in `tests/api_suggestions.rs`:
- `the_summary_of_a_new_site_has_no_crawl_and_no_score`
- `the_summary_after_a_crawl_has_a_score_counts_and_five_top_suggestions`
- `suggestions_come_back_ranked_and_paged`
- `filtering_by_category_and_status_works`
- `a_suggestion_shows_its_why_fix_and_items`
- `marking_one_target_done_changes_one_item`
- `marking_the_whole_suggestion_dismissed_changes_every_item`
- `an_unknown_status_is_422`
- `a_page_lists_the_findings_against_it`
- `another_workspaces_suggestion_summary_and_findings_are_404`
- `crawled_text_in_evidence_is_returned_as_json_text`

**Command:** `cargo test --test api_suggestions` then the whole backend suite and `cargo llvm-cov --fail-under-lines 80 -- --test-threads=4`. **Commit:** `feat: add suggestion and summary endpoints`

---

## Part B: web app

### Task 6: Shared tokens, the web project, the API client and sign-in

**Files:** create `shared/tokens.css` and everything under `web/` except the feature folders. Modify `landing/src/index.css` to import the shared tokens.

**Interfaces — produces:**
- `shared/tokens.css`: the `@theme` block, the `:root` gutter, the base element styles and `::selection`, moved out of `landing/src/index.css` unchanged. The landing page must look and test exactly as before.
- `web/` runs on port 5174. `vite.config.ts` proxies `/api` and `/healthz` to `http://127.0.0.1:8787`, so the browser never needs the backend's address.
- `api/client.ts`: `class ApiError extends Error { status: number }` and `async function api<T>(path: string, init?: { method?, body?, token? }): Promise<{ data: T; meta: Meta | null }>`. It reads the envelope, throws `ApiError` with the envelope's `error` text, and throws `ApiError(0, "Margincite can't be reached. Check that the backend is running.")` on a network failure.
- `api/types.ts`: TypeScript types for every response in this plan and the existing sites, crawls and pages responses.
- `auth/session.ts`: `getToken()`, `setToken(t)`, `clearToken()` over `sessionStorage`, each wrapped so a blocked storage does not throw.
- `SignIn`: one field for the access token. On submit it calls `GET /api/v1/me`; success stores the token and goes to `/`, a 401 shows `"That token was not recognised."`. The page says plainly that this is the development sign-in and where the token lives (`backend/.env`).
- `RequireSession`: wraps the app's routes; with no token, or after any 401, it clears the token and shows sign-in.
- `ui/Shell`: the header (logo, Sites link, the signed-in email, Sign out) and a `main` region; `ui/States`: `Loading`, `Empty`, `ErrorState` with a Retry button.
- `ui/safeLink.ts`: `safeHref(url: string): string | null`, which returns the address only when it parses as http or https.

**Tests** (`web/tests/unit`, MSW for the API):
- `api_reads_data_and_meta_from_the_envelope`, `api_throws_the_envelope_error_text`, `api_explains_a_network_failure`
- `signing_in_with_a_good_token_enters_the_app`, `a_bad_token_shows_the_not_recognised_message`
- `a_401_on_any_request_returns_to_sign_in`
- `signing_out_clears_the_token`
- `blocked_session_storage_does_not_crash_the_app`
- `safeHref_allows_http_and_https_only`: `javascript:alert(1)`, `data:text/html,x` and `//evil.test` give null
- Landing page: its existing unit, build and browser suites still pass.

**Command:** `pnpm --dir web exec vitest run tests/unit --maxWorkers=2`, then the landing suites. **Commit:** `feat: add the web app shell, API client and sign-in`

### Task 7: Sites, adding a site, and watching the crawl

**Files:** create `web/src/sites/{SitesPage,AddSitePage,CrawlStatus}.tsx`, `web/src/api/hooks.ts`.

**Interfaces — produces:**
- Routes `/` (sites) and `/sites/new`.
- `SitesPage`: each site as a row with its host, score or "No score yet", open suggestion count, and when it was last crawled. An empty state invites adding the first site.
- `AddSitePage`: one address field. A 422 shows the server's reason under the field and keeps what was typed; a 409 says the site is already added and links to it. Success goes to `/sites/:id`.
- `CrawlStatus({ crawlId })`: polls `GET /api/v1/crawls/{id}` every 2 seconds while the status is `queued` or `running`, and stops when it ends. It shows pages crawled so far, and at the end one plain sentence per stop reason: finished, stopped at the page limit, blocked by robots.txt, or could not be reached.
- `hooks.ts`: `useSites`, `useSite`, `useSummary`, `useCrawl`, `useCreateSite`, `useStartCrawl`, built on TanStack Query.

**Tests:**
- `the_sites_list_shows_each_site_with_its_score_and_counts`
- `an_empty_account_is_invited_to_add_a_site`
- `the_list_shows_an_error_with_retry_when_the_backend_is_down`
- `adding_a_site_goes_to_its_page`
- `an_invalid_address_shows_the_reason_and_keeps_the_input`
- `an_already_added_site_links_to_the_existing_one`
- `crawl_status_polls_until_the_crawl_ends_then_stops`: fake timers, three responses, no fourth request
- `each_stop_reason_has_its_own_sentence`

**Commit:** `feat: add the sites list, add-site and crawl progress screens`

### Task 8: The site overview

**Files:** create `web/src/sites/SitePage.tsx`, `web/src/ui/{StatusTag,Table}.tsx`.

**Interfaces — produces:**
- Route `/sites/:id`, with tabs to Overview, Suggestions, Pages and Settings.
- Overview shows: the score as a large number out of 100 with one line saying how it is worked out (`"100 minus the share of pages affected by each open check, weighted by how serious it is."`); counts of open, done and dismissed; open counts by category; the top five suggestions as links; crawl facts (pages crawled, sitemap found or not, llms.txt found or not, shown as information); `CrawlStatus` for a crawl in progress; and a "Crawl again" button that reports the 409 and 429 messages from the server as they are.
- Before any crawl has finished, the overview shows the crawl progress and no score. When the score is null after a finished crawl, it says `"No score: no pages could be read."` and gives the stop reason.

**Tests:**
- `the_overview_shows_score_counts_and_top_suggestions`
- `a_site_still_crawling_shows_progress_and_no_score`
- `a_site_with_no_readable_pages_explains_why_there_is_no_score`
- `crawl_again_starts_a_crawl_and_shows_progress`
- `crawl_again_shows_the_daily_limit_message`
- `llms_txt_is_shown_as_information_not_as_a_problem`
- `an_unknown_site_shows_not_found`

**Commit:** `feat: add the site overview`

### Task 9: Suggestions list and one suggestion

**Files:** create `web/src/suggestions/{SuggestionsPage,SuggestionPage}.tsx`.

**Interfaces — produces:**
- Route `/sites/:id/suggestions`: the ranked list as a table (rank, title, category, severity, open count), with filters for status and category kept in the address so a filtered view can be reloaded and shared. Paged.
- Route `/suggestions/:id`: the title, why it matters, how to fix it, then the items as a table (target, status, first seen, evidence). Evidence is shown as readable lines, not raw JSON. Each row has Done, Dismiss and Reopen; the header has the same three for the whole suggestion. Dismissing the whole suggestion asks for confirmation.
- A status change updates the row at once and rolls back with a message if the server refuses.

**Tests:**
- `suggestions_are_listed_in_the_servers_order_with_their_counts`
- `changing_a_filter_updates_the_address_and_the_request`
- `a_filtered_address_loads_with_that_filter_applied`
- `no_open_suggestions_shows_a_clear_empty_state`
- `a_suggestion_shows_why_fix_and_each_item`
- `marking_an_item_done_updates_its_row`
- `a_failed_change_rolls_back_and_says_so`
- `dismissing_the_whole_suggestion_asks_first`
- `evidence_is_rendered_as_text_lines`

**Commit:** `feat: add the suggestions list and suggestion detail`

### Task 10: Pages, one page, and settings

**Files:** create `web/src/pages/{PagesPage,PageDetail}.tsx`, `web/src/sites/SettingsPage.tsx`.

**Interfaces — produces:**
- Route `/sites/:id/pages`: the latest crawl's pages as a paged table (address, status, words, response time, depth).
- Route `/pages/:id`: the page's facts (title, description, headings, canonical, word count, status, response time, whether it needed JavaScript, structured data found), the findings against it with links to their suggestions, and its extracted text in a scrolling box.
- Route `/sites/:id/settings`: delete the site. The button is enabled only after the site's host is typed exactly, and success returns to `/`.

**Tests:**
- `pages_are_listed_with_status_and_paging`
- `a_page_shows_its_facts_and_its_findings`
- `crawled_markup_is_shown_as_characters`: a title of `<img src=x onerror=alert(1)>` appears as text and no `img` element exists
- `a_crawled_javascript_link_is_not_made_clickable`
- `delete_stays_disabled_until_the_host_is_typed`
- `deleting_a_site_returns_to_the_list`

**Commit:** `feat: add the pages, page detail and site settings screens`

### Task 11: Browser flow, README and coverage

**Files:** create `web/playwright.config.ts`, `web/tests/e2e/flow.spec.ts`, `web/README.md`, `scripts/dev.sh`. Modify the root `README.md`.

- `scripts/dev.sh` starts the backend and the web app together and stops both on Ctrl-C.
- The browser test serves the built web app and answers the API with Playwright routes, because the real backend correctly refuses to crawl a local test site.

**Tests** in `flow.spec.ts`:
- `sign_in_add_a_site_watch_the_crawl_and_open_a_suggestion`: token sign-in, add a site, progress updates, overview with a score, open the top suggestion, mark one item done
- `with_the_backend_down_every_screen_says_so_and_recovers_on_retry`
- `an_expired_token_returns_to_sign_in`
- `nothing_is_cut_off_at_390_and_1280_pixels_wide`: measured element by element, as the landing page's test does
- `the_app_can_be_used_with_the_keyboard_alone`: tab to add a site and submit it, with focus always visible

**README:** what the web app is, the commands, that sign-in uses the development token, how to run both halves with `scripts/dev.sh`, and how to crawl a real site once `CRAWLER_CONTACT` is set.

**Command:**

```bash
export PATH="/opt/homebrew/bin:$PATH"
pnpm --dir web exec tsc --noEmit
pnpm --dir web exec vitest run tests/unit --maxWorkers=2 --coverage
pnpm --dir web build && pnpm --dir web exec playwright test
```

Expected: no type errors, every test passes, coverage 80 or higher. **Commit:** `test: add the web app browser flow and README`

---

## What the owner must supply

- `CRAWLER_CONTACT` in `backend/.env`: a real email address or page, before crawling any real site.
- The address of the company site to test on.
