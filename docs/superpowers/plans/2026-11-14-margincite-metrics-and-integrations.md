# Margincite Metrics and Integrations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give every site the seven numbers on a Semrush project card (AI Visibility, Mentions, Site Health, Visibility, Organic Traffic, Organic Keywords, Backlinks), target-keyword checks, and a passed/failed list of every check, with the integrations that feed them.

**Architecture:** Four new backend areas on the existing Rust service: keywords, Google Search Console, Bing Webmaster Tools, and AI visibility. Each stores its data per workspace, syncs through the existing Postgres job queue, and feeds the existing checks and suggestions. One metrics endpoint combines them. The web app gets a project card, new tabs and a Connections screen.

**Tech Stack:** Rust, axum, sqlx, reqwest, Postgres (existing). New crates: `chacha20poly1305` (or `aes-gcm`), `base64`. React web app (existing).

**Spec:** `docs/superpowers/specs/2026-11-13-margincite-design.md`. This plan fills in its phase 3 (AI visibility, Bing) and the Search Console plan, as the owner asked on 2026-11-14 with a Semrush project card as the reference.

## Global Constraints

Everything in the Global Constraints of `docs/superpowers/plans/2026-11-13-margincite-checks-and-web-app.md` still applies (PATH lines, memory limits, `margincite_dev` only, never `rustup update`, tests first, 80% coverage, workspace separation, envelope, no blanket `cargo fmt`, commit format with no co-author lines, branch `feat/web-app`). In addition:

- **Tests never call a real outside service.** Google, Bing, Anthropic, OpenAI, Perplexity and Gemini are always answered by wiremock in tests. Every client takes its base URL as a constructor argument for this.
- **Secrets at rest are encrypted.** Google refresh tokens and Bing API keys are stored encrypted with `MARGINCITE_SECRET_KEY` (32 random bytes, base64). They are never returned by any endpoint and never logged.
- **AI engine keys are server settings**, read from the environment, never stored in the database: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `PERPLEXITY_API_KEY`, `GEMINI_API_KEY`. An engine without a key is simply off.
- **Every feature works partly when its integration is missing.** A metric that cannot be computed is `null` with a `reason` string; nothing errors because Google or an engine is not set up.
- **Outside calls are bounded:** 60-second timeout for Google and Bing, 180 seconds for AI engines, explicit row limits, and no retries beyond the job queue's.
- **Outside services use a normal HTTP client, not the crawler's guarded client.** Their hosts are fixed in code, not typed by users.
- **Honest copy.** Numbers say where they come from ("from Google Search Console, last 28 days", "from Bing"). Approximations are named as such.

## Review Focus

1. **The OAuth callback is replayed, forged, or arrives for another workspace's site.** The state is single-use, expires after 10 minutes, and is bound to the workspace and site that started it; anything else returns to the app with an error and stores nothing. Tests in Task 3.
2. **Google or Bing answers 401, 403, 429 or 5xx, or returns an unexpected shape.** The integration is marked `needs_reconnecting` or the sync is retried by the queue; the rest of the site keeps working and the screen says what happened. Tests in Tasks 3, 4 and 5.
3. **An AI engine answers with no citations, refuses, pauses, or times out.** The answer is stored with an error or an empty citation list; one engine failing never fails the run. Tests in Task 5.
4. **A brand name that is a common word or a substring of another word** ("Apple" in "pineapple", "cbuddy" in "cbuddyai"). Mentions are matched on whole words, case-insensitively; a domain match counts. Tests in Task 5.
5. **A site with keywords but no Search Console, or Search Console but no keywords.** Keyword checks fall back to the best-matching crawled page; Visibility is `null` with a reason. Tests in Tasks 2 and 3.

---

## API contract (all under `/api/v1`, bearer token, envelope)

The web app is built against exactly this. Times are RFC 3339 strings.

**Integrations status** `GET /integrations` →
`{ secret_key_set: bool, google: { configured: bool }, engines: [{ engine: "claude"|"chatgpt"|"perplexity"|"gemini", configured: bool, model: string }] }`

