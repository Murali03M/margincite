# Margincite Landing Page Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the public Margincite landing page in the handcraft paper style, pre-built to complete HTML, with a working early-access form.

**Architecture:** A standalone Vite and React site in `landing/`, started from the MIT-licensed reference code and rewritten for Margincite. All copy lives in one `content.ts`. The build renders the page to static HTML on the server side and hydrates it in the browser, so every word is in the HTML before JavaScript runs.

**Tech Stack:** React 18, TypeScript, Vite 6, Tailwind 4, lucide-react, Vitest with Testing Library, Playwright, pnpm.

**Spec:** `docs/superpowers/specs/2026-11-13-margincite-design.md`, section 6.12 (landing page), plus sections 3 and 4 for the anti-goals and constraints.

## Where this plan sits

Phase 1 of the spec is five separate pieces. Each gets its own plan and ships working software on its own. This is plan 1.

| Plan | Piece | Status |
|---|---|---|
| 1 | Landing page | this document |
| 2 | Rust backend core: workspaces, sites, crawler, checks, suggestions, app API | not written |
| 3 | Sign-in and the Claude connector | not written |
| 4 | Electron app and dashboard | not written |
| 5 | Google Search Console | not written |

The spec asks for one shared styles package used by both the landing page and the app. That package is extracted in plan 4, when its second user exists. In this plan the styles stay inside `landing/`.

## Global Constraints

- Every shell command starts with `export PATH="/opt/homebrew/bin:$PATH"`. The default PATH has no Node or pnpm, and shell state does not persist between commands.
- Run Vitest with `--maxWorkers=2` and always with a path filter. The machine has 16 GB of RAM shared by many sessions.
- Work on the branch `feat/landing-page`, never on `main`.
- Commit messages use `<type>: <description>` with types feat, fix, refactor, docs, test, chore, perf, ci. No attribution lines.
- The reference is `https://github.com/pulkitxm/claude-directory`, folder `ui-design/handcraft-paper-ui`, at commit `29fe9fecf79efbf6abb07e9cddc5a77123930308`. Its MIT notice, `Copyright (c) 2026 Pulkit`, must stay in `landing/NOTICE.md`.
- None of the reference's fictional content survives: no "Scribbly", no invented user counts, logos, testimonials, statistics or prices.
- Copy describes only what phase 1 does. Later phases are labelled as coming later.
- The built HTML must contain all page text without JavaScript, and must not hide it with `opacity:0`.
- Fonts Kalam and Patrick Hand are served from the site itself. No request goes to Google Fonts.
- Keep the reference's skip link, visible focus outlines and reduced-motion handling.
- Tests first. Coverage of `landing/src` is at least 80% for lines, functions, branches and statements.
- Files stay under 800 lines and functions under 50. Never mutate existing objects; return new ones.
- No secrets in the code. The early-access endpoint comes from `VITE_EARLY_ACCESS_ENDPOINT` and the site address from `SITE_URL`.
- Do not deploy this page publicly before the phase 1 app exists, because the copy describes that app as working.

## Review Focus

1. **The sign-up service is unreachable.** The visitor sees an error message and the email they typed is still in the field. Tests in Task 4 and Task 7.
2. **The visitor double-clicks the submit button.** Exactly one request is sent. Test in Task 4.
3. **JavaScript never loads.** Every section is readable and the form still posts to the service as a normal form. Tests in Task 5 and Task 7.
4. **A 320-pixel-wide phone.** The page does not scroll sideways. Test in Task 7.
5. **`SITE_URL` has a trailing slash or no scheme.** The canonical address has no double slash, and a value without `http` or `https` stops the build with a clear message. Tests in Task 6.

---

## File structure

```
landing/
  NOTICE.md                  MIT notice, source repository and commit
  README.md                  how to run, build, test, and the two environment variables
  package.json
  index.html                 template with <!--app-head--> and <!--app-html--> markers
  vite.config.ts
  vitest.config.ts
  playwright.config.ts
  tsconfig.json
  vite-env.d.ts
  scripts/prerender.mjs      fills the markers, writes robots.txt and sitemap.xml
  src/
    content.ts               every word shown on the page
    seo.ts                   head tags, structured data, robots.txt, sitemap.xml
    entry-client.tsx         hydrates the page
    entry-server.tsx         renders the page to a string
    App.tsx
    index.css
    lib/tokens.ts            from the reference, unchanged
    lib/reveal.ts            scroll reveal that never hides server-rendered content
    lib/earlyAccess.ts       email validation and form submission
    components/
      primitives.tsx         from the reference, minus AvatarStack
      Logo.tsx  Nav.tsx  Hero.tsx  SuggestionSketch.tsx
      Features.tsx  HowItWorks.tsx  ProductDetail.tsx  Cta.tsx  Footer.tsx
  tests/
    setup.ts
    unit/                    Vitest, jsdom
    build/dist.test.ts       Vitest, node, reads landing/dist
    e2e/landing.spec.ts      Playwright
```

---

### Task 1: Bring in the reference and the test tooling

**Files:**
- Create: `landing/` from the reference folder (`src/`, `index.html`, `package.json`, `tsconfig.json`, `vite.config.ts`, `vite-env.d.ts`, `.gitignore`)
- Create: `landing/NOTICE.md`, `landing/vitest.config.ts`, `landing/tests/setup.ts`, `landing/tests/unit/app.test.tsx`
- Create: `.gitignore` at the repository root
- Delete: `landing/src/components/LogoStrip.tsx`, `Stats.tsx`, `Pricing.tsx`, `Testimonials.tsx`, `Blog.tsx`
- Modify: `landing/src/App.tsx`, `landing/src/components/Cta.tsx` (section id only), `landing/package.json`

**Interfaces:**
- Produces: `App` exported from `landing/src/App.tsx`. Section ids in page order: `top`, `features`, `how`, `product`, `early-access`.
- Produces: scripts `test` (`vitest run tests/unit --maxWorkers=2`) and `dev` in `landing/package.json`.

