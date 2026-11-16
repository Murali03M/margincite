# Margincite Core (SEO, AIO, GEO) and Setup-Through-the-UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deepen the three core areas (SEO, AI Overviews, and visibility in AI assistants) and let a person set up every option in the web app, with no settings file to edit.

**Architecture:** A per-workspace settings store holds every credential and option, encrypted, and every feature reads from it first and from the environment second. Three new data sources plug into the existing checks, suggestions and metrics: Grok (a fifth AI engine), Google PageSpeed Insights (page speed), and DataForSEO (search results for rank tracking and AI Overviews, and keyword volumes). New rule-based "answer readiness" checks need no outside data.

**Tech Stack:** Existing Rust backend (axum, sqlx, reqwest, Postgres job queue) and React web app. No new frameworks.

**Direction from the owner (2026-11-15):** Margincite is a platform people set up themselves by connecting their own providers, like T3 Code. The core is SEO, AIO and GEO. Everything must be configurable in the UI. No business layer (billing, plans, team roles) for now. An in-app assistant and connectors inside the Claude, Grok and ChatGPT apps come later.

## Global Constraints

Everything in the Global Constraints of `docs/superpowers/plans/2026-11-14-margincite-metrics-and-integrations.md` and of `docs/superpowers/plans/2026-11-13-margincite-checks-and-web-app.md` still applies: PATH lines, memory limits, `margincite_dev` only, never `rustup update`, tests first, 80% coverage, workspace separation, the response envelope, no blanket `cargo fmt`, commit format with no co-author lines, branch `feat/web-app`, tests never call a real outside service, outside calls bounded and capped at 20 MB, honest copy. In addition:

- **Secrets entered in the UI are write-only.** An endpoint may say a secret is set and when; it never returns the value, not even partly. Secrets are sealed with the existing `Secrets` and never logged.
- **Setting lookup order:** the workspace's saved setting, then the environment variable of the same meaning, then the built-in default. Existing `.env` files keep working.
- **Paid sources are never called without the workspace's own credentials**, and every paid call is counted in a usage table the UI shows.
- **A feature whose source is not set up is `not_run` / `null` with a plain reason and a link to Settings.** Nothing errors.
- **New checks declare their needs** with `CheckDef.needs` so they are left out of the score when their source is missing.

## Review Focus

1. **A secret is saved, then the settings are read back.** No endpoint, log line or error ever contains it. Tests in Task 1.
2. **A workspace with no credentials for a paid source.** No paid call is made, by any path including scheduled jobs. Tests in Tasks 4 and 5.
3. **A provider answers 401, 402 (out of credit), 429 or 5xx, or an unexpected shape.** The setting is marked with a plain error, old data is kept, nothing shows a false zero. Tests in Tasks 2, 4, 5 and 6.
4. **A tracked keyword where the site does not appear in the results at all.** It is stored as "not in the top 100", not as rank 0 or an error. Test in Task 4.
5. **Changing the site's country or language.** Old rankings are kept with their location; new ones are tracked separately, never mixed in one trend. Test in Task 4.

---

## API contract (all under `/api/v1`, bearer token, envelope)

**Settings**
- `GET /settings` → `{ providers: [ProviderOut], google: { client_id_set, client_secret_set, source }, dataforseo: { login_set, password_set, source, status, error, balance: number|null, checked_at }, pagespeed: { key_set, source }, crawler: { contact, contact_source }, secret_key_ready: bool }`
  - `ProviderOut = { provider: "claude"|"chatgpt"|"grok"|"gemini"|"perplexity", key_set: bool, source: "workspace"|"environment"|"none", model: string, default_model: string, status: "ok"|"error"|"unchecked", error: string|null, checked_at: string|null }`
