---
name: Margincite
description: An orienteering map of your website, with the fixes drawn as a course across it.
colors:
  course: "#7c1fc2"
  course-deep: "#4a0f7a"
  course-pale: "#e9d5f7"
  open: "#ffc533"
  open-soft: "#ffe08a"
  thicket: "#bfdd8a"
  fight: "#7db94a"
  marsh: "#2e9bdd"
  contour: "#b4652b"
  sheet: "#ffffff"
  ink: "#1d1524"
  ink-soft: "#4b3f57"
typography:
  display:
    fontFamily: "League Gothic, Arial Narrow, sans-serif"
    fontSize: "clamp(3.4rem, 7.2vw, 6rem)"
    fontWeight: 400
    lineHeight: 0.92
    letterSpacing: "0.005em"
  headline:
    fontFamily: "League Gothic, Arial Narrow, sans-serif"
    fontSize: "clamp(2.9rem, 5.4vw, 4.75rem)"
    fontWeight: 400
    lineHeight: 0.94
    letterSpacing: "0.008em"
  title:
    fontFamily: "League Gothic, Arial Narrow, sans-serif"
    fontSize: "clamp(1.75rem, 2.4vw, 2.125rem)"
    fontWeight: 400
    lineHeight: 1
    letterSpacing: "0.02em"
  lead:
    fontFamily: "Barlow Semi Condensed, Arial Narrow, system-ui, sans-serif"
    fontSize: "clamp(1.25rem, 1.6vw, 1.4375rem)"
    fontWeight: 400
    lineHeight: 1.42
  body:
    fontFamily: "Barlow Semi Condensed, Arial Narrow, system-ui, sans-serif"
    fontSize: "1.1875rem"
    fontWeight: 400
    lineHeight: 1.5
  caption:
    fontFamily: "Barlow Semi Condensed, Arial Narrow, system-ui, sans-serif"
    fontSize: "0.9375rem"
    fontWeight: 400
  label:
    fontFamily: "Barlow Semi Condensed, Arial Narrow, system-ui, sans-serif"
    fontSize: "0.9375rem"
    fontWeight: 600
    lineHeight: 1.25
    letterSpacing: "0.09em"
  button:
    fontFamily: "Barlow Semi Condensed, Arial Narrow, system-ui, sans-serif"
    fontSize: "1.0625rem"
    fontWeight: 600
    lineHeight: 1.1
    letterSpacing: "0.09em"
rounded:
  corner: "4px"
spacing:
  gutter: "clamp(1.25rem, 4vw, 4rem)"
  section: "6rem"
  section-wide: "8rem"
  column-gap: "4rem"
components:
  button-primary:
    backgroundColor: "{colors.course}"
    textColor: "{colors.sheet}"
    typography: "{typography.button}"
    rounded: "{rounded.corner}"
    padding: "0.55rem 1.35rem"
    height: "3.25rem"
  button-primary-hover:
    backgroundColor: "{colors.sheet}"
    textColor: "{colors.course}"
  button-line:
    backgroundColor: "{colors.sheet}"
    textColor: "{colors.course}"
    typography: "{typography.button}"
    rounded: "{rounded.corner}"
    padding: "0.55rem 1.35rem"
    height: "3.25rem"
  button-line-hover:
    backgroundColor: "{colors.course}"
    textColor: "{colors.sheet}"
  button-on-course:
    backgroundColor: "{colors.sheet}"
    textColor: "{colors.course-deep}"
    typography: "{typography.button}"
    rounded: "{rounded.corner}"
    padding: "0.55rem 1.35rem"
    height: "3.25rem"
  button-on-course-hover:
    backgroundColor: "{colors.course-deep}"
    textColor: "{colors.sheet}"
  field:
    backgroundColor: "{colors.sheet}"
    textColor: "{colors.ink}"
    rounded: "{rounded.corner}"
    padding: "0.55rem 1rem"
    height: "3.25rem"
  title-block:
    backgroundColor: "{colors.sheet}"
    textColor: "{colors.ink}"
    rounded: "{rounded.corner}"
    padding: "clamp(1.25rem, 2vw, 2.25rem)"
    width: "clamp(23rem, 31vw, 31rem)"
  example-badge:
    backgroundColor: "{colors.course}"
    textColor: "{colors.sheet}"
    typography: "{typography.label}"
    rounded: "{rounded.corner}"
    padding: "0.125rem 0.5rem"
  map-tag:
    backgroundColor: "{colors.sheet}"
    textColor: "{colors.ink}"
    rounded: "{rounded.corner}"
  description-sheet:
    backgroundColor: "{colors.sheet}"
    textColor: "{colors.ink}"
    rounded: "{rounded.corner}"
  nav-bar:
    backgroundColor: "{colors.sheet}"
    textColor: "{colors.ink}"
    height: "4.5rem"
  nav-link:
    textColor: "{colors.ink}"
    typography: "{typography.label}"
  nav-link-hover:
    textColor: "{colors.course}"
