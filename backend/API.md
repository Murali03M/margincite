# Margincite API

Base path `/api/v1`. Every request needs `Authorization: Bearer <token>`, except `GET /healthz`.

Every response is one envelope:

```json
{ "success": true, "data": {}, "error": null, "meta": null }
```

- `meta` is `{ "total", "page", "limit" }` on paged lists and `null` otherwise.
- Errors are `{ "success": false, "data": null, "error": "plain message", "meta": null }` with status 401, 404, 409, 422, 429 or 502 (an outside service such as Google could not be reached).
- Times are RFC 3339 strings.
- Paging: `page` starts at 1, `limit` defaults to 20 and is capped at 100.
- A resource in another workspace is 404.

## Me

- `GET /me` → `{ user_id, workspace_id, email }`

## Sites

`Site = { id, root_url, host, path_prefix, display_name, page_cap }`

- `GET /sites` → `Site[]`, no meta
- `GET /sites/{id}` → `Site`
- `POST /sites` with `{ "url": string, "name"?: string }` → 201 `Site & { first_crawl_id }`. 422 for an address that is not a public website, 409 if already added or when the workspace already has 25 sites ("You have reached the limit of 25 sites. Delete one to add another."), 429 once the 10 crawls of the day are used.
- `DELETE /sites/{id}` → `{ deleted: true }`

## Crawls

`Crawl = { id, site_id, status, trigger, stop_reason, robots_txt_found, sitemap_found, llms_txt_found, pages_crawled, pages_failed, error, score, html_pages }`

- `status`: `"queued" | "running" | "completed" | "failed"`
- `trigger`: `"manual" | "scheduled" | "first"`
- `stop_reason`: `"finished" | "page_cap" | "robots_blocked" | "site_unreachable" | "time_limit" | null`
- `score`: integer 0 to 100, or `null`
- `time_limit`: the crawl ran for 30 minutes and stopped. It ends as `completed` and keeps the pages it read.
- `robots_txt_found`: boolean, whether the site has a robots.txt. The file's text is never sent.
- `error`: string or `null`

Endpoints:

- `GET /sites/{id}/crawls` → paged `Crawl[]`, newest first
- `POST /sites/{id}/crawls` → 202 `Crawl`. 409 while a crawl is queued or running, 429 after 10 crawls in a day, a site's first crawl included.
- `GET /crawls/{id}` → `Crawl`

## Pages

`PageSummary = { id, crawl_id, url, depth, in_sitemap, status_code, content_type, fetch_error, needed_javascript }`

- `GET /crawls/{id}/pages` → paged `PageSummary[]`
- `GET /pages/{id}` → `PageSummary & { response_ms, bytes, is_https, x_robots_tag, text, facts }`
- `GET /pages/{id}/findings` → `[{ check_id, category, severity, title, evidence }]`, most severe first, no meta

`facts` is `null` or:

```
{ title, meta_description, h1s: string[], headings: [{ level, text }], canonical, robots_meta,
  has_viewport, word_count, structured_data: any[], structured_data_errors,
  links: [{ url, text, nofollow }], images: [{ src, alt | null }], has_scripts,
  published, modified, author,
  question_headings, lists, tables, first_paragraph_words, faq_schema, howto_schema,
  article_schema, organization_schema, breadcrumb_schema, outbound_citations }
```

The last ten facts (answer readiness) are numbers or booleans, or `null` for pages crawled before they existed. `null` means unknown: the checks that read them skip the page and never fail it. A recrawl fills them in.

`text` is the page's extracted text, or `null` for older crawls.

## Suggestions

`SuggestionOut = { id, check_id, category, severity, effort, title, open, done, dismissed, priority }`

- `category`: `"technical" | "ai_readiness" | "content"`
- `severity`, `effort`: 1 to 3
- `open`, `done`, `dismissed`: counts of items in each status
- `priority`: number, higher first

Endpoints:

- `GET /sites/{id}/summary` →

  ```
  { site: Site, latest_crawl: Crawl | null, score: int | null,
    counts: { open, done, dismissed },
    by_category: [{ category, open }],          // always three entries
    top: SuggestionOut[],                        // up to 5, only those with open > 0
    info: { pages_crawled, html_pages, sitemap_found, llms_txt_found, reach_basis: "pages" | "impressions" } }
  ```

  `score` and `info` come from the latest completed crawl. `reach_basis` is `"impressions"` once the site has Search Console data: a page-level suggestion's reach, and so its priority and its weight in the score, is then the share of the site's search impressions that land on its open pages instead of the share of pages (falling back to pages when there are no impressions). Site-level suggestions always reach the whole site. `latest_crawl` is the newest crawl of any status, so it can be running while a score from the previous crawl is shown.

- `GET /sites/{id}/suggestions?status=&category=&page=&limit=` → paged `SuggestionOut[]`, ranked.
  - `status`: `open` (default, at least one open item), `done` (no open items), `dismissed` (at least one dismissed item), `all`.
  - A bad `status` or `category` is 422.

- `GET /suggestions/{id}?status=&page=&limit=` →

  ```
  { suggestion: SuggestionOut, why: string, fix: string,
    items: [{ target, target_kind: "page" | "site", status: "open" | "done" | "dismissed",
              evidence: object, first_seen, last_seen, resolved_at | null }] }
  ```

  Paged over items, sorted by target. `status` defaults to all.

