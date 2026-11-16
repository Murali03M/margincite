# Margincite landing page

The public page for Margincite. It is a static site: the build renders the page to complete HTML, so every word is present before JavaScript runs. It does not depend on the backend.

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
| `pnpm test:e2e` | Browser tests. Builds into `dist-e2e/` and serves that, so `dist/` is left alone. The visit counter is answered locally |

Unit tests run with two workers because the machine is shared. Add a path to run one file, for example `pnpm exec vitest run tests/unit/cta.test.tsx --maxWorkers=2`.

## Settings

| Variable | Used when | Meaning |
|---|---|---|
| `SITE_URL` | build | The public address, for the canonical link, sitemap and robots.txt. Must start with `http://` or `https://`. Defaults to `https://margincite.com`. |
| `BASE_PATH` | build | The path the page is served under, ending in `/`. Defaults to `/`. The GitHub Pages workflow sets it to `/margincite/`. |

## Publishing

In the repository's **Settings > Pages**, set **Source** to **GitHub Actions**. The workflow then tests, builds and publishes the page.

## Visit count

The count in the top bar comes from [Abacus](https://github.com/JasonLovesDoggo/abacus), a free, open-source counting service with no account and no cookies (`src/lib/visits.ts`). A browser tab counts its first view and only reads the count after that. The HTML carries no number; if the service is slow or down, the counter simply does not appear.

## Where things are

- `src/content.ts` holds every word on the page. Components contain no copy of their own.
- `src/components/Exhibit.tsx` draws the example finding in the hero: the file, the numbered markers, the scale and the exhibit label. It is an illustration and must stay labelled as an example.
- `src/components/marks.tsx` holds the drawn marks: the evidence marker, the planned marker and the icons.
- `src/lib/visits.ts` loads the visit count.
- `src/seo.ts` builds the head tags, structured data (including the FAQ), `robots.txt` and `sitemap.xml`.
- `src/index.css` and `../shared/tokens.css` hold the colours, type and the few shared classes.
- `scripts/shot.mjs` takes a screenshot of a page, for checking the design.

## Origin

The first version started from an MIT-licensed design. See `NOTICE.md`.
