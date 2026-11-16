# Margincite: design

Date: 2026-11-13
Status: approved by the owner on 2026-11-13

## Amendment, 2026-11-13: no desktop app

After the crawler backend was built, the owner decided the desktop app is not needed. The Claude connector is the only way into Margincite. Everything in this document about the Electron app, its dashboard screens and its packaging is withdrawn. What that changes:

- **People use Margincite from inside Claude.** They add the connector, sign in in the browser when Claude asks, and then add sites, start crawls and read suggestions in a conversation.
- **There is no dashboard to build.** Sections 6.7 and the Electron parts of 6.1, 6.10 and 6.11 no longer apply.
- **Connecting Google Search Console still needs one small web page**, because Google's consent screen has to return somewhere. That page belongs to the Search Console plan. It is a single page served by the backend, not an app.
- **The accepted trade-off:** someone who does not use Claude cannot use Margincite. This was approach A in section 5.
- **The backend still must be reachable from the internet** for Claude to connect to it, through a tunnel while testing and a host later.

Phase 1 is now five plans: landing page (built), backend foundation and crawler (built), checks and suggestions, sign-in and the Claude connector, Google Search Console.

## Amendment, 2026-11-13 (later the same day): a web app first

The owner then asked for a complete web application in the browser, built before the connector, so they can test Margincite on their own company website. This restores approach B from section 5, with a web app in place of the Electron app:

- **A web app comes first.** It covers adding a site, watching the crawl, the ranked suggestions with evidence, marking items done or dismissed, and the crawled pages. Section 6.7's screens apply again, as web pages, except Connections, which waits for Search Console.
- **There is still no desktop app.**
- **The Claude connector comes after the web app**, on the same backend.
- **Sign-in in the web app is the development token for now.** Real sign-in arrives with the connector plan, which needs it anyway.

Order of work: checks and suggestions plus the web app (one plan), then sign-in and the Claude connector, then Google Search Console.

Lines marked **(my call)** were decided by Claude, not by the owner. Everything else was said or approved by the owner in conversation.

## 1. Intent and why

Margincite is an SEO and GEO (generative engine optimisation) tool. A person adds a website, Margincite reads the whole site, and it returns a ranked list of suggestions. Later phases write the fixes as posts or page rewrites, publish them as drafts through integrations, and track whether AI answer engines cite the site.

It has two doors onto one engine:

- a desktop app built with Electron, usable by anyone
- a Claude connector, so the same actions work from inside Claude

Both talk to one hosted backend written in Rust.

