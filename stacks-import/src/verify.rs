//! Deterministic verification of LLM-extracted recipes against the source
//! page text. Every asserted number must literally appear in the located
//! pages (fabrication fence); formulas are matched modulo documented
//! OCR-normalization (repair-review list); prose quantities found in no
//! structured field are coverage warnings (under-extraction signal).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use regex::Regex;
use rusqlite::params;
use stacks_core::{Conditions, Quantity, Store, Temperature};

use crate::ImportError;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, serde::Serialize)]
pub struct Finding {
    pub kind: String,
    pub fragment: String,
    pub detail: String,
}

#[derive(Debug, serde::Serialize)]
pub struct RecipeVerdict {
    pub external_key: String,
    pub locator: String,
    pub pdf_pages: Vec<i64>,
    pub verdict: Verdict,
    pub fails: Vec<Finding>,
    pub warns: Vec<Finding>,
    pub coverage: Vec<Finding>,
}

#[derive(Debug, serde::Serialize)]
pub struct VerifyReport {
    pub source_dataset: String,
    pub recipes: usize,
    pub pass: usize,
    pub warn: usize,
    pub fail: usize,
    pub coverage_warns: usize,
    pub results: Vec<RecipeVerdict>,
}

/// Normalize text for number search: unicode minus, whitespace removal
/// (also collapses OCR's "1 000"), degree-sign stripped, lowercased.
/// Commas are left in place; callers also try [`decimal_comma_variant`] so
/// decimal-comma printing is covered without destroying formula-adjacent
/// commas like `NaBF4,50`.
pub fn normalize_numbers_text(s: &str) -> String {
    let s = s.replace(['\u{2212}', '–', '—'], "-");
    let s = s.replace(['\u{00B0}', 'º', '°'], "");
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    s.to_lowercase()
}

/// Variant with decimal commas (digit,digit) folded to points.
pub fn decimal_comma_variant(norm_text: &str) -> String {
    let chars: Vec<char> = norm_text.chars().collect();
    let mut out = String::with_capacity(norm_text.len());
    for (i, c) in chars.iter().enumerate() {
        if *c == ','
            && i > 0
            && i + 1 < chars.len()
            && chars[i - 1].is_ascii_digit()
            && chars[i + 1].is_ascii_digit()
        {
            out.push('.');
        } else {
            out.push(*c);
        }
    }
    out
}

