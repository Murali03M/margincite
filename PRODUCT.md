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

One ranked list that covers Google search, Google AI Overviews and AI assistants, with evidence on every item. Free and open source under AGPL-3.0, run on your own machine with your own provider keys (decided 2026-11-16). A Claude connector is planned, not built. A direct reference is the Claude SEO Skill (claudeseoskill.com), an SEO/GEO/AEO audit packaged as a Claude skill; the owner asked on 2026-11-16 for a Margincite Claude skill as well.

Margincite measures and reports. It does not promise rankings or citations.

## Operating Context

- A web app and a Claude connector, both backed by one hosted Rust service. The web app is built first so the owner can test on their own company site; the connector follows. There is no desktop app. The owner settled this on 2026-11-13.
- Google Search Console supplies real queries, clicks and impressions.
- Sites are re-crawled weekly and each fix is confirmed on the next crawl.
- The public landing page is separate from the app. It is hosted on GitHub Pages, its main action is "Get it on GitHub", and it shows a visit count from the Abacus counting service. It no longer collects emails (decided 2026-11-16).

## Capabilities and Constraints

Described on the landing page as working today (see the README's feature table for the full list):

- crawls a whole site, including pages that need JavaScript
- ranks suggestions across technical SEO, AI readiness and content
- uses Google Search Console data when connected
- AI visibility across Claude, ChatGPT, Gemini, Grok and Perplexity, rank tracking with AI Overviews, page speed and backlinks, each with its own connection

Coming later, and always labelled that way: using Margincite from Claude, ChatGPT and Grok as a connector, drafting fixes for approval, and alerts and reports.

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
- The landing page shows no product screenshots. Any finding or interface shown on it is an illustration and is labelled as an example.

## Product Principles

1. Evidence before advice: every suggestion shows what was found and where.
2. Rank by what matters: the busiest pages and biggest wins come first.
3. A person approves every change to their site.
4. Measure, do not promise.
5. Meet people where they already work: inside Claude.

## Accessibility & Inclusion

Keep a skip link, visible focus outlines and reduced-motion handling. Body text meets normal contrast levels. All content is readable without JavaScript.
