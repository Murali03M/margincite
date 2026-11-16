---
name: Margincite
description: An evidence room for your website. Every fix is laid on the bench with numbered markers, a scale and its label.
colors:
  bench: "#eceeea"
  bench-deep: "#dfe2dc"
  paper: "#ffffff"
  ink: "#121315"
  ink-soft: "#464a50"
  ink-faint: "#6b7076"
  rule: "#c4c8c2"
  marker: "#ffd31a"
  marker-deep: "#d9ad00"
  seal: "#c8231b"
  plate: "#121315"
  plate-rule: "#5a5f66"
  on-plate: "#f4f5f2"
  on-plate-soft: "#b9bdc2"
  on-marker: "#121315"
typography:
  display:
    fontFamily: "Archivo Variable, Arial, sans-serif"
    fontSize: "clamp(2.35rem, 4.5vw, 4.6rem)"
    fontWeight: 850
    lineHeight: 0.96
    letterSpacing: "-0.02em"
    fontVariation: "\"wdth\" 125"
  headline:
    fontFamily: "Archivo Variable, Arial, sans-serif"
    fontSize: "clamp(1.9rem, 3.3vw, 3.1rem)"
    fontWeight: 850
    lineHeight: 1
    letterSpacing: "-0.015em"
    fontVariation: "\"wdth\" 125"
  title:
    fontFamily: "Archivo Variable, Arial, sans-serif"
    fontSize: "1.3125rem"
    fontWeight: 750
    lineHeight: 1.15
    letterSpacing: "-0.005em"
    fontVariation: "\"wdth\" 100"
  lead:
    fontFamily: "Public Sans, system-ui, sans-serif"
    fontSize: "clamp(1.125rem, 1.4vw, 1.3125rem)"
    fontWeight: 400
    lineHeight: 1.55
  body:
    fontFamily: "Public Sans, system-ui, sans-serif"
    fontSize: "1.0625rem"
    fontWeight: 400
    lineHeight: 1.6
    fontFeature: "\"tnum\""
  small:
    fontFamily: "Public Sans, system-ui, sans-serif"
    fontSize: "0.9375rem"
    fontWeight: 400
    lineHeight: 1.6
  label:
    fontFamily: "Archivo Variable, Arial, sans-serif"
    fontSize: "0.78rem"
    fontWeight: 700
    lineHeight: 1.2
    letterSpacing: "0.08em"
    fontVariation: "\"wdth\" 112"
  button:
    fontFamily: "Archivo Variable, Arial, sans-serif"
    fontSize: "0.9375rem"
    fontWeight: 750
    lineHeight: 1.1
    letterSpacing: "0.06em"
    fontVariation: "\"wdth\" 112"
  mono:
    fontFamily: "JetBrains Mono, ui-monospace, monospace"
    fontSize: "0.8125rem"
    fontWeight: 400
    lineHeight: 1.6
    fontFeature: "\"liga\" 0"
rounded:
  tag: "2px"
  plate: "3px"
spacing:
  gutter: "clamp(1rem, 4vw, 4rem)"
  heading-gap: "1.25rem"
  block: "3rem"
  column-gap: "3.5rem"
  section: "5rem"
  section-lg: "7rem"
components:
  button-primary:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.on-plate}"
    typography: "{typography.button}"
    rounded: "{rounded.plate}"
    padding: "0.6rem 1.3rem"
    height: "3.25rem"
  button-primary-hover:
    backgroundColor: "{colors.marker}"
    textColor: "{colors.ink}"
  button-line:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    typography: "{typography.button}"
    rounded: "{rounded.plate}"
    padding: "0.6rem 1.3rem"
    height: "3.25rem"
  button-line-hover:
    backgroundColor: "{colors.marker}"
    textColor: "{colors.ink}"
  button-compact:
    padding: "0.45rem 0.95rem"
    height: "2.6rem"
  button-on-marker-hover:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink}"
  badge-example:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.marker}"
    typography: "{typography.label}"
    rounded: "{rounded.plate}"
    padding: "0.25rem 0.5rem"
  serves-tag:
    textColor: "{colors.ink}"
    rounded: "{rounded.tag}"
    padding: "1px 0.375rem"
  paper-sheet:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink}"
    rounded: "{rounded.plate}"
  exhibit-bench:
    backgroundColor: "{colors.bench-deep}"
    rounded: "4px"
    padding: "2rem"
  nav-bar:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink}"
    height: "4rem"
  visit-stamp:
    textColor: "{colors.ink}"
    rounded: "{rounded.plate}"
    padding: "0.2rem 0.375rem 0.125rem"
  section-ink:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.on-plate}"
  section-marker:
    backgroundColor: "{colors.marker}"
    textColor: "{colors.ink}"
