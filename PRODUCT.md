# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Primary: in-house SEO and content marketers at a company. They already use tools such as Ahrefs or Semrush and Google Search Console, and they are now being asked how the company shows up in AI answers. Their job is to find what to fix on the company site, in what order, and to justify that order to others. Confirmed by the owner on 2026-11-13.

The owner's own team is the first user. Outside customers come later.

## Product Purpose

Margincite reads every page of a website and returns a ranked list of suggestions for classic search (SEO) and for AI answer engines (GEO). Each suggestion shows the pages affected, the evidence and the fix. Later phases draft the fix for a person to approve, publish it as a draft, and track which AI engines cite the site.

Success for the first release: the owner runs it on their own site, agrees the suggestions are accurate and useful, and can drive it from Claude.

## Positioning

One ranked list that covers both Google and AI answers, with evidence on every item, used from inside Claude as a connector. Existing tools either track AI visibility or auto-write posts. Few do both, and working inside Claude is uncommon.

Margincite measures and reports. It does not promise rankings or citations.

## Operating Context

- A web app and a Claude connector, both backed by one hosted Rust service. The web app is built first so the owner can test on their own company site; the connector follows. There is no desktop app. The owner settled this on 2026-11-13.
- Google Search Console supplies real queries, clicks and impressions.
- Sites are re-crawled weekly and each fix is confirmed on the next crawl.
- The public landing page is separate from the app and collects early-access requests.

## Capabilities and Constraints

Phase 1, described on the landing page as working:

- crawls a whole site, including pages that need JavaScript
- ranks suggestions across technical SEO, AI readiness and content
- uses Google Search Console data when connected
- works inside Claude as a connector

Coming later, and always labelled that way: drafting fixes for approval, and tracking which AI engines cite the site.

Constraints:

- Nothing is published to a customer's site without a person approving it.
- No bulk page generation.
- llms.txt is reported as information only, never as an important fix.
- The landing page must pass Margincite's own checks: all text present in the HTML without JavaScript, valid metadata, structured data, sitemap, and a robots.txt that lets AI crawlers in.
- Undecided: pricing, the domain, the production host.

## Brand Commitments

- Name: Margincite.
- Core idea: suggestions as notes in the margin of your website.
- Voice: plain, specific and honest. No promises about outcomes.
- The owner asked on 2026-11-13 for the hand-drawn paper look to be replaced entirely.
- The owner named three ways a polished result would still be wrong: too plain to remember, looks like every SaaS site, too playful to trust.

## Evidence on Hand

- The landing page copy in `landing/src/content.ts`.
- One example suggestion, labelled as an example: "AI crawlers are blocked in robots.txt".
- There are no customers, testimonials, logos, usage numbers, benchmarks or prices. None may be invented.
- There are no product screenshots yet, because the app is not built. Any interface shown on the landing page is an illustration and is labelled as an example.

## Product Principles

1. Evidence before advice: every suggestion shows what was found and where.
2. Rank by what matters: the busiest pages and biggest wins come first.
3. A person approves every change to their site.
4. Measure, do not promise.
5. Meet people where they already work: inside Claude.

## Accessibility & Inclusion

Keep a skip link, visible focus outlines and reduced-motion handling. Body text meets normal contrast levels. All content is readable without JavaScript.
