//! Identifier + title extraction from queued payloads. PDFs get a text
//! layer read of the first ~3 pages (pdftotext when available, otherwise a
//! small built-in FlateDecode/text-operator extractor — the workspace has no
//! PDF crate); .txt/.md/.html read directly. Regexes and heuristics are
//! ported from neurotic_library/scripts/catalog/core.py and
//! catalog_cli_backup.py (extract_pdf_metadata, extract_title_from_text).

use std::io::Read;
use std::path::Path;
use std::process::Command;

use regex::Regex;
use stacks_core::bonafides::is_junk_title;

use crate::ImportError;

/// `\b(10\.\d{4,9}/[^\s,;"'>}{]+)` — trailing '.' stripped, as NL did.
pub fn doi_re() -> &'static Regex {
    use std::sync::OnceLock;
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"\b(10\.\d{4,9}/[^\s,;"'>}{]+)"#).unwrap())
}

/// First DOI found in `text` (NL rule: strip a trailing period).
pub fn find_doi(text: &str) -> Option<String> {
    let m = doi_re().captures(text)?.get(1)?.as_str();
    Some(m.trim_end_matches('.').to_string())
}

/// arXiv id from a bare filename: `NNNN.NNNNN.pdf`, `NNNN.NNNNNvK.pdf`,
/// or `NNNN.NNNNN[_-]something.pdf` (NL's ARXIV_PURE_RE / ARXIV_PLUS_RE).
pub fn arxiv_id_from_filename(filename: &str) -> Option<String> {
    use std::sync::OnceLock;
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"^(\d{4}\.\d{4,5})(v\d+)?(?:[_\-].+)?\.pdf$").unwrap()
    });
    Some(re.captures(filename)?.get(1)?.as_str().to_string())
}

/// What the extractor decided a payload is, by extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Pdf,
    /// .txt/.md/.html — document path, no bibliographic identity expected.
    Text,
    Unsupported,
}

pub fn item_kind(path: &Path) -> ItemKind {
    match path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .as_deref()
    {
        Some("pdf") => ItemKind::Pdf,
        Some("txt" | "md" | "html" | "htm") => ItemKind::Text,
        _ => ItemKind::Unsupported,
    }
}

/// Extraction result for one payload.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExtractedIds {
    pub kind: Option<ItemKind>,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    /// First-page title heuristic; `None` when junk or not found.
    pub title: Option<String>,
    /// No usable text layer (scanned PDF etc.).
    pub no_text_layer: bool,
}

/// Pure identifier discovery over already-extracted text + the bare
/// filename. Kept separate from IO for testability.
pub fn identifiers_from(text: &str, filename: &str, kind: ItemKind) -> ExtractedIds {
    let mut out = ExtractedIds {
        kind: Some(kind),
        ..Default::default()
    };
    if kind == ItemKind::Pdf {
        out.doi = find_doi(text);
        out.arxiv_id = arxiv_id_from_filename(filename);
        if text.trim().len() < 20 {
            out.no_text_layer = true;
        } else {
            out.title = guess_title(text);
        }
    }
    out
}

/// Run extraction for a queued payload.
pub fn extract(path: &Path) -> Result<ExtractedIds, ImportError> {
    let kind = item_kind(path);
    let filename = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    match kind {
        ItemKind::Pdf => {
            let text = pdf_head_text(path, 3)?;
            Ok(identifiers_from(&text, &filename, kind))
        }
        ItemKind::Text | ItemKind::Unsupported => Ok(ExtractedIds {
            kind: Some(kind),
            ..Default::default()
        }),
    }
}

/// First-page title heuristic, ported from NL's extract_title_from_text:
/// first plausible line in the first 20 non-empty lines.
pub fn guess_title(first_page_text: &str) -> Option<String> {
    const SKIP_PREFIXES: &[&str] = &[
        "doi:",
        "http",
        "arxiv:",
        "copyright",
        "©",
        "published",
        "journal of",
        "proceedings of",
        "ieee",
        "acm",
        "vol.",
        "volume",
        "page",
        "pp.",
        "issn",
        "isbn",
    ];
    const SKIP_SUFFIXES: &[&str] = &[".com", ".org", ".edu", ".gov"];
    for line in first_page_text.lines().map(str::trim).filter(|l| !l.is_empty()).take(20) {
        let low = line.to_lowercase();
        if line.len() < 10 || line.len() > 300 {
            continue;
        }
        if line.split_whitespace().count() < 3 {
            continue;
        }
        if SKIP_PREFIXES.iter().any(|p| low.starts_with(p)) {
            continue;
        }
        if SKIP_SUFFIXES.iter().any(|s| low.ends_with(s)) {
            continue;
        }
        if low.starts_with("10.") {
            continue;
        }
        if is_junk_title(Some(line)) {
            continue;
        }
        let digits = line.chars().filter(|c| c.is_ascii_digit()).count();
        if digits as f64 > line.len() as f64 * 0.4 {
            continue;
        }
        return Some(line.to_string());
    }
    None
}

