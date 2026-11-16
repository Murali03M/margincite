# Margincite backend

A workspace adds a website. The service crawls it in the background and stores every page it found. Later plans add the checks, the suggestions, sign-in, and the Claude connector.

The crawler calls itself `MarginciteBot`. It obeys `robots.txt`, skips private and internal addresses, and keeps each workspace's sites separate.

## Requirements

Every command starts with this, because `cargo` and `psql` are not on the default `PATH`:

```bash
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"
```

Rust is pinned to 1.100.0 for this crate only. Postgres 14 is already running locally. Use the database `margincite_dev` and nothing else.

## Settings

Only `DATABASE_URL` has to be set in a file. Everything else (AI provider keys and models, Google sign-in, DataForSEO, PageSpeed and the crawler's contact address) can be entered in the app under Settings, per workspace. A value saved in the app wins; the environment variable of the same meaning below is the fallback when a workspace has saved none, so an existing `.env` keeps working. Secrets entered in the app are encrypted in the database and are write-only: the app and the API only say a secret is set, never show it.

The encryption key is made for you. When `MARGINCITE_SECRET_KEY` is not set, the first start creates `.data/secret.key` (32 random bytes, base64, readable by you only) next to where the server runs, and reuses it afterwards. The `.data` folder is git-ignored. **Back it up**: without that key, secrets saved in the app cannot be read and have to be entered again. If the file is damaged the server stops with a message instead of replacing it. `MARGINCITE_DATA_DIR` moves the folder.

Copy the example and edit it:

```bash
cp .env.example .env
```

| Setting | Required | Purpose |
|---|---|---|
| `DATABASE_URL` | yes | Postgres URL for `margincite_dev` |
| `CRAWLER_CONTACT` | no | Fallback email or page URL sent with every crawl request. Set it in the app instead; without one anywhere, adding a site or crawling is refused until you do. |
| `MARGINCITE_DATA_DIR` | no | Folder for the generated `secret.key`. Defaults to `.data`. |
| `BIND_ADDR` | no | Defaults to `127.0.0.1:8787` |
| `DEV_API_TOKENS` | no | Comma-separated `token=email` pairs for local sign-in |
| `CHROME_PATH` | no | Chrome binary used to render pages that need JavaScript. Does nothing unless `RENDER_SITES_I_TRUST=yes` is also set. |
| `RENDER_SITES_I_TRUST` | no | Set to `yes` to allow rendering. Without it, `CHROME_PATH` is ignored and a warning is logged. |
| `MARGINCITE_SECRET_KEY` | no | Encrypts everything secret in the database. 32 random bytes as base64 (`openssl rand -base64 32`). When set, no key file is read or written. An invalid value stops start-up. |
| `PUBLIC_BASE_URL` | no | Where the web app lives, for sign-in redirects. Defaults to `http://localhost:5174`. |
| `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` | no | Google OAuth client for Search Console. Set both or neither; one alone stops start-up. |
| `ANTHROPIC_API_KEY`, `CLAUDE_MODEL` | no | Claude as an AI engine. Model defaults to `claude-opus-5-5`. |
| `OPENAI_API_KEY`, `OPENAI_MODEL` | no | ChatGPT as an AI engine. Model defaults to `gpt-5.6`. |
| `XAI_API_KEY`, `GROK_MODEL` | no | Grok as an AI engine. Model defaults to `grok-4.7`. |
| `PERPLEXITY_API_KEY`, `PERPLEXITY_MODEL` | no | Perplexity as an AI engine. Model defaults to `sonar`. |
| `GEMINI_API_KEY`, `GEMINI_MODEL` | no | Gemini as an AI engine. Model defaults to `gemini-3.5-flash`. |
| `DATAFORSEO_LOGIN`, `DATAFORSEO_PASSWORD` | no | DataForSEO account. Set both or neither. |
| `PAGESPEED_API_KEY` | no | Google PageSpeed Insights key. |

An engine without a key is off. `GET /api/v1/integrations` shows which ones are on for your workspace, and `GET /api/v1/settings` shows where each value comes from.

**Warning:** rendering lets the pages being crawled make your machine fetch internal addresses (for example `127.0.0.1` or a cloud metadata address). Turn it on only on your own machine, for sites you own.

Example `.env`:

```bash
DATABASE_URL=postgres://localhost/margincite_dev
BIND_ADDR=127.0.0.1:8787
DEV_API_TOKENS=dev-token=you@example.com
CRAWLER_CONTACT=https://example.com/margincite-bot
```

## Database

```bash
createdb margincite_dev
set -a && . ./.env && set +a   # the service reads its settings from the environment
cargo run
```

`cargo run` does not read `.env` by itself. `scripts/dev.sh` in the repository root loads it for you and starts the web app too.

Starting the service applies the migrations. It listens on `BIND_ADDR` and runs the background worker in the same process. Ctrl-C stops both.

## Crawl a site

```bash
curl -s -X POST http://127.0.0.1:8787/api/v1/sites \
  -H "authorization: Bearer dev-token" \
  -H "content-type: application/json" \
  -d '{"url":"https://example.com"}'
```

The response includes `first_crawl_id`. Read it back:

```bash
curl -s http://127.0.0.1:8787/api/v1/crawls/CRAWL_ID \
  -H "authorization: Bearer dev-token"
```

The crawler contact (Settings, or `CRAWLER_CONTACT`) is sent on every request the crawler makes. Set it to an address a site owner can use before you point this at a real site.

## Checks that need nothing else

Besides the technical and content checks, seven answer-readiness checks (question headings, a short opening, lists and tables, outside sources, Article, Organization and FAQ structured data) read only the crawled pages. A page crawled before they existed is skipped until the next crawl. `API.md` lists them.

## Page speed

Page speed comes from Google PageSpeed Insights. Add a free API key in Settings (or `PAGESPEED_API_KEY`): requests without a key share one allowance between everyone, it is usually used up, and the page then says to add a key. It tests the home page and four other pages after each crawl, or on demand. In tests PageSpeed is a local mock like every other outside service.

## Search data (DataForSEO)

A DataForSEO login (Settings, or `DATAFORSEO_LOGIN` and `DATAFORSEO_PASSWORD`) unlocks rank tracking, Google AI Overview tracking, search volume and keyword ideas. It is **paid per use** (DataForSEO asks for a $50 minimum top-up), and Margincite never calls it without the workspace's own login. Rough prices, from DataForSEO's pricing pages: a ranking check costs about **$0.022 per keyword** (a 100-result Google page is ten $0.002 pages, plus $0.002 for the AI Overview option), so 50 keywords cost about $1.10 a check, around $33 a month if checked daily or $4.70 weekly. Search volume (one request refreshes all of a site's keywords) and keyword ideas cost $0.09 a request however many keywords it holds, and nothing refreshes them unless a person asks. The Usage screen shows what was spent as an estimate; the DataForSEO dashboard is the real bill. Turn rank tracking off per site in the site's settings, or check weekly, to spend less. In tests DataForSEO is a local mock like every other outside service.

## Tests

Tests never contact a real website. Crawls in tests target a local fake site.

```bash
export DATABASE_URL=postgres://localhost/margincite_dev
CARGO_BUILD_JOBS=4 cargo test -- --test-threads=4
```

Coverage:

```bash
CARGO_BUILD_JOBS=4 cargo llvm-cov --fail-under-lines 80 -- --test-threads=4
```

The Chrome tests need `CHROME_PATH` (they do not need `RENDER_SITES_I_TRUST`). Without it they print a skip reason and pass.