---

# Design System: Margincite

This file records the landing page as built in `landing/`. The built code is the source. Where a plan and the build differ, the build is what is written here.

## Overview

**Creative North Star: "The Course Map"**

Margincite is drawn as an orienteering map. The website is the terrain. The fixes are a course across it: a start triangle, numbered control rings, and straight purple legs between them. The rest of the page is treated as parts of the same printed sheet: a title block, a margin strip that holds the key, ruled legend tables, a control description sheet, and a north arrow and scale bar at the foot.

The sheet is white. A small set of flat spot inks is printed on it, and each ink has one job. Headings are tall condensed gothic capitals. Reading text is a plain semi-condensed sans. Rules are solid and 2px wide. Corners are 4px. Nothing has depth. Things sit on top of each other in print order, and lettering that crosses line work gets a white knock-out behind it.

The look refuses the usual SaaS landing page of gradient hero, product screenshot and three feature cards. It has to be memorable without being playful, because the product asks to be trusted with evidence.

A desktop app was once planned and has been dropped; Margincite is used from inside Claude. If a data-heavy screen is ever built in this look, the note below applies. Dense tables in the app will need a plain readable face for data. The display capitals are for headings and control numbers only. See the note under Typography.

**Key Characteristics:**
- White sheet, flat spot inks, no gradients and no shadows.
- Course purple carries the course, the main rules, the buttons and the main headings.
- Tall condensed gothic capitals for headings. Plain semi-condensed sans for everything read.
- Solid 2px rules divide the sheet. Corners are 4px.
- Open control rings, a start triangle and a double-ring finish mark the order of things.
- Buttons have one active state: ink and ground swap.
- Every illustrated map or report is labelled as an example.

## Colors

The palette is the ink set of an orienteering map, printed flat on white.

### Primary
- **Course Purple** (`course`): the course itself (legs, rings, start triangle, control numbers), the filled button, the 2px rules that frame the nav, the hero and the footer, main headings on white, the example badge, and the ground of the early-access section.
- **Deep Course Purple** (`course-deep`): small purple lettering that needs more weight than course purple gives: row labels in the description sheet, the "start here" label, the sheet caption in the map margin, the honesty statement on yellow, button lettering on the purple field.
- **Pale Course** (`course-pale`): quiet lettering on the purple field, such as the footnote under the form.

### Secondary
- **Open-Land Yellow** (`open`): on the map it means search traffic. It is the one terrain ink that also leaves the map. It is the ground of the example-note section, and it is the alert ink on the purple field: focus outline, invalid field border, error message ground. The skip link also uses it.

### Tertiary
These inks appear only inside a map or a key swatch.
- **Rough-Open Yellow** (`open-soft`): the lighter fringe around open land.
- **Thicket Green** (`thicket`): pages that need JavaScript, lighter areas.
- **Dense-Thicket Green** (`fight`): the darkest green, printed with a darker green dot screen. Used for the "Needs JavaScript" key swatch.
- **Marsh Blue** (`marsh`): slow pages, drawn as horizontal blue hatching. Also the thin magnetic north lines.
- **Contour Brown** (`contour`): contours, form lines and knolls. On the map it means depth from the home page. It is also the quiet contour ground behind some sections, and the three lines in the logo.