- `PATCH /suggestions/{id}` with `{ "status": "open" | "done" | "dismissed", "targets"?: string[] }` → `{ changed: int }`. Without `targets` it changes every item. An unknown status or an empty `targets` list is 422.

## Integrations

- `GET /integrations` → what the caller's workspace is set up with (its saved settings first, the server's environment second). It never returns a key, secret or client id.

  ```
  { secret_key_set: bool,
    google: { configured: bool },
    engines: [{ engine: "claude" | "chatgpt" | "grok" | "gemini" | "perplexity", configured: bool, model: string }] }
  ```

  `engines` always has the five entries in that order. An engine is `configured` when the workspace saved a key for it or the server's environment has one.

## Settings

Everything a person sets up in the app. Secrets are write-only: no endpoint returns one, whole or in part, and they never reach a log line or an error message. They are sealed with the server's secret key before they are stored (422 "Secrets cannot be saved because the server has no secret key yet." when there is none, which cannot happen once the server has started: it makes `.data/secret.key` when `MARGINCITE_SECRET_KEY` is unset). Request bodies of these endpoints are never quoted back in an error: a body that cannot be read is 422 "The request could not be read. Check the fields and try again." The same envelope and sentence answer an unreadable body on `PUT /sites/{id}/settings` and `POST /sites/{id}/keywords/ideas`; an unreadable query string on `GET /usage` is 422 "The address could not be read. Check the values and try again."

Lookup order for every value: the workspace's saved value, then the environment variable of the same meaning, then the built-in default. A saved secret that can no longer be opened counts as not saved.

- `GET /settings` →

  ```
  { providers: [ProviderOut], 
    google: { client_id_set, client_secret_set, source, redirect_uri: string },
    dataforseo: { login_set, password_set, source, status, error, balance: number | null, checked_at },
    pagespeed: { key_set, source },
    crawler: { contact: string | null, contact_source },
    secret_key_ready: bool }
  ProviderOut = { provider: "claude" | "chatgpt" | "grok" | "gemini" | "perplexity", key_set, source: "workspace" | "environment" | "none",
                  model, default_model, status: "ok" | "error" | "unchecked", error: string | null, checked_at: string | null }
  ```

  `source` is `"workspace"`, `"environment"` or `"none"`. `status` is the outcome of the last test; `unchecked` before any.
- `PUT /settings/providers/{provider}` with `{ api_key?, model? }` → `ProviderOut`. Saves, then tests the provider's key with one cheap request. A failed test still saves and reports `status: "error"` with a plain sentence ("The key was rejected.", "The provider answered with HTTP 500.", "The provider could not be reached.", "The provider did not answer in time.", "The provider's answer could not be read."), never the provider's own words. An empty `model` goes back to the environment's or the default. 422 for neither field, an API key with spaces or longer than 512 characters, or a model with characters outside letters, digits and `. - _ :`. 404 for an unknown provider.
- `POST /settings/providers/{provider}/test` → `ProviderOut`. `DELETE /settings/providers/{provider}` removes the workspace's key and model and returns the new `ProviderOut`.
- `PUT /settings/google` with `{ client_id, client_secret }`, `DELETE /settings/google` → `{ client_id_set, client_secret_set, source, redirect_uri }`. `redirect_uri` is the exact address this server sends to Google (built from `PUBLIC_BASE_URL`, always `.../api/v1/oauth/google/callback`); register that one with the OAuth client. It is not a secret. The client ID and secret are used as a pair, never one from each source.
- `PUT /settings/dataforseo` with `{ login, password }` (tested at once with `GET /v3/appendix/user_data`, which returns the balance), `POST /settings/dataforseo/test`, `DELETE /settings/dataforseo` → the `dataforseo` object above.
- `PUT /settings/pagespeed` with `{ api_key }`, `DELETE /settings/pagespeed` → `{ key_set, source }`.
- `PUT /settings/crawler` with `{ contact }` (an email address or an http(s) address, at most 200 characters, no spaces), `DELETE /settings/crawler` → `{ contact, contact_source }`. With no contact anywhere, adding a site and starting a crawl are 422 `"Add a contact address in Settings before crawling. It is sent with every request the crawler makes."`; a scheduled crawl waits a week instead of failing.
- `GET /sites/{id}/settings` and `PUT` with any of `{ display_name?, page_cap? (10 to 5000), country?, language?, device? ("desktop" | "mobile"), track_rankings?, ranking_frequency? ("daily" | "weekly") }` → `{ display_name, page_cap, country, language, device, track_rankings, ranking_frequency, countries: [{ code, name }], languages: [{ code, name }] }`. Defaults are `US`, `en`, `desktop`, true, `weekly`. A field left out is unchanged; an unknown country or language, a page cap out of range, a blank name or any other bad value is 422 and nothing is saved.
- `GET /usage?days=30` (1 to 365) → `[{ source, kind, calls, estimated_cost_usd: number | null, day }]`, newest day first, one row per UTC day, source and kind. Every AI answer is counted (`source` the engine, `kind` `"answer"`, no cost estimate).

