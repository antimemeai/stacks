//! Stamp-in bonafides: is a sha256 in the library catalog ready to be
//! stamped into the corpus without any further extraction/enrichment?
//! The answer is a per-check struct, not a bare bool — the drainer records
//! the failed checks as the queue `reason` (and later as the DLQ reason).

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::error::StoreError;

/// Ported from neurotic_library/scripts/catalog/core.py `is_junk_title`.
const JUNK_TITLE_EXACT: &[&str] = &[
    "untitled",
    "title",
    "fm",
    "doc",
    "cover",
    "acknowledgments",
    "acknowledgements",
    "introduction",
    "applications",
    "references",
    "appendix",
    "abstract",
    "preface",
    "contents",
    "index",
    "bibliography",
    "tiff",
    ".tiff",
    "chapter",
];

const JUNK_TITLE_SUBSTRINGS: &[&str] = &[
    "microsoft word",
    ".doc",
    ".tiff",
    "tardir",
    "fm 1..",
    "fm3",
    "tt78",
    "tt94",
    "tt107",
    "tt108",
    "tt63",
    "javascript is disabled",
    "skip to main content",
    "please enable javascript",
];

/// Character-quality gate: PDF metadata/first-page heuristics often yield
/// non-empty strings that are binary garbage (control chars, U+FFFD
/// replacement chars, mostly punctuation). Those pass the length and word
/// rules below but are not titles. A plausible title is almost all
/// printable, majority-alphabetic text.
pub fn title_is_plausible(title: &str) -> bool {
    let t = title.trim();
    if t.len() < 15 || t.split_whitespace().count() < 3 {
        return false;
    }
    let mut bad = 0usize;
    let mut alpha = 0usize;
    let mut total = 0usize;
    for c in t.chars() {
        if c.is_whitespace() {
            continue;
        }
        total += 1;
        if c.is_control() || c == '\u{FFFD}' {
            bad += 1;
        }
        if c.is_alphabetic() {
            alpha += 1;
        }
    }
    if bad >= 3 || (bad > 0 && bad * 100 >= total) {
        return false;
    }
    alpha >= 8 && alpha * 2 >= total
}

/// Same rules as the Python original: empty/short/few-word titles are junk,
/// as are exact matches and known-junk substrings (all case-insensitive).
/// Binary-garbage character mixes are junk too ([`title_is_plausible`]).
pub fn is_junk_title(title: Option<&str>) -> bool {
    let Some(title) = title else {
        return true;
    };
    let t = title.trim();
    if !title_is_plausible(t) {
        return true;
    }
    let low = t.to_lowercase();
    if JUNK_TITLE_EXACT.contains(&low.as_str()) {
        return true;
    }
    JUNK_TITLE_SUBSTRINGS.iter().any(|junk| low.contains(junk))
}

/// One bonafides verdict. `ready` is derived, never stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bonafides {
    /// A `paper` row exists for the sha256.
    pub paper_exists: bool,
    /// Title is present and not junk ([`is_junk_title`]).
    pub title_ok: bool,
    /// doi OR arxiv_id present.
    pub has_identifier: bool,
    /// Enrichment exists and openalex_topics has >= 1 topic with a subfield.
    pub topics_with_subfield: bool,
    /// Enrichment s2_tldr present.
    pub s2_tldr_present: bool,
    /// Enrichment unpaywall_oa_status present.
    pub unpaywall_status_present: bool,
}

impl Bonafides {
    pub fn is_ready(&self) -> bool {
        self.paper_exists
            && self.title_ok
            && self.has_identifier
            && self.topics_with_subfield
            && self.s2_tldr_present
            && self.unpaywall_status_present
    }

    /// Names of the failed checks, for the queue `reason` payload.
    pub fn failed_checks(&self) -> Vec<&'static str> {
        let mut failed = Vec::new();
        if !self.paper_exists {
            failed.push("paper_exists");
        }
        if !self.title_ok {
            failed.push("title_ok");
        }
        if !self.has_identifier {
            failed.push("has_identifier");
        }
        if !self.topics_with_subfield {
            failed.push("topics_with_subfield");
        }
        if !self.s2_tldr_present {
            failed.push("s2_tldr_present");
        }
        if !self.unpaywall_status_present {
            failed.push("unpaywall_status_present");
        }
        failed
    }
}