### Neutral
- **Sheet White** (`sheet`): the page ground, the title block, tags, knock-outs behind map lettering, button lettering on purple.
- **Map Black** (`ink`): body text, sub-headings, legend table rules, the frame of the description sheet, trails and boulders on the map. It is a purple-tinted black, not pure black.
- **Soft Black** (`ink-soft`): secondary text on white.

### What the inks mean on a map

| Ink | Key label |
| --- | --- |
| Contour Brown | Depth from home |
| Open-Land Yellow | Search traffic |
| Thicket Green | Needs JavaScript |
| Marsh Blue | Slow pages |
| Course Purple | Fix order |

### Named Rules
**The Spot Ink Rule.** Every fill is one flat, solid ink. No gradients. No drop shadows. No translucent fills. No blend modes, including on the course where it crosses terrain.

**The Course Purple Rule.** The course is always plain Course Purple. Legs, rings, the start triangle and control numbers are never tinted, never made translucent, and never drawn in another ink on a map. On the purple field the course symbols swap to white, which is the same inversion buttons use.

**The Screened Line Rule.** Line work may be printed at reduced strength. The build does this in three places: the contour ground behind a section (16 to 30 percent), the magnetic north lines (60 percent) and the trails (80 percent). This applies to thin lines only. It is never a way to lighten a fill, a button, a border or lettering.

**The Map Ink Rule.** Green and blue stay on the map and in key swatches. Do not use them as section grounds, button colours or status colours.

## Typography

**Display Font:** League Gothic (with Arial Narrow, sans-serif)
**Body Font:** Barlow Semi Condensed (with Arial Narrow, system-ui, sans-serif)
**Label Font:** Barlow Semi Condensed at weight 600, in spaced capitals

**Character:** The display face is a tall, narrow gothic set only in capitals, like the title lettering on a printed map. The body face is a plain semi-condensed sans that reads easily and shares the narrow proportions. Body text uses tabular figures.

Loaded weights: League Gothic 400. Barlow Semi Condensed 400, 500 and 600.

### Hierarchy
- **Display** (400, `clamp(3.4rem, 7.2vw, 6rem)`, line-height 0.92, capitals): the one page heading. Course Purple, with the key word in Map Black. Inside the title block from 80rem wide it is `clamp(3.9rem, 5.6vw, 5.5rem)`.
- **Headline** (400, `clamp(2.9rem, 5.4vw, 4.75rem)`, line-height 0.94, capitals): section headings. Course Purple on white, Map Black on yellow, white on purple.
- **Title** (400, `clamp(1.75rem, 2.4vw, 2.125rem)`, line-height 1, tracking 0.02em, capitals): item headings inside a section. Always Map Black.
- **Lead** (400, `clamp(1.25rem, 1.6vw, 1.4375rem)`, line-height 1.42): the paragraph directly under a heading.
- **Body** (400, 1.1875rem, line-height 1.5): all other reading text. Paragraphs are capped between 20rem and 38rem wide.
- **Caption** (400, 0.9375rem): the map note, key entries and the copyright line.
- **Label** (600, 0.9375rem, tracking 0.09em, line-height 1.25, capitals): map lettering. Nav links, row labels in the description sheet, the sheet caption, badges.
- **Button** (600, 1.0625rem, tracking 0.09em, line-height 1.1, capitals): button lettering.

Weight 500 marks the fix titles in the numbered list under the small-screen map. Weight 600 marks short statements that must stand out in body text, such as form messages.

On the map, place names and tags are set in Barlow Semi Condensed 600. Control numbers are set in League Gothic at 3.3 times the map lettering size.

### Named Rules
**The No Stretch Rule.** Lettering is never stretched, squeezed or scaled on one axis to fit a box. The box is sized to the lettering. Map tags measure their text and size the box from it. The map itself crops to fill its frame and is never distorted.

**The Capitals Rule.** League Gothic is always capitals at weight 400. It is never faked bold, never set in lower case, and never used for running text.

