# Margincite landing page

The public marketing page for Margincite. It is a static site: the build renders the page to complete HTML, so every word is present before JavaScript runs. It does not depend on the backend.

It is published with GitHub Pages by `.github/workflows/landing-pages.yml` on every push to `main` that changes `landing/` or `shared/`.

## On this machine

Node and pnpm live under Homebrew and are not on the default PATH:

```bash
export PATH="/opt/homebrew/bin:$PATH"
```

## Commands

| Command | What it does |
|---|---|
| `pnpm dev` | Development server at http://localhost:5173 |
| `pnpm build` | Type check, build, render to static HTML, write `robots.txt` and `sitemap.xml` into `dist/` |
| `pnpm preview` | Serve the built site at http://localhost:4173 |
| `pnpm test` | Unit tests |
| `pnpm test:build` | Checks on the built files. Run `pnpm build` first |
| `pnpm test:e2e` | Browser tests. Builds into `dist-e2e/` with a test sign-up address and serves that, so `dist/` is left alone |
| `pnpm map` | Redraw the map artwork in `public/map/` |

Unit tests run with two workers because the machine is shared. Add a path to run one file, for example `pnpm exec vitest run tests/unit/cta.test.tsx --maxWorkers=2`.

## Settings

| Variable | Used when | Meaning |
|---|---|---|
| `SITE_URL` | build | The public address, for the canonical link, sitemap and robots.txt. Must start with `http://` or `https://`. Defaults to `https://margincite.com`. |
| `VITE_EARLY_ACCESS_ENDPOINT` | build | Where the early-access form posts. Must be an `https` address. Without it the form answers "Sign-up is not connected yet." and the build prints a warning. |
| `BASE_PATH` | build | The path the page is served under, ending in `/`. Defaults to `/`. The GitHub Pages workflow sets it to `/margincite/`. |

The endpoint must accept a JSON `POST` of `{ "email": "…" }` with `Accept: application/json` and answer with a 2xx status. Formspree works this way. Without JavaScript the form posts to the same address as a normal form.

## Publishing

In the repository's **Settings > Pages**, set **Source** to **GitHub Actions**. The workflow then tests, builds and publishes the page. To collect early-access sign-ups, add a repository variable `EARLY_ACCESS_ENDPOINT` under **Settings > Secrets and variables > Actions > Variables** and run the workflow again.

## Where things are

- `src/content.ts` holds every word on the page. Components contain no copy of their own.
- `src/components/CourseMap.tsx` draws the course over the map. The positions of the controls are constants at the top of the file.
- `scripts/generate-map.mjs` draws the terrain. It is seeded, so it produces the same files every time. The output in `public/map/` is committed.
- `src/seo.ts` builds the head tags, structured data, `robots.txt` and `sitemap.xml`.
- `src/index.css` holds the colours, type and the few shared classes.
- `scripts/shot.mjs` takes a screenshot of a page, for checking the design.

The map in the hero is an illustration of an imaginary site. It is labelled as an example on the page and must stay labelled.

## Origin

The first version started from an MIT-licensed design. See `NOTICE.md`.