- [ ] **Step 1: Create the branch**

```bash
git checkout -b feat/landing-page
```

- [ ] **Step 2: Copy the reference in at the pinned commit**

Clone into a temporary directory outside the repository, check out only the one folder, and copy the listed files. Do not copy `package-lock.json`, `README.md` or `prompt.md`. Do not run anything from inside the clone.

```bash
export PATH="/opt/homebrew/bin:$PATH"
REF="$(mktemp -d)/ref"
git clone --filter=blob:none --no-checkout https://github.com/pulkitxm/claude-directory.git "$REF"
git -C "$REF" sparse-checkout set ui-design/handcraft-paper-ui
git -C "$REF" checkout 29fe9fecf79efbf6abb07e9cddc5a77123930308
```

- [ ] **Step 3: Write `landing/NOTICE.md`**

It states that the landing page started from the reference folder at the pinned commit, then gives the full MIT licence text from the reference's root `LICENSE` file, including the line `Copyright (c) 2026 Pulkit`.

Verify: `grep -c "Copyright (c) 2026 Pulkit" landing/NOTICE.md` prints `1`.

- [ ] **Step 4: Add the test tooling**

In `landing/package.json`: rename the package to `margincite-landing`, add dev dependencies `vitest` (3.x), `@vitest/coverage-v8` (3.x), `jsdom`, `@testing-library/react` (16.x), `@testing-library/user-event` (14.x), `@testing-library/jest-dom`, and replace `playwright` with `@playwright/test` at 1.56.0. Remove `motion` if `grep -r "from \"motion" landing/src` finds nothing.

`landing/vitest.config.ts`: jsdom environment, `tests/setup.ts` as the setup file, include `tests/unit/**` and `tests/build/**`, coverage over `src/**` excluding `src/entry-client.tsx`, thresholds of 80 for lines, functions, branches and statements.

`landing/tests/setup.ts`: imports `@testing-library/jest-dom/vitest`.

Root `.gitignore`: `node_modules`, `dist`, `dist-ssr`, `coverage`, `test-results`, `playwright-report`, `.env*`.

Run: `pnpm --dir landing install`

- [ ] **Step 5: Write the failing test** in `landing/tests/unit/app.test.tsx`

```tsx
it("renders only the Margincite sections, in order", () => {
	const { container } = render(<App />);
	const ids = [...container.querySelectorAll("main > section")].map((s) => s.id);
	expect(ids).toEqual(["top", "features", "how", "product", "early-access"]);
});

it("keeps the skip link", () => {
	render(<App />);
	expect(screen.getByRole("link", { name: "Skip to content" })).toHaveAttribute("href", "#top");
});
```

- [ ] **Step 6: Run it and see it fail**

Run: `pnpm --dir landing exec vitest run tests/unit/app.test.tsx --maxWorkers=2`
Expected: FAIL, the id list still contains the reference's extra sections.

- [ ] **Step 7: Trim `App.tsx`**

Remove the five deleted sections from `App.tsx` and delete their files. Change the `Cta` section id from `get-started` to `early-access`.

- [ ] **Step 8: Run the test and the type check**

Run: `pnpm --dir landing exec vitest run tests/unit/app.test.tsx --maxWorkers=2 && pnpm --dir landing exec tsc --noEmit`
Expected: 2 tests pass, no type errors.

- [ ] **Step 9: Commit**

```bash
git add .gitignore landing
git commit -m "chore: start landing page from the handcraft paper reference"
```

---

### Task 2: Margincite copy, and the nav, logo, hero and footer