/// Text of the first `pages` pages of a PDF. Prefers `pdftotext` when on
/// PATH; otherwise the built-in extractor below (FlateDecode streams +
/// text-showing operators — enough for identifier discovery on born-digital
/// PDFs, which is all this pipeline asks of it).
pub fn pdf_head_text(path: &Path, pages: u32) -> Result<String, ImportError> {
    if let Ok(text) = pdftotext_head(path, pages) {
        return Ok(text);
    }
    builtin_pdf_text(path)
}

fn pdftotext_head(path: &Path, pages: u32) -> Result<String, ImportError> {
    let out = Command::new("pdftotext")
        .arg("-f")
        .arg("1")
        .arg("-l")
        .arg(pages.to_string())
        .arg(path)
        .arg("-")
        .output()?;
    if !out.status.success() {
        return Err(ImportError::Io(std::io::Error::other(format!(
            "pdftotext failed ({}) on {}",
            out.status,
            path.display()
        ))));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Minimal PDF text pull: decompress FlateDecode streams, collect literal
/// and hex strings shown by Tj/TJ/'/" operators. Not a layout engine — word
/// order is stream order — which is all the DOI regex and title heuristic
/// need.
fn builtin_pdf_text(path: &Path) -> Result<String, ImportError> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?.read_to_end(&mut bytes)?;
    let mut text = String::new();
    let mut rest = &bytes[..];
    while let Some(start) = find_subslice(rest, b"stream") {
        let after = &rest[start + 6..];
        // stream keyword is followed by CRLF or LF
        let body_start = if after.starts_with(b"\r\n") {
            2
        } else if after.starts_with(b"\n") || after.starts_with(b"\r") {
            1
        } else {
            rest = after;
            continue;
        };
        let body = &after[body_start..];
        let Some(end) = find_subslice(body, b"endstream") else {
            break;
        };
        let stream = &body[..end];
        rest = &body[end + 9..];
        // Only bother with streams that look like content streams.
        let decoded = if looks_flate(stream) {
            let mut v = Vec::new();
            match flate2::read::ZlibDecoder::new(stream).read_to_end(&mut v) {
                Ok(_) => v,
                Err(_) => continue,
            }
        } else {
            stream.to_vec()
        };
        extract_text_operators(&decoded, &mut text);
    }
    Ok(text)
}

fn looks_flate(stream: &[u8]) -> bool {
    stream.len() > 2 && stream[0] == 0x78
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|w| w == needle)
}

/// Pull strings out of `(literal) Tj`, `[(a) 5 (b)] TJ`, `(x) '`, `(x) "`
/// constructs and join with spaces/newlines.
fn extract_text_operators(content: &[u8], out: &mut String) {
    let s = String::from_utf8_lossy(content);
    let mut chars = s.char_indices().peekable();
    let mut pending_literal: Option<String> = None;
    let mut array_literals: Vec<String> = Vec::new();
    let mut in_array = false;
    while let Some((_, c)) = chars.next() {
        match c {
            '(' => {
                let mut depth = 1;
                let mut lit = String::new();
                while let Some((_, c2)) = chars.next() {
                    match c2 {
                        '\\' => {
                            if let Some((_, esc)) = chars.next() {
                                let e = match esc {
                                    'n' => '\n',
                                    'r' => '\r',
                                    't' => '\t',
                                    other => other,
                                };
                                lit.push(e);
                            }
                        }
                        '(' => {
                            depth += 1;
                            lit.push(c2);
                        }
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                            lit.push(c2);
                        }
                        _ => lit.push(c2),
                    }
                }
                if in_array {
                    array_literals.push(lit);
                } else {
                    pending_literal = Some(lit);
                }
            }
            '[' => in_array = true,
            ']' => {
                if in_array && !array_literals.is_empty() {
                    out.push_str(&array_literals.join(""));
                    out.push(' ');
                    array_literals.clear();
                }
                in_array = false;
            }
            'T' => {
                // Tj or TJ: flush pending literal (TJ arrays flush on ']')
                if chars.peek().is_some_and(|(_, c2)| *c2 == 'j') {
                    chars.next();
                    if let Some(lit) = pending_literal.take() {
                        out.push_str(&lit);
                        out.push('\n');
                    }
                }
            }
            '\'' | '"' => {
                if let Some(lit) = pending_literal.take() {
                    out.push_str(&lit);
                    out.push('\n');
                }
            }
            _ => {}
        }
    }
}

