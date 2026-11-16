# Margincite

Margincite checks a website for three things and tells you what to fix:

- **SEO:** how well search engines can crawl, read and rank the pages.
- **AIO:** whether Google's AI Overviews appear for your keywords and cite you.
- **GEO:** how often AI assistants (Claude, ChatGPT, Gemini, Grok, Perplexity) mention or cite the site when asked your customers' questions.

You add a site, it crawls the pages, runs the checks, and gives a ranked list of suggestions. Each suggestion says what was found, why it matters and how to fix it. You run it on your own machine with your own provider keys.

Website: https://murali03m.github.io/margincite/

**Status: early.** It works end to end on a local machine. Sign-in is a development token, so do not put an instance on the open internet yet. See `SECURITY.md`.

## What it does

| Area | What you get | Needs |
|---|---|---|
| Crawl and site health | A score, every page found, and checks for titles, headings, links, structured data, robots rules and AI-crawler access | Nothing |
| Suggestions | A ranked to-do list built from the failed checks, with the pages each one affects | Nothing |
| Keywords | Checks that each target keyword is used where it counts | Nothing |
| Page speed | Core Web Vitals and performance scores | A free Google PageSpeed Insights key |
| Organic traffic and positions | Clicks, impressions and positions from your own search data | Google Search Console |
| Backlinks | How many links point at the site and which pages they reach | Bing Webmaster Tools key (free) |
| AI visibility and mentions | Which assistants mention or cite you, and for which questions | A key for at least one AI provider |
| Rank tracking and AI Overviews | Weekly or daily positions for your keywords, and whether an AI Overview appears and cites you | DataForSEO account (paid) |
| Search volume and keyword ideas | Monthly searches and related keywords | DataForSEO account (paid) |

Everything in the "Needs" column is optional. A feature without its connection is simply off, and the app says how to turn it on.

## Run it

You need Rust (the version is pinned in `backend/rust-toolchain.toml`), PostgreSQL 14 or later, Node 20 or later, and pnpm.

```bash
createdb margincite_dev
cp backend/.env.example backend/.env    # then set DATABASE_URL for your machine
pnpm --dir web install
bash scripts/dev.sh
```

Open http://localhost:5174 and sign in with the token from the `DEV_API_TOKENS` line in `backend/.env`. In the example file that is `dev-token`.

Then:

1. Open **Settings** and enter a contact address for the crawler. It is sent with every request so site owners can reach you, and crawling is refused until one is set.
2. Choose **Add a site** and enter the address.
3. Read the overview once the crawl finishes, and work through the suggestions.

## Connect your providers

Every connection is set up in the app, not in a file. Workspace-wide ones are under **Settings**; per-site ones are on the site's **Connections** screen.

| Connection | Where | What to enter |
|---|---|---|
| AI providers (Claude, ChatGPT, Gemini, Grok, Perplexity) | Settings | An API key, and optionally a model name |
| DataForSEO | Settings | Login and password |
| Google PageSpeed Insights | Settings | An API key |
| Google sign-in for Search Console | Settings, then the site's Connections | An OAuth client ID and secret, then approve access per site |
| Bing Webmaster Tools | The site's Connections | An API key and the site address as Bing lists it |

Keys you enter are encrypted before they are stored, and nothing returns them afterwards. The app only says whether one is set. The encryption key is created on first start at `backend/.data/secret.key`. **Back that file up**: without it, stored keys cannot be read and have to be entered again.

Settings in `backend/.env` still work as a fallback for a workspace that has saved none. `backend/README.md` lists them.

### Setting up Google Search Console

1. In the [Google Cloud console](https://console.cloud.google.com/), create a project or pick one.
2. Open **APIs & Services > Library**, find **Google Search Console API** and choose **Enable**.
3. Open **APIs & Services > OAuth consent screen**. Choose **External**, fill in the app name and your email, and add the scope `https://www.googleapis.com/auth/webmasters.readonly`.
4. While the app is in **Testing**, add your own Google account under **Test users**.
5. Open **Credentials > Create credentials > OAuth client ID** and choose **Web application**.
6. Under **Authorized redirect URIs**, add the address shown on the Settings screen. On a local machine it is `http://localhost:5174/api/v1/oauth/google/callback`.
7. Enter the client ID and secret under **Settings**.
8. Open the site's **Connections** screen, choose **Connect Google Search Console**, approve access and pick the property.

Google expires sign-ins after about 7 days for apps whose consent screen is in **Testing**. The connection then shows **needs reconnecting**. Moving the consent screen to **In production** avoids this.

## What it costs

Margincite itself is free. The outside services charge you directly:

- **AI visibility runs** send one request per question per engine. Ten questions on four engines is 40 requests, billed by each provider at its own rates. Web search is often billed on top.
- **DataForSEO** charges per keyword checked and has a minimum top-up.
- **Search Console, Bing Webmaster Tools and PageSpeed Insights** are free.

The **Settings** screen shows how many calls each service has made, and each site has daily limits on paid runs. Set a spending limit with each provider as well.

## Layout

| Folder | What it is |
|---|---|
| `backend/` | The Rust service: API, crawler, checks, integrations and background jobs. See `backend/README.md` and `backend/API.md` |
| `web/` | The web app (React, TypeScript, Vite). See `web/README.md` |
| `landing/` | The public landing page |
| `shared/` | Styles shared by `web/` and `landing/` |
| `scripts/` | `dev.sh` starts the backend and the web app together |

## Contributing

See `CONTRIBUTING.md` for how to run the tests and the rules a change has to meet. Report security problems privately, as described in `SECURITY.md`.

## Licence

Copyright (C) 2026 The Margincite contributors.

Margincite is free software under the GNU Affero General Public License, version 3. See `LICENSE`. Anyone may use, change and share it. Anyone who runs a changed version as a service for others must make their changes available under the same licence.

The landing page began from an MIT-licensed design, credited in `landing/NOTICE.md`.