**The Data Face Note (planned, not built).** If this look is ever used for a data-heavy screen, dense tables need a plain readable face for data. Do not set table data in League Gothic. Barlow Semi Condensed with tabular figures is the only reading face the build has. Whether it serves dense tables at small sizes, or a plainer face is added for data, has not been decided.

## Layout

**One left edge.** Every section sits in one container: as wide as the screen minus a gutter of `clamp(1.25rem, 4vw, 4rem)` on each side, up to 92rem. The hero uses the same gutter as padding, so its title block lines up with the sections below it. The hero itself stops growing at 100rem.

**Vertical rhythm.** Sections have 6rem of space above and below, and 8rem from 48rem wide. The footer has 3.5rem. The nav bar is 4.5rem tall and stays at the top while scrolling. Anchor jumps leave 5.5rem of room for it.

**Uneven columns.** Two-column sections never split evenly. The build uses 5:7, 5:6 and 6:5, with a 4rem gap. The heading column of the legend section stays in view while its table scrolls, from 80rem wide.

**Section grounds.** Sections change ground instead of using cards:
1. White.
2. White with a brown contour ground at 16 percent, and 2px Course Purple rules top and bottom.
3. Open-Land Yellow with a brown contour ground at 30 percent.
4. Course Purple with a white contour ground at 20 percent.
5. White for the footer.

**Breakpoints.** 40rem, 48rem, 64rem and 80rem.

**The hero at 80rem and wider.** The map is the ground of the whole hero, edge to edge. The title block sits over it at the left. The margin strip is fixed to the bottom edge. Each control carries a tag with its fix. The hero is at least 42rem tall and at most the height of the first screen under the nav.

**The hero below 80rem.** The words come first on plain white, with no frame. From 48rem the heading and the text sit in two columns, aligned at the bottom. Then a 2px Course Purple rule, then the map as a cropped figure with larger lettering and no tags. The three fixes follow as a list, each with its number inside a control ring. The margin strip comes last and wraps.

**Nav.** Links show from 64rem. The early-access button shows from 40rem. Below 64rem a menu button opens a panel under the bar.

## Elevation & Depth

There is none. The system is flat. No shadows, no blur, no glow, no raised surfaces.

Order is shown the way a printed map shows it, by what is printed over what:
1. White sheet.
2. Area inks: rough open, open land, thicket, dense thicket, marsh.
3. Line work: north lines, form lines, contours, knolls, trails, boulders.
4. Place names, each with a white halo.
5. The course.
6. Labels and tags on solid white knock-outs.

A panel over the map, such as the title block or a tag, is a solid white box with a solid border. It hides what is under it completely.

### Named Rules
**The Flat Sheet Rule.** If something needs to stand out from what is behind it, give it a solid white ground and a solid rule. Never a shadow, a blur or a see-through panel.

**The Knock-Out Rule.** Lettering on the map never has line work running through it. Give it a white box behind, or a white outline around each letter.

## Shapes

**Corners.** One radius: gently eased corners (4px) on buttons, fields, the title block, tags, the description sheet and form messages.

**Rules.** Solid and 2px. Course Purple rules frame the big parts of the sheet: under the nav, around the title block, above the margin strip, above and below the contour section, above the copyright line. Map Black rules draw tables: the legend rows and the description sheet. The description sheet has a 3px Map Black outer frame.

**Course symbols.** All are drawn in one stroke weight with round caps and joins, and take the colour of their context.
- Start: an open triangle. On a map it points at the first control.
- Control: an open ring.
- Finish: two open concentric rings.
- Planned: an open ring with a dashed stroke. It marks things that are coming later.
- Leg: a straight line between two marks. It stops short of both and never touches or enters a ring.

**Sheet furniture.** A north arrow and a scale bar sit at the foot of the page. Key swatches are small 48 by 30 samples of each ink, drawn the way the map draws it, with a thin Map Black frame.