/// Variant for spelled-out numerals: whitespace collapsed to single spaces
/// so word-boundary checks work (`for six hours` stays three words).
pub fn word_variant(s: &str) -> String {
    let s = s.replace(['\u{2212}', '–', '—'], "-");
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Substring search with digit boundaries: the needle must not be fused
/// into a longer digit run (`"0"` must not match inside `"101"`).
pub fn contains_number(norm_text: &str, needle: &str) -> bool {
    let bytes = norm_text.as_bytes();
    let n = needle.as_bytes();
    if n.is_empty() || bytes.len() < n.len() {
        return false;
    }
    let alpha = n[0].is_ascii_alphabetic();
    let bad = |c: u8| {
        if alpha {
            c.is_ascii_alphanumeric()
        } else {
            c.is_ascii_digit() || c == b'.'
        }
    };
    for start in 0..=(bytes.len() - n.len()) {
        if &bytes[start..start + n.len()] != n {
            continue;
        }
        let before_ok = start == 0 || !bad(bytes[start - 1]);
        let after = start + n.len();
        let after_ok = after == bytes.len() || !bad(bytes[after]);
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

const NUMBER_WORDS: [(&str, f64); 13] = [
    ("half", 0.5),
    ("one", 1.0),
    ("two", 2.0),
    ("three", 3.0),
    ("four", 4.0),
    ("five", 5.0),
    ("six", 6.0),
    ("seven", 7.0),
    ("eight", 8.0),
    ("nine", 9.0),
    ("ten", 10.0),
    ("eleven", 11.0),
    ("twelve", 12.0),
];

/// Candidate string forms of a numeric value as it might appear in print:
/// digits, plus spelled-out English numerals for small integers
/// ("six hours").
pub fn value_candidates(v: f64) -> Vec<String> {
    let mut c = vec![];
    if v.fract() == 0.0 && v.abs() < 1e15 {
        c.push(format!("{}", v as i64));
    }
    for (word, n) in NUMBER_WORDS {
        if (n - v).abs() < 1e-9 {
            c.push(word.to_string());
        }
    }
    let plain = format!("{v}");
    if !c.contains(&plain) {
        c.push(plain);
    }
    c
}

/// Normalize a formula for comparison: strip sub/superscript digits to
/// ASCII, drop whitespace and hydrate-dot variants. Documented in the
/// verification report.
pub fn normalize_formula(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '₀'..='₉' => char::from_digit(c as u32 - '₀' as u32, 10).unwrap(),
            '⁰'..='⁹' => char::from_digit(c as u32 - '⁰' as u32, 10).unwrap(),
            '•' | '·' | '⋅' | '.' => '.',
            _ => c,
        })
        .filter(|c| !c.is_whitespace())
        .collect()
}

/// Number assertions extracted from one recipe's structured fields.
#[derive(Debug, Default)]
struct NumberAssertions {
    /// (fragment, candidates)
    items: Vec<(String, Vec<String>)>,
}

impl NumberAssertions {
    fn push(&mut self, label: String, q: &Quantity) {
        self.items.push((
            format!("{label} value {}", q.value),
            value_candidates(q.value),
        ));
        if let stacks_core::Operator::Range { min, max } = &q.operator {
            self.items
                .push((format!("{label} range-min {min}"), value_candidates(*min)));
            self.items
                .push((format!("{label} range-max {max}"), value_candidates(*max)));
        }
    }

    fn all_values(&self) -> BTreeSet<String> {
        self.items
            .iter()
            .flat_map(|(_, c)| c.iter().cloned())
            .collect()
    }
}

fn collect_conditions_numbers(acc: &mut NumberAssertions, c: &Conditions) {
    if let Some(t) = &c.temperature {
        match t {
            Temperature::Scalar(q) => acc.push("temperature".to_string(), q),
            Temperature::MinMax { min, max } => {
                acc.push("temperature min".to_string(), min);
                acc.push("temperature max".to_string(), max);
            }
            Temperature::Series { points } => {
                for p in points {
                    acc.push("temperature series point".to_string(), &p.temperature);
                    // Series *time* coordinates are an extraction-side
                    // structuring device (t=0 origins, ramp endpoints); prose
                    // states durations instead, which are asserted via
                    // `duration`. Not asserted literally.
                }
            }
        }
    }
    if let Some(q) = &c.duration {
        acc.push("duration".to_string(), q);
    }
    if let Some(q) = &c.pressure {
        acc.push("pressure".to_string(), q);
    }
}

/// Parse a provenance locator like `pp. 219-220` / `p. 233` into printed
/// page numbers (range capped at 4 pages).
pub fn parse_locator(locator: &str) -> Vec<i64> {
    let re = Regex::new(r"pp?\.\s*(\d+)(?:\s*-\s*(\d+))?").unwrap();
    let Some(caps) = re.captures(locator) else {
        return vec![];
    };
    let start: i64 = caps[1].parse().unwrap_or(0);
    let end: i64 = caps
        .get(2)
        .and_then(|m| m.as_str().parse().ok())
        .unwrap_or(start)
        .min(start + 4);
    (start..=end).collect()
}

/// Index printed page numbers -> page text files. The printed number is
/// found in the first or last short lines of each page file.
pub fn build_page_index(pages_dir: &Path) -> Result<BTreeMap<i64, PathBuf>, ImportError> {
    let num = Regex::new(r"^\d{1,4}$").unwrap();
    let mut index = BTreeMap::new();
    for entry in std::fs::read_dir(pages_dir)? {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !(name.starts_with("page-") && name.ends_with(".txt")) {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        let lines: Vec<&str> = text.lines().collect();
        let mut probe: Vec<&str> = lines.iter().take(6).copied().collect();
        probe.extend(lines.iter().rev().take(3).copied());
        for l in probe {
            let l = l.trim();
            // OCR often spaces digits: "2 7 3" means page 273.
            let squashed: String = l.chars().filter(|c| !c.is_whitespace()).collect();
            if num.is_match(l)
                || (!squashed.is_empty() && squashed.len() <= 4 && num.is_match(&squashed))
            {
                let candidate = if num.is_match(l) {
                    l
                } else {
                    squashed.as_str()
                };
                if let Ok(n) = candidate.parse::<i64>() {
                    index.entry(n).or_insert(path.clone());
                    break;
                }
            }
        }
    }
    Ok(index)
}

fn strip_properties(text: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for line in text.lines() {
        let up = line.trim().to_uppercase();
        if up.starts_with("PROPERTIES") {
            skipping = true;
            continue;
        }
        if skipping && (up.starts_with("REFERENCE") || up.starts_with("SYNONYM")) {
            skipping = false;
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Regex for prose quantities used by the coverage check.
fn coverage_fragments(text: &str) -> Vec<String> {
    let re = Regex::new(
        r"(?i)(\d+(?:[.,]\d+)?)\s*(g\.|g\b|ml\.|ml\b|l\.|°C|C\b|hours?|hrs?|h\b|min\.|minutes?|days?|mm\b|atm\b)",
    )
    .unwrap();
    re.find_iter(text).map(|m| m.as_str().to_string()).collect()
}

pub fn verify_extraction(
    store: &Store,
    source_dataset: &str,
    pages_dir: &Path,
) -> Result<VerifyReport, ImportError> {
    let index = build_page_index(pages_dir)?;
    let conn = store.raw();
    let mut stmt = conn.prepare(
        "SELECT r.id, r.external_key, p.locator FROM recipe r
         JOIN provenance p ON p.id = r.provenance_id
         WHERE r.external_key LIKE ?1 || ':%' AND p.extraction_method = 'llm_extracted'
         ORDER BY r.id",
    )?;
    let recipes: Vec<(i64, String, String)> = stmt
        .query_map(params![source_dataset], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?
        .collect::<Result<_, _>>()?;

    let mut results = Vec::new();
    for (recipe_id, external_key, locator) in recipes {
        let printed = parse_locator(&locator);
        // Printed pages straddle PDF pages; a recipe's prose may begin or
        // end one page off the printed locator, so widen by one page each
        // side when gathering text.
        let mut widened: Vec<i64> = printed.iter().flat_map(|p| [p - 1, *p, p + 1]).collect();
        widened.sort_unstable();
        widened.dedup();
        let mut page_text = String::new();
        let mut pdf_pages = Vec::new();
        for p in &widened {
            if let Some(path) = index.get(p) {
                pdf_pages.push(*p);
                page_text.push_str(&std::fs::read_to_string(path)?);
                page_text.push('\n');
            }
        }
        let norm_text = normalize_numbers_text(&page_text);
        let norm_text_comma = decimal_comma_variant(&norm_text);
        let norm_words = word_variant(&page_text);
        let norm_text_formula = normalize_formula(&page_text);

        let mut assertions = NumberAssertions::default();
        let mut formulas: Vec<(String, String)> = Vec::new(); // (formula, role context)

        let mut step_stmt = conn.prepare(
            "SELECT id, conditions_json FROM recipe_step WHERE recipe_id = ?1 ORDER BY ordering",
        )?;
        let steps: Vec<(i64, String)> = step_stmt
            .query_map([recipe_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        for (step_id, conditions_json) in &steps {
            let conditions: Conditions = serde_json::from_str(conditions_json)?;
            collect_conditions_numbers(&mut assertions, &conditions);
            let mut mat_stmt = conn.prepare(
                "SELECT m.formula, sm.value, sm.range_min, sm.range_max, sm.unit
                 FROM step_material sm JOIN material m ON m.id = sm.material_id
                 WHERE sm.step_id = ?1",
            )?;
            type MaterialRow = (
                Option<String>,
                Option<f64>,
                Option<f64>,
                Option<f64>,
                Option<String>,
            );
            let mats: Vec<MaterialRow> = mat_stmt
                .query_map([step_id], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
                })?
                .collect::<Result<_, _>>()?;
            for (formula, value, rmin, rmax, unit) in mats {
                if let Some(f) = formula {
                    formulas.push((f, format!("step {step_id}")));
                }
                if let (Some(v), Some(u)) = (value, unit) {
                    let q = Quantity {
                        value: v,
                        unit: stacks_core::Unit::from_token(&u)
                            .unwrap_or(stacks_core::Unit::Other(u.clone())),
                        operator: match (rmin, rmax) {
                            (Some(a), Some(b)) => stacks_core::Operator::Range { min: a, max: b },
                            _ => stacks_core::Operator::Eq,
                        },
                    };
                    assertions.push(format!("quantity {u}"), &q);
                }
            }
        }

        let mut fails = Vec::new();
        let mut warns = Vec::new();
        let mut coverage = Vec::new();

        if page_text.is_empty() {
            fails.push(Finding {
                kind: "locator".to_string(),
                fragment: locator.clone(),
                detail: "no page text found for locator".to_string(),
            });
        }

        for (label, candidates) in &assertions.items {
            let found = candidates.iter().any(|c| {
                if c.as_bytes()[0].is_ascii_alphabetic() {
                    contains_number(&norm_words, c.as_str())
                } else {
                    contains_number(&norm_text, c.as_str())
                        || contains_number(&norm_text_comma, c.as_str())
                }
            });
            if !found {
                fails.push(Finding {
                    kind: "number".to_string(),
                    fragment: label.clone(),
                    detail: format!(
                        "asserted value not found in located page text (tried {candidates:?})"
                    ),
                });
            }
        }

        for (formula, ctx) in &formulas {
            let needle = normalize_formula(formula);
            if !norm_text_formula.contains(&needle) {
                warns.push(Finding {
                    kind: "formula".to_string(),
                    fragment: formula.clone(),
                    detail: format!(
                        "formula not found in page text after normalization ({ctx}); likely OCR-mangled in source — human review"
                    ),
                });
            }
        }

        let known = assertions.all_values();
        let stripped = strip_properties(&page_text);
        for frag in coverage_fragments(&stripped) {
            let norm_frag = normalize_numbers_text(&frag);
            let num: String = norm_frag
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
                .collect();
            if num.is_empty() {
                continue;
            }
            if !known.contains(&num) {
                coverage.push(Finding {
                    kind: "coverage".to_string(),
                    fragment: frag,
                    detail: "prose quantity matched no structured field".to_string(),
                });
            }
        }

        let verdict = if !fails.is_empty() {
            Verdict::Fail
        } else if !warns.is_empty() {
            Verdict::Warn
        } else {
            Verdict::Pass
        };
        results.push(RecipeVerdict {
            external_key,
            locator,
            pdf_pages,
            verdict,
            fails,
            warns,
            coverage,
        });
    }

    let pass = results
        .iter()
        .filter(|r| r.verdict == Verdict::Pass)
        .count();
    let warn = results
        .iter()
        .filter(|r| r.verdict == Verdict::Warn)
        .count();
    let fail = results
        .iter()
        .filter(|r| r.verdict == Verdict::Fail)
        .count();
    let coverage_warns = results.iter().map(|r| r.coverage.len()).sum();
    Ok(VerifyReport {
        source_dataset: source_dataset.to_string(),
        recipes: results.len(),
        pass,
        warn,
        fail,
        coverage_warns,
        results,
    })
}

/// Render the human-readable markdown summary.
pub fn render_markdown(report: &VerifyReport) -> String {
    let mut md = String::new();
    md.push_str(&format!(
        "# Extraction verification — {}\n\n",
        report.source_dataset
    ));
    md.push_str(&format!(
        "{} recipes: **{} pass, {} warn, {} fail**; {} coverage warnings.\n\n",
        report.recipes, report.pass, report.warn, report.fail, report.coverage_warns
    ));
    md.push_str("Matching rules: located text is the printed locator pages widened by one page each side (printed pages straddle PDF pages). Numbers match after: unicode minus → `-`; all whitespace removed (covers OCR `1 000`); degree signs stripped; lowercasing; a decimal-comma variant is tried in parallel (formula-adjacent commas like `NaBF4,50` are preserved in the primary variant). Spelled-out numerals one–twelve match against a whitespace-collapsed word variant. All matches require token boundaries (no `0` inside `101`, no `one` inside `done`). Series time coordinates are extraction-side structure and are not asserted literally; prose durations are asserted via `duration`. Formulas compare with subscript digits folded to ASCII and hydrate dots unified.\n\n");
    md.push_str("Interpretation: **FAIL** = a structured value absent from the source (fabrication fence; exit code 1). **WARN** = formula not literally in the page text — almost always OCR subscript mangling repaired during extraction; queued for human review, not auto-fail. **coverage** = prose quantity on the located pages matched by no structured field; dominated by *neighbouring* recipes that share the widened pages, so it is a recall signal, not an error count.\n\n");
    md.push_str(
        "| recipe | locator | verdict | fails | warns | coverage |\n|---|---|---|---|---|---|\n",
    );
    for r in &report.results {
        let v = match r.verdict {
            Verdict::Pass => "pass",
            Verdict::Warn => "WARN",
            Verdict::Fail => "**FAIL**",
        };
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            r.external_key,
            r.locator,
            v,
            r.fails.len(),
            r.warns.len(),
            r.coverage.len()
        ));
    }
    for r in &report.results {
        if r.fails.is_empty() && r.warns.is_empty() && r.coverage.is_empty() {
            continue;
        }
        md.push_str(&format!("\n## {} ({})\n", r.external_key, r.locator));
        for f in &r.fails {
            md.push_str(&format!(
                "- **FAIL [{}]** `{}` — {}\n",
                f.kind, f.fragment, f.detail
            ));
        }
        for f in &r.warns {
            md.push_str(&format!(
                "- WARN [{}] `{}` — {}\n",
                f.kind, f.fragment, f.detail
            ));
        }
        for f in &r.coverage {
            md.push_str(&format!("- coverage: `{}` — {}\n", f.fragment, f.detail));
        }
    }
    md
}