**Files:**
- Create: `landing/src/content.ts`, `landing/src/components/SuggestionSketch.tsx`
- Delete: `landing/src/components/BoardSketch.tsx`
- Modify: `landing/src/components/Logo.tsx`, `Nav.tsx`, `Hero.tsx`, `Footer.tsx`, `primitives.tsx` (remove `AvatarStack`), `landing/index.html` (set the title to `Margincite` and delete the reference's description tag; Task 6 replaces both)
- Test: `landing/tests/unit/content.test.ts`, `landing/tests/unit/chrome.test.tsx`

**Interfaces:**
- Produces: `content` exported from `landing/src/content.ts`, declared `as const`. Later tasks read `content.features`, `content.how`, `content.product`, `content.cta` and `content.seo`.
- Produces: `SuggestionSketch({ className }: { className?: string })`, an inline SVG with `role="img"` and `aria-label="Sketch of a Margincite suggestions list"`.

- [ ] **Step 1: Write `landing/src/content.ts`**

This is the exact copy. Components contain no visible text of their own.

```ts
const sectionLinks = [
	{ label: "Features", href: "#features" },
	{ label: "How it works", href: "#how" },
	{ label: "A closer look", href: "#product" },
] as const;

const earlyAccess = { label: "Request early access", href: "#early-access" } as const;

export const content = {
	brand: "Margincite",
	nav: { links: sectionLinks, cta: earlyAccess },
	hero: {
		tag: "SEO and AI search, one list",
		titleLead: "Notes in the",
		titleAccent: "margin",
		titleTail: "for your website",
		body: "Margincite reads every page of your site and hands back a ranked list of fixes: what to change so Google, ChatGPT, Claude and Perplexity can find, read and cite you.",
		primary: earlyAccess,
		secondary: { label: "See how it works", href: "#how" },
		arrowNote: "start here",
		sketchCaption: "example.com: 17 notes",
		sketchNotes: [
			"AI crawlers blocked in robots.txt",
			"38 pages have no description",
			"12 pages rank 5 to 20 on Google",
		],
		badge: "Example report",
		status: "Early access. Desktop app for Mac.",
	},
	features: {
		heading: "What it does today",
		intro: "Four things, all in the early-access build.",
		items: [
			{ title: "Reads the whole site", body: "Follows your sitemap and links page by page, including pages that only show their content after JavaScript runs." },
			{ title: "Ranks what to fix", body: "Every note shows the pages affected, the evidence and the fix. The biggest wins sit at the top." },
			{ title: "Uses your real search data", body: "Connect Google Search Console and the ranking follows your actual queries, clicks and impressions." },
			{ title: "Works inside Claude", body: "Add Margincite to Claude as a connector, then ask it to audit your site or walk you through the list." },
		],
	},
	how: {
		heading: "How it works",
		steps: [
			{ title: "Add your site", body: "Paste the address. Margincite crawls it politely and follows your robots.txt." },
			{ title: "Read the notes", body: "Get a ranked list covering technical SEO, AI readiness and content." },
			{ title: "Fix and re-check", body: "Mark notes done. Margincite re-crawls every week and confirms each fix." },
		],
	},
	product: {
		heading: "One note, up close",
		intro: "An example of what a single suggestion looks like.",
		tag: "Example",
		note: {
			title: "AI crawlers are blocked in robots.txt",
			parts: [
				{ label: "What we found", body: "robots.txt disallows GPTBot, ClaudeBot and PerplexityBot on every page." },
				{ label: "Why it matters", body: "These crawlers cannot read the site, so AI answers have nothing of yours to cite." },
				{ label: "How to fix it", body: "Remove the three Disallow rules, or allow the sections you want read." },
			],
		},
		honesty: "Margincite measures and reports. It does not promise rankings or citations.",
		coming: {
			heading: "Coming later",
			items: [
				"Drafting the fix for you, published only after you approve it",
				"Tracking which AI answer engines cite your site",
			],
		},
	},
	cta: {
		heading: "Want the notes for your site?",
		body: "Margincite is in early access. Leave your email and we will write when there is a seat for you.",
		emailLabel: "Email address",
		placeholder: "you@yourcompany.com",
		button: "Request early access",
		sending: "Sending…",
		success: "Thanks. You are on the list.",
		errors: {
			invalid_email: "That email address does not look right.",
			not_configured: "Sign-up is not connected yet. Please try again later.",
			network: "We could not reach the sign-up service. Please try again.",
			rejected: "The sign-up service did not accept that. Please try again.",
		},
		footnote: "We only use your email to contact you about early access.",
	},
	footer: { blurb: "SEO and AI search suggestions for your website.", links: sectionLinks },
	seo: {
		title: "Margincite: SEO and AI search suggestions for your website",
		description: "Margincite crawls your whole site and returns a ranked list of fixes for Google and for AI answers like ChatGPT, Claude and Perplexity. Now in early access.",
	},
} as const;

export type ErrorReason = keyof typeof content.cta.errors;
```

- [ ] **Step 2: Write the failing tests**

`landing/tests/unit/content.test.ts`:

```ts
const text = JSON.stringify(content);

it("has none of the reference's fictional content", () => {
	expect(text).not.toMatch(/scribbly/i);
});

it("makes no invented number claims", () => {
	expect(text).not.toMatch(/\d[\d,]*\+/);
	expect(text).not.toMatch(/trusted by|customers love|#1/i);
});

it("links only to sections that exist", () => {
	const ids = ["#top", "#features", "#how", "#product", "#early-access"];
	const hrefs = [...content.nav.links, content.nav.cta, content.hero.primary, content.hero.secondary].map((l) => l.href);
	for (const href of hrefs) expect(ids).toContain(href);
});
```

`landing/tests/unit/chrome.test.tsx`:

```tsx
it("shows the Margincite name in the logo", () => {
	render(<Logo />);
	expect(screen.getByText("Margincite")).toBeInTheDocument();
});

it("renders the nav links and call to action from content", () => {
	render(<Nav />);
	for (const link of content.nav.links) {
		expect(screen.getAllByRole("link", { name: link.label })[0]).toHaveAttribute("href", link.href);
	}
	expect(screen.getAllByRole("link", { name: "Request early access" })[0]).toHaveAttribute("href", "#early-access");
});

it("opens and closes the mobile menu", async () => {
	render(<Nav />);
	const button = screen.getByRole("button", { name: "Open menu" });
	await userEvent.click(button);
	expect(screen.getByRole("button", { name: "Close menu" })).toHaveAttribute("aria-expanded", "true");
});

it("renders the hero headline and both buttons", () => {
	render(<Hero />);
	expect(screen.getByRole("heading", { level: 1 }).textContent?.replace(/\s+/g, " ").trim()).toBe("Notes in the margin for your website");
	expect(screen.getByRole("link", { name: /Request early access/ })).toHaveAttribute("href", "#early-access");
	expect(screen.getByRole("link", { name: "See how it works" })).toHaveAttribute("href", "#how");
	expect(screen.getByText("Early access. Desktop app for Mac.")).toBeInTheDocument();
});

it("draws the suggestions sketch with an accessible name", () => {
	render(<SuggestionSketch />);
	expect(screen.getByRole("img", { name: "Sketch of a Margincite suggestions list" })).toBeInTheDocument();
});

it("renders the footer without the removed sections", () => {
	const { container } = render(<Footer />);
	expect(screen.getByText("SEO and AI search suggestions for your website.")).toBeInTheDocument();
	expect(container.querySelector('a[href="#pricing"], a[href="#notes"]')).toBeNull();
});

it("leaves no reference text anywhere on the page", () => {
	const { container } = render(<App />);
	expect(container.textContent).not.toMatch(/scribbly|12,400|brainstorm/i);
});
```

- [ ] **Step 3: Run them and see them fail**

Run: `pnpm --dir landing exec vitest run tests/unit/content.test.ts tests/unit/chrome.test.tsx --maxWorkers=2`
Expected: the content tests pass, the component tests fail on "Scribbly" text and missing links.

- [ ] **Step 4: Rewrite the four components against `content`**

- `Logo.tsx`: keep the reference mark, change the word to `content.brand`.
- `Nav.tsx`: links and button from `content.nav`. The home link's label is `Margincite home`.
- `Hero.tsx`: text from `content.hero`. The headline keeps the reference's underlined accent word, now `titleAccent`. Remove the avatar row and its user count and show `content.hero.status` there. Replace `BoardSketch` with `SuggestionSketch`, captioned `sketchCaption`, with the corner badge showing `badge`.
- `SuggestionSketch.tsx`: an inline SVG in the reference's sketch style showing the three `sketchNotes` as ranked rows numbered 1 to 3, plus one sticky note. Colours come from the theme variables.
- `Footer.tsx`: `content.footer.blurb`, the three section links, and `© Margincite`. Remove every other link column.
- `primitives.tsx`: delete `AvatarStack`.

- [ ] **Step 5: Run the tests and the type check**

Run: `pnpm --dir landing exec vitest run tests/unit --maxWorkers=2 && pnpm --dir landing exec tsc --noEmit`
Expected: all pass except "leaves no reference text anywhere on the page", which still fails on the three sections Task 3 rewrites. Mark that one test `it.fails` for now, with a comment naming Task 3.

- [ ] **Step 6: Commit**

```bash
git add landing
git commit -m "feat: add Margincite copy, nav, hero and footer"
```

---

### Task 3: Features, how it works, and the close-up of one suggestion

**Files:**
- Modify: `landing/src/components/Features.tsx`, `HowItWorks.tsx`, `ProductDetail.tsx`, `landing/tests/unit/chrome.test.tsx` (remove `it.fails`)
- Test: `landing/tests/unit/sections.test.tsx`

**Interfaces:**
- Consumes: `content.features`, `content.how`, `content.product` from Task 2.

- [ ] **Step 1: Write the failing tests** in `landing/tests/unit/sections.test.tsx`

```tsx
it("lists the four features", () => {
	render(<Features />);
	expect(screen.getByRole("heading", { level: 2, name: "What it does today" })).toBeInTheDocument();
	for (const item of content.features.items) {
		expect(screen.getByRole("heading", { level: 3, name: item.title })).toBeInTheDocument();
		expect(screen.getByText(item.body)).toBeInTheDocument();
	}
});

it("shows the three steps as an ordered list, in order", () => {
	render(<HowItWorks />);
	const items = within(screen.getByRole("list")).getAllByRole("listitem");
	expect(items.map((li) => within(li).getByRole("heading").textContent)).toEqual(["Add your site", "Read the notes", "Fix and re-check"]);
});

it("shows one example suggestion with its three parts", () => {
	render(<ProductDetail />);
	expect(screen.getByText("Example")).toBeInTheDocument();
	expect(screen.getByRole("heading", { level: 3, name: "AI crawlers are blocked in robots.txt" })).toBeInTheDocument();
	for (const part of content.product.note.parts) {
		expect(screen.getByText(part.label)).toBeInTheDocument();
		expect(screen.getByText(part.body)).toBeInTheDocument();
	}
});

it("states the honesty line and what is coming later", () => {
	render(<ProductDetail />);
	expect(screen.getByText("Margincite measures and reports. It does not promise rankings or citations.")).toBeInTheDocument();
	const coming = screen.getByRole("list", { name: "Coming later" });
	expect(within(coming).getAllByRole("listitem")).toHaveLength(2);
});
```

- [ ] **Step 2: Run them and see them fail**

Run: `pnpm --dir landing exec vitest run tests/unit/sections.test.tsx --maxWorkers=2`
Expected: FAIL, the sections still show the reference's text.

- [ ] **Step 3: Rewrite the three sections against `content`**

- `Features.tsx`: section id `features`, four cards. Pick one lucide icon per card that exists in the installed version; icons are decorative and carry `aria-hidden`.
- `HowItWorks.tsx`: section id `how`, an `<ol>` with three steps.
- `ProductDetail.tsx`: section id `product`. One note card with the `tag`, the note title as an `h3`, and the three labelled parts as a description list. Below it the `honesty` line, then the coming-later list as a `<ul aria-label="Coming later">`.

Keep each section's existing reveal wiring. Task 5 fixes its behaviour.

- [ ] **Step 4: Run all unit tests**

Remove `it.fails` from "leaves no reference text anywhere on the page".

Run: `pnpm --dir landing exec vitest run tests/unit --maxWorkers=2 && pnpm --dir landing exec tsc --noEmit`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add landing
git commit -m "feat: add features, how it works and example suggestion sections"
```

---

### Task 4: Early-access form

**Files:**
- Create: `landing/src/lib/earlyAccess.ts`
- Modify: `landing/src/components/Cta.tsx`, `landing/vite-env.d.ts` (declare `VITE_EARLY_ACCESS_ENDPOINT?: string`)
- Test: `landing/tests/unit/earlyAccess.test.ts`, `landing/tests/unit/cta.test.tsx`

**Interfaces:**
- Consumes: `content.cta` and `ErrorReason` from Task 2.
- Produces, in `earlyAccess.ts`:
  - `isValidEmail(raw: string): boolean`
  - `type SubmitResult = { ok: true } | { ok: false; reason: ErrorReason }`
  - `submitEarlyAccess(input: { email: string; honeypot: string }, deps: { endpoint: string | undefined; fetchFn: typeof fetch }): Promise<SubmitResult>`
- Produces: `Cta({ endpoint, fetchFn }: { endpoint?: string; fetchFn?: typeof fetch })`. The defaults are `import.meta.env.VITE_EARLY_ACCESS_ENDPOINT` and the global `fetch`.

The endpoint is any form service that accepts a JSON `POST` of `{ "email": "…" }` with `Accept: application/json` and answers with a 2xx status, which is how Formspree behaves.

- [ ] **Step 1: Write the failing tests for the library** in `landing/tests/unit/earlyAccess.test.ts`

```ts
const endpoint = "https://forms.example.test/f/abc";
const okFetch = () => vi.fn().mockResolvedValue(new Response("{}", { status: 200 }));

it.each(["a@b.co", "  First.Last@Example.com  "])("accepts %s", (email) => {
	expect(isValidEmail(email)).toBe(true);
});

it.each(["", "a@b", "a b@c.com", "@c.com", "a@", `${"x".repeat(250)}@c.com`])("rejects %s", (email) => {
	expect(isValidEmail(email)).toBe(false);
});

it("posts the trimmed email as JSON", async () => {
	const fetchFn = okFetch();
	const result = await submitEarlyAccess({ email: "  a@b.co ", honeypot: "" }, { endpoint, fetchFn });
	expect(result).toEqual({ ok: true });
	const [url, init] = fetchFn.mock.calls[0];
	expect(url).toBe(endpoint);
	expect(init.method).toBe("POST");
	expect(JSON.parse(init.body)).toEqual({ email: "a@b.co" });
	expect(init.headers).toMatchObject({ Accept: "application/json", "Content-Type": "application/json" });
});

it("reports an invalid email without calling the service", async () => {
	const fetchFn = okFetch();
	expect(await submitEarlyAccess({ email: "nope", honeypot: "" }, { endpoint, fetchFn })).toEqual({ ok: false, reason: "invalid_email" });
	expect(fetchFn).not.toHaveBeenCalled();
});

it("reports not_configured when there is no endpoint", async () => {
	expect(await submitEarlyAccess({ email: "a@b.co", honeypot: "" }, { endpoint: undefined, fetchFn: okFetch() })).toEqual({ ok: false, reason: "not_configured" });
});

it("reports not_configured for an endpoint that is not https", async () => {
	expect(await submitEarlyAccess({ email: "a@b.co", honeypot: "" }, { endpoint: "http://x.test", fetchFn: okFetch() })).toEqual({ ok: false, reason: "not_configured" });
});

it("pretends to succeed and sends nothing when the honeypot is filled", async () => {
	const fetchFn = okFetch();
	expect(await submitEarlyAccess({ email: "a@b.co", honeypot: "Acme" }, { endpoint, fetchFn })).toEqual({ ok: true });
	expect(fetchFn).not.toHaveBeenCalled();
});

it("reports rejected on a non-2xx answer", async () => {
	const fetchFn = vi.fn().mockResolvedValue(new Response("{}", { status: 422 }));
	expect(await submitEarlyAccess({ email: "a@b.co", honeypot: "" }, { endpoint, fetchFn })).toEqual({ ok: false, reason: "rejected" });
});

it("reports network when the request throws", async () => {
	const fetchFn = vi.fn().mockRejectedValue(new TypeError("failed to fetch"));
	expect(await submitEarlyAccess({ email: "a@b.co", honeypot: "" }, { endpoint, fetchFn })).toEqual({ ok: false, reason: "network" });
});
```

- [ ] **Step 2: Run them and see them fail**

Run: `pnpm --dir landing exec vitest run tests/unit/earlyAccess.test.ts --maxWorkers=2`
Expected: FAIL, module not found.

- [ ] **Step 3: Implement `earlyAccess.ts`**

`isValidEmail` trims, requires a length of at most 254, and matches `^[^\s@]+@[^\s@]+\.[^\s@]+$`. `submitEarlyAccess` checks in this order: honeypot, email, endpoint, then sends. The request is aborted after 10 seconds, which counts as `network`. Failures are logged with `console.error` and never thrown.

- [ ] **Step 4: Run them and see them pass**

Run: `pnpm --dir landing exec vitest run tests/unit/earlyAccess.test.ts --maxWorkers=2`
Expected: PASS.

- [ ] **Step 5: Write the failing tests for the form** in `landing/tests/unit/cta.test.tsx`

```tsx
const endpoint = "https://forms.example.test/f/abc";
const email = () => screen.getByLabelText("Email address");
const submit = () => screen.getByRole("button", { name: "Request early access" });

it("is a real form that works without JavaScript", () => {
	const { container } = render(<Cta endpoint={endpoint} />);
	const form = container.querySelector("form")!;
	expect(form).toHaveAttribute("action", endpoint);
	expect(form).toHaveAttribute("method", "post");
	expect(email()).toHaveAttribute("name", "email");
});

it("confirms after a successful request", async () => {
	const fetchFn = vi.fn().mockResolvedValue(new Response("{}", { status: 200 }));
	render(<Cta endpoint={endpoint} fetchFn={fetchFn} />);
	await userEvent.type(email(), "a@b.co");
	await userEvent.click(submit());
	expect(await screen.findByRole("status")).toHaveTextContent("Thanks. You are on the list.");
});

it("shows an error and keeps the typed email when the service is unreachable", async () => {
	const fetchFn = vi.fn().mockRejectedValue(new TypeError("failed to fetch"));
	render(<Cta endpoint={endpoint} fetchFn={fetchFn} />);
	await userEvent.type(email(), "a@b.co");
	await userEvent.click(submit());
	expect(await screen.findByRole("alert")).toHaveTextContent("We could not reach the sign-up service. Please try again.");
	expect(email()).toHaveValue("a@b.co");
});

it("shows the invalid-email message and sends nothing", async () => {
	const fetchFn = vi.fn();
	render(<Cta endpoint={endpoint} fetchFn={fetchFn} />);
	await userEvent.type(email(), "nope");
	await userEvent.click(submit());
	expect(await screen.findByRole("alert")).toHaveTextContent("That email address does not look right.");
	expect(fetchFn).not.toHaveBeenCalled();
});

it("sends one request when the button is clicked twice", async () => {
	let release!: (r: Response) => void;
	const fetchFn = vi.fn().mockReturnValue(new Promise<Response>((r) => { release = r; }));
	render(<Cta endpoint={endpoint} fetchFn={fetchFn} />);
	await userEvent.type(email(), "a@b.co");
	await userEvent.click(submit());
	expect(screen.getByRole("button", { name: "Sending…" })).toBeDisabled();
	await userEvent.click(screen.getByRole("button", { name: "Sending…" }));
	release(new Response("{}", { status: 200 }));
	await screen.findByRole("status");
	expect(fetchFn).toHaveBeenCalledTimes(1);
});

it("hides the honeypot field from people", () => {
	const { container } = render(<Cta endpoint={endpoint} />);
	const trap = container.querySelector('input[name="company"]')!;
	expect(trap).toHaveAttribute("tabindex", "-1");
	expect(trap).toHaveAttribute("autocomplete", "off");
	expect(trap.closest("[aria-hidden='true']")).not.toBeNull();
});
```

- [ ] **Step 6: Run them and see them fail**

Run: `pnpm --dir landing exec vitest run tests/unit/cta.test.tsx --maxWorkers=2`
Expected: FAIL.

- [ ] **Step 7: Rewrite `Cta.tsx`**

Section id `early-access`. Text from `content.cta`. The form has `action={endpoint}` and `method="post"`, an email input named `email`, and a honeypot input named `company` inside a visually hidden wrapper marked `aria-hidden="true"`. On submit it prevents the default, calls `submitEarlyAccess`, and moves through four states: idle, sending, sent, error. While sending, the button is disabled and reads `content.cta.sending`. Success shows `content.cta.success` in an element with `role="status"`. An error shows `content.cta.errors[reason]` in an element with `role="alert"` and leaves the input value untouched. Show `content.cta.footnote` under the form. Remove the reference's "Free forever" line.

- [ ] **Step 8: Run all unit tests**

Run: `pnpm --dir landing exec vitest run tests/unit --maxWorkers=2 && pnpm --dir landing exec tsc --noEmit`
Expected: all pass.

- [ ] **Step 9: Commit**

```bash
git add landing
git commit -m "feat: add early-access form with validation and error states"
```

---

### Task 5: Scroll reveal that never hides server-rendered content

The reference starts every revealed block at `opacity: 0` and shows it from JavaScript. Rendered to static HTML, that leaves three sections invisible when JavaScript does not run.

**Files:**
- Modify: `landing/src/lib/reveal.ts`
- Test: `landing/tests/unit/reveal.test.tsx`

**Interfaces:**
- Produces: `useReveal<T extends HTMLElement = HTMLDivElement>(rootMargin?: string): { ref: RefObject<T | null>; shown: boolean }`. The signature is unchanged from the reference, so the three sections need no edits.

New behaviour: `shown` is `true` on the server and on the first browser render. After mount, an element whose top edge is below the bottom of the viewport is set to hidden and then shown when it scrolls into view. Elements already on screen are never hidden. With reduced motion requested, or with no `IntersectionObserver` available, nothing is ever hidden.

- [ ] **Step 1: Write the failing tests** in `landing/tests/unit/reveal.test.tsx`

Use a small `Probe` component that renders `<div ref={ref} data-testid="probe" style={{ opacity: shown ? 1 : 0 }} />`. Stub `IntersectionObserver` with a class that records its callback so the test can fire it, and stub `Element.prototype.getBoundingClientRect` to set the element's `top`.

```tsx
it("is shown in server-rendered HTML", () => {
	expect(renderToString(<Probe />)).not.toMatch(/opacity:\s*0/);
});

it("hides an element below the fold after mount, then shows it on scroll", () => {
	stubRectTop(window.innerHeight + 200);
	render(<Probe />);
	expect(screen.getByTestId("probe")).toHaveStyle({ opacity: "0" });
	act(() => fireIntersection(true));
	expect(screen.getByTestId("probe")).toHaveStyle({ opacity: "1" });
});

it("never hides an element that is already on screen", () => {
	stubRectTop(100);
	render(<Probe />);
	expect(screen.getByTestId("probe")).toHaveStyle({ opacity: "1" });
});

it("never hides anything when reduced motion is requested", () => {
	stubMatchMedia(true);
	stubRectTop(window.innerHeight + 200);
	render(<Probe />);
	expect(screen.getByTestId("probe")).toHaveStyle({ opacity: "1" });
});

it("never hides anything when IntersectionObserver is missing", () => {
	vi.stubGlobal("IntersectionObserver", undefined);
	stubRectTop(window.innerHeight + 200);
	render(<Probe />);
	expect(screen.getByTestId("probe")).toHaveStyle({ opacity: "1" });
});
```

- [ ] **Step 2: Run them and see them fail**

Run: `pnpm --dir landing exec vitest run tests/unit/reveal.test.tsx --maxWorkers=2`
Expected: FAIL on "is shown in server-rendered HTML" and "never hides an element that is already on screen".

- [ ] **Step 3: Change `useReveal`** to the behaviour described above.

- [ ] **Step 4: Run all unit tests with coverage**

Run: `pnpm --dir landing exec vitest run tests/unit --maxWorkers=2 --coverage`
Expected: all pass, and every coverage figure is 80 or higher. If a figure is lower, add tests for the uncovered lines before continuing.

- [ ] **Step 5: Commit**

```bash
git add landing
git commit -m "fix: keep revealed sections visible without JavaScript"
```

---

### Task 6: Static HTML build, search metadata and fonts

**Files:**
- Create: `landing/src/seo.ts`, `landing/src/entry-server.tsx`, `landing/src/entry-client.tsx`, `landing/scripts/prerender.mjs`
- Delete: `landing/src/main.tsx`
- Modify: `landing/index.html`, `landing/package.json`, `landing/src/index.css`
- Test: `landing/tests/unit/seo.test.ts`, `landing/tests/build/dist.test.ts`

**Interfaces:**
- Consumes: `App` from Task 1, `content.seo` and `content.brand` from Task 2.
- Produces, in `seo.ts`:
  - `AI_CRAWLERS: readonly string[]` with exactly `GPTBot`, `OAI-SearchBot`, `ChatGPT-User`, `ClaudeBot`, `Claude-SearchBot`, `Claude-User`, `PerplexityBot`, `Google-Extended`, `Bingbot`
  - `normalizeSiteUrl(raw: string): string`, which trims trailing slashes and throws `Error("SITE_URL must start with http:// or https://")` otherwise
  - `buildJsonLd(siteUrl: string): object[]`
  - `buildHead(siteUrl: string): string`
  - `buildRobots(siteUrl: string): string`
  - `buildSitemap(siteUrl: string, lastmod: string): string`
- Produces, in `entry-server.tsx`: `render(): string`, plus re-exports of everything in `seo.ts` so the prerender script has one import.
- Produces: script `build` in `landing/package.json`, which leaves the finished site in `landing/dist`.

- [ ] **Step 1: Write the failing tests for the metadata** in `landing/tests/unit/seo.test.ts`

```ts
const site = "https://margincite.com";

it("trims trailing slashes from the site address", () => {
	expect(normalizeSiteUrl("https://margincite.com/")).toBe(site);
	expect(normalizeSiteUrl("https://margincite.com///")).toBe(site);
});

it.each(["margincite.com", "ftp://margincite.com", ""])("refuses %s", (raw) => {
	expect(() => normalizeSiteUrl(raw)).toThrow("SITE_URL must start with http:// or https://");
});

it("builds the head tags", () => {
	const head = buildHead(site);
	expect(head).toContain(`<title>${content.seo.title}</title>`);
	expect(head).toContain(`<meta name="description" content="${content.seo.description}">`);
	expect(head).toContain(`<link rel="canonical" href="https://margincite.com/">`);
	expect(head).toContain(`<meta property="og:url" content="https://margincite.com/">`);
	expect(head).toContain(`<meta property="og:title" content="${content.seo.title}">`);
	expect(head).toContain(`<meta name="twitter:card" content="summary">`);
	expect(head).not.toContain("margincite.com//");
});

it("describes the organisation, site and app in structured data", () => {
	const types = buildJsonLd(site).map((entry) => (entry as { "@type": string })["@type"]);
	expect(types).toEqual(["Organization", "WebSite", "SoftwareApplication"]);
});

it("invents no ratings, reviews or prices", () => {
	expect(JSON.stringify(buildJsonLd(site))).not.toMatch(/aggregateRating|review|offers|price/i);
});

it("embeds structured data that parses and cannot close its script tag", () => {
	const json = buildHead(site).match(/<script type="application\/ld\+json">(.*?)<\/script>/s)![1];
	expect(JSON.parse(json)).toHaveLength(3);
	expect(json).not.toContain("<");
});

it("lets every search and AI crawler in", () => {
	const robots = buildRobots(site);
	expect(robots).toContain("User-agent: *\nAllow: /");
	for (const bot of AI_CRAWLERS) expect(robots).toContain(`User-agent: ${bot}\nAllow: /`);
	expect(robots).not.toContain("Disallow");
	expect(robots).toContain("Sitemap: https://margincite.com/sitemap.xml");
});

it("lists the one page in the sitemap", () => {
	const xml = buildSitemap(site, "2026-11-13");
	expect(xml).toContain("<loc>https://margincite.com/</loc>");
	expect(xml).toContain("<lastmod>2026-11-13</lastmod>");
});
```

- [ ] **Step 2: Run them and see them fail**

Run: `pnpm --dir landing exec vitest run tests/unit/seo.test.ts --maxWorkers=2`
Expected: FAIL, module not found.

- [ ] **Step 3: Implement `seo.ts`**

`buildJsonLd` returns three entries. `Organization`: name and url. `WebSite`: name, url and description. `SoftwareApplication`: name, description, `applicationCategory` of `BusinessApplication`, `operatingSystem` of `macOS`, and url. `buildHead` serialises them into one `<script type="application/ld+json">`, with every `<` written as `<`. Attribute values are HTML-escaped.

- [ ] **Step 4: Run them and see them pass**

Run: `pnpm --dir landing exec vitest run tests/unit/seo.test.ts --maxWorkers=2`
Expected: PASS.

- [ ] **Step 5: Write the failing test for the built site** in `landing/tests/build/dist.test.ts`

Start the file with `// @vitest-environment node`. Read `landing/dist` relative to the test file.

```ts
const html = readFileSync(join(dist, "index.html"), "utf8");
const root = html.slice(html.indexOf('<div id="root">'));

it("has every section's text in the HTML itself", () => {
	for (const text of [
		"margin",
		content.hero.body,
		content.features.heading,
		content.features.items[3].body,
		content.how.steps[2].title,
		content.product.note.title,
		content.product.honesty,
		content.cta.heading,
	]) expect(root).toContain(text);
});

it("hides nothing and leaves no template markers", () => {
	expect(html).not.toMatch(/opacity:\s*0[;"]/);
	expect(html).not.toContain("<!--app-head-->");
	expect(html).not.toContain("<!--app-html-->");
});

it("has exactly one title and one canonical link", () => {
	expect(html.match(/<title>/g)).toHaveLength(1);
	expect(html.match(/rel="canonical"/g)).toHaveLength(1);
});

it("serves its own fonts", () => {
	const css = readdirSync(join(dist, "assets")).filter((f) => f.endsWith(".css")).map((f) => readFileSync(join(dist, "assets", f), "utf8")).join("");
	expect(css).toMatch(/@font-face[^}]*Kalam/);
	expect(css).toMatch(/@font-face[^}]*Patrick Hand/);
	expect(html + css).not.toContain("fonts.googleapis.com");
	expect(readdirSync(join(dist, "assets")).some((f) => f.endsWith(".woff2"))).toBe(true);
});

it("writes robots.txt and sitemap.xml", () => {
	expect(readFileSync(join(dist, "robots.txt"), "utf8")).toContain("Sitemap: ");
	expect(readFileSync(join(dist, "sitemap.xml"), "utf8")).toContain("<loc>");
});
```

- [ ] **Step 6: Build the pipeline**

- `index.html`: remove the title and description tags, put `<!--app-head-->` in the head and `<div id="root"><!--app-html--></div>` in the body, and point the script at `/src/entry-client.tsx`.
- `entry-client.tsx`: imports `index.css` and calls `hydrateRoot` on `#root` with `<StrictMode><App /></StrictMode>`.
- `entry-server.tsx`: `render()` returns `renderToString(<StrictMode><App /></StrictMode>)`.
- Fonts: add `@fontsource/kalam` and `@fontsource/patrick-hand`, and import the Kalam 400 and 700 and Patrick Hand 400 stylesheets from `entry-client.tsx`.
- `scripts/prerender.mjs`: reads `SITE_URL` (default `https://margincite.com`), normalises it, imports `dist-ssr/entry-server.js`, replaces the two markers in `dist/index.html`, writes `dist/robots.txt` and `dist/sitemap.xml` with today's date as `lastmod`, then deletes `dist-ssr`. Any failure exits with a non-zero status and the error message.
- `package.json` scripts:
  - `build`: `tsc --noEmit && vite build && vite build --ssr src/entry-server.tsx --outDir dist-ssr && node scripts/prerender.mjs`
  - `test:build`: `vitest run tests/build --maxWorkers=2`
  - `preview`: `vite preview --port 4173 --strictPort`

- [ ] **Step 7: Build, then run the built-site test**

Run: `pnpm --dir landing build && pnpm --dir landing test:build`
Expected: the build finishes with no errors or hydration warnings, and 5 tests pass.

- [ ] **Step 8: Check the bad-address path by hand**

Run: `SITE_URL=margincite.com pnpm --dir landing build; echo "exit $?"`
Expected: the message `SITE_URL must start with http:// or https://` and `exit 1`.

- [ ] **Step 9: Commit**

```bash
git add landing
git commit -m "feat: pre-build the landing page to static HTML with search metadata"
```

---

### Task 7: Browser tests and the README

**Files:**
- Create: `landing/playwright.config.ts`, `landing/tests/e2e/landing.spec.ts`, `landing/README.md`
- Modify: `landing/package.json` (script `test:e2e`: `playwright test`)

**Interfaces:**
- Consumes: the `build` and `preview` scripts from Task 6.

`playwright.config.ts`: one Chromium project, one worker, base address `http://localhost:4173`, and a web server started with `VITE_EARLY_ACCESS_ENDPOINT=https://forms.example.test/f/abc pnpm build && pnpm preview`.

- [ ] **Step 1: Install the browser**

Run: `pnpm --dir landing exec playwright install chromium`

- [ ] **Step 2: Write the tests** in `landing/tests/e2e/landing.spec.ts`

```ts
const endpoint = "https://forms.example.test/f/abc";
const headings = ["What it does today", "How it works", "One note, up close", "Want the notes for your site?"];

test.describe("with JavaScript off", () => {
	test.use({ javaScriptEnabled: false });

	test("every section is readable", async ({ page }) => {
		await page.goto("/");
		await expect(page.getByRole("heading", { level: 1 })).toContainText("Notes in the");
		for (const name of headings) {
			await expect(page.getByRole("heading", { level: 2, name })).toBeVisible();
		}
		expect(await page.content()).not.toMatch(/opacity:\s*0[;"]/);
	});

	test("the form still posts to the service", async ({ page }) => {
		await page.goto("/");
		const form = page.locator("#early-access form");
		await expect(form).toHaveAttribute("action", endpoint);
		await expect(form).toHaveAttribute("method", "post");
	});
});

test("requesting early access confirms", async ({ page }) => {
	await page.route(endpoint, (route) => route.fulfill({ status: 200, contentType: "application/json", body: "{}" }));
	await page.goto("/");
	await page.getByLabel("Email address").fill("a@b.co");
	await page.getByRole("button", { name: "Request early access" }).click();
	await expect(page.getByRole("status")).toHaveText(/Thanks\. You are on the list\./);
});

test("an unreachable service shows an error and keeps the email", async ({ page }) => {
	await page.route(endpoint, (route) => route.abort());
	await page.goto("/");
	await page.getByLabel("Email address").fill("a@b.co");
	await page.getByRole("button", { name: "Request early access" }).click();
	await expect(page.getByRole("alert")).toHaveText(/We could not reach the sign-up service/);
	await expect(page.getByLabel("Email address")).toHaveValue("a@b.co");
});

test("a 320 pixel wide phone does not scroll sideways", async ({ page }) => {
	await page.setViewportSize({ width: 320, height: 640 });
	await page.goto("/");
	expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(320);
});

test("the mobile menu opens and leads to a section", async ({ page }) => {
	await page.setViewportSize({ width: 390, height: 800 });
	await page.goto("/");
	await page.getByRole("button", { name: "Open menu" }).click();
	await page.getByRole("link", { name: "How it works" }).last().click();
	await expect(page).toHaveURL(/#how$/);
});

test("the page loads with no console errors", async ({ page }) => {
	const errors: string[] = [];
	page.on("console", (message) => { if (message.type() === "error") errors.push(message.text()); });
	page.on("pageerror", (error) => errors.push(error.message));
	await page.goto("/");
	await page.waitForLoadState("networkidle");
	expect(errors).toEqual([]);
});
```

- [ ] **Step 3: Run them**

Run: `pnpm --dir landing test:e2e`
Expected: 7 tests pass. A failure here is a defect in an earlier task's code. Fix the code, not the test, and add a unit test for the defect in the owning task's test file.

- [ ] **Step 4: Write `landing/README.md`**

It covers: what the folder is, the commands (`dev`, `build`, `preview`, `test`, `test:build`, `test:e2e`), the two environment variables (`SITE_URL` and `VITE_EARLY_ACCESS_ENDPOINT`, with the shape of request the endpoint must accept), the PATH line needed on this machine, where the copy lives (`src/content.ts`), and the rule that the page is not deployed publicly before the phase 1 app exists.

- [ ] **Step 5: Run everything once**

Run: `pnpm --dir landing exec vitest run tests/unit --maxWorkers=2 --coverage && pnpm --dir landing build && pnpm --dir landing test:build && pnpm --dir landing test:e2e`
Expected: every suite passes and coverage is 80 or higher on all four figures.

- [ ] **Step 6: Commit**

```bash
git add landing
git commit -m "test: add browser tests and README for the landing page"
```

---

## What the owner must supply before this goes public

- The domain, set as `SITE_URL`.
- A form-service endpoint, set as `VITE_EARLY_ACCESS_ENDPOINT`. Until then the form reports "Sign-up is not connected yet."
- A static host.
