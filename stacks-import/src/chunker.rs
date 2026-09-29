//! Text chunking for stamped payloads, ported from
//! neurotic_library/scripts/wave_corpus/chunker.py: section-header-aware
//! structural chunking for papers, markdown-header chunking for .md, and
//! the recursive (word-window) fallback. Same defaults: 400-word chunks,
//! 50 overlap, 25 min words.

use regex::Regex;

#[derive(Debug, Clone, PartialEq)]
pub struct TextChunk {
    pub section: String,
    pub text: String,
    pub word_count: i64,
}

pub const CHUNK_SIZE: usize = 400;
pub const CHUNK_OVERLAP: usize = 50;
pub const MIN_WORDS: usize = 25;

/// Academic section headers — standalone lines, optionally numbered.
fn section_re() -> &'static Regex {
    use std::sync::OnceLock;
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?m)^(?:\d+\s+)?(?:Abstract|ABSTRACT|Introduction|INTRODUCTION|Related Work|RELATED WORK|Background|BACKGROUND|Preliminaries|PRELIMINARIES|Methods?|METHODS?|Methodology|METHODOLOGY|Approach|APPROACH|Model|MODEL|Architecture|ARCHITECTURE|Experiments?|EXPERIMENTS?|Evaluation|EVALUATION|Results?|RESULTS?|Empirical|EMPIRICAL|Discussion|DISCUSSION|Analysis|ANALYSIS|Conclusion|CONCLUSION|Concluding|CONCLUDING|Summary|SUMMARY|Future Work|FUTURE WORK|References?|REFERENCES?|Bibliography|BIBLIOGRAPHY|Acknowledgments?|ACKNOWLEDGMENTS?|Appendix|APPENDIX)(?:\s+[A-Z])?\s*$").unwrap()
    })
}

fn md_header_re() -> &'static Regex {
    use std::sync::OnceLock;
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^(#{1,4}\s+.+)$").unwrap())
}

fn make_chunk(text: &str, section: &str) -> TextChunk {
    let text = text.trim().to_string();
    TextChunk {
        section: section.to_string(),
        word_count: text.split_whitespace().count() as i64,
        text,
    }
}

/// Split a section body into overlapping word windows, dropping windows
/// below `min_words`.
fn windowed(body: &str, section: &str, chunk_size: usize, chunk_overlap: usize, min_words: usize) -> Vec<TextChunk> {
    let words: Vec<&str> = body.split_whitespace().collect();
    let step = (chunk_size - chunk_overlap).max(1);
    let mut out = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let end = (i + chunk_size).min(words.len());
        let chunk = make_chunk(&words[i..end].join(" "), section);
        if chunk.word_count as usize >= min_words {
            out.push(chunk);
        }
        i += step;
    }
    out
}

/// Chunk by academic section headers; oversized sections are split by word
/// count with overlap. Faithful port of NL's chunk_structural.
pub fn chunk_structural(text: &str, chunk_size: usize, chunk_overlap: usize, min_words: usize) -> Vec<TextChunk> {
    let text = Regex::new(r"\n+").unwrap().replace_all(text, "\n");
    let mut sections: Vec<(String, Vec<String>)> = Vec::new();
    let mut current_header = "Preamble".to_string();
    let mut current_lines: Vec<String> = Vec::new();
    for line in text.lines() {
        let stripped = line.trim();
        if section_re().is_match(stripped) {
            if !current_lines.is_empty() {
                sections.push((std::mem::take(&mut current_header), std::mem::take(&mut current_lines)));
            }
            current_header = stripped.to_string();
        } else {
            current_lines.push(stripped.to_string());
        }
    }
    if !current_lines.is_empty() {
        sections.push((current_header, current_lines));
    }

    let mut out = Vec::new();
    for (header, lines) in sections {
        let body = lines.join("\n");
        if body.split_whitespace().count() == 0 {
            continue;
        }
        out.extend(windowed(&body, &header, chunk_size, chunk_overlap, min_words));
    }
    out
}

/// Word-window fallback (NL's chunk_recursive degenerates to this for
/// whitespace text; the paragraph/sentence separator ladder never fired in
/// practice since it joined words identically).
pub fn chunk_recursive(text: &str, chunk_size: usize, chunk_overlap: usize, min_words: usize) -> Vec<TextChunk> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    windowed(text, "Preamble", chunk_size, chunk_overlap, min_words)
}

/// Chunk markdown by `#`-header boundaries (NL's chunk_markdown).
pub fn chunk_markdown(text: &str, chunk_size: usize, chunk_overlap: usize, min_words: usize) -> Vec<TextChunk> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    let re = md_header_re();
    let mut out = Vec::new();
    let mut current_header = "Preamble".to_string();
    // Split keeping the headers, as re.split with a capture group did.
    let mut last_end = 0;
    let mut parts: Vec<String> = Vec::new();
    for cap in re.captures_iter(text) {
        let m = cap.get(0).unwrap();
        if m.start() > last_end {
            parts.push(text[last_end..m.start()].to_string());
        }
        parts.push(m.as_str().to_string());
        last_end = m.end();
    }
    if last_end < text.len() {
        parts.push(text[last_end..].to_string());
    }
    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if part.starts_with('#') {
            current_header = part.to_string();
            continue;
        }
        out.extend(windowed(part, &current_header, chunk_size, chunk_overlap, min_words));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(n: usize) -> String {
        (0..n).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn structural_splits_on_section_headers() {
        let text = format!("Introduction\n{}\nMethods\n{}\nConclusion\n{}", words(30), words(60), words(40));
        let chunks = chunk_structural(&text, CHUNK_SIZE, CHUNK_OVERLAP, MIN_WORDS);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].section, "Introduction");
        assert_eq!(chunks[1].section, "Methods");
        assert_eq!(chunks[1].word_count, 60);
        assert_eq!(chunks[2].section, "Conclusion");
    }

    #[test]
    fn structural_windows_oversized_sections_with_overlap() {
        let text = format!("Methods\n{}", words(900));
        let chunks = chunk_structural(&text, CHUNK_SIZE, CHUNK_OVERLAP, MIN_WORDS);
        // step 350: windows at 0,350,700 → 3 chunks (last is 200 words)
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].word_count, 400);
        // overlap: chunk 2 starts at word 350
        assert!(chunks[1].text.starts_with("w350"));
    }

    #[test]
    fn structural_drops_small_chunks() {
        let text = format!("Introduction\n{}\nMethods\n{}", words(10), words(30));
        let chunks = chunk_structural(&text, CHUNK_SIZE, CHUNK_OVERLAP, MIN_WORDS);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].section, "Methods");
    }

    #[test]
    fn structural_preamble_and_numbered_headers() {
        let text = format!("{}\n2 Results\n{}", words(40), words(30));
        let chunks = chunk_structural(&text, CHUNK_SIZE, CHUNK_OVERLAP, MIN_WORDS);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].section, "Preamble");
        assert_eq!(chunks[1].section, "2 Results");
    }

    #[test]
    fn markdown_chunks_by_headers() {
        let text = format!("# Setup\n{}\n## Details\n{}", words(40), words(50));
        let chunks = chunk_markdown(&text, CHUNK_SIZE, CHUNK_OVERLAP, 20);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].section, "# Setup");
        assert_eq!(chunks[1].section, "## Details");
    }

    #[test]
    fn recursive_fallback_windows() {
        let chunks = chunk_recursive(&words(100), 400, 50, 10);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].section, "Preamble");
        assert!(chunk_recursive("", 400, 50, 10).is_empty());
    }
}
