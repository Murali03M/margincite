//! The codes DataForSEO wants for the countries and languages a site can be
//! tracked in. The lists a person chooses from live in `sites::locale`; every
//! entry there has a code here (a test holds them together).

/// ISO 3166 alpha-2 country and DataForSEO's `location_code`. For these
/// countries the code is the ISO numeric code plus 2000, and each one was
/// checked against DataForSEO's Google SERP and Google Ads location lists.
const LOCATIONS: &[(&str, u32)] = &[
    ("AR", 2032),
    ("AU", 2036),
    ("AT", 2040),
    ("BE", 2056),
    ("BR", 2076),
    ("CA", 2124),
    ("CL", 2152),
    ("CO", 2170),
    ("CZ", 2203),
    ("DK", 2208),
    ("EG", 2818),
    ("FI", 2246),
    ("FR", 2250),
    ("DE", 2276),
    ("GR", 2300),
    ("HK", 2344),
    ("HU", 2348),
    ("IN", 2356),
    ("ID", 2360),
    ("IE", 2372),
    ("IL", 2376),
    ("IT", 2380),
    ("JP", 2392),
    ("KE", 2404),
    ("MY", 2458),
    ("MX", 2484),
    ("NL", 2528),
    ("NZ", 2554),
    ("NG", 2566),
    ("NO", 2578),
    ("PK", 2586),
    ("PH", 2608),
    ("PL", 2616),
    ("PT", 2620),
    ("RO", 2642),
    ("SA", 2682),
    ("SG", 2702),
    ("ZA", 2710),
    ("KR", 2410),
    ("ES", 2724),
    ("SE", 2752),
    ("CH", 2756),
    ("TW", 2158),
    ("TH", 2764),
    ("TR", 2792),
    ("UA", 2804),
    ("AE", 2784),
    ("GB", 2826),
    ("US", 2840),
    ("VN", 2704),
];

/// The `location_code` for a country code such as `US`.
pub fn location_code(country: &str) -> Option<u32> {
    LOCATIONS
        .iter()
        .find(|(code, _)| *code == country)
        .map(|(_, location)| *location)
}

/// The `language_code` the Google SERP endpoint wants. Our `zh` is Simplified
/// Chinese, which that endpoint calls `zh-CN`.
pub fn serp_language(language: &str) -> &str {
    match language {
        "zh" => "zh-CN",
        other => other,
    }
}

/// The `language_code` the Google Ads keyword endpoints want. Assumed from
/// DataForSEO's naming; not confirmed against their Ads language list.
pub fn ads_language(language: &str) -> &str {
    match language {
        "zh" => "zh_CN",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sites::locale::{COUNTRIES, LANGUAGES};

    #[test]
    fn every_country_a_site_can_choose_has_a_location_code() {
        for (code, name) in COUNTRIES {
            assert!(location_code(code).is_some(), "no location code for {name}");
        }
        assert_eq!(LOCATIONS.len(), COUNTRIES.len(), "a code without a country");
    }

    #[test]
    fn the_documented_codes_are_right() {
        for (country, code) in [
            ("US", 2840),
            ("IN", 2356),
            ("GB", 2826),
            ("CA", 2124),
            ("AU", 2036),
            ("DE", 2276),
            ("FR", 2250),
            ("SG", 2702),
            ("AE", 2784),
            ("NL", 2528),
            ("HK", 2344),
            ("TW", 2158),
        ] {
            assert_eq!(location_code(country), Some(code), "{country}");
        }
        assert_eq!(location_code("XX"), None);
    }

    #[test]
    fn every_language_a_site_can_choose_has_a_search_code() {
        for (code, name) in LANGUAGES {
            assert!(!serp_language(code).is_empty(), "{name}");
            assert!(!ads_language(code).is_empty(), "{name}");
        }
        assert_eq!(serp_language("zh"), "zh-CN");
        assert_eq!(serp_language("en"), "en");
        assert_eq!(ads_language("zh"), "zh_CN");
        assert_eq!(ads_language("hi"), "hi");
    }
}