---

# Design System: Margincite

## Overview

**Creative North Star: "The Evidence Room"**

Margincite's public page is set as a forensic bench. A finding is laid down on white paper over a grey bench, numbered yellow markers are set beside each piece of evidence, a black-and-white scale is laid along two edges, and a printed exhibit label says what was found and what the fix is. The page argues by showing: every fix arrives with its evidence. It is light because people read it at a desk, and it is dense the way a case file is dense, with ruled logs and labelled rows rather than cards and gradients.

The materials are few and physical. Bench grey and paper white are the grounds. Evidence-black ink does the structure: rules, plates, borders, buttons. Marker yellow is the one loud colour, and it belongs to marks: the tents, the key words of the headline, the lettering on ink badges, the closing field and the hover ground. Seal red appears only on what was found. Type is wide, heavy Archivo capitals for anything printed by a label machine or set as a heading, Public Sans for reading, and JetBrains Mono for files, commands, counts and recorded values.

Motion is one authored moment, on the exhibit, on load: markers set down 1, 2, 3, the scale drawn up and then along, the exhibit label placed. The visit count in the top bar rolls its digit wheels once when the number arrives. Under reduced motion all of it is off and everything is already in place.

**Key Characteristics:**
- Light grounds (bench, paper) with ink structure and one yellow for marks.
- Seal red is reserved; it is never a decoration or an accent.
- Wide (112% to 125%) heavy Archivo capitals for headings, labels and buttons.
- Mono for every file name, command, count and recorded value.
- Ruled logs (tables and lists with ink and grey rules) instead of cards.
- Forensic props drawn in code: numbered A-frame markers, a scale, a numbering-stamp counter.
- Square-shouldered corners (3px) and one soft shadow, only for paper on the bench.

## Colors

A near-neutral bench-and-paper palette with evidence-black ink, one saturated yellow for marks and one red held back for findings.

