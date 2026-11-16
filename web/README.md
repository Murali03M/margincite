# Margincite web app

The browser app for Margincite. Add a site, watch the crawl, and work through the ranked suggestions. It is a Vite and React project that talks only to the backend's API (`backend/API.md`). It follows `DESIGN.md`.

## On this machine

```bash
export PATH="/opt/homebrew/bin:$PATH"
```

## Commands

Run these from `web/`, or use `pnpm --dir web ...` from the repository root.

| Command | What it does |
|---|---|
| `pnpm dev` | Development server at http://localhost:5174. It proxies `/api` and `/healthz` to the backend at 127.0.0.1:8787 |
| `pnpm build` | Type check and build into `dist/` |
| `pnpm preview` | Serve `dist/` at http://localhost:5174 |
| `pnpm test` | Unit tests (Vitest, with the API mocked by MSW) |
| `pnpm exec vitest run tests/unit --maxWorkers=2 --coverage` | Unit tests with coverage. `src/` must stay at 80% or more |
| `pnpm test:e2e` | Browser tests. Builds into `dist-e2e/` and serves it on port 5175 |

Unit tests use two workers because the machine is shared. Add a path to run one file, for example `pnpm exec vitest run tests/unit/pages.test.tsx --maxWorkers=2`.

The browser tests answer the API with Playwright routes (`tests/e2e/fakeApi.ts`). They do not start the backend, because the real backend correctly refuses to crawl a local test site.

## Signing in

Sign-in is the development sign-in. Paste an access token from `DEV_API_TOKENS` in `backend/.env` (for the example file, `dev-token`). The token is kept in the browser tab's session storage and is gone when the tab closes.

## Running both halves

From the repository root:

```bash
scripts/dev.sh
```

This starts the backend (`cargo run` in `backend/`) and the web app (`pnpm dev` in `web/`), and Ctrl-C stops both. It needs `backend/.env` (copy `backend/.env.example`) and a running Postgres.

## Crawling a real site

Set `CRAWLER_CONTACT` in `backend/.env` to a real email address or web page first. It is sent with every request so site owners can reach you. Then open http://localhost:5174, sign in, choose Add a site, and enter the address. The overview shows the crawl's progress and then the score and suggestions.

## Where things are

- `src/App.tsx` declares every route.
- `src/api/` holds the client, the response types and the TanStack Query hooks.
- `src/ui/` holds the shared parts: buttons, fields, tables, states and the shell.
- `src/sites/`, `src/suggestions/`, `src/pages/` hold the screens.