## Keywords

`Keyword = { id, keyword, target_url: string | null, landing_url: string | null, landing_source: "set" | "search_console" | "best_match" | "home" | null, position: number | null, clicks: int | null, impressions: int | null,
  search_volume: int | null, competition: string | null, cpc: number | null }`

A keyword is a word or phrase the site wants to be found for. `target_url` is the page the person chose for it. `landing_url` is the page the checks judge it on, picked from the latest completed crawl: the chosen page when it was crawled (`set`), else the crawled page Search Console shows most for that query (`search_console`), else the crawled page whose title, main heading and description hold the most of the keyword's words (`best_match`), else the home page (`home`). Both are `null` before the first completed crawl. `position`, `clicks` and `impressions` come from Search Console, last 28 days, and are `null` until it is connected. `search_volume` (average monthly searches), `competition` (`LOW`, `MEDIUM` or `HIGH`) and `cpc` (cost per click in US dollars) come from Google Ads data through DataForSEO, for the site's country and language, and are `null` until refreshed and whenever Google has no data (never zero).

- `GET /sites/{id}/keywords` → `Keyword[]`, oldest first, no meta
- `POST /sites/{id}/keywords` with `{ "keyword": string, "target_url"?: string }` → 201 `Keyword`. The keyword is trimmed and its spaces collapsed. `target_url` may be a web address on the site or a path such as `/pricing`; blank means none.
  - 422 for an empty keyword, one over 100 characters, one with no letter or number, or a `target_url` that is not on the site.
  - 409 for a duplicate (case-insensitive) and 409 `"A site can have at most 50 keywords."`
- `PATCH /keywords/{id}` with `{ "target_url": string | null }` → `Keyword`. `null` clears it. Same 422 rule.
- `DELETE /keywords/{id}` → `{ deleted: true }`

Changing keywords does not re-run checks. The next crawl does.

### Keyword research (DataForSEO)

Paid per use, and never asked without the workspace's own DataForSEO login (Settings, or `DATAFORSEO_LOGIN` and `DATAFORSEO_PASSWORD`). Each request is counted in `/usage` under `dataforseo` with an estimated $0.09 (DataForSEO bills the Google Ads endpoints per request, whatever the number of keywords).