- `PUT /settings/providers/{provider}` `{ api_key?: string, model?: string }` → `ProviderOut`. Saves, then tests the key with one cheap call. A failed test still saves and reports `status: "error"` with the provider's reason.
- `POST /settings/providers/{provider}/test` → `ProviderOut`. `DELETE /settings/providers/{provider}` removes the workspace's key and model.
- `PUT /settings/google` `{ client_id, client_secret }`, `DELETE /settings/google`
- `PUT /settings/dataforseo` `{ login, password }` (tested against the account endpoint, which also returns the balance), `POST /settings/dataforseo/test`, `DELETE /settings/dataforseo`
- `PUT /settings/pagespeed` `{ api_key }`, `DELETE /settings/pagespeed`
- `PUT /settings/crawler` `{ contact }` (an email address or an http(s) URL)
- `GET /sites/{id}/settings` / `PUT` `{ display_name?, page_cap? (10–5000), country? , language?, device? ("desktop"|"mobile"), track_rankings? bool, ranking_frequency? ("daily"|"weekly") }` → the site settings, plus `countries: [{ code, name }]` and `languages: [{ code, name }]` to choose from.
- `GET /usage?days=30` → `[{ source: "dataforseo"|"pagespeed"|<provider>, kind, calls, estimated_cost_usd: number|null, day }]`

**Rankings and AI Overviews**
- `GET /sites/{id}/rankings` → `{ location: { country, language, device }, checked_at, keywords: [{ keyword_id, keyword, position: int|null, previous: int|null, url: string|null, ai_overview: "cited"|"present"|"absent"|"unknown", top: [{ position, domain, url, title }] (up to 10), history: [{ at, position }] (up to 30) }] }`. `position: null` with data means not in the top 100.
- `POST /sites/{id}/rankings/check` → 202; checks every keyword now. 422 without DataForSEO credentials; 409 while a check is queued or running; 429 after 5 manual checks in a UTC day.
- `GET /sites/{id}/ai-overviews` → `[{ keyword_id, keyword, status: "cited"|"present"|"absent"|"unknown", cited_domains: string[], your_urls: string[], checked_at }]`

**Keyword research**
- `POST /sites/{id}/keywords/volumes` → refreshes search volume for the site's keywords; returns 202. `GET /sites/{id}/keywords` gains `search_volume: int|null`, `competition: string|null`, `cpc: number|null`.
- `POST /sites/{id}/keywords/ideas` `{ seed: string }` → `{ ideas: [{ keyword, search_volume, competition, cpc }] }` (up to 50; nothing saved until the person adds one). 422 without credentials; 429 after 20 a day per workspace.

**Page speed**
- `GET /sites/{id}/speed` → `{ checked_at, pages: [{ url, strategy: "mobile", performance: int|null, lcp_ms, cls, inp_ms, tbt_ms, field_data: bool, error: string|null }] }`
- `POST /sites/{id}/speed/check` → 202 (home page plus the 4 pages with the most impressions or, without Search Console, the 4 shallowest pages). 429 after 5 a day.

**Metrics** gains `rankings: { tracked: int, in_top_3: int|null, in_top_10: int|null, average_position: number|null, checked_at, reason }`, `ai_overviews: { tracked: int, present: int|null, cited: int|null, checked_at, reason }`, `speed: { performance: int|null, checked_at, reason }`. When rankings exist, `visibility.percent` is the share of tracked keywords in the top 10 from rankings, and its source says so.

---

### Task 1: Settings store and UI-configurable credentials

**Files:** `migrations/0013_settings.sql`, `src/settings/{mod,repo,routes,resolve,test_provider}.rs`, changes in `src/config.rs`, `src/secrets.rs`, `src/main.rs`, `src/integrations/google/*`, `src/integrations/status.rs`, `src/ai/{run,routes,suggest}.rs`, `src/crawl/client.rs` call sites, `src/sites/{repo,routes}.rs`, tests.