**Terrain.** The terrain is generated by `landing/scripts/generate-map.mjs` from one fixed seed, so it is the same on every run. Contours are thin brown lines, with every fifth one heavier. Form lines are dashed. Trails are dashed Map Black. Marsh is horizontal blue hatching. Dense thicket carries a dot screen. To change the terrain, change the script and regenerate. Do not edit the SVG files by hand.

**The logo.** A small square map sheet with a ruled margin, a purple dot in the margin and three brown lines in the body, next to the name in display capitals.

### Named Rules
**The Open Ring Rule.** A control is an open ring. It is never filled, so the ground under it stays visible. This holds on the map and wherever a control ring is used as a marker.

**The Short Leg Rule.** Course legs are straight and stop short of the marks at both ends.

## Components

### Buttons
Plain ruled blocks with spaced capital lettering.
- **Shape:** eased corners (4px), 2px solid border, at least 3.25rem tall, padding 0.55rem by 1.35rem. An optional arrow sits 0.6rem after the label.
- **Primary:** Course Purple ground, white lettering, Course Purple border.
- **Line:** white ground, Course Purple lettering, Course Purple border.
- **On the purple field:** white ground, Deep Course Purple lettering, white border.
- **Hover:** ink and ground swap. Primary becomes white with purple lettering. Line becomes purple with white lettering. On the purple field the button becomes Deep Course Purple with white lettering. The border does not change. The swap takes 140ms.
- **Focus:** a 3px solid Course Purple outline, 3px away from the edge. On the purple field the outline is Open-Land Yellow.
- **Compact:** in the nav bar the button is at least 2.75rem tall.
- **Full width:** inside the title block and the mobile menu the button fills the width.
- **Sending:** while a form is sending, the button is disabled and its label changes. The build also fades the button to 72 percent. That fade conflicts with the Spot Ink Rule and is not part of the system. Do not copy it.

**The Inversion Rule.** The only active state on a button is a strict swap of ink and ground. No lift, no shadow, no scale, no tint, no darker shade of the same colour.

### Inputs / Fields
The build has one field, on the purple field.
- **Style:** white ground, 2px white border, eased corners (4px), at least 3.25rem tall, Map Black text at 1.125rem.
- **Focus:** 3px Open-Land Yellow outline, 2px away from the edge.
- **Invalid:** the border turns Open-Land Yellow. The message sits below on an Open-Land Yellow ground in Map Black at weight 600.
- **Success:** the form is replaced by a white box with Deep Course Purple lettering at weight 600 and a small finish symbol.

A field on a white ground has not been built.

### Navigation
- **Bar:** white, 4.5rem tall, a 2px Course Purple rule under it, fixed to the top while scrolling.
- **Links:** label lettering in Map Black. On hover they turn Course Purple and gain an underline.
- **Menu button:** a line button holding a three-bar icon, which becomes a cross when open.
- **Menu panel:** white, with a 2px Course Purple rule above. Links are set in display capitals at 2.25rem in Course Purple, one per row. The early-access button sits below them at full width on the narrowest screens.

### Title Block
The ruled white panel that carries the page heading over the map. White ground, 2px Course Purple border, eased corners (4px), padding `clamp(1.25rem, 2vw, 2.25rem)`, width `clamp(23rem, 31vw, 31rem)`. It holds the heading, the lead paragraph, both buttons stacked at full width, and a one-line status. It only appears from 80rem wide. Below that the same content sits on the plain sheet with no frame.

### Course Map
The signature component. A generated terrain with a course overprinted on it.
- **Course:** Course Purple, stroke 6 in a 1600 by 900 drawing. Start triangle, then controls 1, 2 and 3 as open rings of radius 31. The last leg runs off the bottom of the sheet, so the course is seen to continue.
- **Control numbers:** display face, Course Purple, with a white halo. The number sits outside the ring at its upper left. Where a leg arrives there, the number moves above the ring.
- **Tags:** a white box with a Course Purple border and eased corners, holding the fix in Map Black at weight 600. The tag sits to the right of its control. Tags are dropped where the map is small, and the fixes are listed under the map instead.
- **Place names:** Map Black at weight 600 with a white halo.
- **Start label:** "start here" in label lettering, Deep Course Purple, on a white knock-out.
- **Motion:** on load the course is drawn once, in order. Each leg draws in 620ms. Each ring and number appears in 520ms. Each tag slides in 10px from the left in 520ms. Steps are 260ms apart. Easing is `cubic-bezier(0.16, 1, 0.3, 1)`. With reduced motion set, the course is simply there.
- **Text alternative:** the map has one label that says it is an illustrated example and lists the three fixes in order.