**Keywords**
- `GET /sites/{id}/keywords` → `[{ id, keyword, target_url: string|null, landing_url: string|null, landing_source: "set"|"search_console"|"best_match"|"home"|null, position: number|null, clicks: int|null, impressions: int|null }]` (position, clicks and impressions from Search Console, last 28 days, `null` when not connected)
- `POST /sites/{id}/keywords` `{ keyword, target_url? }` → 201 the keyword. 422 for empty, over 100 characters, or a `target_url` outside the site; 409 duplicate (case-insensitive); 409 `"A site can have at most 50 keywords."`
- `PATCH /keywords/{id}` `{ target_url: string|null }`, `DELETE /keywords/{id}`
- Changing keywords does not re-run checks; the next crawl does.

**Checks list** `GET /sites/{id}/checks` →
`{ crawl_id: uuid|null, checks: [{ check_id, title, category, status: "passed"|"failed"|"not_run", open: int, reason: string|null }] }`. `not_run` when the check needs something missing (Search Console, keywords, an AI run), with `reason`.

**Metrics** `GET /sites/{id}/metrics` →
```
{ site_health: { score: int|null, previous: int|null, history: [{ at, score }], reason: string|null },
  ai_visibility: { percent: number|null, mentions: int|null, citations: int|null, answers: int, last_run_at: string|null,
                   engines: [{ engine, percent, mentions, answers }], reason: string|null },
  visibility: { percent: number|null, in_top_10: int|null, keywords: int, reason: string|null },
  organic: { clicks: int|null, impressions: int|null, keywords: int|null, avg_position: number|null,
             clicks_previous: int|null, keywords_previous: int|null, daily: [{ date, clicks, impressions }],
             source: "search_console", reason: string|null },
  backlinks: { total: int|null, linked_pages: int|null, top_pages: [{ url, count }], source: "bing", synced_at: string|null, reason: string|null } }
```
`reason` is a plain sentence such as `"Connect Google Search Console to see organic traffic."` whenever the value is `null`.

**Google Search Console**
- `GET /sites/{id}/integrations/google` → `{ status: "not_connected"|"choose_property"|"connected"|"needs_reconnecting", property: string|null, synced_at: string|null, error: string|null }`
- `POST /sites/{id}/integrations/google/start` → `{ auth_url }`. 422 when Google is not configured or `MARGINCITE_SECRET_KEY` is missing.
- `GET /oauth/google/callback?code&state` (no bearer; browser redirect) → 302 to `{PUBLIC_BASE_URL}/sites/{site_id}/connections?google=connected` or `?google=error&reason=<code>`.
- `GET /sites/{id}/integrations/google/properties` → `[{ property, permission }]`
- `PUT /sites/{id}/integrations/google` `{ property }` → the status; starts a sync.
- `POST /sites/{id}/integrations/google/sync` → 202. `DELETE /sites/{id}/integrations/google` → disconnects and deletes its data.
- `GET /sites/{id}/search/queries?page&limit&sort=clicks|impressions|position` → paged `[{ query, clicks, impressions, ctr, position, top_page }]`
- `GET /sites/{id}/search/pages?page&limit` → paged `[{ url, clicks, impressions, ctr, position }]`

**Bing Webmaster Tools**
- `GET /sites/{id}/integrations/bing` → `{ status: "not_connected"|"connected"|"needs_reconnecting", site_url: string|null, synced_at: string|null, error: string|null }`
- `PUT /sites/{id}/integrations/bing` `{ api_key, site_url }` → validates against Bing, then the status; starts a sync. 422 with Bing's reason when the key is wrong or the site is not in that Bing account.
- `POST /sites/{id}/integrations/bing/sync` → 202. `DELETE /sites/{id}/integrations/bing`.
- `GET /sites/{id}/backlinks?page&limit` → paged `[{ target_url, count }]`; `GET /sites/{id}/backlinks/sources?target_url=` → `[{ source_url, anchor_text }]`