**Interfaces — produces:**
- Tables: `workspace_settings(workspace_id, key text, value_sealed text null, value_plain text null, updated_at, primary key (workspace_id, key))`; `provider_status(workspace_id, provider, status, error, checked_at, primary key (workspace_id, provider))`; `usage_events(id, workspace_id, source, kind, calls int, estimated_cost_usd double null, created_at)`; `sites` gains `country text default 'US'`, `language text default 'en'`, `device text default 'desktop'`, `track_rankings bool default true`, `ranking_frequency text default 'weekly'`.
- Setting keys: `provider.<name>.api_key` (sealed), `provider.<name>.model`, `google.client_id`, `google.client_secret` (sealed), `dataforseo.login`, `dataforseo.password` (sealed), `pagespeed.api_key` (sealed), `crawler.contact`.
- `settings::resolve::{provider(pool, config, ws, name) -> ResolvedProvider { api_key: Option<String>, model: String, source }, google(..) -> Option<GoogleCredentials>, dataforseo(..) -> Option<(String, String)>, pagespeed_key(..) -> Option<String>, crawler_contact(..) -> String }`. Every existing reader of `Config.engines`, `Config.google` and `Config.crawler_contact` switches to these.
- `usage::record(pool, ws, source, kind, calls, estimated_cost_usd)`.
- **Secret key bootstrap:** when `MARGINCITE_SECRET_KEY` is not set, the backend creates `backend/.data/secret.key` (mode 0600, git-ignored) with 32 random bytes on first start and uses it, so nothing needs to be typed. `secret_key_ready` is then always true. An unreadable or wrong-length file stops start-up with a plain message.
- `CRAWLER_CONTACT` is no longer required at start-up. With none set anywhere, adding a site or starting a crawl is refused with 422 `"Add a contact address in Settings before crawling. It is sent with every request the crawler makes."`
- Provider tests, one cheap request each, with base URLs injectable for tests: Claude `GET /v1/models`; ChatGPT `GET /v1/models`; Grok `GET https://api.x.ai/v1/models`; Gemini `GET /v1beta/models` with `x-goog-api-key`; Perplexity a 1-token chat completion. 401/403 → `"The key was rejected."`; other failures → a plain sentence with the status code.
- Endpoints for Settings, site settings and usage as in the contract. The existing `GET /integrations` keeps working and reflects the workspace's settings.

**Tests:** round trip of each setting; secrets never appear in any response (assert on the raw body for every settings endpoint) or in captured logs; workspace value beats environment, environment beats default; another workspace cannot read or change settings; provider test success, 401, and network failure; saving a bad key still saves and reports the error; AI runs and Google sign-in use workspace credentials when set and environment ones otherwise; the secret key file is created once, reused, and refused when malformed; crawling without a contact is 422 and works after one is saved; site settings validation (page cap bounds, unknown country, unknown language); usage rows are written and listed per day.

**Commit:** `feat: let every credential and option be set up in the app`

### Task 2: Grok as an AI engine

**Files:** `src/ai/engines/{grok.rs,mod.rs}`, `src/config.rs` (`XAI_API_KEY`, `GROK_MODEL` default `grok-4.7`), everything that lists engines (now five, order: claude, chatgpt, grok, gemini, perplexity), tests.

`POST https://api.x.ai/v1/responses`, bearer key, body `{ model, input: [{ role: "user", content: prompt }], tools: [{ type: "web_search" }] }`. Text from `output[]` items of type `message`, `content[]` of type `output_text`. Citations, in this order of preference and merged without duplicates: `annotations[]` of type `url_citation` on those content parts (`url`, `title`), then a top-level `citations` array (strings or objects with `url`). Read xAI's current docs (`https://docs.x.ai/developers/tools/web-search` and its linked Citations page) before writing the parser and say in the report which shape is documented.

**Tests:** parses text and both citation shapes; 401, 429, 5xx and an unexpected shape become an answer with an error; Grok appears in settings, `/integrations`, runs and metrics; a workspace with only a Grok key can run AI visibility.

**Commit:** `feat: add Grok as an AI visibility engine`

### Task 3: Answer-readiness checks (AIO, no outside data)

**Files:** `src/crawl/extract.rs` (new facts), `src/checks/{answer.rs,catalog.rs,mod.rs}`, tests.