### Margin Strip and Key
The strip along the bottom of the map. White, with a 2px Course Purple rule above. On the left: the sheet caption in label lettering, the example badge, and a short note that this is an illustration. On the right: the key, a row of swatches with caption text. It wraps into rows on small screens.

### Example Badge
A small Course Purple block with white label lettering, padding 0.125rem by 0.5rem. It marks illustrated content as an example. The build sets its corners at 3px. That is drift from the one radius. Use 4px.

**The Example Label Rule.** The hero map shows an imaginary site. It always carries the example badge and the note that it is an illustration and not a screenshot. Any other illustrated report or interface carries the badge too.

### Legend Table
How a list of features is set. Each row has a map symbol in a fixed left column (4.5rem, or 6.5rem from 40rem wide), then a title and a body paragraph. Rows are divided by 2px Map Black rules, with one more closing the table. Row padding is 1.75rem. Symbols are key swatches or the north arrow. There are no cards.

### Course Steps
How a short process is set. Three marks in order: start triangle, control ring, finish double ring. Each is 4.5rem, in Course Purple. A 5px Course Purple line joins one mark to the next from 48rem wide. A title and a short paragraph sit under each mark.

### Control Description Sheet
How one suggestion is shown in detail. A white box with a 3px Map Black frame and eased corners. The head row holds a control ring with the number inside it, the title, and the example badge. Below, each row is divided by a 2px Map Black rule. From 40rem wide the row label sits in a 10.5rem left column with a 2px rule to its right. Row labels are label lettering in Deep Course Purple.

### Contour Ground
A quiet ground for sections that are not the map. The contour lines alone, in brown or white, tiled at 1600px wide behind the content. Strength is set per section between 16 and 30 percent, and the position is shifted so no two sections show the same piece.

### Planned List
A list of things that are coming later. Each item starts with the dashed planned ring at 1.75rem.

## Do's and Don'ts

### Do:
- **Do** print every fill as one flat ink from the palette.
- **Do** keep the course plain Course Purple, with open control rings and straight legs that stop short of each mark.
- **Do** make the hover state of every button a strict swap of ink and ground, with the border unchanged.
- **Do** size boxes to their lettering. Measure the text, then draw the box.
- **Do** label every illustrated map, report or interface with the example badge.
- **Do** divide content with solid 2px rules and changes of ground.
- **Do** put a white knock-out or a white halo behind any lettering that sits on the map.
- **Do** use 4px corners on every box.
- **Do** set League Gothic in capitals at weight 400 for headings and control numbers only.
- **Do** keep the skip link, visible focus outlines and reduced-motion handling on every new surface.
- **Do** use a plain readable face with tabular figures for data if this look is ever used for a data-heavy screen.

### Don't:
- **Don't** use gradients, drop shadows, glows, blurs or translucent fills.
- **Don't** put a blend mode on the course, or tint it, or draw it in another ink on a map.
- **Don't** fill a control ring.
- **Don't** give a button any active state other than the ink and ground swap. No lift, no scale, no darker shade.
- **Don't** stretch, squeeze or scale lettering on one axis to fit a box.
- **Don't** show the hero map, or any illustrated report, without its example label.
- **Don't** use green or blue outside a map or a key swatch.
- **Don't** set running text or table data in League Gothic.
- **Don't** build the SaaS hero of gradient, screenshot and three feature cards. Lists are ruled tables, not cards.
- **Don't** edit the generated terrain files by hand. Change the script and regenerate.
- **Don't** lighten a fill, a border or lettering with opacity. Reduced strength is for thin map line work only.