**AI visibility**
- `GET /sites/{id}/ai/settings` / `PUT` `{ brand_name, aliases: string[], competitors: string[] }` (competitors are domains; at most 10 aliases, 10 competitors)
- `GET /sites/{id}/ai/prompts` → `[{ id, text, created_at }]`; `POST` `{ text }` (at most 25 per site, 10–300 characters); `DELETE /ai/prompts/{id}`
- `POST /sites/{id}/ai/prompts/suggest` → `{ suggestions: string[] }` (10 questions; uses Claude; 422 when no `ANTHROPIC_API_KEY`). Not saved until the person adds them.
- `POST /sites/{id}/ai/runs` → 202 `Run`. 422 when no engine is configured or no prompts exist; 409 while a run is queued or running; 429 after 3 manual runs in a UTC day.
- `GET /sites/{id}/ai/runs?page&limit`, `GET /ai/runs/{id}` → `Run = { id, status: "queued"|"running"|"completed"|"failed", trigger: "manual"|"scheduled", engines: string[], answers_total, answers_done, error, started_at, finished_at, created_at }`
- `GET /ai/runs/{id}/answers` → `[{ id, prompt_id, prompt, engine, model, mentioned: bool, cited: bool, citation_rank: int|null, cited_domains: string[], competitor_mentions: string[], error: string|null }]`
- `GET /ai/answers/{id}` → the above plus `answer_text` and `citations: [{ url, title, domain }]`
- `GET /sites/{id}/ai/domains` → top 20 domains cited across the latest completed run: `[{ domain, citations, answers, is_you: bool, is_competitor: bool }]`

---

### Task 1: Secrets, settings and integration status

**Files:** `src/secrets.rs`, `src/config.rs`, `src/integrations/{mod,status}.rs`, `.env.example`, `README.md`, tests.

**Interfaces — produces:**
- `Secrets::from_base64(key: &str) -> Result<Secrets, ConfigError>`, `fn seal(&self, plain: &str) -> String` (random nonce, base64 output), `fn open(&self, sealed: &str) -> Result<String, SecretError>`.
- `Config` gains `secret_key: Option<Secrets>` (`MARGINCITE_SECRET_KEY`), `public_base_url: Url` (`PUBLIC_BASE_URL`, default `http://localhost:5174`), `google: Option<GoogleCredentials { client_id, client_secret }>` (`GOOGLE_CLIENT_ID` and `GOOGLE_CLIENT_SECRET`, both or neither), and `engines: EngineSettings` with an optional key and a model per engine: `ANTHROPIC_API_KEY`/`CLAUDE_MODEL` (default `claude-opus-5-5`), `OPENAI_API_KEY`/`OPENAI_MODEL` (default `gpt-5.6`), `PERPLEXITY_API_KEY`/`PERPLEXITY_MODEL` (default `sonar`), `GEMINI_API_KEY`/`GEMINI_MODEL` (default `gemini-3.5-flash`). An invalid secret key, or only one of the two Google values, fails start-up with a message naming the variable.
- `GET /integrations` as in the contract. Never returns a key.

**Tests:** seal/open round trip; open fails on a changed byte; two seals of one value differ; a key that is not 32 bytes is refused; each new setting and its default; half-set Google credentials refused; `/integrations` reports configured engines and never includes a key.

**Commit:** `feat: add encrypted secrets and integration settings`

### Task 2: Target keywords, keyword checks and the checks list

**Files:** `migrations/0007_keywords.sql`, `src/keywords/{mod,repo,routes,landing}.rs`, `src/checks/{keywords.rs, catalog.rs, context.rs, mod.rs}`, `src/crawl/repo.rs` (`load_context`), `src/suggestions/routes.rs` (checks list), tests.

**Interfaces — produces:**
- Table `site_keywords(id, workspace_id, site_id fk cascade, keyword text, target_url text null, created_at)`, unique on `(site_id, lower(keyword))`.
- `CrawlContext` gains `keywords: Vec<KeywordTarget { keyword: String, target_url: Option<String> }>` and `search: Option<SearchData>` (filled by Task 3; `None` until then).
- `landing::choose(keyword, target_url, pages, search) -> Option<(url, LandingSource)>`: the set URL when it was crawled; else the page Search Console shows most impressions for that query; else the crawled working page whose title, main heading and description contain the most of the keyword's words; else the home page.
- New checks, category content, page target, evidence `{ keyword, url, landing_source }`, target string `"<keyword>" on <url>`:

  | id | sev | effort | Triggers when |
  |---|---|---|---|
  | `keyword_not_in_title` | 2 | 1 | none of the keyword's words in the title (all words for a multi-word keyword) |
  | `keyword_not_in_h1` | 2 | 1 | not in any main heading |
  | `keyword_not_in_description` | 1 | 1 | not in the meta description |
  | `keyword_not_in_body` | 2 | 2 | not in the page text |
  | `keyword_stuffing` | 2 | 1 | the keyword is more than 3% of the page's words, or appears 3+ times in the title |
  | `keyword_cannibalization` | 2 | 3 | site-level per keyword, Search Console only: two or more pages each get at least 20% of the keyword's impressions. Evidence lists the pages and their shares |

  Matching is case-insensitive, on whole words, after collapsing whitespace and stripping punctuation.