New `PageFacts` fields, all from the HTML already fetched: `question_headings: u32` (headings ending in `?` or starting with who, what, when, where, why, how, can, does, is, are, should), `lists: u32`, `tables: u32`, `first_paragraph_words: u32`, `faq_schema: bool`, `howto_schema: bool`, `article_schema: bool`, `organization_schema: bool`, `breadcrumb_schema: bool`, `outbound_citations: u32` (links to other hosts inside the main text). Old stored pages without these fields are treated as unknown, not as failing.

New checks, category `ai_readiness`, page target, applied only to working HTML pages with 300 or more words that are not the home page, unless noted:

| id | sev | effort | Triggers when |
|---|---|---|---|
| `answer_no_question_headings` | 1 | 2 | no heading is phrased as a question |
| `answer_long_opening` | 1 | 1 | the first paragraph is longer than 80 words, so there is no short direct answer near the top |
| `answer_no_lists_or_tables` | 1 | 2 | 600 or more words and no list or table |
| `answer_no_sources` | 1 | 2 | 600 or more words and no link to an outside source |
| `schema_no_article` | 2 | 2 | an article-like page (has a date or an author) with no Article, BlogPosting or NewsArticle structured data |
| `schema_no_organization` | 2 | 1 | site-level: the home page has no Organization or WebSite structured data |
| `schema_faq_missing` | 1 | 2 | 3 or more question headings and no FAQPage structured data |

Copy says these make a page easier for AI answers to quote; it never promises an AI Overview or a citation.

**Tests:** one per check firing and one not firing; pages under the word limits are skipped; pages stored before this change are skipped; schema detection across `@graph`, arrays and nested `@type` arrays.

**Commit:** `feat: add answer-readiness checks for AI Overviews`

### Task 4: Rank tracking and AI Overview tracking (DataForSEO search results)

**Files:** `migrations/0014_rankings.sql`, `src/integrations/dataforseo/{mod,client,locations}.rs`, `src/rankings/{mod,repo,run,routes}.rs`, `src/checks/rankings.rs`, `src/metrics/{rankings,ai_overviews,visibility}.rs`, tests.

Read DataForSEO's current docs before writing the parser (`https://docs.dataforseo.com/v3/serp/google/organic/live/advanced/`, which is long: read on with offsets until the `ai_overview` item is covered; and `https://docs.dataforseo.com/v3/appendix/user_data/`).

- Client: HTTP Basic auth with the workspace's login and password; base `https://api.dataforseo.com`, injectable for tests. `POST /v3/serp/google/organic/live/advanced` with one task `[{ keyword, location_code, language_code, device, depth: 100, load_async_ai_overview: true }]`; items at `tasks[0].result[0].items[]`. `GET /v3/appendix/user_data` for the credential test and balance. A task `status_code` other than 20000 is an error with its `status_message`; 40200-range payment errors become `"Your DataForSEO balance has run out."`.
- `locations.rs`: a built-in table of at least these countries with DataForSEO location codes: US 2840, India 2356, UK 2826, Canada 2124, Australia 2036, Germany 2276, France 2250, Singapore 2702, UAE 2784, Netherlands 2528; and languages en, hi, de, fr, es, nl, ar. Confirm the codes against DataForSEO's locations documentation.
- Tables: `rank_checks(id, workspace_id, site_id fk cascade, keyword_id, keyword text, country, language, device, position int null, url text null, ai_overview text, ai_cited_domains jsonb, ai_your_urls jsonb, top jsonb, checked_at)`; `rank_runs(id, workspace_id, site_id fk cascade, status, trigger, keywords_total, keywords_done, error, started_at, finished_at, created_at)` with one active run per site.
- A result belongs to the site when its domain is the site's host or a subdomain of it (ignoring `www.`). `position` is the best `rank_absolute` among the site's organic items, `null` when none in the top 100. `ai_overview`: `absent` when the results have no AI Overview item, `cited` when one of its references is the site's, `present` otherwise, `unknown` when the item could not be read.
- Job kind `rank_run`: one request per keyword, at most 4 in flight; records usage per call (estimate $0.002 plus $0.002 for the AI Overview option per keyword; label it an estimate). Scheduled per site by `ranking_frequency` when `track_rankings` is on, the site has keywords, and the workspace has DataForSEO credentials; otherwise not scheduled and not called.
- Checks, both `needing(.., &[Need::Rankings])` (add the need and its availability query):

  | id | category | sev | effort | Triggers when |
  |---|---|---|---|---|
  | `rank_dropped` | technical | 2 | 2 | a keyword fell 5 or more places since the previous check in the same location, or left the top 100 |
  | `ai_overview_not_cited` | ai_readiness | 2 | 3 | a keyword's results show an AI Overview that cites other sites and not this one. Evidence lists the cited domains |

