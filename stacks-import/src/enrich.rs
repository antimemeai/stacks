//! Blocking HTTP enrichment clients (CrossRef, OpenAlex, arXiv, Semantic
//! Scholar, Unpaywall) mapped into the paper / paper_enrichment column
//! shapes NL wrote. All network access sits behind the [`EnrichClient`]
//! trait so the pipeline is testable offline; [`HttpEnricher`] is the live
//! implementation (ureq). No secrets in logs: the S2 key only ever travels
//! in the x-api-key header.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

const DEFAULT_CONTACT: &str = "hiya@antimeme.ai";
/// Max identity-resolution API attempts per item before the DLQ (NL's rule).
pub const MAX_IDENTITY_ATTEMPTS: u32 = 3;
const BATCH_SIZE: usize = 50;

/// Polite-pool contact email (CrossRef mailto, Unpaywall email, UA).
pub fn contact_email() -> String {
    std::env::var("STACKS_CONTACT_EMAIL").unwrap_or_else(|_| DEFAULT_CONTACT.to_string())
}

/// S2 key resolution: env S2_API_KEY, then /srv/stacks/post_it.txt
/// (`S2_API_KEY=s2k-...` line), then the unauthenticated slow lane.
/// The value is never logged anywhere.
pub fn resolve_s2_key() -> Option<String> {
    if let Ok(key) = std::env::var("S2_API_KEY") {
        if !key.trim().is_empty() {
            return Some(key.trim().to_string());
        }
    }
    let postit = std::path::Path::new("/srv/stacks/post_it.txt");
    if let Ok(text) = std::fs::read_to_string(postit) {
        for line in text.lines() {
            if let Some(value) = line.trim().strip_prefix("S2_API_KEY=") {
                let value = value.trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

/// paper-table-shaped bibliographic fields (JSON shapes exactly as NL wrote
/// them: authors = [{"given","family"}]).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BiblioMeta {
    pub title: Option<String>,
    pub authors: Option<String>,
    pub year: Option<i64>,
    pub journal: Option<String>,
    #[serde(rename = "abstract")]
    pub abstract_: Option<String>,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
}

impl BiblioMeta {
    pub fn merge_missing(&mut self, other: &BiblioMeta) {
        if self.title.is_none() {
            self.title = other.title.clone();
        }
        if self.authors.is_none() {
            self.authors = other.authors.clone();
        }
        if self.year.is_none() {
            self.year = other.year;
        }
        if self.journal.is_none() {
            self.journal = other.journal.clone();
        }
        if self.abstract_.is_none() {
            self.abstract_ = other.abstract_.clone();
        }
        if self.doi.is_none() {
            self.doi = other.doi.clone();
        }
        if self.arxiv_id.is_none() {
            self.arxiv_id = other.arxiv_id.clone();
        }
    }
}

/// paper_enrichment-table-shaped fields (topics = [{"name","score",
/// "subfield"}] max 5, concepts = [{"name","score"}] max 10,
/// s2_fields_of_study = [{"category","source"}] — NL's exact JSON shapes).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EnrichmentMeta {
    pub openalex_id: Option<String>,
    pub openalex_topics: Option<String>,
    pub openalex_concepts: Option<String>,
    pub openalex_cited_by: Option<i64>,
    pub s2_paper_id: Option<String>,
    pub s2_tldr: Option<String>,
    pub s2_fields_of_study: Option<String>,
    pub s2_influential_citation_count: Option<i64>,
    pub unpaywall_oa_status: Option<String>,
    pub unpaywall_oa_url: Option<String>,
}

impl EnrichmentMeta {
    pub fn merge_missing(&mut self, other: &EnrichmentMeta) {
        if self.openalex_id.is_none() {
            self.openalex_id = other.openalex_id.clone();
        }
        if self.openalex_topics.is_none() {
            self.openalex_topics = other.openalex_topics.clone();
        }
        if self.openalex_concepts.is_none() {
            self.openalex_concepts = other.openalex_concepts.clone();
        }
        if self.openalex_cited_by.is_none() {
            self.openalex_cited_by = other.openalex_cited_by;
        }
        if self.s2_paper_id.is_none() {
            self.s2_paper_id = other.s2_paper_id.clone();
        }
        if self.s2_tldr.is_none() {
            self.s2_tldr = other.s2_tldr.clone();
        }
        if self.s2_fields_of_study.is_none() {
            self.s2_fields_of_study = other.s2_fields_of_study.clone();
        }
        if self.s2_influential_citation_count.is_none() {
            self.s2_influential_citation_count = other.s2_influential_citation_count;
        }
        if self.unpaywall_oa_status.is_none() {
            self.unpaywall_oa_status = other.unpaywall_oa_status.clone();
        }
        if self.unpaywall_oa_url.is_none() {
            self.unpaywall_oa_url = other.unpaywall_oa_url.clone();
        }
    }
}

/// One OpenAlex work, mapped. Title-search hits also carry doi/title.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OpenAlexMapped {
    pub openalex_id: Option<String>,
    pub doi: Option<String>,
    pub title: Option<String>,
    pub topics_json: Option<String>,
    pub concepts_json: Option<String>,
    pub cited_by: Option<i64>,
}

impl OpenAlexMapped {
    fn into_parts(self) -> (BiblioMeta, EnrichmentMeta) {
        (
            BiblioMeta {
                title: self.title,
                doi: self.doi,
                ..Default::default()
            },
            EnrichmentMeta {
                openalex_id: self.openalex_id,
                openalex_topics: self.topics_json,
                openalex_concepts: self.concepts_json,
                openalex_cited_by: self.cited_by,
                ..Default::default()
            },
        )
    }
}

/// Merged outcome of one identity-resolution ladder run.
#[derive(Debug, Clone, Default)]
pub struct FetchOutcome {
    pub biblio: BiblioMeta,
    pub enrichment: EnrichmentMeta,
    /// API calls actually made (identity resolution only).
    pub identity_attempts: u32,
    /// Which rung resolved the identity, for the reason/summary trail.
    pub resolved_via: Option<&'static str>,
}

/// The network seam. Every method is infallible at the trait level
/// (Option/empty results) — the pipeline treats None as "try the next rung".
pub trait EnrichClient {
    /// Batch OpenAlex lookup by DOI (OR-filter, <= 50 per request).
    /// Key: bare DOI as queried.
    fn openalex_batch(&self, dois: &[String]) -> HashMap<String, OpenAlexMapped>;
    fn openalex_doi(&self, doi: &str) -> Option<OpenAlexMapped>;
    fn openalex_title_search(&self, query: &str) -> Option<OpenAlexMapped>;
    fn crossref(&self, doi: &str) -> Option<BiblioMeta>;
    fn arxiv(&self, arxiv_id: &str) -> Option<BiblioMeta>;
    fn s2(&self, doi: &str) -> Option<EnrichmentMeta>;
    fn unpaywall(&self, doi: &str) -> Option<EnrichmentMeta>;
}

// ---------- pure mapping functions (tested against fixtures) ----------

pub fn map_crossref(msg: &serde_json::Value) -> BiblioMeta {
    let title = msg
        .get("title")
        .and_then(|t| t.as_array())
        .and_then(|a| a.first())
        .and_then(|t| t.as_str())
        .map(str::to_string);
    let authors = msg.get("author").and_then(|a| a.as_array()).and_then(|a| {
        if a.is_empty() {
            return None;
        }
        let v: Vec<serde_json::Value> = a
            .iter()
            .map(|a| {
                serde_json::json!({
                    "given": a.get("given").and_then(|g| g.as_str()).unwrap_or(""),
                    "family": a.get("family").and_then(|f| f.as_str()).unwrap_or(""),
                })
            })
            .collect();
        serde_json::to_string(&v).ok()
    });
    let year = ["published-print", "published-online", "issued", "created"]
        .iter()
        .find_map(|f| {
            msg.get(f)?
                .get("date-parts")?
                .as_array()?
                .first()?
                .as_array()?
                .first()?
                .as_i64()
        });
    let journal = msg
        .get("container-title")
        .and_then(|t| t.as_array())
        .and_then(|a| a.first())
        .and_then(|t| t.as_str())
        .map(str::to_string);
    let abstract_ = msg
        .get("abstract")
        .and_then(|a| a.as_str())
        .map(str::to_string);
    BiblioMeta {
        title,
        authors,
        year,
        journal,
        abstract_,
        ..Default::default()
    }
}

pub fn map_openalex_work(work: &serde_json::Value) -> Option<OpenAlexMapped> {
    let id = work.get("id").and_then(|i| i.as_str())?;
    let topics: Vec<serde_json::Value> = work
        .get("topics")
        .and_then(|t| t.as_array())
        .map(|ts| {
            ts.iter()
                .take(5)
                .map(|t| {
                    serde_json::json!({
                        "name": t.get("display_name").and_then(|v| v.as_str()),
                        "score": t.get("score").and_then(|v| v.as_f64()),
                        "subfield": t.get("subfield")
                            .and_then(|s| s.get("display_name"))
                            .and_then(|v| v.as_str()),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let concepts: Vec<serde_json::Value> = work
        .get("concepts")
        .and_then(|c| c.as_array())
        .map(|cs| {
            cs.iter()
                .take(10)
                .map(|c| {
                    serde_json::json!({
                        "name": c.get("display_name").and_then(|v| v.as_str()),
                        "score": c.get("score").and_then(|v| v.as_f64()),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Some(OpenAlexMapped {
        openalex_id: Some(id.to_string()),
        doi: work
            .get("doi")
            .and_then(|d| d.as_str())
            .map(|d| d.replace("https://doi.org/", "")),
        title: work
            .get("title")
            .and_then(|t| t.as_str())
            .map(str::to_string),
        topics_json: serde_json::to_string(&topics).ok(),
        concepts_json: serde_json::to_string(&concepts).ok(),
        cited_by: work.get("cited_by_count").and_then(|c| c.as_i64()),
    })
}

pub fn map_s2(paper: &serde_json::Value) -> EnrichmentMeta {
    let tldr = paper
        .get("tldr")
        .and_then(|t| {
            t.get("text")
                .and_then(|v| v.as_str())
                .or_else(|| t.as_str())
        })
        .map(str::to_string);
    let fos = paper
        .get("s2FieldsOfStudy")
        .and_then(|f| f.as_array())
        .and_then(|f| {
            if f.is_empty() {
                return None;
            }
            let v: Vec<serde_json::Value> = f
                .iter()
                .map(|f| {
                    serde_json::json!({
                        "category": f.get("category").and_then(|v| v.as_str()),
                        "source": f.get("source").and_then(|v| v.as_str()),
                    })
                })
                .collect();
            serde_json::to_string(&v).ok()
        });
    EnrichmentMeta {
        s2_paper_id: paper
            .get("paperId")
            .and_then(|p| p.as_str())
            .map(str::to_string),
        s2_tldr: tldr,
        s2_fields_of_study: fos,
        s2_influential_citation_count: paper
            .get("influentialCitationCount")
            .and_then(|c| c.as_i64()),
        ..Default::default()
    }
}

pub fn map_unpaywall(data: &serde_json::Value) -> EnrichmentMeta {
    let best = data.get("best_oa_location").cloned().unwrap_or_default();
    let oa_url = best
        .get("url_for_pdf")
        .and_then(|u| u.as_str())
        .or_else(|| best.get("url").and_then(|u| u.as_str()))
        .map(str::to_string);
    EnrichmentMeta {
        unpaywall_oa_status: data
            .get("oa_status")
            .and_then(|s| s.as_str())
            .map(str::to_string),
        unpaywall_oa_url: oa_url,
        ..Default::default()
    }
}

/// arXiv Atom response → biblio (title, authors, doi when the entry has
/// one). Parsed with regexes — no XML crate in the workspace.
pub fn map_arxiv_atom(xml: &str, arxiv_id: &str) -> Option<BiblioMeta> {
    use std::sync::OnceLock;
    static ENTRY: OnceLock<regex::Regex> = OnceLock::new();
    static TITLE: OnceLock<regex::Regex> = OnceLock::new();
    static DOI: OnceLock<regex::Regex> = OnceLock::new();
    static NAME: OnceLock<regex::Regex> = OnceLock::new();
    let entry_re = ENTRY
        .get_or_init(|| regex::Regex::new(r"(?s)<entry>(.*?)</entry>").unwrap());
    let entry = entry_re.captures(xml)?.get(1)?.as_str();
    let title_re = TITLE.get_or_init(|| {
        regex::Regex::new(r"(?s)<title>(.*?)</title>").unwrap()
    });
    let title = title_re
        .captures(entry)?
        .get(1)?
        .as_str()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let name_re = NAME.get_or_init(|| regex::Regex::new(r"<name>(.*?)</name>").unwrap());
    let authors: Vec<serde_json::Value> = name_re
        .captures_iter(entry)
        .map(|c| {
            let full = c.get(1).unwrap().as_str().trim();
            let (given, family) = match full.rsplit_once(' ') {
                Some((g, f)) => (g, f),
                None => ("", full),
            };
            serde_json::json!({"given": given, "family": family})
        })
        .collect();
    let doi_re = DOI.get_or_init(|| {
        regex::Regex::new(r"(?s)<(?:arxiv:)?doi[^>]*>(.*?)</(?:arxiv:)?doi>").unwrap()
    });
    let doi = doi_re
        .captures(entry)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string());
    Some(BiblioMeta {
        title: Some(title),
        authors: serde_json::to_string(&authors).ok(),
        doi,
        arxiv_id: Some(arxiv_id.to_string()),
        ..Default::default()
    })
}

/// NL's word-overlap guard for title-search hits: words of >= 4 chars,
/// intersection must reach max(1, 15% of query words).
pub fn title_overlap_ok(query: &str, candidate_title: &str) -> bool {
    use std::sync::OnceLock;
    static WORD: OnceLock<regex::Regex> = OnceLock::new();
    let word = WORD.get_or_init(|| regex::Regex::new(r"\w{4,}").unwrap());
    let words = |s: &str| {
        word.find_iter(&s.to_lowercase())
            .map(|m| m.as_str().to_string())
            .collect::<std::collections::HashSet<_>>()
    };
    let query_words = words(query);
    let title_words = words(candidate_title);
    if query_words.is_empty() {
        return false;
    }
    let overlap = query_words.intersection(&title_words).count();
    overlap >= 1.max((query_words.len() as f64 * 0.15) as usize)
}

// ---------- the live client ----------

pub struct HttpEnricher {
    agent: ureq::Agent,
    contact: String,
    s2_key: Option<String>,
    last_call: Mutex<Instant>,
}

impl Default for HttpEnricher {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpEnricher {
    pub fn new() -> Self {
        let contact = contact_email();
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(30))
            .user_agent(&format!("stacks-import/0.1 (mailto:{contact})"))
            .build();
        Self {
            agent,
            contact,
            s2_key: resolve_s2_key(),
            last_call: Mutex::new(Instant::now() - Duration::from_secs(10)),
        }
    }

    /// Serialize requests with a small per-provider delay.
    fn pace(&self, delay: Duration) {
        let mut last = self.last_call.lock().unwrap();
        let wait = delay.saturating_sub(last.elapsed());
        if !wait.is_zero() {
            std::thread::sleep(wait);
        }
        *last = Instant::now();
    }

    fn get_json(&self, url: &str, delay: Duration, auth_s2: bool) -> Option<serde_json::Value> {
        for attempt in 0..2 {
            self.pace(delay);
            let mut req = self.agent.get(url);
            if auth_s2 {
                if let Some(key) = &self.s2_key {
                    req = req.set("x-api-key", key);
                }
            }
            match req.call() {
                Ok(resp) => return resp.into_json().ok(),
                Err(ureq::Error::Status(404, _)) => return None,
                Err(ureq::Error::Status(429, _)) if attempt == 0 => {
                    std::thread::sleep(Duration::from_secs(10));
                }
                Err(_) => return None,
            }
        }
        None
    }
}

impl EnrichClient for HttpEnricher {
    fn openalex_batch(&self, dois: &[String]) -> HashMap<String, OpenAlexMapped> {
        let mut out = HashMap::new();
        for chunk in dois.chunks(BATCH_SIZE) {
            let filter = chunk
                .iter()
                .map(|d| format!("https://doi.org/{d}"))
                .collect::<Vec<_>>()
                .join("|");
            let url = format!(
                "https://api.openalex.org/works?filter=doi:{}&per-page={}&mailto={}",
                urlencoding(&filter),
                BATCH_SIZE,
                self.contact
            );
            let Some(json) = self.get_json(&url, Duration::from_millis(250), false) else {
                continue;
            };
            let Some(results) = json.get("results").and_then(|r| r.as_array()) else {
                continue;
            };
            for work in results {
                if let Some(mapped) = map_openalex_work(work) {
                    if let Some(doi) = mapped.doi.clone() {
                        out.insert(doi.to_lowercase(), mapped);
                    }
                }
            }
        }
        out
    }

    fn openalex_doi(&self, doi: &str) -> Option<OpenAlexMapped> {
        let url = format!(
            "https://api.openalex.org/works/doi:{}?mailto={}",
            urlencoding(doi),
            self.contact
        );
        map_openalex_work(&self.get_json(&url, Duration::from_millis(250), false)?)
    }

    fn openalex_title_search(&self, query: &str) -> Option<OpenAlexMapped> {
        let url = format!(
            "https://api.openalex.org/works?search={}&per-page=1&mailto={}",
            urlencoding(&query[..query.len().min(100)]),
            self.contact
        );
        let json = self.get_json(&url, Duration::from_millis(250), false)?;
        let work = json.get("results")?.as_array()?.first()?;
        let title = work.get("title").and_then(|t| t.as_str()).unwrap_or("");
        if !title_overlap_ok(query, title) {
            return None;
        }
        map_openalex_work(work)
    }

    fn crossref(&self, doi: &str) -> Option<BiblioMeta> {
        let url = format!(
            "https://api.crossref.org/works/{}?mailto={}",
            urlencoding(doi),
            self.contact
        );
        let json = self.get_json(&url, Duration::from_millis(250), false)?;
        Some(map_crossref(json.get("message")?))
    }

    fn arxiv(&self, arxiv_id: &str) -> Option<BiblioMeta> {
        let url = format!(
            "https://export.arxiv.org/api/query?id_list={}",
            urlencoding(arxiv_id)
        );
        self.pace(Duration::from_millis(500));
        let resp = self.agent.get(&url).call().ok()?;
        let xml = resp.into_string().ok()?;
        map_arxiv_atom(&xml, arxiv_id)
    }

    fn s2(&self, doi: &str) -> Option<EnrichmentMeta> {
        let url = format!(
            "https://api.semanticscholar.org/graph/v1/paper/DOI:{}?fields=paperId,tldr,s2FieldsOfStudy,influentialCitationCount",
            urlencoding(doi)
        );
        // Slow lane without a key: ~1 req/sec.
        let delay = if self.s2_key.is_some() {
            Duration::from_millis(200)
        } else {
            Duration::from_millis(1100)
        };
        let json = self.get_json(&url, delay, true)?;
        Some(map_s2(&json))
    }

    fn unpaywall(&self, doi: &str) -> Option<EnrichmentMeta> {
        let url = format!(
            "https://api.unpaywall.org/v2/{}?email={}",
            urlencoding(doi),
            self.contact
        );
        let json = self.get_json(&url, Duration::from_millis(250), false)?;
        Some(map_unpaywall(&json))
    }
}

/// Minimal percent-encoding for query values (no new dep for this).
fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Run the identity-resolution ladder for one item. `batch` holds
/// pre-fetched OpenAlex results keyed by lowercase bare DOI (empty when
/// batching was skipped). The ladder: batch hit → individual OpenAlex DOI →
/// arXiv API → OpenAlex title search → exhausted. Aux calls (crossref, s2,
/// unpaywall) run once a DOI is known and do not count against
/// MAX_IDENTITY_ATTEMPTS.
pub fn resolve_identity(
    client: &dyn EnrichClient,
    doi: Option<&str>,
    arxiv_id: Option<&str>,
    title: Option<&str>,
    batch: &HashMap<String, OpenAlexMapped>,
) -> FetchOutcome {
    let mut out = FetchOutcome::default();
    let mut doi = doi.map(str::to_string);

    // Rung 1: batch-pre-fetched OpenAlex (no new call).
    if let Some(d) = &doi {
        if let Some(hit) = batch.get(&d.to_lowercase()) {
            let (b, e) = hit.clone().into_parts();
            out.biblio.merge_missing(&b);
            out.enrichment.merge_missing(&e);
            out.resolved_via = Some("openalex_batch");
        }
    }
    // Rung 2: individual OpenAlex DOI lookup.
    if out.resolved_via.is_none() && doi.is_some() && out.identity_attempts < MAX_IDENTITY_ATTEMPTS
    {
        out.identity_attempts += 1;
        if let Some(hit) = client.openalex_doi(doi.as_ref().unwrap()) {
            let (b, e) = hit.into_parts();
            out.biblio.merge_missing(&b);
            out.enrichment.merge_missing(&e);
            out.resolved_via = Some("openalex_doi");
        }
    }
    // Rung 3: arXiv API (may yield a DOI for the rest of the ladder).
    if out.resolved_via.is_none() && arxiv_id.is_some() && out.identity_attempts < MAX_IDENTITY_ATTEMPTS
    {
        out.identity_attempts += 1;
        if let Some(b) = client.arxiv(arxiv_id.unwrap()) {
            if doi.is_none() {
                doi = b.doi.clone();
            }
            out.biblio.merge_missing(&b);
            out.resolved_via = Some("arxiv");
        }
        // A DOI from arXiv gets one OpenAlex pass for topics.
        if out.enrichment.openalex_topics.is_none()
            && doi.is_some()
            && out.identity_attempts < MAX_IDENTITY_ATTEMPTS
        {
            out.identity_attempts += 1;
            if let Some(hit) = client.openalex_doi(doi.as_ref().unwrap()) {
                let (b, e) = hit.into_parts();
                out.biblio.merge_missing(&b);
                out.enrichment.merge_missing(&e);
            }
        }
    }
    // Rung 4: title search with the word-overlap guard.
    let usable_title = title.filter(|t| !is_junk(t));
    if out.enrichment.openalex_topics.is_none()
        && usable_title.is_some()
        && out.identity_attempts < MAX_IDENTITY_ATTEMPTS
    {
        out.identity_attempts += 1;
        if let Some(hit) = client.openalex_title_search(usable_title.unwrap()) {
            // The found work's DOI supersedes an extracted DOI that resolved
            // nowhere — the title hit is now the identity claim.
            if hit.doi.is_some() {
                doi = hit.doi.clone();
            }
            let (b, e) = hit.into_parts();
            out.biblio.merge_missing(&b);
            out.enrichment.merge_missing(&e);
            if out.resolved_via.is_none() {
                out.resolved_via = Some("openalex_title_search");
            }
        }
    }

    if out.resolved_via.is_none() {
        return out;
    }
    // Aux enrichment once a DOI is known (not counted as identity attempts).
    if let Some(d) = &doi {
        if out.biblio.title.is_none() || out.biblio.authors.is_none() {
            if let Some(b) = client.crossref(d) {
                out.biblio.merge_missing(&b);
            }
        }
        if let Some(e) = client.s2(d) {
            out.enrichment.merge_missing(&e);
        }
        if let Some(e) = client.unpaywall(d) {
            out.enrichment.merge_missing(&e);
        }
    }
    out.biblio.doi = doi;
    out
}

fn is_junk(title: &str) -> bool {
    stacks_core::bonafides::is_junk_title(Some(title))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossref_mapping() {
        let msg = serde_json::json!({
            "title": ["Novel perovskite oxides for catalysis"],
            "author": [{"given": "Jane", "family": "Doe"}, {"given": "John", "family": "Smith"}],
            "published-print": {"date-parts": [[2021, 5, 1]]},
            "container-title": ["J. Mater. Chem. A"],
            "abstract": "We report..."
        });
        let b = map_crossref(&msg);
        assert_eq!(b.title.as_deref(), Some("Novel perovskite oxides for catalysis"));
        assert_eq!(b.year, Some(2021));
        assert_eq!(b.journal.as_deref(), Some("J. Mater. Chem. A"));
        let authors: serde_json::Value = serde_json::from_str(&b.authors.unwrap()).unwrap();
        assert_eq!(authors[0]["family"], "Doe");
        assert_eq!(authors[1]["given"], "John");
    }

    #[test]
    fn openalex_mapping_shapes_match_nl() {
        let work = serde_json::json!({
            "id": "https://openalex.org/W123",
            "doi": "https://doi.org/10.1234/x.y",
            "title": "A paper",
            "cited_by_count": 42,
            "topics": [
                {"display_name": "Perovskites", "score": 0.99,
                 "subfield": {"display_name": "Materials Chemistry"}},
                {"display_name": "Oxides", "score": 0.5,
                 "subfield": {"display_name": "Inorganic Chemistry"}}
            ],
            "concepts": [{"display_name": "Chemistry", "score": 0.8}]
        });
        let m = map_openalex_work(&work).unwrap();
        assert_eq!(m.openalex_id.as_deref(), Some("https://openalex.org/W123"));
        assert_eq!(m.doi.as_deref(), Some("10.1234/x.y"));
        assert_eq!(m.cited_by, Some(42));
        let topics: serde_json::Value = serde_json::from_str(&m.topics_json.unwrap()).unwrap();
        // NL shape: {"name","score","subfield"} with subfield a bare string
        assert_eq!(topics[0]["name"], "Perovskites");
        assert_eq!(topics[0]["subfield"], "Materials Chemistry");
        let concepts: serde_json::Value = serde_json::from_str(&m.concepts_json.unwrap()).unwrap();
        assert_eq!(concepts[0]["name"], "Chemistry");
    }

    #[test]
    fn s2_mapping() {
        let paper = serde_json::json!({
            "paperId": "abc123",
            "tldr": {"text": "Perovskites are neat."},
            "s2FieldsOfStudy": [{"category": "Materials Science", "source": "external"}],
            "influentialCitationCount": 7
        });
        let e = map_s2(&paper);
        assert_eq!(e.s2_paper_id.as_deref(), Some("abc123"));
        assert_eq!(e.s2_tldr.as_deref(), Some("Perovskites are neat."));
        assert_eq!(e.s2_influential_citation_count, Some(7));
        let fos: serde_json::Value = serde_json::from_str(&e.s2_fields_of_study.unwrap()).unwrap();
        assert_eq!(fos[0]["category"], "Materials Science");
    }

    #[test]
    fn unpaywall_mapping() {
        let data = serde_json::json!({
            "oa_status": "gold",
            "best_oa_location": {"url_for_pdf": "https://x/y.pdf", "url": "https://x/y"}
        });
        let e = map_unpaywall(&data);
        assert_eq!(e.unpaywall_oa_status.as_deref(), Some("gold"));
        assert_eq!(e.unpaywall_oa_url.as_deref(), Some("https://x/y.pdf"));
    }

    #[test]
    fn arxiv_atom_mapping() {
        let xml = r#"<?xml version="1.0"?>
        <feed xmlns="http://www.w3.org/2005/Atom" xmlns:arxiv="http://arxiv.org/schemas/atom">
          <title>ArXiv Query</title>
          <entry>
            <title>  Superconductivity in twisted
                     bilayer graphene  </title>
            <author><name>Jane Q. Public</name></author>
            <author><name>John Smith</name></author>
            <arxiv:doi>10.1103/PhysRevLett.121.087001</arxiv:doi>
          </entry>
        </feed>"#;
        let b = map_arxiv_atom(xml, "1803.00001").unwrap();
        assert_eq!(b.title.as_deref(), Some("Superconductivity in twisted bilayer graphene"));
        assert_eq!(b.doi.as_deref(), Some("10.1103/PhysRevLett.121.087001"));
        assert_eq!(b.arxiv_id.as_deref(), Some("1803.00001"));
        let authors: serde_json::Value = serde_json::from_str(&b.authors.unwrap()).unwrap();
        assert_eq!(authors[0]["family"], "Public");
    }

    #[test]
    fn title_overlap_guard() {
        assert!(title_overlap_ok(
            "Synthesis and characterization of novel perovskite oxides",
            "Synthesis and Characterization of Novel Perovskite Oxides for Catalysis"
        ));
        assert!(!title_overlap_ok(
            "Synthesis and characterization of novel perovskite oxides",
            "Completely unrelated results in organic polymer science"
        ));
        assert!(!title_overlap_ok("", "anything at all here"));
    }

    /// Live network smoke: only runs with STACKS_LIVE_TESTS=1.
    #[test]
    #[ignore = "live network; run with STACKS_LIVE_TESTS=1"]
    fn live_openalex_doi_lookup() {
        if std::env::var("STACKS_LIVE_TESTS").as_deref() != Ok("1") {
            return;
        }
        let client = HttpEnricher::new();
        let hit = client.openalex_doi("10.1038/nature12373").unwrap();
        assert!(hit.topics_json.is_some());
        assert_eq!(hit.doi.as_deref(), Some("10.1038/nature12373"));
    }

    /// Stub client: scripted responses per DOI/title, records call order.
    #[derive(Default)]
    struct StubClient {
        calls: Mutex<Vec<String>>,
        openalex: HashMap<String, OpenAlexMapped>,
        title_hit: Option<OpenAlexMapped>,
        crossref: Option<BiblioMeta>,
    }

    impl EnrichClient for StubClient {
        fn openalex_batch(&self, _dois: &[String]) -> HashMap<String, OpenAlexMapped> {
            self.calls.lock().unwrap().push("batch".into());
            HashMap::new()
        }
        fn openalex_doi(&self, doi: &str) -> Option<OpenAlexMapped> {
            self.calls.lock().unwrap().push(format!("oa_doi:{doi}"));
            self.openalex.get(&doi.to_lowercase()).cloned()
        }
        fn openalex_title_search(&self, query: &str) -> Option<OpenAlexMapped> {
            self.calls.lock().unwrap().push(format!("oa_title:{query}"));
            self.title_hit.clone()
        }
        fn crossref(&self, doi: &str) -> Option<BiblioMeta> {
            self.calls.lock().unwrap().push(format!("cr:{doi}"));
            self.crossref.clone()
        }
        fn arxiv(&self, id: &str) -> Option<BiblioMeta> {
            self.calls.lock().unwrap().push(format!("arxiv:{id}"));
            None
        }
        fn s2(&self, _doi: &str) -> Option<EnrichmentMeta> {
            None
        }
        fn unpaywall(&self, _doi: &str) -> Option<EnrichmentMeta> {
            None
        }
    }

    fn oa_hit(doi: &str) -> OpenAlexMapped {
        OpenAlexMapped {
            openalex_id: Some("https://openalex.org/W9".into()),
            doi: Some(doi.into()),
            title: Some("The actual paper title".into()),
            topics_json: Some(r#"[{"name":"T","score":0.9,"subfield":"Materials Chemistry"}]"#.into()),
            concepts_json: None,
            cited_by: Some(3),
        }
    }

    #[test]
    fn ladder_batch_hit_makes_no_calls() {
        let client = StubClient::default();
        let mut batch = HashMap::new();
        batch.insert("10.1/x".to_string(), oa_hit("10.1/x"));
        let out = resolve_identity(&client, Some("10.1/x"), None, None, &batch);
        assert_eq!(out.resolved_via, Some("openalex_batch"));
        assert_eq!(out.identity_attempts, 0);
        assert!(client.calls.lock().unwrap().iter().all(|c| !c.starts_with("oa_doi")));
    }

    #[test]
    fn ladder_individual_then_title_fallback() {
        let client = StubClient::default();
        let out = resolve_identity(&client, Some("10.1/miss"), None, None, &HashMap::new());
        // individual miss, no arxiv, no title → exhausted
        assert_eq!(out.resolved_via, None);
        assert_eq!(out.identity_attempts, 1);

        let client = StubClient {
            title_hit: Some(oa_hit("10.1/found")),
            ..Default::default()
        };
        let out = resolve_identity(
            &client,
            Some("10.1/miss"),
            None,
            Some("A long and perfectly plausible paper title"),
            &HashMap::new(),
        );
        assert_eq!(out.resolved_via, Some("openalex_title_search"));
        assert_eq!(out.biblio.doi.as_deref(), Some("10.1/found"));
        assert_eq!(out.identity_attempts, 2);
        let calls = client.calls.lock().unwrap();
        assert!(calls[0].starts_with("oa_doi"));
        assert!(calls[1].starts_with("oa_title"));
    }

    #[test]
    fn ladder_respects_attempt_cap() {
        let client = StubClient::default();
        // doi miss (1) + arxiv miss (1) + arxiv-doi oa miss skipped + title miss (1) = 3
        let out = resolve_identity(
            &client,
            Some("10.1/miss"),
            Some("2301.00001"),
            Some("A long and perfectly plausible paper title"),
            &HashMap::new(),
        );
        assert_eq!(out.resolved_via, None);
        assert_eq!(out.identity_attempts, MAX_IDENTITY_ATTEMPTS);
    }
}