- `GET /sites/{id}/checks` as in the contract. Status rules: `not_run` for keyword checks with no keywords, for Search Console checks with no Search Console data, for AI checks with no completed AI run; `failed` when the check has open items in the latest completed crawl; otherwise `passed`.
- Keyword endpoints as in the contract.

**Tests:** each new check firing and not firing; multi-word keywords; punctuation and case; whole-word matching ("buddy" does not match "cbuddy"); landing choice in each of its four orders; target URL outside the site is 422; 51st keyword is 409; duplicates by case are 409; another workspace's keyword is 404; the checks list marks keyword checks `not_run` with no keywords and `passed`/`failed` correctly after a crawl.

**Commit:** `feat: add target keywords, keyword checks and the checks list`

### Task 3: Google Search Console

**Files:** `migrations/0008_search_console.sql`, `src/integrations/google/{mod,oauth,client,sync,repo,routes}.rs`, `src/checks/search.rs`, `src/suggestions/rank.rs` (traffic-weighted reach), tests.

**Interfaces — produces:**
- Tables:
  - `oauth_states(id uuid pk, workspace_id, site_id, provider text, created_at)`, single-use, valid 10 minutes.
  - `google_connections(site_id pk fk cascade, workspace_id, refresh_token_sealed text, property text null, status text, error text null, synced_at timestamptz null, created_at)`.
  - `search_rows(id, workspace_id, site_id fk cascade, query text, page text, clicks int, impressions int, ctr double, position double)` for the last 28 days, replaced on each sync.
  - `search_daily(workspace_id, site_id fk cascade, date date, clicks int, impressions int, primary key (site_id, date))` for the last 90 days.
  - `search_previous(site_id pk fk cascade, workspace_id, clicks int, impressions int, keywords int)` for the 28 days before.