The owner will test it on their own site first. If it works, they will open it to outside customers. It is therefore built for customers from the first day (sign-up, each customer's data kept separate), with billing left until customers arrive.

The gap it aims at: existing tools either track AI visibility (Profound, Peec, Otterly) or auto-write and auto-publish posts (SEObot, Outrank, seo.ai). Few do both, and working inside Claude is uncommon.

## 2. Phases

The product is four projects. They are built in this order, and each one gets its own plan. **This document specifies phase 1 in detail** and records the later phases only far enough that phase 1 does not block them.

| Phase | What it adds |
|---|---|
| 1 | Landing page, accounts, add a site, crawl, checks, ranked suggestions, paper-style dashboard, Google Search Console, Claude connector |
| 2 | Writing fixes and posts with AI, human approval, publishing as drafts to the site |
| 3 | AI visibility tracking per engine, Bing Webmaster Tools |
| 4 | Billing, Claude directory listing |

### Release target for phase 1

The owner said they will test on their own site first and publish only if it works. So phase 1 ships as owner-only **(my call on the details)**:

- The backend runs on the owner's machine, with a tunnel on a fixed address so the identity provider, Google and Claude all have a stable address to redirect to.
- The desktop app is a local, unsigned build for macOS. Code signing, installers for other systems and automatic updates belong to the publish step.
- Sign-up is limited to an allowlist of email addresses. The sign-up, workspace and separation code is all real, so opening it later means removing the allowlist, not rebuilding.
- Putting it on a production host with open sign-up is a separate step the owner triggers when they decide to publish. That step brings in the privacy policy, abuse controls for the crawler, and Google's consent-screen review. None of those are phase 1 work.

## 3. Goals, non-goals, anti-goals

### Goals for phase 1

1. A person can sign up, add a site, and get a full crawl of it.
2. They see a ranked list of suggestions. Each one shows the evidence (which pages, what was found) and the fix.
3. They can connect Google Search Console, and the suggestions use real query data.
4. They can add Margincite to Claude as a custom connector and do all of the above from a Claude conversation.
5. One customer can never see another customer's data.

Phase 1 is successful when the owner runs it on their own site, agrees the suggestions are accurate and useful, and can drive it from Claude.

### Non-goals for phase 1

- Writing posts or page rewrites
- Publishing anything to a site
- AI visibility tracking (asking ChatGPT, Claude, Gemini, Perplexity)
- Bing Webmaster Tools
- Billing, plans, usage limits by plan
- Claude directory submission
- Team roles and invitations (one person per workspace is enough) **(my call)**
- Backlink data and keyword search-volume databases **(my call)**

### Anti-goals (a failure even if it works)

- Publishing to a customer's site without a person approving it. Google's spam policy targets mass-produced pages.
- Bulk page generation.
- Promising that a change will make AI engines cite the site. The 2026 research is unsettled and mostly vendor-written, so Margincite measures and reports. It does not promise.
- Crawling that ignores robots.txt or hammers a site.
- Presenting llms.txt as an important fix. Ahrefs found 97% of those files receive no requests.

## 4. Constraints

- **Claude connector limits.** Each tool call must finish within 240 seconds and return under about 150,000 characters. A crawl takes longer than that, so crawls run in the background and the connector starts them and checks on them.
- **Connector standard.** Remote server over Streamable HTTP. OAuth where each user signs in with their own account. Claude's client follows the 2025-03-26, 2025-06-18 and 2025-11-25 authorisation specs and can register itself through Dynamic Client Registration (DCR). The redirect address `https://claude.ai/api/mcp/auth_callback` must be allowed, plus a loopback redirect for Claude Code. The 2026-07-28 MCP spec prefers Client ID Metadata Documents (CIMD) over DCR, so both should be supported.
- **Not supported by Claude:** resource subscriptions, sampling, draft capabilities. Do not depend on them.
- **Directory listing later** needs every tool annotated as read-only or not, a privacy policy, and a production deployment. Phase 1 annotates tools now so phase 4 is not a rewrite.
- **Google sign-in review.** Before outside customers can connect Search Console, Google will probably need to review the app's consent screen. This was not confirmed by research. It does not block testing on the owner's own account.
- **UI reference.** The owner shared https://github.com/pulkitxm/claude-directory/tree/main/ui-design/handcraft-paper-ui as the reference for the landing page. It is a React, Vite and Tailwind landing page for a fictional product. The repository's root licence is MIT and covers this folder, so its code may be used as the starting point as long as the MIT copyright notice is kept. Its fictional content (product name, copy, logos, testimonials, numbers) is replaced entirely.
- **Development machine.** 16 GB RAM shared by many sessions. Run Vitest with two workers, limit parallel Rust build and test jobs, and filter test paths. Tools live under `/opt/homebrew/bin` (Node 26, pnpm, Postgres 14, ngrok, gh) and `~/.cargo/bin` (Rust 1.81, which is from 2024 and needs `rustup update` first). Docker is installed. The default shell PATH includes none of these.
- **Owner's standing rules.** Tests first, 80% coverage, immutable data patterns, files under 800 lines, functions under 50 lines, no hardcoded secrets, input validated at every boundary, rate limiting on all endpoints, a consistent API response envelope.

## 5. Approaches

Three ways of building it were considered.

| Approach | What you get | Trade-off |
|---|---|---|
| A. Connector only | Claude does all the thinking and writing | Cheapest to build and run, but useless to anyone without Claude |
| B. Own app plus connector | Works on its own and inside Claude | Bigger build, and from phase 2 Margincite pays for AI usage per post |
| C. Tracker only | An AI visibility dashboard | The most crowded space, and it fixes nothing |

**The owner chose B.** It serves customers who do not use Claude, keeps the Claude connector as the differentiator, and both doors share one engine so nothing is built twice.

B was first proposed with a web app, a Python backend and Prisma. The owner then chose an Electron desktop app, a backend written entirely in Rust, and Postgres without Prisma. Prisma was dropped because it has no maintained Rust client.

## 6. Phase 1 design

### 6.1 How it gets made

Decided by the owner:

- **Backend:** Rust, for the whole backend: the API, the crawler, the checks, the background jobs and the Claude connector.
- **App:** Electron.
- **Database:** Postgres, without Prisma.

Filled in around those **(my calls)**:

- **Where things run:** the Rust backend and Postgres are hosted. The Electron app is a desktop window onto that backend and holds no product logic or data of its own. Postgres and the Claude connector both need a server, and customers sign up to shared accounts.
- **Rust web framework:** axum on tokio.
- **Database access:** sqlx, which also runs the migrations and checks queries against the real database at build time.
- **Background work:** a job queue stored in Postgres, so there is no second service to run. The builder picks the library.
- **Crawler:** reqwest for fetching and a Rust HTML parser. A headless Chromium, driven from Rust, for pages that need JavaScript.
- **Connector:** the official Rust MCP SDK (`rmcp`), served by the same backend at its own path, over Streamable HTTP. The builder confirms while planning that it covers Streamable HTTP and the authorisation metadata Claude needs.
- **App interface:** React, TypeScript, Vite and Tailwind inside Electron, matching the UI reference's stack. The same interface could later be served as a website with little extra work, but that is not in phase 1.
- **Sign-in:** a hosted identity provider that supports the MCP authorisation flow with both DCR and CIMD. WorkOS AuthKit is the first choice. The builder confirms this against current vendor documentation while planning and may substitute an equivalent. Margincite does not build its own password storage or OAuth server. The backend only verifies the tokens the provider issues.
- **AI model use:** none in phase 1. Checks are rules, and suggestions use written templates. This keeps phase 1 cheap and fully testable. Through the connector, the user's own Claude does any interpreting.
- **Hosting:** one container for the backend, with Chromium inside it. For connector testing, a tunnel (ngrok) to the local server. The production host is the builder's choice.

### 6.2 Parts

One engine, two doors. Each part has one job.

| Part | Job | Depends on |
|---|---|---|
| Accounts | Sign-in, workspaces, checking every request belongs to a workspace | Identity provider |
| Sites | Add, list and remove sites for a workspace | Accounts |
| Crawler | Fetch every page of a site and store what it found | Sites, job queue |
| Checks | Turn stored pages into findings. Pure functions, no network | Crawler output |
| Suggestions | Rank findings and track their status | Checks |
| Search Console | Connect a Google account, pull query and index data | Accounts, Sites |
| App API | Serve the desktop app | All of the above |
| Connector | Expose the same actions to Claude as tools | All of the above |
| Desktop app | The Electron app and its paper-style dashboard | App API |

The App API and the Connector both call the same service functions. Neither contains product logic of its own. All parts except the desktop app are Rust modules in one backend.

### 6.3 Data

Every table except `workspaces` carries a `workspace_id`. All reads and writes go through repository functions that require it, and tests prove a second workspace cannot read the first one's rows.

- `workspaces`, `users` (identity provider user id, workspace)
- `sites` (workspace, root address, display name, crawl settings)
- `crawls` (site, status, started, finished, page count, error)
- `pages` (crawl, address, status code, title, meta description, headings, canonical, robots directives, word count, extracted text, structured data found, links out, response time, whether content needed JavaScript)
- `findings` (crawl, check id, category, severity, target, evidence). The target is the one page or the one search query the finding is about. One finding per check per target.
- `suggestions` (site, check id, first seen, last seen). One per check per site. It is the group the user sees, such as "38 pages have no description".
- `suggestion_items` (suggestion, target, status of open, done or dismissed, first seen, last seen). One per page or query inside the group. Status lives here, and items persist across crawls **(my call)**. This is also the unit phase 2 writes a fix against: one page or one query, not one whole check.
- `gsc_connections` (workspace, site, Google property, encrypted refresh token)
- `gsc_rows` (site, date range, query, page, clicks, impressions, position)

### 6.4 Crawler

1. Read robots.txt and the sitemap, then follow internal links from the home page.
2. Fetch each page's raw HTML first. If the raw HTML contains very little text, load the page in a headless browser and record that the content needed JavaScript. That fact is itself a finding, because most AI crawlers do not run JavaScript.
3. Store the fields listed under `pages`.

Rules:

- Obey robots.txt. Identify as `MarginciteBot` with a contact address.
- At most two requests at a time per site, with a short delay.
- A page cap per crawl. Default 500 in phase 1 **(my call)**.
- Because the server fetches addresses that users type in, it must refuse private and internal network addresses, refuse non-HTTP schemes, and re-check after every redirect.
- Any signed-in user may crawl any public site, within the page cap and a per-workspace daily crawl limit **(my call)**. Connecting Search Console is what proves ownership, and Google enforces that.
- A crawl that fails part-way keeps the pages it got and records the error. The user sees "crawled 212 of about 400 pages, stopped because …".
- **Scope (my call):** a site is one host, with and without `www` treated as the same. Other subdomains such as `blog.` or `docs.` are separate sites the user adds themselves. A site may also be limited to a sub-path.
- **Schedule:** every site is re-crawled weekly by the backend, whether or not the desktop app is open, and can be re-crawled by hand at any time. Search Console data is refreshed with each crawl. Scheduled crawls do not count against the daily crawl limit **(my call)**.
- **Retention (my call):** page text is kept for the two most recent crawls of a site. Older crawls keep their findings and counts but drop the page text.

### 6.5 Checks

Each check is a small pure function: pages in, findings out. Each has an id, a category, a severity, and a written template for the evidence and the fix.

**Technical**
- Broken internal links, error pages, redirect chains
- Missing, duplicate, too-long or too-short titles and meta descriptions
- Missing or multiple main headings
- Canonical problems, pages blocked from indexing that are also in the sitemap
- Sitemap missing, or sitemap and crawl disagreeing
- Images without alt text
- Missing mobile viewport, not HTTPS, slow responses, heavy pages

**AI readiness**
- robots.txt rules for the AI crawlers: GPTBot, OAI-SearchBot, ChatGPT-User, ClaudeBot, Claude-SearchBot, Claude-User, PerplexityBot, Google-Extended, Bingbot
- Content that only appears after JavaScript runs
- Structured data present and well-formed (organisation, article, FAQ, product, breadcrumb)
- Visible published and updated dates, and author information
- llms.txt present or absent, reported as information only

**Content**
- Thin pages
- Near-duplicate pages

**Search Console (only when connected)**
- Queries with many impressions and a low click rate
- Pages ranking in positions 5 to 20, which are the cheapest to improve
- Queries the site appears for that have no page dedicated to them
- Pages in the crawl that Google has not indexed

### 6.6 Suggestions

Findings are grouped by check into suggestions and ranked **(my calls)**:

- Rank is severity times reach, divided by a fixed effort score per check.
- Reach is the share of the site's Search Console impressions that land on the affected pages, when Search Console is connected. Without it, reach is the share of pages affected. The screen says which one is in use, so a missing description on a busy page outranks one on a page nobody visits.

Each suggestion shows what was found, which pages or queries, why it matters, and how to fix it.

Status is per item (one page or one query), not per check:

- A user can mark single items done or dismissed, or do it for the whole group at once.
- The next crawl checks every done item. Fixed items stay done, and unfixed ones reopen.
- A dismissed item stays dismissed. A page that newly fails the same check shows up as a new open item, so dismissing 3 pages never hides the next 80.
- A group shows as done only when every item in it is done or dismissed.

### 6.7 Dashboard

The owner's reference is for the landing page (6.12). Carrying the same look into the app is **my call**, so the product reads as one brand. The app shares the landing page's colours, corner shapes, shadows and basic components through one shared styles package.

The look: off-white dotted paper, thick ink outlines with uneven corners, hard offset shadows, red and blue-pen accents, yellow sticky notes.

Handwriting fonts (Kalam, Patrick Hand) are used for headings, labels and sticky notes. Tables, long text and page addresses use a plain readable font **(my call)**, because handwriting fonts are hard to read in dense data. Motion respects the reduced-motion setting, and focus outlines stay visible.

Electron specifics **(my calls)**:

- Sign-in opens the system browser and returns to the app, so the app never sees a password. The session token is stored with the operating system's secure storage, not in a plain file.
- The window runs with Electron's isolation settings on: no Node access from the page, context isolation, sandboxing, and only the backend's address allowed for requests and navigation.
- The app shows a clear "can't reach Margincite" state when the backend is offline.

Screens:

1. **Sign in / sign up**
2. **Sites:** each site as a sticky note with its score and last crawl date, plus "add a site"
3. **Add a site:** one address field, then live crawl progress
4. **Site overview:** score, crawl status, the top five suggestions, counts by category
5. **Suggestions:** the ranked list, filterable by category and status, each opening to show evidence and fix
6. **Page detail:** everything found on one page
7. **Connections:** Google Search Console, and the steps to add Margincite to Claude
8. **Settings:** account, delete site, delete account

```
┌──────────────────────────────────────────────────────────┐
│ Margincite        Sites  Suggestions  Connections   (me) │
├──────────────────────────────────────────────────────────┤
│  example.com            score 62      crawled 2 days ago │
│                                                          │
│  ┌─ Top suggestions ─────────────────┐  ┌─ sticky ─────┐ │
│  │ 1  AI crawlers blocked in robots  │  │ 214 pages    │ │
│  │ 2  38 pages have no description   │  │ 17 open      │ │
│  │ 3  12 pages rank 5-20 on Google   │  │ 4 done       │ │
│  └───────────────────────────────────┘  └──────────────┘ │
└──────────────────────────────────────────────────────────┘
```

Every screen has a loading state, an empty state and an error state with a plain message.

### 6.8 Connector tools

Each tool finishes well inside 240 seconds, returns paginated results under the size limit, and is annotated as read-only or not.

| Tool | Read-only | What it does |
|---|---|---|
| `list_sites` | yes | Sites in the workspace |
| `add_site` | no | Add a site |
| `start_crawl` | no | Start a background crawl, returns a crawl id |
| `get_crawl_status` | yes | Progress and result of a crawl |
| `get_site_summary` | yes | Score, counts, top suggestions |
| `list_suggestions` | yes | Ranked suggestions, with filters and paging |
| `get_suggestion` | yes | One suggestion with its evidence and pages |
| `get_page` | yes | Everything stored for one page, including its text |
| `update_suggestion_status` | no | Mark done, dismissed or reopened, for chosen items in a suggestion or for the whole suggestion |
| `get_search_performance` | yes | Search Console queries and pages, when connected |

A tool called for a site outside the caller's workspace returns "not found", never "forbidden", so it does not confirm the site exists.

### 6.9 Errors

- The crawler records per-page failures and carries on. A crawl only fails as a whole when the site cannot be reached at all.
- When Google revokes access or a token expires, the connection shows as "needs reconnecting" and Search Console checks are skipped, with a visible note. Nothing fails silently.
- API errors use one envelope: a success flag, the data, an error message, and paging details where relevant.
- Server logs carry the detail. Messages shown to users do not expose internals.

### 6.10 Security

- Refuse internal network addresses in the crawler, as described above.
- Encrypt Google tokens at rest. Secrets come from the environment and are checked at start-up.
- Rate limit every endpoint and every connector tool.
- Validate every input with schemas at the API and connector boundaries.
- Treat crawled page content as untrusted data. It is shown as text, never run or rendered as markup, and when returned through the connector it is clearly marked as page content.
- Keep the Electron isolation settings from 6.7 on. Crawled content is never loaded into the app window as a page.

### 6.11 Testing

- Tests first, 80% coverage, measured separately for the Rust backend and the app interface.
- Rust unit tests for every check, using small saved pages.
- Crawler tests against a small fixture site served locally, covering robots.txt, redirects, a JavaScript-only page, and refused internal addresses.
- Integration tests for the API and for workspace separation, against a real test Postgres.
- Connector tests that call each tool through an MCP client.
- Interface component tests with Vitest.
- One end-to-end test that drives the Electron app: sign in, add a site, watch the crawl, open a suggestion, mark it done.

### 6.12 Landing page

A public marketing website for Margincite, in the style of the owner's reference. It is separate from the Electron app and does not depend on the backend, so it can be built first or alongside.

Starting point: the reference's code, with the MIT notice kept and all fictional content replaced.

Sections **(my calls on what to keep)**:

| Reference section | Margincite landing page |
|---|---|
| Nav | Keep: logo, section links, the call-to-action button |
| Hero | Keep: what Margincite does in one sentence, with a sketch of the suggestions board |
| Features | Keep: crawl, suggestions, Search Console, works inside Claude |
| How it works | Keep: add your site, read the notes in the margin, fix and re-check |
| Product detail | Keep: a closer look at one suggestion with its evidence and fix |
| Call to action | Keep: "Request early access" |
| Footer | Keep |
| Logo strip, stats, testimonials | Leave out until there are real customers and real numbers. Invented ones are not used. |
| Pricing | Leave out until phase 4 sets prices |
| Blog | Leave out until there are real posts |

Rules:

- **The page must pass Margincite's own checks.** The reference renders everything with JavaScript, which is the exact problem Margincite reports on other sites. The landing page is therefore pre-built to complete HTML, so all its text is present without JavaScript. It also has proper titles and descriptions, structured data, a sitemap, and a robots.txt that lets search and AI crawlers in.
- **Only true claims.** The copy describes what phase 1 actually does. Later phases are described as coming, not as available.
- **Call to action.** While the release is owner-only, the button is "Request early access". It collects an email address through a simple form service, since the backend is not public yet **(my call)**. It becomes "Download" when the app is published.
- **Fonts.** Kalam and Patrick Hand are served from the site itself. The reference names them but does not load them.
- **Accessibility.** Keep the reference's skip link, visible focus outlines and reduced-motion handling. Body text meets normal contrast levels.
- **Hosting.** A static host, on the Margincite domain once the owner has it. Until then it is built and previewed locally.

Tests: component tests for each section, a check that the built HTML contains the page text without running JavaScript, and one browser test of the early-access form.

## 7. Later phases, as far as they shape phase 1

- **Phase 2, writing and publishing.** Uses the Claude API from the backend. Drafts always wait for approval and are published as drafts. Stored page text and suggestions are the inputs, which is why phase 1 stores extracted text. Which publishing integration comes first depends on what the owner's test site runs on, which is not yet known.
- **Phase 3, AI visibility.** Runs a set of customer questions against each AI engine on a schedule and records who is cited. Engines overlap little in what they cite, so results are kept per engine. Bing Webmaster Tools is added here. Its AI Performance report may have no interface for automatic access.
- **Phase 4, billing and directory.** Adds plans, a privacy policy, and the Claude directory submission.

## 8. What the owner needs to provide

These need the owner's own accounts and cannot be done by the builder alone:

1. The address of the site to test on. **Still open.** Phase 1 is judged on this one site, so the builder needs it before planning the crawler in detail: roughly how many pages it has (the 500-page cap may need raising), whether its content needs JavaScript, whether it sits behind bot protection that would block `MarginciteBot`, and whether it spans subdomains.
2. A Google Cloud project, to create the sign-in credentials for Search Console.
3. An account with the identity provider.
4. A hosting account, when it is time to put it online.
5. The domain, which the landing page needs before it can go public. margincite.com and margincite.ai showed no registration on 2026-11-13. No formal trademark search has been done.
6. An email address or form-service account to receive early-access requests from the landing page.

## 9. Left to the builder

- Library choices within the stack above (job queue, HTML parser, headless browser driver, Electron packaging tool)
- Exact thresholds for each check (title length, thin-page word count, slow response)
- The scoring formula for the site score, provided it is explained on screen
- Folder layout, provided files stay small and are organised by feature
- Production host
