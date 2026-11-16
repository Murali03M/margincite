//! Every check Margincite can make, with the words a person reads.

use super::context::Category;

/// Something a check cannot run without. A check whose need is missing for a
/// site is `not_run` there, and is left out of that site's score.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    /// At least one target keyword.
    Keywords,
    /// Synced Search Console data.
    SearchConsole,
    /// A completed AI visibility run.
    AiRun,
    /// At least one page speed result without an error.
    Speed,
    /// At least one keyword checked in the site's search results.
    Rankings,
}

impl Need {
    /// The sentence shown in the checks list when this need is missing.
    pub fn reason(self) -> &'static str {
        match self {
            Need::Keywords => "Add target keywords to run this check.",
            Need::SearchConsole => "Connect Google Search Console to run this check.",
            Need::AiRun => "Run an AI visibility check to see this.",
            Need::Speed => "Run a page speed check to see this.",
            Need::Rankings => {
                "Check your keyword rankings to see this. It needs a DataForSEO login in Settings."
            }
        }
    }
}

/// What a site has, which decides which checks can run for it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Available {
    pub keywords: bool,
    pub search_console: bool,
    pub ai_run: bool,
    pub speed: bool,
    pub rankings: bool,
}

impl Available {
    pub fn has(self, need: Need) -> bool {
        match need {
            Need::Keywords => self.keywords,
            Need::SearchConsole => self.search_console,
            Need::AiRun => self.ai_run,
            Need::Speed => self.speed,
            Need::Rankings => self.rankings,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CheckDef {
    pub id: &'static str,
    pub category: Category,
    /// 1 (minor) to 3 (serious).
    pub severity: u8,
    /// 1 (minutes) to 3 (a project).
    pub effort: u8,
    pub title: &'static str,
    pub why: &'static str,
    pub fix: &'static str,
    /// What the check cannot run without. Empty for checks that need only the crawl.
    pub needs: &'static [Need],
}

impl CheckDef {
    /// The first missing need's sentence, or `None` when the check can run.
    pub fn not_run_reason(&self, available: Available) -> Option<&'static str> {
        self.needs
            .iter()
            .find(|need| !available.has(**need))
            .map(|need| need.reason())
    }
}

/// The most a site could lose: the severity of every check that can run for it.
pub fn severity_total(available: Available) -> u32 {
    CHECKS
        .iter()
        .filter(|check| check.not_run_reason(available).is_none())
        .map(|check| u32::from(check.severity))
        .sum()
}

pub fn all() -> &'static [CheckDef] {
    CHECKS
}

pub fn get(id: &str) -> Option<&'static CheckDef> {
    CHECKS.iter().find(|check| check.id == id)
}

const fn def(
    id: &'static str,
    category: Category,
    severity: u8,
    effort: u8,
    title: &'static str,
    why: &'static str,
    fix: &'static str,
) -> CheckDef {
    CheckDef {
        id,
        category,
        severity,
        effort,
        title,
        why,
        fix,
        needs: &[],
    }
}

const fn needing(check: CheckDef, needs: &'static [Need]) -> CheckDef {
    CheckDef { needs, ..check }
}

use Category::{AiReadiness as AI, Content as C, Technical as T};

