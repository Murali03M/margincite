//! Checks that read PageSpeed Insights results. They run only when the site
//! has some.

use serde_json::json;

use super::context::{CrawlContext, Finding};

const SLOW_LCP_MS: i32 = 2500;
const SHIFT_CLS: f64 = 0.1;
const SLOW_INP_MS: i32 = 200;
const SLOW_TBT_MS: i32 = 300;

pub fn run(ctx: &CrawlContext) -> Vec<Finding> {
    let Some(speed) = ctx.speed.as_ref() else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for page in &speed.pages {
        if let Some(lcp) = page.lcp_ms.filter(|lcp| *lcp > SLOW_LCP_MS) {
            found.push(Finding::page(
                "speed_slow_lcp",
                &page.url,
                json!({"lcp_ms": lcp}),
            ));
        }
        if let Some(cls) = page.cls.filter(|cls| *cls > SHIFT_CLS) {
            found.push(Finding::page(
                "speed_layout_shift",
                &page.url,
                json!({"cls": cls}),
            ));
        }
        // Interaction is judged on field INP when there is one, else on lab
        // blocking time.
        match (page.inp_ms, page.tbt_ms) {
            (Some(inp), _) if inp > SLOW_INP_MS => found.push(Finding::page(
                "speed_slow_interaction",
                &page.url,
                json!({"inp_ms": inp}),
            )),
            (None, Some(tbt)) if tbt > SLOW_TBT_MS => found.push(Finding::page(
                "speed_slow_interaction",
                &page.url,
                json!({"tbt_ms": tbt}),
            )),
            _ => {}
        }
    }
    found
}

#[cfg(test)]
#[path = "speed_tests.rs"]
mod tests;