/// Whole-document text for chunking. Same dual path as [`pdf_head_text`]
/// (pdftotext when on PATH, else the builtin extractor); .txt/.md/.html are
/// read directly. Capped at MAX_TEXT_CHARS so pathological PDFs can't
/// explode the chunker.
pub fn full_text(path: &Path) -> Result<String, ImportError> {
    const MAX_TEXT_CHARS: usize = 2_000_000;
    let mut text = match item_kind(path) {
        ItemKind::Pdf => {
            match pdftotext_full(path) {
                Ok(t) => t,
                Err(_) => builtin_pdf_text(path)?,
            }
        }
        ItemKind::Text => std::fs::read_to_string(path)?,
        ItemKind::Unsupported => String::new(),
    };
    text.truncate(MAX_TEXT_CHARS);
    Ok(text)
}

fn pdftotext_full(path: &Path) -> Result<String, ImportError> {
    let out = Command::new("pdftotext").arg(path).arg("-").output()?;
    if !out.status.success() {
        return Err(ImportError::Io(std::io::Error::other(format!(
            "pdftotext failed ({}) on {}",
            out.status,
            path.display()
        ))));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doi_regex_real_examples() {
        assert_eq!(
            find_doi("Available online at https://doi.org/10.1016/j.jclepro.2020.121234 and more"),
            Some("10.1016/j.jclepro.2020.121234".to_string())
        );
        assert_eq!(
            find_doi("DOI: 10.1103/PhysRevB.104.174433."),
            Some("10.1103/PhysRevB.104.174433".to_string())
        );
        assert_eq!(find_doi("no identifier here"), None);
        // stops at quotes/braces/whitespace
        assert_eq!(
            find_doi(r#"doi = {10.1002/adma.202001234},"#),
            Some("10.1002/adma.202001234".to_string())
        );
    }

    #[test]
    fn arxiv_regex_nl_filenames() {
        // Real names from ~/neurotic_library/lib/hardware_and_architecture
        assert_eq!(arxiv_id_from_filename("2502.16631.pdf"), Some("2502.16631".into()));
        assert_eq!(arxiv_id_from_filename("2412.06464v3.pdf"), Some("2412.06464".into()));
        assert_eq!(arxiv_id_from_filename("2208.09235v1.pdf"), Some("2208.09235".into()));
        assert_eq!(arxiv_id_from_filename("2301.00001_smith.pdf"), Some("2301.00001".into()));
        assert_eq!(arxiv_id_from_filename("2301.00001-smith.pdf"), Some("2301.00001".into()));
        assert_eq!(arxiv_id_from_filename("Hart1995_FirmsContracts.pdf"), None);
        assert_eq!(arxiv_id_from_filename("2502.16631.txt"), None);
    }

    #[test]
    fn title_heuristic_skips_front_matter() {
        let text = "Journal of Materials Chemistry A\n\
                    Vol. 12, pp. 123-130\n\
                    doi: 10.1039/d1ta00001a\n\
                    Synthesis and characterization of novel perovskite oxides\n\
                    John Smith, Jane Doe\n";
        assert_eq!(
            guess_title(text),
            Some("Synthesis and characterization of novel perovskite oxides".to_string())
        );
        assert_eq!(guess_title("short\n"), None);
        assert_eq!(guess_title("2023 2024 2025 2026 2027 2028"), None);
    }

    #[test]
    fn identifiers_pure_fn() {
        let ids = identifiers_from(
            "Some preamble https://doi.org/10.1039/d1ta00001a rest",
            "2301.00001.pdf",
            ItemKind::Pdf,
        );
        assert_eq!(ids.doi.as_deref(), Some("10.1039/d1ta00001a"));
        assert_eq!(ids.arxiv_id.as_deref(), Some("2301.00001"));
        let ids = identifiers_from("", "scan.pdf", ItemKind::Pdf);
        assert!(ids.no_text_layer);
    }

    /// A minimal fake PDF: one uncompressed content stream with Tj text.
    #[test]
    fn builtin_extractor_reads_plain_stream() {
        let pdf = b"%PDF-1.4\n1 0 obj << >> endobj\n2 0 obj << /Length 60 >> stream\nBT /F1 12 Tf (https://doi.org/10.1234/test.5678) Tj ET\nendstream\nendobj\ntrailer << >>\n%%EOF";
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), pdf).unwrap();
        let text = builtin_pdf_text(tmp.path()).unwrap();
        assert!(text.contains("10.1234/test.5678"), "got: {text:?}");
    }

    #[test]
    fn builtin_extractor_reads_flate_stream() {
        use flate2::write::ZlibEncoder;
        use flate2::Compression;
        use std::io::Write;
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
        enc.write_all(b"BT (doi: 10.9999/flate.works-here) Tj ET").unwrap();
        let compressed = enc.finish().unwrap();
        let mut pdf = b"%PDF-1.4\n1 0 obj << /Filter /FlateDecode >> stream\n".to_vec();
        pdf.extend_from_slice(&compressed);
        pdf.extend_from_slice(b"\nendstream\nendobj\n%%EOF");
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), &pdf).unwrap();
        let text = builtin_pdf_text(tmp.path()).unwrap();
        assert!(text.contains("10.9999/flate.works-here"), "got: {text:?}");
    }
}