### Primary
- **Marker Yellow** (#ffd31a): the evidence tents, the highlighted key words of the hero headline (laid behind the text as a flat band), the lettering on ink badges and ink plates ("Example", "Exhibit 1 of 17", the SEO/AIO/GEO column plates, the `$` prompt on the ink section), the whole "Run it on your own site" field, text selection, and the hover ground of every button.
- **Marker Shade** (#d9ad00): only inside the marker drawing, for the folded side and the top lip of the tent. Not used as a surface or text colour.

### Secondary
- **Seal Red** (#c8231b): the blocked `Disallow: /` lines in the exhibit (bold mono). 5.66:1 on paper.

### Neutral
- **Evidence Ink** (#121315 light, #eceee9 dark): text, every structural rule and border, and the primary button ground (its lettering is the bench colour, so the button inverts with the room).
- **Ink Soft** (#464a50): lead paragraphs, secondary copy, notes, the file name in the exhibit header. 8.9:1 on paper, 7.6:1 on bench.
- **Ink Faint** (#6b7076): line numbers, table column headers, the `$` prompt in the terminal, label names in the exhibit tag. 4.99:1 on paper.
- **Bench** (#eceeea): the page ground behind the hero, "SEO, AIO and GEO" and "A closer look". A grey with a very slight green cast (oklch hue 129, chroma 0.006).
- **Bench Deep** (#dfe2dc): the recessed bench the exhibit is laid on in the hero.
- **Paper** (#ffffff): the nav bar, the "What it does today" and FAQ sections, and every sheet laid on the bench (exhibit document, exhibit tag, sample list, finding sheet, terminal).
- **Rule Grey** (#c4c8c2): the thin row rules inside logs, tables, sheets and the mobile menu. Never used for text.
- **Plate** (#121315 light, #08090a dark): solid dark fills that stay dark in both rooms: badges, the exhibit tag head, the SEO/AIO/GEO column plates, the "How it works" band and the footer. **On-Plate** (#f4f5f2) and **On-Plate Soft** (#b9bdc2) letter them; **Plate Rule** (#5a5f66) draws borders on them.
- **On-Marker** (#121315 in both rooms): lettering on marker yellow and the strokes and numbers on the evidence markers. Markers are physical yellow objects, so they never invert.

### Two rooms
Dark is the default, also without JavaScript. A sun/moon switch in the top bar (inside the menu on phones) turns on the light room; the choice is kept in this browser under `margincite:theme`, and a script in `index.html` applies it before the page paints.

| Token | Light room | Dark room |
|---|---|---|
| bench | #eceeea | #131416 |
| bench-deep | #dfe2dc | #0b0c0d |
| paper | #ffffff | #1d1f23 (with a 1px #2f3338 edge, since shadows read weakly on dark) |
| ink | #121315 | #eceee9 |
| ink-soft | #464a50 | #b3b8be |
| ink-faint | #6b7076 | #8d939a (5.2:1 on paper) |
| rule | #c4c8c2 | #34383e |
| seal | #c8231b | #ff6a5e (5.2:1 on paper) |
| marker | #ffd31a | #ffd31a |

The marker-yellow field ("Run it on your own site") is lit the same in both rooms: `.on-marker` resets the light values inside it.

### Named Rules
**The Seal Rule.** Seal red marks only what was found (the blocked lines in an exhibit). It is not an error colour, a link colour, a price marker or an accent.

**The Marker Is A Mark Rule.** Marker yellow is for things that point at evidence or ask to be acted on: tents, highlighted key words, ink-plate lettering, the closing field, hover grounds. It is never a body-text colour on light grounds and never a gradient.

**The Faint-On-Paper Rule.** Ink Faint is used only on white paper, where it holds 4.99:1. On bench it drops to 4.28:1 and on bench-deep to 3.82:1, so faint text on those grounds becomes Ink Soft.

## Typography

**Display Font:** Archivo Variable (width axis), with Arial
**Body Font:** Public Sans (400, 600, 700), with system-ui
**Label/Mono Font:** JetBrains Mono (400, 700), with ui-monospace

**Character:** Archivo stretched wide and set very heavy reads like a label printer and stencilled case lettering; Public Sans is a plain public-service text face that keeps the reading calm. Mono marks anything that was recorded rather than written.

### Hierarchy
- **Display** (850, clamp(2.35rem, 4.5vw, 4.6rem), 0.96, width 125%): the one hero headline. Its key words sit on a marker-yellow band (padding 0.08em, cloned across line breaks).
- **Headline** (850, clamp(1.9rem, 3.3vw, 3.1rem), 1, width 125%): section headings.
- **Title** (750, 1.3125rem, 1.15, width 100%): row and item headings in logs, FAQ questions, the finding-sheet title. Raised to 1.5rem for the three "How it works" steps.
- **Lead** (400, clamp(1.125rem, 1.4vw, 1.3125rem), 1.55): the paragraph under a heading, in Ink Soft, held to the 38rem measure.
- **Body** (400, 1.0625rem, 1.6, tabular figures): all running text. `text-wrap: pretty` on paragraphs, `balance` on headings.
- **Small** (400, 0.9375rem): status lines, the "Needs" column, the costs note, footer copy.
- **Label** (700, 0.78rem, 0.08em, uppercase, width 112%): nav links (0.8125rem), table headers, exhibit-tag names, badges, the hero feature strip (0.8125rem).
- **Mono** (400 or 700, 0.8125rem; 0.9375rem for code lines from 640px): file names, robots.txt lines, commands, recorded values in the exhibit tag, reach counts, SEO/AIO/GEO tags (0.75rem bold), the visit count. Ligatures off.

### Named Rules
**The Label Printer Rule.** Every small uppercase word is Archivo at 112% width, weight 700, tracked 0.08em. Small capitals are never set in Public Sans or mono.

**The Recorded Value Rule.** If a value was read off a site or typed into a terminal (a file, a line, a command, a count, a site name), it is set in JetBrains Mono. Prose about it is Public Sans.

## Layout

One container, `wrap`: full width minus a fluid gutter of clamp(1rem, 4vw, 4rem) on each side, capped at 90rem, centred. Every section, the nav and the footer share that left edge.

From 64rem (lg) sections use a 12-column grid with a 3.5rem column gap. Common splits are 5/7 (hero, "Run it") and 4/8 (features, FAQ, with the heading column sticky at 6rem from the top in features). "A closer look" uses a full-width sample list then a 7/5 split. Below 64rem everything is one column.

Sections are separated by a change of ground (bench, paper, ink, marker) and, where two light grounds meet, a 1px ink rule. Section padding is 5rem top and bottom, 7rem from 64rem. Heading to lead is 1.25rem; lead to the next block is about 3rem.

The hero places the headline, the exhibit and the words with explicit grid rows on desktop (headline and words left in five columns, exhibit right in seven, vertically centred). The source order is headline, exhibit, words and buttons, so phones read in that order without reordering. A ruled strip of four feature labels sits under the hero between two ink rules.

Logs (the features table, the SEO/AIO/GEO comparison, the sample list, the finding sheet, the FAQ) are tables or definition lists. From 48rem (md) they lay out as columns; below it each row stacks, and column names repeat inline where needed (the SEO/AIO/GEO mono prefix on phones).

Breakpoints are Tailwind's defaults: 40rem, 48rem, 64rem. The nav is 4rem tall and sticky; in-page anchors scroll with 5rem padding.

## Elevation & Depth

The system is flat except for one material: paper laid on the bench. A sheet of paper casts a soft, short shadow with a slight drop, which is what makes the bench read as a surface. Ink plates, badges, buttons and sections have no shadow. Depth otherwise comes from the bench-deep recess under the exhibit and from the exhibit tag overlapping the bench's bottom edge by 2rem on desktop.

### Shadow Vocabulary
- **Paper on bench** (`box-shadow: 0 1px 2px rgb(18 19 21 / 0.12), 0 14px 28px -10px rgb(18 19 21 / 0.28)`): every paper sheet: the exhibit document and tag, the sample list, the finding sheet, the terminal.

### Named Rules
**The One Shadow Rule.** Only paper casts a shadow, and it is always the same shadow. There are no hover lifts and no hard offset shadows.

## Shapes

Corners are square-shouldered: 3px on buttons, badges, plates, sheets and the visit stamp; 2px on the small SEO/AIO/GEO tags. Nothing is pill-shaped or circular.

Borders are ink and come in three weights: 1px for section edges, the nav bottom and the hero strip; 1.5px for the scale, the exhibit tag, the sample list and the SEO/AIO/GEO tags; 2px for buttons, the finding sheet, the terminal and the visit stamp. Inside logs, the header rule is 2px ink and row rules are 1px Rule Grey.

Two shapes are drawn rather than boxed. The evidence marker is a folded A-frame tent (yellow face, ink outline 1.5px with round joins, a shaded side and lip, an Archivo 850 numeral). The planned marker is the same tent as a dashed outline (2px, dash 4 / 3.2) for things that are coming later.

## Components

### Buttons
Ink plates with wide label lettering; hover lays marker yellow under the words.
- **Shape:** square-shouldered (3px), 2px ink border, minimum height 3.25rem.
- **Primary:** ink ground, bench-coloured lettering, padding 0.6rem 1.3rem, Archivo 750 at 112% width, uppercase, 0.06em tracking. A GitHub mark leads and an arrow trails on GitHub links.
- **Line:** transparent ground, ink lettering, same border.
- **Compact:** 2.6rem tall, 0.45rem 0.95rem padding, 0.8125rem lettering. Used in the top bar; on phones the GitHub button is icon-only.
- **Hover / Focus:** ground and colour swap to marker yellow and ink over 160ms on the ease-out-expo curve. On the marker-yellow field the hover ground is paper instead. Focus is a 3px ink outline offset 3px; on ink grounds (the How it works band and the footer) it turns marker yellow.

### Chips (tags and badges)
- **SEO/AIO/GEO tags:** mono 0.75rem bold, 1.5px ink border, 2px corners. They say which kind of search a feature or fix serves.
- **Example badge:** label lettering in marker yellow on an ink plate. Every illustrated finding carries it.

### Cards / Containers (paper sheets)
- **Corner Style:** 3px.
- **Background:** paper on a bench ground.
- **Shadow Strategy:** the paper-on-bench shadow, always.
- **Border:** none on the exhibit document; ink on sheets that stand alone (see drift on 1.5px vs 2px).
- **Internal structure:** a header strip (mono file name or caption on the left, a label or badge on the right) over a 2px ink or 1px grey rule, then ruled rows padded 1rem by 0.75 to 1rem (1.5rem sides from 640px).

### Navigation
Paper bar, 4rem tall, sticky, 1px ink bottom rule. Wordmark left (a yellow "M" marker beside "MARGINCITE" in Archivo 850 at 118%, hidden under 375px). Label-lettered section links in the middle from 64rem, underlined 2px on hover. Right: the visit stamp, then the compact GitHub button (icon-only under 640px) and, under 64rem, a compact line button for the menu. The open menu is a paper panel under a 1px ink rule with 1.75rem Archivo 850 capitals separated by grey rules, and a full-width GitHub button on phones.

### Evidence Marker
The signature mark. The yellow tent with a number, set beside each piece of evidence and used as the rank number in lists (1, 2, 3 in rank order) and as the step number in "How it works". On ink it carries an ink ground so the dashed step connector stops at it.

### Forensic Scale
A ruler drawn in CSS: alternating ink and paper units (1.25rem, 1.5rem in the footer) over fine ticks every fifth of a unit, inside a 1.5px ink border. Laid as an L along the left and bottom of the exhibit document, and full-width across the top of the footer.

### Exhibit Tag
A paper label with a 1.5px ink border: an ink title strip in marker-yellow label lettering ("Exhibit 1 of 17"), then ruled rows of a faint label name (4.25rem column) and a bold mono value. On desktop it is 19rem wide and overlaps the bench's lower right corner.

### Visit Stamp
A numbering stamp in the top bar: a 2px ink border with 3px corners, rotated -2deg, a tiny "Visits" label over six mono digits. Each digit is a wheel of 0 to 9 that rolls to its value once the count arrives (900ms, ease-out-expo, 70ms stagger per place). It renders nothing on the server and nothing if the count fails; it pulses while loading.

### Signature Motion
One sequence on the hero exhibit, all on `cubic-bezier(0.16, 1, 0.3, 1)`:
- Markers 1, 2, 3 set down (fade in from 14px above, 560ms), starting at 200ms, 180ms apart.
- The vertical scale leg is drawn upward from the corner at 820ms (clip reveal, 700ms).
- The horizontal leg is drawn left to right at 1070ms (700ms).
- The exhibit tag is placed at 1300ms (fade, 12px rise, 3px blur to sharp, 620ms).

Under `prefers-reduced-motion: reduce` every animation and transition is cut to near zero and smooth scrolling is off. The page is complete without the sequence.

## Do's and Don'ts

### Do:
- **Do** lay any example finding on paper over the bench, with numbered markers on the exact lines, and an Example badge beside it.
- **Do** number markers in rank or step order, and use the same tent everywhere a number marks evidence.
- **Do** set every file, command, count and recorded value in JetBrains Mono, and every small uppercase word with the label style.
- **Do** build lists of facts as ruled logs: a 2px ink header rule, 1px Rule Grey row rules.
- **Do** keep corners at 3px (2px for the SEO/AIO/GEO tags) and borders in ink.
- **Do** keep every section on the one `wrap` left edge and the 12-column grid from 64rem.
- **Do** put any new motion behind `prefers-reduced-motion: no-preference` and on the ease-out-expo curve.

### Don't:
- **Don't** use seal red for anything except what was found.
- **Don't** stamp prices or costs on features. Connecting an outside service is the visitor's choice; the "Needs" column names the service and the FAQ answers cost questions.
- **Don't** put a shadow on anything that is not paper laid on the bench, and don't use hard offset shadows.
- **Don't** use gradients as fills or backgrounds. The only gradients in the system are the scale's tick pattern.
- **Don't** set Ink Faint text on bench or bench-deep grounds.
- **Don't** show a finding or interface without the Example badge; the page shows illustrations, not screenshots.
- **Don't** add small kicker lines above headings; headings stand on their own.
- **Don't** use pill shapes, large radii, or translucent borders.

### Known drift (in the build, not part of the system)
- The "How it works" dashed step connector uses On-Ink Soft at 60% opacity and the command chip border uses it at 50%. Opacity on borders is outside the system; these should be solid On-Ink Soft or a new token.
- The visit stamp's "Visits" label is 0.5625rem (9px) on phones and 0.625rem from 640px, below the 0.6875rem used for the smallest labels elsewhere.
- The FAQ plus icon turns over 200ms on the default ease, not 160ms ease-out-expo; the visit stamp's loading pulse is Tailwind's default `animate-pulse`.
- Paper sheet borders differ side by side in "A closer look": 1.5px on the sample list, 2px on the finding sheet (and 1.5px on the exhibit tag, 2px on the terminal).
- One-off values: the exhibit bench's 4px radius; Archivo widths of 104% (honesty line), 108% (mobile menu links) and 118% (wordmark) outside the 100/112/125 set.
- "How it works" list items set a `--step` variable with no animation that reads it.
- The direction calls the bench "cool"; the built bench is a near-neutral grey with a slight green cast (hue 129), while the ink family is cool (hue 253 to 264).