- OAuth: authorisation URL `https://accounts.google.com/o/oauth2/v2/auth` with `response_type=code`, `scope=https://www.googleapis.com/auth/webmasters.readonly`, `access_type=offline`, `prompt=consent`, `redirect_uri={PUBLIC_BASE_URL}/api/v1/oauth/google/callback`, `state=<oauth_states.id>`. Token exchange and refresh at `https://oauth2.googleapis.com/token`. Access tokens are kept only in memory for one sync.
- Client against `https://www.googleapis.com/webmasters/v3/`: `GET sites` (properties), `POST sites/{urlencoded property}/searchAnalytics/query` with `{ startDate, endDate, dimensions, rowLimit: 25000, dataState: "final" }`. Dates end 3 days before today (Search Console's delay).
- `GoogleSyncJob`, job kind `google_sync`, payload `{ workspace_id, site_id }`: query+page rows for 28 days, date rows for 90 days, totals for the previous 28 days. Enqueued on property choice, on "Sync now", after every completed crawl, and daily (only one queued per site). A 400/401 from the token endpoint marks the connection `needs_reconnecting` with a plain error; 429 and 5xx fail the job for the queue to retry.
- `SearchData { pages: HashMap<String, PageStats>, queries: Vec<QueryRow> }` loaded into `CrawlContext.search` when a connection is `connected`. Page URLs are matched after `crawl::urls::normalize` on both sides.
- New checks, category technical unless noted:

  | id | sev | effort | Triggers when |
  |---|---|---|---|
  | `search_low_ctr` | 2 | 1 | a page with 200+ impressions whose CTR is below half the expected CTR for its average position (expected: 1→28%, 2→15%, 3→10%, 4→7%, 5→5%, 6–10→3%, 11+→1%). Evidence: impressions, clicks, CTR, position |
  | `search_striking_distance` | 2 | 2 | a query+page with average position 5 to 20 and 100+ impressions. Target `"<query>" on <url>` |
  | `search_no_impressions` | 1 | 2 | a working crawled page in the sitemap with zero impressions in 28 days. Title says it is an approximation: "Pages with no Google search impressions in 28 days" |

- Reach (spec 6.6): when the site has Search Console data, a page-level suggestion's reach is the share of the site's impressions landing on its open pages instead of the share of pages; `info.reach_basis` in the summary becomes `"impressions"`.
- Endpoints as in the contract. `GET /sites/{id}/keywords` fills position, clicks and impressions from `search_rows`.
- Metrics: organic clicks, impressions, distinct queries with impressions, impression-weighted average position, previous-period values, daily series; Visibility = share of the site's target keywords whose impression-weighted average position is 10 or better, `null` with a reason when there are no keywords or no Search Console.

**Tests** (wiremock for Google's three hosts): start returns a URL with every required parameter; callback stores a sealed token and redirects to connected; a replayed state, an expired state, a state for another workspace, and a missing code each redirect with an error and store nothing; the token endpoint's `invalid_grant` marks the connection `needs_reconnecting`; properties are listed; sync stores rows and replaces the previous sync's; a 429 fails the job for retry; the three checks firing and not firing; traffic-weighted reach; keywords show position from search rows; metrics with and without a connection; disconnect deletes the token and rows; the sealed token never appears in any response.

**Commit:** `feat: connect Google Search Console for organic traffic, keywords and visibility`

### Task 4: Bing Webmaster Tools backlinks

**Files:** `migrations/0009_bing.sql`, `src/integrations/bing/{mod,client,sync,repo,routes}.rs`, tests.

**Interfaces — produces:**
- Tables `bing_connections(site_id pk fk cascade, workspace_id, api_key_sealed, site_url, status, error, synced_at, created_at)`, `backlink_targets(workspace_id, site_id fk cascade, target_url, count, primary key (site_id, target_url))`, `backlink_sources(workspace_id, site_id fk cascade, target_url, source_url, anchor_text)`.
- Client against `https://ssl.bing.com/webmaster/api.svc/json/`, with `apikey` as a query parameter: `GetUserSites`, `GetLinkCounts?siteUrl&page` (0-based pages, stop when a page is empty or after 20 pages), `GetUrlLinks?siteUrl&link&page` for the 10 most-linked targets (first page only). Responses are wrapped in `{ "d": ... }`; parse field names case-insensitively and tolerate missing fields, because the documented shapes are old. Record any field you could not map in the log, not as an error.
- Sync job kind `bing_sync`; on connect, "Sync now", and weekly. 401/403 marks `needs_reconnecting`.
- Metrics: backlinks total (sum of counts), linked pages (number of targets), top 5 targets.

**Tests** (wiremock): a key whose account lacks the site is 422 with a plain reason; a good key stores a sealed key and syncs; link counts across two pages are summed; sources are stored for the top targets; an unexpected shape yields an empty result and no crash; 401 marks needs reconnecting; metrics with and without a connection; another workspace cannot read backlinks.

**Commit:** `feat: add backlinks from Bing Webmaster Tools`

### Task 5: AI visibility

**Files:** `migrations/0010_ai_visibility.sql`, `src/ai/{mod,engines/{mod,claude,openai,perplexity,gemini},detect,suggest,run,repo,routes}.rs`, `src/checks/ai_answers.rs`, tests.

**Interfaces — produces:**
- Tables `ai_settings(site_id pk, workspace_id, brand_name, aliases jsonb, competitors jsonb)`, `ai_prompts(id, workspace_id, site_id fk cascade, text, created_at)`, `ai_runs(id, workspace_id, site_id fk cascade, status, trigger, engines jsonb, answers_total, answers_done, error, started_at, finished_at, created_at)`, `ai_answers(id, workspace_id, run_id fk cascade, site_id, prompt_id, prompt_text, engine, model, answer_text, citations jsonb, mentioned bool, cited bool, citation_rank int null, competitor_mentions jsonb, error text null, input_tokens int, output_tokens int, searches int, created_at)`.
- `#[async_trait] trait Engine: Send + Sync { fn id(&self) -> EngineId; fn model(&self) -> &str; async fn ask(&self, prompt: &str) -> Result<EngineAnswer, EngineError>; }` with `EngineAnswer { text: String, citations: Vec<Citation { url: String, title: Option<String>, domain: String }>, input_tokens, output_tokens, searches }`. One HTTP request style per engine, each tested against recorded-shape fixtures:
  - **Claude** (raw HTTP, no Rust SDK exists): `POST https://api.anthropic.com/v1/messages`, headers `x-api-key`, `anthropic-version: 2026-10-21`, `anthropic-beta: server-side-fallback-2026-07-01`, `content-type: application/json`. Body `{ model, max_tokens: 4096, output_config: { effort: "low" }, fallbacks: "default", tools: [{ type: "web_search_20260209", name: "web_search", max_uses: 5 }], messages: [{ role: "user", content: prompt }] }`. Do not send `thinking` or `temperature`. Text is the concatenated `text` blocks; citations are every `citations[]` entry of type `web_search_result_location` (`url`, `title`). `stop_reason: "pause_turn"`: send the assistant content back unchanged as the next assistant message and repeat, at most 3 times. `stop_reason: "refusal"`: an `EngineError::Refused`. `usage.server_tool_use.web_search_requests` is the search count. A `web_search_tool_result` whose `content` is an object is a search error: record it, keep the answer.
  - **ChatGPT**: `POST https://api.openai.com/v1/responses`, bearer key, body `{ model, input: prompt, tools: [{ type: "web_search" }] }`. Text from `output[]` items of type `message`, `content[]` of type `output_text`; citations from their `annotations[]` of type `url_citation` (`url`, `title`).
  - **Perplexity**: `POST https://api.perplexity.ai/chat/completions`, bearer key, body `{ model, messages: [{ role: "user", content: prompt }] }`. Text from `choices[0].message.content`. Citations from `search_results[].url`/`title` when present, else `citations[]` strings, checked at the top level and inside `choices[0].message`.
  - **Gemini**: `POST https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent`, header `x-goog-api-key`, body `{ contents: [{ parts: [{ text: prompt }] }], tools: [{ google_search: {} }] }`. Text from `candidates[0].content.parts[].text`. Citations from `candidates[0].groundingMetadata.groundingChunks[].web` (`uri`, `title`). The `uri` is a Google redirect, so the domain comes from `title`; do not follow the redirect.
- `detect::mentioned(text, brand, aliases, site_host) -> bool`: whole-word, case-insensitive match of the brand name or an alias, or the site's host (with or without `www.`) appearing in the text. `detect::cited(citations, site_host) -> (bool, Option<rank>)`: a citation whose domain is the site's host or a subdomain of it; rank is its 1-based position among citations. `detect::competitors(text, citations, competitors) -> Vec<String>`.
- `suggest::suggest_prompts(claude, site_title, description, keywords, brand) -> Vec<String>`: one Claude request (same headers, no web search) using structured outputs (`output_config.format` with a JSON schema of `{ questions: string[] }`) asking for 10 questions a potential customer would ask an AI assistant when looking for what the site offers, written without naming the brand. Validate and trim; drop duplicates and any naming the brand.
- `AiRunJob`, job kind `ai_run`: for each prompt and each configured engine, ask with at most 2 requests in flight per engine; store every answer, including failed ones with `error`; update `answers_done` as it goes; mark the run completed when all are stored (failed if every answer failed). Scheduled weekly per site once the site has prompts and an engine is configured; only one queued per site.
- New check `ai_not_mentioned`, category ai_readiness, sev 2, effort 3, site-level target per prompt (`"<prompt>"`): in the latest completed run, no engine mentioned or cited the site for that prompt. Evidence: engines asked, the domains they cited instead, competitor mentions. Runs on the next crawl and immediately after an AI run (re-reconcile the latest crawl's suggestions when a run completes).
- Metrics: AI visibility percent = answers where the site was mentioned or cited ÷ answers without an error, latest completed run; mentions = answers mentioning the site; citations = answers citing it; per-engine breakdown.
- Endpoints as in the contract.

**Tests** (wiremock per engine): each engine parses text and citations from a realistic response; Claude's pause_turn is continued and its refusal recorded; Claude's search error inside a result is tolerated; a 500 or timeout becomes an answer with an error and the run still completes; mention detection on whole words, aliases, domains, and not on substrings; citation rank; competitor detection; prompt limits (25, length bounds); manual run limits (409 while running, 429 on the 4th in a day, 422 with no engine or no prompts); suggestions parse and drop brand-naming questions; `ai_not_mentioned` fires per prompt and resolves when a later run mentions the site; metrics; another workspace cannot read runs, answers or prompts; no API key ever appears in a stored row or a response.

**Commit:** `feat: track AI visibility across Claude, ChatGPT, Perplexity and Gemini`

### Task 6: Metrics endpoint

**Files:** `src/metrics/{mod,routes}.rs`, tests. (May be built progressively inside Tasks 2–5; this task finishes it and its tests.)

`GET /sites/{id}/metrics` exactly as in the contract, combining crawl score history (last 12 completed crawls), Task 3's organic and visibility numbers, Task 4's backlinks and Task 5's AI numbers. Each block computed independently; one block failing returns that block with `reason: "Could not load this number right now."` and logs the cause.

**Tests:** a new site with nothing connected returns every block with its reason; a fully connected fixture returns every number; one failing block does not fail the others; another workspace's site is 404.

**Commit:** `feat: add the site metrics endpoint`

### Task 7: Web app

**Files:** under `web/src/`: `api/{types,hooks}.ts`, `sites/{SitesPage,SiteOverview,SitePage}.tsx`, new `metrics/MetricCards.tsx`, `keywords/KeywordsPage.tsx`, `checks/ChecksPage.tsx`, `search/SearchPage.tsx`, `backlinks/BacklinksPage.tsx`, `ai/{AiPage,AnswerPage,PromptsPanel,AiSettingsPanel}.tsx`, `connections/ConnectionsPage.tsx`, tests in `web/tests/unit/` and one new e2e flow.

Keep the current light dashboard look already in `web/src/index.css` and the existing components.

- **Project card** (`MetricCards`): seven tiles in this order: AI Visibility, Mentions, Site Health, Visibility, Organic Traffic, Organic Keywords, Backlinks. Each shows the number, a small change against the previous period where the API gives one, and its source in a tooltip or caption. A `null` value shows "n/a" with the `reason` and a link to the screen that fixes it (Connections, Keywords or AI). Used on the sites list (one card per site) and at the top of the overview.
- **Tabs** on the site page: Overview, Suggestions, Checks, Keywords, AI Visibility, Search, Backlinks, Pages, Connections, Settings. On narrow screens the tab row scrolls sideways inside itself.
- **Checks**: every check grouped by category with passed, failed (with open count, linking to its suggestion) and not run (with the reason), like the Semrush ideas list.
- **Keywords**: add, remove, set a target page; table of keyword, landing page and how it was chosen, position, clicks, impressions.
- **AI Visibility**: brand, aliases and competitors; prompts with add, remove and "Suggest questions" (shows the suggestions, each with an Add button); "Run now" with progress while a run is going; results as a grid of prompts by engines, each cell showing cited, mentioned or not, opening the answer (answer text as plain text, its citations as safe links); top cited domains with you and competitors marked; engines without a key listed as "not set up" with the variable name to set.
- **Search**: clicks and impressions over 90 days as a simple line or bar chart built from SVG (no chart library), totals with change, top queries and top pages tables. Not connected: an explanation and a link to Connections.
- **Backlinks**: total and linked pages, targets table, and a target's sources on demand. Not connected: explanation and link.
- **Connections**: Google Search Console (connect button that goes to Google, property chooser after return, status, sync now, disconnect); Bing (API key field, site URL field, save, status, sync now, disconnect, a short note on where to find the key in Bing Webmaster Tools); AI engines (configured or not, model). When the server has no `MARGINCITE_SECRET_KEY` or no Google credentials, say exactly which variable to set in `backend/.env`. Handles `?google=connected` and `?google=error&reason=` on return.

**Tests:** every new screen's loading, empty, error and main states; the card's n/a state with its link; the property chooser after a Google return; the Bing form showing the server's 422 reason; suggestion Add buttons; run progress polling stopping when the run ends; crawled answer text and citation links rendered safely; the tab row at 390 px; one e2e flow: add keywords, open AI Visibility, add a suggested prompt, run, see the grid, open an answer. Coverage of `web/src` at least 80%.

**Commit:** `feat: add the project card, keywords, checks, AI visibility, search, backlinks and connections screens`

---

## What the owner must supply to light up each number

| Number | Needs |
|---|---|
| Site Health, keyword checks, checks list | nothing |
| Organic Traffic, Organic Keywords, Visibility | a Google Cloud OAuth client (`GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`) with `http://localhost:5174/api/v1/oauth/google/callback` as a redirect URI, the site verified in Search Console, and target keywords for Visibility |
| Backlinks | a Bing Webmaster Tools API key and the site verified in Bing |
| AI Visibility, Mentions | at least one of `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `PERPLEXITY_API_KEY`, `GEMINI_API_KEY`, and some prompts |
| Any integration | `MARGINCITE_SECRET_KEY` (generated locally) |