static CHECKS: &[CheckDef] = &[
    def(
        "broken_page", T, 3, 2,
        "Pages that return an error",
        "These pages did not load, so visitors and search engines hit a dead end.",
        "Restore the page, or redirect its address to the closest working page.",
    ),
    def(
        "broken_internal_link", T, 3, 1,
        "Pages linking to broken pages",
        "These pages send visitors to addresses on your site that show an error.",
        "Edit each link to point at a working page, or remove it.",
    ),
    def(
        "redirect_chain", T, 1, 1,
        "Pages reached through several redirects",
        "Each extra redirect slows the page down and can lose visitors on the way.",
        "Point links and redirects straight at the final address.",
    ),
    def(
        "title_missing", T, 3, 1,
        "Pages with no title",
        "The title is the headline people see in search results and browser tabs, and these pages have none.",
        "Give each page a short title that says what the page is about.",
    ),
    def(
        "title_duplicate", T, 2, 2,
        "Pages sharing the same title",
        "When pages share a title, people and search engines cannot tell them apart.",
        "Write a different title for each page that describes what is unique about it.",
    ),
    def(
        "title_length", T, 1, 1,
        "Titles that are too short or too long",
        "Very short titles say little, and long ones are cut off in search results.",
        "Aim for a title of about 10 to 65 characters that puts the main idea first.",
    ),
    def(
        "description_missing", T, 2, 1,
        "Pages with no meta description",
        "Without a description, search engines pick their own snippet, which may not say what you want.",
        "Add a meta description of one or two sentences that tells a reader why to visit.",
    ),
    def(
        "description_duplicate", T, 1, 2,
        "Pages sharing the same description",
        "Identical descriptions give no reason to choose one page over another.",
        "Write a description specific to each page.",
    ),
    def(
        "description_length", T, 1, 1,
        "Descriptions that are too short or too long",
        "Short descriptions say little, and long ones are cut off in search results.",
        "Aim for a description of about 50 to 160 characters.",
    ),
    def(
        "h1_missing", T, 2, 1,
        "Pages with no main heading",
        "The main heading tells readers and machines what the page is about, and these pages have none.",
        "Add one clear main heading (H1) near the top of the page.",
    ),
    def(
        "h1_multiple", T, 1, 1,
        "Pages with more than one main heading",
        "Several main headings blur what the page is mainly about.",
        "Keep one main heading (H1) and turn the others into smaller headings.",
    ),
    def(
        "canonical_broken", T, 2, 2,
        "Pages whose canonical address is broken",
        "The canonical address tells search engines which page to keep, and here it points at a page that shows an error.",
        "Change the canonical address to a working page, usually the page itself.",
    ),
    def(
        "noindex_in_sitemap", T, 3, 1,
        "Pages in the sitemap that tell search engines to ignore them",
        "The sitemap asks search engines to list these pages while the pages ask not to be listed. They are told two opposite things.",
        "Decide which is right: remove the noindex instruction, or take the page out of the sitemap.",
    ),
    def(
        "sitemap_missing", T, 2, 2,
        "No sitemap found",
        "A sitemap is a list of your pages that helps search engines and AI tools find all of them.",
        "Publish a sitemap at /sitemap.xml and list it in your robots.txt file.",
    ),
    def(
        "not_in_sitemap", T, 1, 2,
        "Working pages missing from the sitemap",
        "These pages load fine but are not in your sitemap, so they may be found late or not at all.",
        "Add these pages to the sitemap, or make the sitemap generate itself from your pages.",
    ),
    def(
        "image_alt_missing", T, 1, 2,
        "Images with no alt text",
        "Alt text describes an image for people who cannot see it and for search engines and AI tools that read the page.",
        "Add a short alt description to each meaningful image. Decorative images can have an empty one.",
    ),
    def(
        "viewport_missing", T, 2, 1,
        "Pages with no mobile viewport setting",
        "Without it, phones show a shrunken desktop page that is hard to read.",
        "Add the viewport meta tag to the page template.",
    ),
    def(
        "not_https", T, 3, 2,
        "Pages served without HTTPS",
        "Browsers warn visitors about pages that are not secure, and many will turn away.",
        "Install a certificate and redirect every http address to its https version.",
    ),
    def(
        "slow_response", T, 2, 3,
        "Pages that respond slowly",
        "These pages took longer than a second and a half to answer, and slow pages lose visitors.",
        "Ask your developer or host to look at caching, image sizes and server speed for these pages.",
    ),
    def(
        "heavy_page", T, 1, 3,
        "Pages with very large HTML",
        "The page code alone is over 2 MB, which is slow to download and hard for crawlers to read in full.",
        "Cut repeated or unused markup, and split very long pages into several.",
    ),
    def(
        "ai_crawler_blocked", AI, 3, 1,
        "AI crawlers that robots.txt turns away",
        "Your robots.txt file tells this AI crawler to stay out, so its product cannot read your pages or quote them.",
        "If you want to be readable by this crawler, remove or loosen its Disallow rule in robots.txt.",
    ),
    def(
        "needs_javascript", AI, 3, 3,
        "Pages with almost no text before JavaScript runs",
        "These pages send very little text and rely on scripts. Many search and AI crawlers do not run scripts, so they may see a nearly empty page.",
        "Open the page with JavaScript turned off. If the main text is missing, ask your developer to send it in the page itself, using server-side or pre-built pages.",
    ),
    def(
        "structured_data_missing", AI, 1, 2,
        "Pages with no structured data",
        "Structured data is a hidden label that states plainly what a page is, such as an article, a product or an organisation.",
        "Add JSON-LD structured data that fits the page, such as Organization, Article, FAQ or Product.",
    ),
    def(
        "structured_data_invalid", AI, 2, 1,
        "Pages with broken structured data",
        "The structured data on these pages has a syntax error, so machines ignore it.",
        "Fix the JSON-LD so it is valid. A free validator such as the Schema.org validator shows where it breaks.",
    ),
    def(
        "date_missing", AI, 1, 1,
        "Articles with no visible date",
        "Readers and AI tools use the published or updated date to judge whether a page is current, and these pages show none.",
        "Show a published date, and an updated date when you revise the page.",
    ),
    def(
        "author_missing", AI, 1, 1,
        "Articles with no author",
        "Naming who wrote a page helps readers and AI tools judge whether to trust it, and these pages name nobody.",
        "Add the author's name, ideally linked to a short profile page.",
    ),
    def(
        "thin_content", C, 2, 3,
        "Pages with very little text",
        "These pages have fewer than 150 words, which is rarely enough to answer a question or stand apart from other pages.",
        "Expand the page with what a visitor needs to know, or merge it into a stronger page.",
    ),
    def(
        "duplicate_content", C, 2, 2,
        "Pages with identical text",
        "When several pages carry the same text, search engines and AI tools pick one and may ignore the rest.",
        "Rewrite the pages so each is distinct, or merge them and redirect the extra addresses.",
    ),
    needing(def(
        "keyword_not_in_title", C, 2, 1,
        "Pages whose title leaves out a target keyword",
        "The title is the strongest hint of what a page is about, and it does not use the words of a keyword this page is meant to be found for.",
        "Work the keyword into the title in a natural way, near the start if it reads well.",
    ), KEYWORDS),
    needing(def(
        "keyword_not_in_h1", C, 2, 1,
        "Pages whose main heading leaves out a target keyword",
        "The main heading tells readers what the page is about, and it does not use the words of a keyword this page is meant to be found for.",
        "Rewrite the main heading so it says the keyword in plain words.",
    ), KEYWORDS),
    needing(def(
        "keyword_not_in_description", C, 1, 1,
        "Pages whose description leaves out a target keyword",
        "Search engines often show the meta description in bold around the words people searched for, and this one does not use the keyword.",
        "Rewrite the description so it mentions the keyword and says why to visit.",
    ), KEYWORDS),
    needing(def(
        "keyword_not_in_body", C, 2, 2,
        "Pages whose text never uses a target keyword",
        "The text of the page does not contain the words of a keyword it is meant to be found for, so it may not look like an answer to that search.",
        "Cover the topic in the page text and use the keyword where it fits naturally.",
    ), KEYWORDS),
    needing(def(
        "keyword_stuffing", C, 2, 1,
        "Pages that repeat a target keyword too often",
        "The keyword makes up more than 3% of the page's words, or is repeated in the title. Repetition like this reads badly and search engines may discount it.",
        "Cut repeats and use synonyms or plain pronouns. Say the keyword once or twice per section at most.",
    ), KEYWORDS),
    needing(def(
        "keyword_cannibalization", C, 2, 3,
        "Keywords where several of your pages compete",
        "Search Console shows two or more of your pages each getting a real share of the searches for one keyword, so they split the audience between them.",
        "Pick the page that should answer the keyword. Merge the others into it, or rewrite them to target different searches.",
    ), SEARCH_KEYWORDS),
    needing(def(
        "search_low_ctr", T, 2, 1,
        "Pages that get few clicks for where they rank",
        "These pages show up in Google Search but fewer than half as many people click them as usually do at their position, so the title or description may not be doing its job.",
        "Rewrite the title and description to say plainly what the page offers and why to click.",
    ), SEARCH),
    needing(def(
        "search_striking_distance", T, 2, 2,
        "Searches where a page ranks just off the first page",
        "These pages average between position 5 and 20 for a search that shows them often, close enough that a better answer could move them up.",
        "Improve the page for that search: answer it more fully, use its words in the title and headings, and link to the page from related pages.",
    ), SEARCH),
    needing(def(
        "search_no_impressions", T, 1, 2,
        "Pages with no Google search impressions in 28 days",
        "Search Console reports no impressions for these sitemap pages in the last 28 days. This is an approximation: new pages and very specific pages may simply not have been shown yet.",
        "Check the page is linked from other pages, is not blocked, and answers a question people search for. Merge or remove it if it does not.",
    ), SEARCH),
    needing(def(
        "ai_not_mentioned", AI, 2, 3,
        "Questions where AI assistants do not mention or cite your site",
        "In the latest AI visibility check, none of the assistants asked this question mentioned your site or cited one of its pages in their answer. This is a sample of what assistants said on one day, and answers change from run to run.",
        "Read the answers to see which sites were cited instead. Make sure a page on your site answers this question plainly, with the facts people look for, and that it is easy to find from your other pages.",
    ), AI_RUN),
    needing(def(
        "speed_slow_lcp", T, 2, 3,
        "Pages where the main content loads slowly",
        "On a phone, the largest piece of content on these pages takes more than 2.5 seconds to appear, according to Google PageSpeed Insights. Slow pages lose visitors.",
        "Compress and resize the largest image, remove scripts that block the page from drawing, and ask your host about caching.",
    ), SPEED),
    needing(def(
        "speed_layout_shift", T, 2, 2,
        "Pages where the layout jumps while loading",
        "Google PageSpeed Insights measures a layout shift above 0.1 on a phone, which means content moves around as the page loads and visitors click the wrong thing.",
        "Give images, ads and embeds a set width and height, and avoid adding content above what is already showing.",
    ), SPEED),
    needing(def(
        "speed_slow_interaction", T, 2, 3,
        "Pages that react slowly to taps and clicks",
        "On a phone, these pages take too long to respond to a tap. This uses Google's real-visitor figure for interaction (over 200 ms) when there is one, and otherwise the lab test's blocking time (over 300 ms).",
        "Cut or delay scripts you do not need on first load, and split long-running scripts into smaller pieces.",
    ), SPEED),
    needing(def(
        "rank_dropped", T, 2, 2,
        "Keywords that dropped in Google's results",
        "In the latest check of Google's results, these keywords fell 5 or more places since the check before, or left the top 100. Rankings move for many reasons, and this is a sample from the days you checked.",
        "Open the page that used to rank and see what changed on it or on the sites above it. Check that it still loads, is not blocked from search, and still answers the search plainly.",
    ), RANKINGS),
    needing(def(
        "ai_overview_not_cited", AI, 2, 3,
        "Keywords where Google's AI Overview cites other sites, not yours",
        "For these keywords Google showed an AI Overview in the latest check, and it linked to other sites and not to yours. Overviews change from search to search, so this is a sample, not a measure.",
        "Look at which sites the overview cites and what they cover that your page does not. Give the page a short, direct answer near the top, with the facts and sources people look for.",
    ), RANKINGS),
    def(
        "answer_no_question_headings", AI, 1, 2,
        "Pages with no heading phrased as a question",
        "People ask AI assistants questions, and a heading that asks the question in plain words makes it easier for an answer to quote the text under it.",
        "Turn one or two section headings into the questions your readers ask, and answer each one directly below it.",
    ),
    def(
        "answer_long_opening", AI, 1, 1,
        "Pages that open with a long first paragraph",
        "The first paragraph is longer than 80 words, so there is no short direct answer near the top for an AI answer to quote.",
        "Start with a one- or two-sentence answer, then add the detail below it.",
    ),
    def(
        "answer_no_lists_or_tables", AI, 1, 2,
        "Long pages with no list or table",
        "Steps, comparisons and facts laid out as a list or table are easier for an AI answer to pick up than the same facts buried in long paragraphs.",
        "Where the text compares options or gives steps, set it out as a list or a table.",
    ),
    def(
        "answer_no_sources", AI, 1, 2,
        "Long pages that link to no outside source",
        "The page makes its case without pointing to any other site. Linking to the studies or official pages behind a claim helps readers and AI tools judge whether to trust it.",
        "Link the main facts and figures to the original source.",
    ),
    def(
        "schema_no_article", AI, 2, 2,
        "Articles with no Article structured data",
        "These pages show a date or an author but do not label themselves as an article in structured data, so machines have to guess what they are.",
        "Add Article or BlogPosting JSON-LD with the headline, date and author.",
    ),
    def(
        "schema_no_organization", AI, 2, 1,
        "Home page with no Organization or WebSite structured data",
        "Structured data on the home page states plainly who runs the site, which helps search engines and AI tools connect your pages to one name.",
        "Add Organization (or WebSite) JSON-LD to the home page with your name, address and logo.",
    ),
    def(
        "schema_faq_missing", AI, 1, 2,
        "Pages with several questions but no FAQ structured data",
        "These pages ask three or more questions in their headings, but the structured data does not say they are a list of questions and answers.",
        "Add FAQPage JSON-LD that matches the questions and answers shown on the page.",
    ),
];

const KEYWORDS: &[Need] = &[Need::Keywords];
const SEARCH: &[Need] = &[Need::SearchConsole];
const SEARCH_KEYWORDS: &[Need] = &[Need::Keywords, Need::SearchConsole];
const AI_RUN: &[Need] = &[Need::AiRun];
const SPEED: &[Need] = &[Need::Speed];
const RANKINGS: &[Need] = &[Need::Rankings];