fn present(s: &Option<String>) -> bool {
    s.as_deref().map(str::trim).is_some_and(|s| !s.is_empty())
}

/// Run all bonafides checks for one sha256 against paper + paper_enrichment.
pub fn check_bonafides(conn: &Connection, sha256: &str) -> Result<Bonafides, StoreError> {
    let paper: Option<(Option<String>, Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT title, doi, arxiv_id FROM paper WHERE sha256 = ?1",
            params![sha256],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok();
    let enrichment: Option<(Option<String>, Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT openalex_topics, s2_tldr, unpaywall_oa_status
             FROM paper_enrichment WHERE sha256 = ?1",
            params![sha256],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok();

    let (title_ok, has_identifier) = match &paper {
        Some((title, doi, arxiv_id)) => (
            !is_junk_title(title.as_deref()),
            present(doi) || present(arxiv_id),
        ),
        None => (false, false),
    };

    let (topics_with_subfield, s2_tldr_present, unpaywall_status_present) = match &enrichment {
        Some((topics, tldr, oa_status)) => (
            topics_have_subfield(topics.as_deref()),
            present(tldr),
            present(oa_status),
        ),
        None => (false, false, false),
    };

    Ok(Bonafides {
        paper_exists: paper.is_some(),
        title_ok,
        has_identifier,
        topics_with_subfield,
        s2_tldr_present,
        unpaywall_status_present,
    })
}

/// openalex_topics is a JSON array of `{name, score, subfield}` objects.
fn topics_have_subfield(topics: Option<&str>) -> bool {
    let Some(topics) = topics else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(topics) else {
        return false;
    };
    value.as_array().is_some_and(|topics| {
        topics.iter().any(|t| {
            t.get("subfield")
                .and_then(|s| s.as_str())
                .is_some_and(|s| !s.trim().is_empty())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{insert_enrichment, insert_paper, LibraryPaper, LibraryStore, PaperEnrichment};

    fn paper(sha256: &str, title: Option<&str>, doi: Option<&str>, arxiv: Option<&str>) -> LibraryPaper {
        let mut p = paper_default(sha256);
        p.title = title.map(str::to_string);
        p.doi = doi.map(str::to_string);
        p.arxiv_id = arxiv.map(str::to_string);
        p
    }

    fn paper_default(sha256: &str) -> LibraryPaper {
        serde_json::from_value(serde_json::json!({
            "sha256": sha256,
            "filename": format!("{sha256}.pdf"),
        }))
        .unwrap()
    }

    fn enrichment(sha256: &str, topics: Option<&str>, tldr: Option<&str>, oa: Option<&str>) -> PaperEnrichment {
        serde_json::from_value(serde_json::json!({
            "sha256": sha256,
            "openalex_topics": topics,
            "s2_tldr": tldr,
            "unpaywall_oa_status": oa,
        }))
        .unwrap()
    }

    const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const GOOD_TITLE: &str = "Synthesis and characterization of novel perovskite oxides";
    const GOOD_TOPICS: &str = r#"[{"name": "Perovskites", "score": 0.9, "subfield": "Materials Chemistry"}]"#;

    fn ready_store() -> LibraryStore {
        let store = LibraryStore::open_in_memory().unwrap();
        insert_paper(store.raw(), &paper(SHA, Some(GOOD_TITLE), Some("10.1/x"), None)).unwrap();
        insert_enrichment(store.raw(), &enrichment(SHA, Some(GOOD_TOPICS), Some("a tldr"), Some("gold"))).unwrap();
        store
    }

    #[test]
    fn junk_title_binary_garbage_rejected() {
        // Production examples: PDF metadata "titles" that are binary junk —
        // control chars, U+FFFD replacement chars, mostly punctuation.
        let garbage = "]t\u{FFFD}\u{FFFD}F U \u{FFFD}M\u{3}\u{FFFD}\u{FFFD}\u{C2}\u{C2}A\u{FFFD}P\u{FFFD}O t Wk qrz zzz";
        assert!(is_junk_title(Some(garbage)));
        assert!(!title_is_plausible(garbage));
        assert!(is_junk_title(Some("\u{0}\u{1}\u{2} not a real title at all")));
        // Mostly punctuation/digits is not a title either.
        assert!(is_junk_title(Some("1234 5678 9012 3456 7890 12")));
        // Clean titles — including digit-heavy and symbol-bearing ones — pass.
        assert!(title_is_plausible(GOOD_TITLE));
        assert!(!is_junk_title(Some(GOOD_TITLE)));
        assert!(!is_junk_title(Some("2003-2008 World Outlook for Defense Industry Equipment")));
        assert!(!is_junk_title(Some("Logistics 4.0 and the impact of IoT on supply chains")));
    }

    #[test]
    fn junk_title_rules() {
        assert!(is_junk_title(None));
        assert!(is_junk_title(Some("")));
        assert!(is_junk_title(Some("   ")));
        assert!(is_junk_title(Some("short title"))); // < 15 chars
        assert!(is_junk_title(Some("abcdefghij klmno"))); // >= 15 chars but < 3 words
        assert!(is_junk_title(Some("Untitled"))); // exact, case-insensitive
        assert!(is_junk_title(Some("Microsoft Word - something.doc"))); // substring
        assert!(is_junk_title(Some("skip to main content here please"))); // substring
        assert!(is_junk_title(Some("a file with .tiff in the name"))); // substring
        assert!(is_junk_title(Some("Please enable JavaScript to view this page properly")));
        assert!(!is_junk_title(Some(GOOD_TITLE)));
        // 'references' is only an exact match, not a substring:
        assert!(!is_junk_title(Some("References to prior art in ceramic synthesis methods")));
    }

    #[test]
    fn bonafides_all_pass() {
        let store = ready_store();
        let b = check_bonafides(store.raw(), SHA).unwrap();
        assert!(b.is_ready());
        assert!(b.failed_checks().is_empty());
    }

    #[test]
    fn bonafides_missing_paper() {
        let store = LibraryStore::open_in_memory().unwrap();
        let b = check_bonafides(store.raw(), SHA).unwrap();
        assert!(!b.is_ready());
        assert!(b.failed_checks().contains(&"paper_exists"));
    }

    #[test]
    fn bonafides_junk_title_fails() {
        let store = LibraryStore::open_in_memory().unwrap();
        insert_paper(store.raw(), &paper(SHA, Some("untitled document"), Some("10.1/x"), None)).unwrap();
        insert_enrichment(store.raw(), &enrichment(SHA, Some(GOOD_TOPICS), Some("a tldr"), Some("gold"))).unwrap();
        let b = check_bonafides(store.raw(), SHA).unwrap();
        assert!(b.failed_checks() == ["title_ok"]);
    }

    #[test]
    fn bonafides_identifier_either_or() {
        let store = ready_store();
        let sha_b = format!("{SHA}").replace('a', "b");
        insert_paper(store.raw(), &paper(&sha_b, Some(GOOD_TITLE), None, Some("2301.00001"))).unwrap();
        insert_enrichment(store.raw(), &enrichment(&sha_b, Some(GOOD_TOPICS), Some("t"), Some("closed"))).unwrap();
        let b = check_bonafides(store.raw(), &sha_b).unwrap();
        assert!(b.has_identifier);

        let sha_c = format!("{SHA}").replace('a', "c");
        insert_paper(store.raw(), &paper(&sha_c, Some(GOOD_TITLE), None, None)).unwrap();
        let b = check_bonafides(store.raw(), &sha_c).unwrap();
        assert!(b.failed_checks().contains(&"has_identifier"));
    }

    #[test]
    fn bonafides_enrichment_checks() {
        let store = LibraryStore::open_in_memory().unwrap();
        insert_paper(store.raw(), &paper(SHA, Some(GOOD_TITLE), Some("10.1/x"), None)).unwrap();
        // No enrichment row at all.
        let b = check_bonafides(store.raw(), SHA).unwrap();
        assert!(b.failed_checks() == ["topics_with_subfield", "s2_tldr_present", "unpaywall_status_present"]);

        // Topics without subfield.
        insert_enrichment(store.raw(), &enrichment(SHA, Some(r#"[{"name": "x", "score": 0.1}]"#), Some("t"), Some("gold"))).unwrap();
        let b = check_bonafides(store.raw(), SHA).unwrap();
        assert!(b.failed_checks() == ["topics_with_subfield"]);
    }
}
