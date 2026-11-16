//! What DataForSEO charges, for the usage screen's estimates. From
//! DataForSEO's pricing pages (Live mode): a Google organic result page of up
//! to 10 results costs $0.002, a request is billed per page asked for, the AI
//! Overview option adds one more page's price, and every Google Ads keyword
//! request costs $0.09 whatever the number of keywords. These are estimates:
//! the account's own billing is the truth.

/// One Google organic request for 100 results with the AI Overview option:
/// 10 pages of $0.002, plus $0.002.
pub const SERP_REQUEST_USD: f64 = 0.022;

/// One Google Ads search volume or keyword ideas request.
pub const ADS_REQUEST_USD: f64 = 0.09;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrations::dataforseo::client::SERP_DEPTH;

    #[test]
    fn a_serp_request_costs_its_pages_plus_the_overview_option() {
        let page = 0.002;
        let pages = f64::from(SERP_DEPTH / 10);
        assert!((SERP_REQUEST_USD - (pages * page + page)).abs() < 1e-9);
    }
}