- `GET /sites/{id}/keywords/volumes` → `{ status: "idle" | "queued" | "running" | "failed", checked_at: string | null, error: string | null }`. `queued` and `running` mean a refresh is waiting or under way (`error` is `null`). `failed` means the last refresh ended without storing anything, and `error` is a fixed plain sentence (the login was rejected, the balance has run out, DataForSEO is limiting requests, took too long or could not answer, the login is no longer set up, the site's country cannot be searched, no keyword can be searched, or "The refresh stopped because of an internal error. Try again."); it never repeats a response body. Otherwise `idle`. `checked_at` is when volumes were last stored, `null` when never, and a failed refresh leaves it alone. A new refresh being queued clears the failure. 404 for another workspace's site.
- `POST /sites/{id}/keywords/volumes` → 202 `{ queued: true }`. 429 after 5 refreshes for the site in a UTC day ("You can refresh search volume 5 times a day for each site. Try again tomorrow."; kept in `keyword_volume_requests`, which a deleted site does not take with it). Asking while a refresh is already waiting pulls that one forward and uses nothing from the allowance. A refusal uses up nothing. The paid answer is saved on the job the moment it arrives, so a retry after a failure to store it stores that answer without a second paid request (a timeout, where DataForSEO may or may not have billed, is retried and may be billed again). Refreshes the volume, competition and cost per click of all the site's keywords with one `POST /v3/keywords_data/google_ads/search_volume/live` request (up to 1000 keywords; a keyword over 80 characters or 10 words, which DataForSEO refuses, is left out and keeps `null`). 422 without a login or without keywords. Only a person starts it; there is no schedule. It is a `keyword_volumes` job, at most one waiting per site. The endpoint allows 12 requests a minute per account: a rate limit (HTTP 429 or status 40202), a timeout or a provider error fails the job so the queue retries it; a rejected login or a balance that has run out ends it, keeps the old numbers and marks the DataForSEO setting with a plain sentence. Keywords are matched to the answer without regard to case.
- `POST /sites/{id}/keywords/ideas` with `{ "seed": string }` → `{ ideas: [{ keyword, search_volume, competition, cpc }] }`: up to 50 ideas from `keywords_for_keywords/live`, biggest volume first, ideas without a volume last, each keyword once. Nothing is saved. The seed is trimmed and must be at most 80 characters and 10 words with a letter or number in it (422). 422 without a login ("Keyword ideas need a DataForSEO login. Add it in Settings, then try again."); 429 after 20 searches for the workspace in a UTC day ("You have searched for keyword ideas 20 times today. Try again tomorrow."), counted in `keyword_idea_requests`, which has no site and so does not give searches back when a site is deleted; 502 with one of the fixed sentences below when DataForSEO cannot answer. A refusal (422) uses up nothing.

The sentences are the ones in Rankings: login rejected, balance run out, rate limited, too slow, could not answer, or an unrecognised format. The provider's message, the request and the login never reach a stored error, a response or a log line.

## Checks

- `GET /sites/{id}/checks` → every check Margincite makes, in catalog order:

  ```
  { crawl_id: uuid | null,
    checks: [{ check_id, title, category: "technical" | "ai_readiness" | "content",
               status: "passed" | "failed" | "not_run", open: int, reason: string | null }] }
  ```

  `crawl_id` is the latest completed crawl. A check is `not_run`, with a plain `reason`, when the site lacks what it needs (target keywords, Search Console data, an AI visibility run) or has no completed crawl yet. Otherwise it is `failed` when it has open items (`open` counts them) and `passed` when it has none. A dismissed item is not open. `not_run` checks have `open` 0 and are left out of the site's score.

## Google Search Console

Needs `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` and `MARGINCITE_SECRET_KEY` on the server. The refresh token is stored encrypted and is never returned. Search Console data is final about three days late, so every window ends three days ago.

`Connection = { status: "not_connected" | "choose_property" | "connected" | "needs_reconnecting", property: string | null, synced_at: string | null, error: string | null }`

`error` is a plain sentence ("Google no longer accepts this connection. Connect Google Search Console again.") or `null`. A site "has Search Console data" when its connection is `connected` and at least one row has synced; the checks list, the checks that read the data, the score and the metrics all use that one test.

- `GET /sites/{id}/integrations/google` → `Connection`
- `POST /sites/{id}/integrations/google/start` → `{ auth_url }`. Send the browser there. 422 when Google or `MARGINCITE_SECRET_KEY` is not set up on the server. The response also sets the cookie `margincite_oauth` (random nonce; `Path=/api/v1/oauth/google; HttpOnly; SameSite=Lax; Max-Age=600`, plus `Secure` when `PUBLIC_BASE_URL` is https). Only the nonce's SHA-256 is stored, and the callback only works in the browser that holds it. The sign-in must come back within 10 minutes and works once.
- `GET /oauth/google/callback?code&state` has no bearer token: it is Google sending the browser back. It always answers 302 to `{PUBLIC_BASE_URL}/sites/{site_id}/connections?google=connected`, or `?google=error&reason=<code>`. When the state is unknown, already used, does not match a site, or the `margincite_oauth` cookie is missing or does not match (a link sent to someone else's browser), the callback refuses before asking Google anything; the callback also clears the cookie on every response. In those cases the site is not known and the redirect goes to `{PUBLIC_BASE_URL}/?google=error&reason=invalid_state`. The address is built from `PUBLIC_BASE_URL` and a site id from the server's own table, never from a request parameter. Reason codes: `invalid_state`, `expired`, `denied`, `missing_code`, `token_rejected`, `no_refresh_token`, `google_unavailable`, `not_configured`, `server_error`. Nothing is stored on an error. Connecting again on a site that already had a property keeps it and starts a sync.
- `GET /sites/{id}/integrations/google/properties` → `[{ property, permission }]`: the Search Console properties the person can read (unverified ones are left out), with ones that look like this site first. 422 when not connected or when Google no longer accepts the token (the connection becomes `needs_reconnecting`), 502 when Google cannot be reached.
- `PUT /sites/{id}/integrations/google` with `{ "property": string }` → `Connection`. The property must be one of the listed ones (else 422). Choosing a different property than before drops the old rows. Starts a sync.
- `POST /sites/{id}/integrations/google/sync` → 202 `Connection`. 422 unless `connected` with a property. At most one sync waits per site; asking again moves a waiting one to now.
- `DELETE /sites/{id}/integrations/google` → `Connection` (always `not_connected`). Deletes the token and every synced row, and re-runs the checks on the latest crawl.

A sync runs as a `google_sync` job: a crawl finishing, a property being chosen, "Sync now" and a daily schedule each queue one. It reads query and page rows for 28 days (up to 25,000 rows), daily totals for 90 days, and the 28 days before for comparison, replaces the stored data in one transaction, then re-runs the checks on the latest completed crawl. Google answering 429, 5xx or something unexpected fails the job and the queue retries it (3 attempts). A refused token (`invalid_grant`) or a 401, 403 or 404 from the API marks the connection `needs_reconnecting`; rows already synced are kept but the numbers are hidden.

- `GET /sites/{id}/search/queries?page&limit&sort=clicks|impressions|position` → paged `[{ query, clicks, impressions, ctr, position, top_page }]`, one row per query over the last 28 days. `ctr` is 0 to 1. `position` is averaged by impressions, and `sort=position` puts the best (lowest) first. `top_page` is the page that showed most for that query. Empty when not connected. A bad `sort` is 422.
- `GET /sites/{id}/search/pages?page&limit` → paged `[{ url, clicks, impressions, ctr, position }]`, most impressions first.

New checks (they are `not_run` without Search Console data): `search_low_ctr`, `search_striking_distance`, `search_no_impressions`. `keyword_cannibalization` also reads the data. Page addresses are matched after the crawler's URL normalisation.

## Bing Webmaster Tools

Needs only `MARGINCITE_SECRET_KEY` on the server (422 with a plain message otherwise). The person pastes an API key from Bing Webmaster Tools, Settings, API access. It is stored encrypted and is never returned or logged; it travels to Bing as the `apikey` query parameter, so request addresses are never logged either.

`Connection = { status: "not_connected" | "connected" | "needs_reconnecting", site_url: string | null, synced_at: string | null, error: string | null }`

- `GET /sites/{id}/integrations/bing` → `Connection`
- `PUT /sites/{id}/integrations/bing` with `{ "api_key": string, "site_url": string }` → `Connection`, and starts a sync. `site_url` must be http(s), carry no credentials and have the site's host (a leading `www.` is ignored on both sides), else 422. The key is checked with Bing's `GetUserSites`: 422 `"Bing did not accept that API key. ..."` when Bing refuses it, 422 `"That site is not in this Bing Webmaster Tools account. ..."` when the account does not list that address (compared apart from case and a trailing slash; the address is stored as Bing lists it), 502 when Bing cannot be reached. Connecting again replaces the key; a different `site_url` drops the old rows.
- `POST /sites/{id}/integrations/bing/sync` → 202 `Connection`. 422 unless `connected`. At most one sync waits per site.
- `DELETE /sites/{id}/integrations/bing` → `Connection` (always `not_connected`). Deletes the key and every synced row.
- `GET /sites/{id}/backlinks?page&limit` → paged `[{ target_url, count }]`, most linked first. Empty when not connected.
- `GET /sites/{id}/backlinks/sources?target_url=` → `[{ source_url, anchor_text }]` for one of the 10 most linked pages (others have none). `target_url` is required (422). Empty when unknown.

A sync runs as a `bing_sync` job: connecting, "Sync now" and a weekly schedule each queue one. It reads `GetLinkCounts` page by page (0-based; it stops at an empty page, a page with nothing new, or after 20 pages), then `GetUrlLinks` page 0 for the 10 most linked pages (at most 100 links each), and replaces the stored rows in one transaction. 401 or 403 (or a 400 that names the key) marks the connection `needs_reconnecting` and schedules nothing; 429, 5xx and unreachable fail the job so the queue retries it, and after the last try the connection keeps a plain `error` and the next weekly sync is still scheduled. Bing's answers are read defensively: field names match in any case, counts may be text, `{"d": ...}` is optional, and rows without a page address are skipped (the field names that could not be mapped are logged, never values). Only an empty list (`{"d":[]}`, `{"d":null}`, `[]`) means no backlinks. An answer that is not JSON, has no list, or has rows none of which has a page address is treated as a failed attempt: the stored backlinks are kept, the connection's `error` reads "Bing replied in a format Margincite does not recognise. The last good data is kept.", and the queue retries. A scheduled sync that finds no `MARGINCITE_SECRET_KEY` is put off a week instead of ending the chain.

## Metrics

- `GET /sites/{id}/metrics` →

  ```
  { site_health: { score: int | null, previous: int | null, history: [{ at, score }], reason: string | null },
    ai_visibility: { percent, mentions, citations: number | null, answers: int, last_run_at: string | null,
                     engines: [{ engine, percent, mentions, answers }], reason: string | null },
    visibility: { percent: number | null, in_top_10: int | null, keywords: int,
                  source: "rankings" | "search_console" | null, reason: string | null },
    organic: { clicks, impressions, keywords: int | null, avg_position: number | null,
               clicks_previous, keywords_previous: int | null,
               daily: [{ date, clicks, impressions }], source: "search_console", reason: string | null },
    backlinks: { total, linked_pages: int | null, top_pages: [{ url, count }], source: "bing",
                 synced_at: string | null, reason: string | null },
    speed: { performance: int | null, checked_at: string | null, reason: string | null },
    rankings: { tracked: int, in_top_3: int | null, in_top_10: int | null, average_position: number | null,
                checked_at: string | null, reason: string | null },
    ai_overviews: { tracked: int, present: int | null, cited: int | null,
                    checked_at: string | null, reason: string | null } }
  ```

  Every block is computed on its own. A number that cannot be shown is `null` with a plain `reason`; a block whose query fails is `null` with `"Could not load this number right now."` and the others are unaffected.

  - `site_health`: `score` is the latest completed crawl's score, `previous` the next older scored crawl's, `history` the last 12 completed crawls that have a score, oldest first (`at` is when the crawl finished).
  - `organic`: from Search Console, last 28 days against the 28 before. `clicks` and `impressions` are the sum of the daily series over the 28 days ending on its latest day. `keywords` is the distinct queries with impressions and `avg_position` is averaged by impressions. `daily` is up to 90 days, oldest first, `date` as `YYYY-MM-DD`. The previous values come from the same daily series. Hidden (anonymised) queries are in the daily totals but not in `keywords` or `avg_position`.
  - `visibility`: the share of keywords in the top ten, rounded to one decimal. When keywords have been checked in Google's results (Rankings below), `source` is `"rankings"`, the share is over the keywords that have a result at the site's country, language and device, and `keywords` is how many that is. Otherwise `source` is `"search_console"`: the share of the site's target keywords whose average position (over every page, weighted by impressions) is 10 or better, `keywords` is how many target keywords there are, and a keyword Search Console never showed counts as not in the top ten. `source` is `null` when neither exists.
  - `rankings`: from the latest result of each current keyword at the site's location. `tracked` is how many keywords have a result; `in_top_3` and `in_top_10` count the ones that rank there; `average_position` is over the keywords that rank at all and is `null` when none does (a keyword outside the top 100 is not a zero). `checked_at` is when the newest finished check finished. Before the first check everything but `tracked` (0) is `null` and `reason` says to add keywords, or to set up DataForSEO and check.
  - `ai_overviews`: from the same results. `present` counts keywords whose results had a Google AI Overview (cited or not), `cited` those where it cited the site. A sample from one day, not a measure.
  - `backlinks`: from Bing Webmaster Tools. `total` is the sum of the link counts, `linked_pages` the number of linked pages, `top_pages` the five most linked, `synced_at` the last sync. `null` with a `reason` when Bing is not connected ("Connect Bing Webmaster Tools to see backlinks."), needs reconnecting, or has not finished its first sync. A synced site with no links is `0`, not `null`.
  - `speed`: the PageSpeed Insights performance score (0 to 100, on a phone) of the home page, or the rounded average of the tested pages when the home page has none. `null` with a `reason` before the first test, or "Page speed could not be measured on the last check. Open Page speed to see why." when results exist but every one ended in an error. See Page speed.
  - `ai_visibility`: from the latest completed AI run. `answers` counts answers that have no error; `percent` is the share of them that mention or cite the site (one decimal), `mentions` those that mention it, `citations` those that cite one of its pages, `last_run_at` when that run finished. `engines` lists only engines that gave at least one answer, in the order claude, chatgpt, grok, gemini, perplexity; each `percent` uses the same rule over that engine's answers and `mentions` counts that engine's mentions. Before any run completes, `percent`, `mentions` and `citations` are `null`, `answers` is 0, and `reason` is "Run an AI visibility check to see this."; when the newest run failed and none has completed, the reason says no engine could answer.

## Page speed

Google PageSpeed Insights, on a phone (`strategy=mobile`), for the home page and four more pages: the crawled working pages with the most Search Console impressions or, without any, the shallowest. Only pages of the workspace's own site are ever sent. A key (Settings, or `PAGESPEED_API_KEY`) is optional; without one PageSpeed allows a low request rate.

- `GET /sites/{id}/speed` → `{ checked_at: string | null, pages: [{ url, strategy: "mobile", performance: int | null, lcp_ms, cls, inp_ms, tbt_ms: number | null, field_data: bool, error: string | null }] }`, home page first. `checked_at` is when the newest test was attempted, whether it worked or not, so a run in which every page failed still moves it (each page keeps its last good numbers; `null` before the first test). Real-visitor (Chrome UX Report) numbers are used when PageSpeed has them for that page and then `field_data` is `true`; when PageSpeed only has numbers for the whole site (`origin_fallback`), they are not used as the page's: the lab numbers are, and `field_data` is `false`; `inp_ms` exists only from that data.
- `POST /sites/{id}/speed/check` → 202 `{ queued: true }`. 429 after 5 manual checks for the site in a UTC day (kept in `speed_manual_checks`, which a deleted site does not take with it).

A test is a `speed_run` job, pages one at a time, queued after every completed crawl and by "Check now"; it stores every result and deletes the site's rows for pages that were not tested this time (the home page and the top four of the latest crawl) in one transaction, so a page that dropped out stops counting in the metric and the checks, then re-runs the checks on the latest crawl. It runs in a worker lane shared with the search volume refresh, apart from crawls, syncs, rank runs and AI runs. A page that fails keeps its last good numbers and date and gets an `error`, one of a few fixed sentences ("PageSpeed is limiting requests right now. Try again later.", "Google is not accepting page speed tests without an API key right now. Add a free PageSpeed key in Settings." when no key is set and Google answers 429, "PageSpeed rejected the API key. Check it in Settings.", "PageSpeed took too long to test this page.", "PageSpeed could not test this page.", or a changed-format message). Errors never contain the request address, a response body or the key, and neither do log lines. Each PageSpeed call is counted in `/usage` under `pagespeed` (cost 0, the API is free).

New checks (technical, `not_run` until a page has a result without an error): `speed_slow_lcp` (LCP over 2500 ms), `speed_layout_shift` (CLS over 0.1), `speed_slow_interaction` (INP over 200 ms, or total blocking time over 300 ms when there is no INP).

## Rankings and AI Overviews

Where the site's target keywords stand in Google's results, and whether Google's AI Overview cites the site, read through DataForSEO's live Google organic endpoint (`POST /v3/serp/google/organic/live/advanced`, depth 100, `load_async_ai_overview: true`, one request per keyword, Basic auth with the workspace's login). It is paid per use and is never asked without the workspace's own login (Settings, or `DATAFORSEO_LOGIN` and `DATAFORSEO_PASSWORD`), by any path: a manual check, a scheduled check or a retried job all stop at "no login". Results are tracked per country, language and device from the site's settings; a result is kept with its location and a trend never mixes two.

- `GET /sites/{id}/rankings` → `{ location: { country, language, device }, checked_at: string | null, last_run: { id, status: "queued" | "running" | "completed" | "failed", error: string | null, finished_at: string | null } | null, keywords: [{ keyword_id, keyword, position: int | null, previous: int | null, url: string | null, ai_overview: "cited" | "present" | "absent" | "unknown", top: [{ position, domain, url, title }], history: [{ at, position: int | null }] }] }`. Every current keyword is listed. `position` is the best place any of the site's organic results holds (`rank_absolute` in DataForSEO's answer; the host or a subdomain, ignoring `www.`), `null` with a non-empty `history` when the site is not in the top 100 (stored as "not in the top 100", never as 0), and `null` with an empty `history` when the keyword has not been checked here. `previous` is the position of the check before, `url` the ranking page, `top` the first ten organic results, `history` up to 30 checks, oldest first. `checked_at` is set only when a check has finished (a run in progress leaves it alone, though its finished keywords already show), so a screen can poll until it changes; a run that failed without checking anything does not change it. `last_run` is the site's newest run of any outcome, including one that stored nothing, `null` when there never was one. `error` is the fixed plain sentence the run stored: set when it failed (login rejected, balance used up, rate limited, too slow, could not answer, stopped by an internal error) and also on a `completed` run that has a note ("1 of 2 keywords could not be checked." or the sentence that stopped it part way); `null` while queued or running and after a clean run. `finished_at` is `null` until it ends.
- `POST /sites/{id}/rankings/check` → 202 `{ queued: true }`. 422 without a DataForSEO login ("Rank tracking needs a DataForSEO login. Add it in Settings, then try again.") or without keywords; 409 while a check is queued or running (the database allows one per site); 429 after 5 manual checks for the site in a UTC day. A refusal uses up nothing.
- `GET /sites/{id}/ai-overviews` → `[{ keyword_id, keyword, status: "cited" | "present" | "absent" | "unknown", cited_domains: string[], your_urls: string[], checked_at }]`, for each keyword that has been checked. `cited` means one of the AI Overview's references is on the site; `present` means there is an AI Overview that does not cite it (`cited_domains` lists who it cites, without `www.`); `absent` means the results had none; `unknown` means the AI Overview was there but could not be read (for example it had not loaded). References are read from the overview's own `references`, each element's, and each expanded element's components'.

A check is a `rank_run` job in a worker lane of its own, four keywords in flight at once. A keyword that cannot be checked keeps its old result and the run goes on; the run is `completed` when at least one keyword was stored (with a note such as "1 of 2 keywords could not be checked.") and `failed` when none was. A login that is refused or a balance that has run out stops the run, and marks the DataForSEO setting with a plain sentence. Errors are fixed sentences made from DataForSEO's status codes (40100-range: login rejected; 40200, 40203, 40210 and HTTP 402: "Your DataForSEO balance has run out."; 40202, 40209 and HTTP 429: rate limited; 50401: too slow; anything else: could not answer). The provider's message, the request, the login and the `Authorization` value never reach a stored error, a response or a log line. A retried job resumes its run and asks only the keywords it has not stored. Each answered keyword is counted in `/usage` under `dataforseo` / `serp` with an estimated $0.022: DataForSEO bills each 10 results as one $0.002 page, so depth 100 is $0.02, plus $0.002 for the AI Overview option (refunded when there is none).

Schedule: when tracking is on for a site (`track_rankings`), it has keywords and the workspace has a login, a run is queued a few minutes after any of: a keyword is added, the site's settings are saved, a login is saved. Setting something up never brings an already waiting run closer. Each run queues the next one `ranking_frequency` later (a day or a week). A run that comes due while tracking is off, there are no keywords or there is no login asks nothing and stops the chain until one of those changes; one that comes due soon after a manual check is skipped and the chain goes on. When a run ends the checks run again on the latest crawl.

New checks, `not_run` until a keyword has been checked at the site's location: `rank_dropped` (technical; a keyword fell 5 or more places since the check before in the same location, or left the top 100) and `ai_overview_not_cited` (ai readiness; Google's AI Overview cites other sites and not this one; the evidence lists them). Their items are named `"<keyword>" in Google results`, point at the ranking page (or the home page) in their evidence, and settle when a later check no longer finds them.

## AI visibility

Asks AI assistants the questions customers ask and notes whether the site is mentioned or cited. Each engine is optional and is turned on by a key saved in Settings, or in the server's environment as a fallback (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `XAI_API_KEY`, `GEMINI_API_KEY`, `PERPLEXITY_API_KEY`; models from `CLAUDE_MODEL`, `OPENAI_MODEL`, `GROK_MODEL` (default `grok-4.7`), `GEMINI_MODEL`, `PERPLEXITY_MODEL`). Keys saved in the app are sealed in the database; no key is ever returned or logged. An answer stores only its text (cut at 20,000 characters), its citations and token counts; an engine's own error text is never stored, only a plain sentence such as "The engine rejected the API key (HTTP 401)."

- `GET /sites/{id}/ai/settings`, `PUT` with `{ brand_name, aliases: string[], competitors: string[] }` → the saved settings (all three default to empty). The brand name and each alias are at most 100 characters, at most 10 aliases and 10 competitors, else 422. Competitors are domains; an address such as `https://www.rival.com/pricing` is reduced to `rival.com`, and anything that is not a domain is 422. Duplicates (any case) are dropped.
- `GET /sites/{id}/ai/prompts` → `[{ id, text, created_at }]`. `POST` with `{ text }` → 201; the text is trimmed and must be 10 to 300 characters (422), at most 25 per site (409 `"A site can have at most 25 questions."`), unique ignoring case (409). `DELETE /ai/prompts/{id}` keeps the answers already stored and re-runs the checks so the question's item resolves.
- `POST /sites/{id}/ai/prompts/suggest` → `{ suggestions: string[] }`: up to 10 questions written by Claude without web search, none naming the brand, 10 to 300 characters each. Nothing is saved. 422 without a Claude key (saved in Settings or `ANTHROPIC_API_KEY`), 502 when Claude fails, 429 ("You have asked for suggestions 10 times today for this site. Try again tomorrow.") after 10 requests for the site in a UTC day. Every request counts, including one Claude then fails.
- `POST /sites/{id}/ai/runs` → 202 `Run`. 422 when no engine is set up or the site has no questions; 409 while a run is queued or running (the database allows one per site); 429 after 3 manual runs for the site in a UTC day ("You have used today's 3 AI visibility checks for this site. Try again tomorrow."), and 429 after 10 manual runs for the workspace in a UTC day, across all its sites ("You have used today's 10 AI visibility checks for this workspace. Try again tomorrow."). The workspace count is kept in `ai_usage`, which a deleted site does not take with it, so deleting and re-adding a site does not give runs back. Scheduled runs are not counted.
- `GET /sites/{id}/ai/runs?page&limit` (newest first), `GET /ai/runs/{id}` → `Run = { id, status: "queued" | "running" | "completed" | "failed", trigger: "manual" | "scheduled", engines: string[], answers_total, answers_done, error: string | null, started_at, finished_at, created_at }`. `engines` are the engines that were set up when the run was made. A run is `failed` only when every answer failed. Grok is asked through xAI's Responses API with its web search tool; its citations are the inline `url_citation` annotations first, then the rest of the response's `citations` list, each address once.
- `GET /ai/runs/{id}/answers` → `[{ id, prompt_id, prompt, engine, model, mentioned, cited, citation_rank: int | null, cited_domains: string[], competitor_mentions: string[], error: string | null }]`, ordered by question then engine.
- `GET /ai/answers/{id}` → the same plus `answer_text`, `citations: [{ url, title: string | null, domain }]` and `notes: string[]` (things that went wrong without spoiling the answer, such as a web search that failed).
- `GET /sites/{id}/ai/domains` → the 20 most cited domains in the latest completed run: `[{ domain, citations, answers, is_you, is_competitor }]`. Empty before a run.

Detection: `mentioned` is a whole-word, case-insensitive match of the brand name or an alias, or the site's host (with or without `www.`) in the answer text, so "Apple" is not found in "pineapple". `cited` is a citation on the site's host or a subdomain; `citation_rank` is the 1-based position of the first such citation. Competitors are found by a cited domain, the domain in the text, or the domain's first label (three letters or more) as a whole word. Gemini's citation addresses are Google redirects that are never followed; its citation domain is read from the citation title, so it is empty when the title is not a host name.

A run is an `ai_run` job. The person's run queues one with the run's id; a weekly run is queued when the first question is added (if an engine is set up) and after each run, and makes its own `scheduled` run when it comes due, unless a run is already under way. Each engine is asked at most 2 questions at a time, one engine failing never fails the run, and a retried job asks again only what it has not stored. When a run completes the checks run again on the latest crawl: `ai_not_mentioned` (category ai readiness) has one item per question, target `"<question>"` with the quotes, when at least one engine answered and none mentioned or cited the site; the item resolves when a later run does.

### Answer-readiness checks

Seven checks in the `ai_readiness` category read only the crawled page, so they need no outside data. They make a page easier for an AI answer to quote; they do not promise an AI Overview or a citation. Page checks apply to working HTML pages of 300 or more words that are not the home page, and skip pages whose facts are unknown.

| id | severity | effort | fires when |
|---|---|---|---|
| `answer_no_question_headings` | 1 | 2 | no heading ends in `?` or starts with who, what, when, where, why, how, can, does, is, are, should |
| `answer_long_opening` | 1 | 1 | the first paragraph outside navigation is over 80 words |
| `answer_no_lists_or_tables` | 1 | 2 | 600 or more words and no list or table in the main text |
| `answer_no_sources` | 1 | 2 | 600 or more words and no link to another host in the main text |
| `schema_no_article` | 2 | 2 | the page has a date or an author but no Article, BlogPosting, NewsArticle, TechArticle or ScholarlyArticle structured data |
| `schema_no_organization` | 2 | 1 | site-level: the home page has no Organization, Corporation, LocalBusiness or NGO structured data (a `WebSite` block alone does not count; types written as `https://schema.org/Article` or `http://schema.org/Organization` are read as the bare name) |
| `schema_faq_missing` | 1 | 2 | 3 or more question headings and no FAQPage structured data |

Structured data types are found anywhere in a JSON-LD block (`@graph`, arrays, nested objects, `@type` arrays). "Main text" leaves out `nav`, `header`, `footer` and `aside`.