- Endpoints and metrics as in the contract. History is per (keyword, country, language, device).

**Tests** (wiremock): credential test success, 401 and out-of-balance; a keyword in position 7; a keyword absent from the top 100 stored as `position: null`; subdomain match; AI Overview absent, present and cited; an unreadable AI Overview item is `unknown`; a failed keyword does not fail the run; usage recorded per keyword; no call at all without credentials (manual 422, scheduled skipped); 409 and 429 limits; changing country starts a separate history; both checks firing and resolving; metrics with and without data; another workspace cannot read rankings.

**Commit:** `feat: track keyword rankings and Google AI Overviews`

### Task 5: Keyword volumes and ideas

**Files:** `migrations/0015_keyword_metrics.sql`, `src/integrations/dataforseo/keywords.rs`, `src/keywords/{repo,routes}.rs`, tests.

- `site_keywords` gains `search_volume int null`, `competition text null`, `cpc double null`, `metrics_checked_at`.
- Volumes: `POST /v3/keywords_data/google_ads/search_volume/live` with one task `[{ keywords: [...up to 1000], location_code, language_code }]`; per keyword `search_volume`, `competition`, `cpc`. One request refreshes all of a site's keywords. The endpoint allows 12 requests a minute per account: on 429, fail the job for retry.
- Ideas: read DataForSEO's docs and use the Google Ads "keywords for keywords" live endpoint (`/v3/keywords_data/google_ads/keywords_for_keywords/live`) with the seed; return up to 50 ideas sorted by search volume. Nothing is saved.
- Usage recorded for each call (estimate $0.05 per request for volumes and for ideas; label it an estimate and tell the owner in the report what the docs' pricing page says).

**Tests:** volumes stored and shown on the keywords list; a keyword DataForSEO has no data for keeps `null`; ideas returned sorted and capped; 422 without credentials; 429 after 20 idea requests in a day; usage recorded.

**Commit:** `feat: add search volume and keyword ideas`

### Task 6: Page speed

**Files:** `migrations/0016_speed.sql`, `src/integrations/pagespeed/{mod,client}.rs`, `src/speed/{mod,repo,run,routes}.rs`, `src/checks/speed.rs`, `src/metrics/speed.rs`, tests.

- `GET https://www.googleapis.com/pagespeedonline/v5/runPagespeed?url=&strategy=mobile&category=performance` plus `&key=` when a key is set (it works without one at a low rate). Base URL injectable. Timeout 90 seconds.
- Read: `lighthouseResult.categories.performance.score` (0–1, stored as 0–100), `lighthouseResult.audits["largest-contentful-paint"].numericValue` (ms), `["cumulative-layout-shift"].numericValue`, `["total-blocking-time"].numericValue`; field data when present from `loadingExperience.metrics`: `LARGEST_CONTENTFUL_PAINT_MS.percentile`, `CUMULATIVE_LAYOUT_SHIFT_SCORE.percentile` (divide by 100), `INTERACTION_TO_NEXT_PAINT.percentile`. Prefer field data and set `field_data: true`.
- Only pages that belong to one of the workspace's sites are ever sent to PageSpeed.
- Table `speed_results(workspace_id, site_id fk cascade, url, strategy, performance, lcp_ms, cls, inp_ms, tbt_ms, field_data, error, checked_at, primary key (site_id, url, strategy))`. Job kind `speed_run`, pages one at a time; runs after each completed crawl and on demand.
- Checks, `needing(.., &[Need::Speed])`, category technical:

  | id | sev | effort | Triggers when |
  |---|---|---|---|
  | `speed_slow_lcp` | 2 | 3 | LCP over 2500 ms |
  | `speed_layout_shift` | 2 | 2 | CLS over 0.1 |
  | `speed_slow_interaction` | 2 | 3 | INP over 200 ms (field data), or TBT over 300 ms when there is no field data |

**Tests:** lab-only and field-data responses; a 429 or 500 stores an error for that page and continues; the three checks firing and not firing; only the workspace's own pages are sent; metrics; limits.

**Commit:** `feat: add page speed from Google PageSpeed Insights`

### Task 7: Web app

**Files:** under `web/src/`: `settings/{SettingsPage,ProvidersPanel,GooglePanel,DataPanel,CrawlerPanel,UsagePanel}.tsx`, `sites/SettingsPage.tsx` (site options), `rankings/RankingsPage.tsx`, `keywords/{KeywordsPage,IdeasPanel}.tsx`, `speed/SpeedPanel.tsx`, `metrics/MetricCards.tsx`, `connections/ConnectionsPage.tsx`, `api/{types,hooks}.ts`, routes, tests.

Keep the current light dashboard look and existing components.

- **Settings** (top-level, in the header beside Sites): one place for everything that used to need `backend/.env`.
  - *AI providers:* Claude, ChatGPT, Grok, Gemini, Perplexity. Each: status tag, where the key comes from (saved here, from the server file, or not set), a password-type field to enter or replace the key, a model field prefilled with the default, Save, Test, Remove. A saved key is shown as "Saved" with no characters of it.
  - *Google Search Console sign-in:* client id and client secret fields, the exact redirect address to register shown with a copy button, and a short numbered guide.
  - *Search data (DataForSEO):* login and password fields, Test, the balance, and what it unlocks (rank tracking, AI Overviews, search volume, keyword ideas).
  - *Page speed:* optional API key.
  - *Crawler:* contact address, with one sentence on why it is needed.
  - *Usage:* calls and estimated cost by source for the last 30 days, with "estimate" stated.
- **Site settings:** name, page limit, country, language, device, rank tracking on or off, how often.
- **Rankings tab:** table of keyword, position with change, landing page, AI Overview status, search volume; a row opens the top 10 results and a small SVG position history; "Check now" with progress; not set up → an explanation and a link to Settings.
- **Keywords tab:** volume, competition and cost per click columns; "Refresh volumes"; a "Find keyword ideas" panel with a seed field and Add buttons.
- **AI Visibility tab:** Grok added everywhere engines are listed; an "AI Overviews" section listing each keyword's status and the domains Google's overview cites.
- **Overview:** the project card gains Rankings (keywords in the top 10) and Page speed; Site Health, AI Visibility and the rest stay.
- **Connections tab** on a site now links to Settings for credentials and keeps the per-site connect actions (Google property, Bing).
- **First-run guide:** on the sites list, when nothing is set up, a short checklist with links: add a crawler contact, add a site, add keywords, connect an AI provider, connect search data.

**Tests:** every new screen's loading, empty, error and main states; a saved secret is never rendered; saving a key shows the provider's test result; the redirect address is shown and copyable; rankings rows, change arrows, not-in-top-100 wording and the history chart; ideas Add buttons; site settings validation messages from the server; the first-run checklist ticking off; one e2e flow: open Settings, save a provider key (mocked), set a contact, add a keyword, open Rankings. Coverage of `web/src` at least 80%.

**Commit:** `feat: add the settings screen, rankings, keyword research and page speed to the web app`

---

## What the owner sets up, all in the app

| To get | Enter in Settings |
|---|---|
| Crawling | a contact address |
| AI visibility | a key for any of Claude, ChatGPT, Grok, Gemini, Perplexity |
| Organic traffic and keywords | Google client id and secret, then Connect on the site |
| Backlinks | a Bing key on the site's Connections tab |
| Rankings, AI Overviews, search volume, keyword ideas | a DataForSEO login and password (paid per use, $50 minimum top-up) |
| Page speed | nothing (a key is optional) |
