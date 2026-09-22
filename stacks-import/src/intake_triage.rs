//! Wave G: intake/ triage and lib_ussr import into library.db.
//! Classification is deterministic (path components, extensions, sha256
//! dedupe); every decision lands in intake_triage as the audit artifact.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use stacks_core::library::{LibraryDocument, LibraryStore};
use stacks_core::StoreError;

use crate::ImportError;

const ROOT: &str = "/home/patrick/neurotic_library";

#[derive(Debug, Default, serde::Serialize)]
pub struct TriageStats {
    pub walked: u64,
    pub imported: u64,
    pub archives: u64,
    pub personal: u64,
    pub skipped_media: u64,
    pub skipped_web: u64,
    pub duplicates_paper: u64,
    pub duplicates_document: u64,
    pub reimport_skipped: u64,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub by_collection: HashMap<String, u64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Decision {
    Import(&'static str), // kind
    Archive,
    Personal(&'static str),
    Skip(&'static str),
}

const PERSONAL_COMPONENTS: &[&str] = &[
    "letters",
    "school",
    "school_mess",
    "homework",
    "personal",
    "photos",
    "family",
];
const ARCHIVE_EXTS: &[&str] = &["zip", "tar", "gz", "tgz", "txz", "xz", "7z", "rar", "bz2"];
const MEDIA_EXTS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "avif", "mov", "mp4", "mp3", "wav", "svg", "ico",
];
const DOC_EXTS: &[&str] = &[
    "pdf", "txt", "doc", "docx", "odt", "rtf", "md", "epub", "djvu",
];

/// The deterministic triage rule. Returns the decision and the rule name.
fn classify(rel: &Path) -> (Decision, &'static str) {
    let components: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    let filename = rel
        .file_name()
        .map(|f| f.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let ext = rel
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    for c in &components {
        if PERSONAL_COMPONENTS.contains(&c.as_str()) {
            return (
                Decision::Personal("personal-material path component"),
                "R_personal_path",
            );
        }
    }
    if filename.starts_with("letter") || filename.contains("homework") {
        return (
            Decision::Personal("personal-material filename"),
            "R_personal_name",
        );
    }
    if filename.contains("html?") || ext == "htm" || ext == "html" {
        return (Decision::Skip("web scrape artifact"), "R_web");
    }
    if ["js", "css", "ds_store", "pyc", "sample"].contains(&ext.as_str()) {
        return (Decision::Skip("web scrape artifact"), "R_web");
    }
    if ARCHIVE_EXTS.contains(&ext.as_str()) {
        return (Decision::Archive, "R_archive_ext");
    }
    if MEDIA_EXTS.contains(&ext.as_str()) {
        return (Decision::Skip("media file, not a document"), "R_media_ext");
    }
    if DOC_EXTS.contains(&ext.as_str()) {
        // book-ish directories -> book; journal-ish naming or loose -> paper
        let bookish = components.iter().any(|c| {
            [
                "paladin press collection",
                "bonus books, guides, and manuals",
                "the modern gunsmith",
                "gun pdf's",
                "bonus content",
            ]
            .contains(&c.as_str())
        });
        if bookish {
            return (Decision::Import("book"), "R_doc_book_dir");
        }
        return (Decision::Import("paper"), "R_doc_default");
    }
    (Decision::Skip("unrecognized file type"), "R_other")
}

fn walk(root: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() {
            walk(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, ImportError> {
    use sha2::Digest;
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut h = sha2::Sha256::new();
    let mut buf = [0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}

fn insert_triage(
    conn: &rusqlite::Connection,
    path: &str,
    decision: &str,
    rule: &str,
    reason: Option<&str>,
    sha256: Option<&str>,
) -> Result<(), StoreError> {
    conn.execute(
        "INSERT OR IGNORE INTO intake_triage (path, decision, rule, reason, sha256)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![path, decision, rule, reason, sha256],
    )?;
    Ok(())
}

/// Triage intake/: personal flagged not imported, archives recorded
/// unopened, dedupe by sha256 against paper+document, everything auditable.
pub fn triage_intake(
    store: &mut LibraryStore,
    intake_dir: &Path,
) -> Result<TriageStats, ImportError> {
    let mut stats = TriageStats::default();
    let nl_root = Path::new(ROOT);

    let mut files = Vec::new();
    walk(intake_dir, &mut files)?;
    files.sort();
    stats.walked = files.len() as u64;

    // dedupe sets: catalog papers and already-imported documents
    let paper_sha: std::collections::HashSet<String> = {
        let mut stmt = store.raw().prepare("SELECT sha256 FROM paper")?;
        let v = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<_, _>>()?;
        drop(stmt);
        v
    };
    let doc_sha: std::collections::HashSet<String> = {
        let mut stmt = store.raw().prepare("SELECT sha256 FROM document")?;
        let v = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<_, _>>()?;
        drop(stmt);
        v
    };
    let triaged: std::collections::HashSet<String> = {
        let mut stmt = store.raw().prepare("SELECT path FROM intake_triage")?;
        let v = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<_, _>>()?;
        drop(stmt);
        v
    };

    for path in files {
        let rel = path
            .strip_prefix(nl_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        if triaged.contains(&rel) {
            stats.reimport_skipped += 1;
            continue;
        }
        let (decision, rule) = classify(Path::new(&rel));
        store.with_transaction(|conn| {
            match decision {
                Decision::Personal(reason) => {
                    insert_triage(conn, &rel, "personal", rule, Some(reason), None)?;
                    stats.personal += 1;
                }
                Decision::Skip(reason) => {
                    insert_triage(conn, &rel, "skipped", rule, Some(reason), None)?;
                    if reason.contains("media") {
                        stats.skipped_media += 1;
                    } else {
                        stats.skipped_web += 1;
                    }
                }
                Decision::Archive => {
                    let sha = sha256_file(&path)?;
                    let filename = path.file_name().unwrap().to_string_lossy().to_string();
                    let inserted = insert_document_if_new(
                        conn,
                        &doc_sha,
                        LibraryDocument {
                            sha256: sha.clone(),
                            family: "intake".to_string(),
                            kind: "archive".to_string(),
                            title: Some(filename.clone()),
                            authors: None,
                            year: None,
                            language: None,
                            pages: None,
                            source_url: None,
                            download_url: None,
                            path: rel.clone(),
                            location_root: ROOT.to_string(),
                            bytes: std::fs::metadata(&path).ok().map(|m| m.len() as i64),
                            retrieved_at: None,
                            text_layer_path: None,
                            collection: Some("intake".to_string()),
                            original_language: None,
                            transliterated_title: None,
                            soviet_stratum: None,
                        },
                    )?;
                    if inserted {
                        stats.archives += 1;
                        insert_triage(conn, &rel, "archive", rule, None, Some(&sha))?;
                    } else {
                        stats.duplicates_document += 1;
                        insert_triage(
                            conn,
                            &rel,
                            "duplicate",
                            "R_dedupe_document",
                            None,
                            Some(&sha),
                        )?;
                    }
                }
                Decision::Import(kind) => {
                    let sha = sha256_file(&path)?;
                    if paper_sha.contains(&sha) {
                        stats.duplicates_paper += 1;
                        insert_triage(conn, &rel, "duplicate", "R_dedupe_paper", None, Some(&sha))?;
                        return Ok(());
                    }
                    let collection = path
                        .parent()
                        .and_then(|p| p.strip_prefix(intake_dir).ok())
                        .and_then(|p| {
                            let s = p.to_string_lossy().to_string();
                            if s.is_empty() {
                                None
                            } else {
                                Some(s)
                            }
                        })
                        .unwrap_or_else(|| "intake".to_string());
                    let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                    let inserted = insert_document_if_new(
                        conn,
                        &doc_sha,
                        LibraryDocument {
                            sha256: sha.clone(),
                            family: "intake".to_string(),
                            kind: kind.to_string(),
                            title: Some(stem),
                            authors: None,
                            year: None,
                            language: None,
                            pages: None,
                            source_url: None,
                            download_url: None,
                            path: rel.clone(),
                            location_root: ROOT.to_string(),
                            bytes: std::fs::metadata(&path).ok().map(|m| m.len() as i64),
                            retrieved_at: None,
                            text_layer_path: None,
                            collection: Some(collection.clone()),
                            original_language: None,
                            transliterated_title: None,
                            soviet_stratum: None,
                        },
                    )?;
                    if inserted {
                        stats.imported += 1;
                        *stats.by_collection.entry(collection).or_default() += 1;
                        insert_triage(conn, &rel, "imported", rule, None, Some(&sha))?;
                    } else {
                        stats.duplicates_document += 1;
                        insert_triage(
                            conn,
                            &rel,
                            "duplicate",
                            "R_dedupe_document",
                            None,
                            Some(&sha),
                        )?;
                    }
                }
            }
            Ok::<_, ImportError>(())
        })?;
    }
    Ok(stats)
}

fn insert_document_if_new(
    conn: &rusqlite::Connection,
    existing: &std::collections::HashSet<String>,
    doc: LibraryDocument,
) -> Result<bool, StoreError> {
    if existing.contains(&doc.sha256) {
        return Ok(false);
    }
    stacks_core::library::insert_document(conn, &doc)
}

/// Import all lib_ussr files as documents with Soviet-specific fields;
/// topic dir becomes the collection tag.
pub fn import_lib_ussr(
    store: &mut LibraryStore,
    ussr_dir: &Path,
) -> Result<TriageStats, ImportError> {
    let mut stats = TriageStats::default();
    let nl_root = Path::new(ROOT);
    let mut files = Vec::new();
    walk(ussr_dir, &mut files)?;
    files.sort();
    stats.walked = files.len() as u64;

    let doc_sha: std::collections::HashSet<String> = {
        let mut stmt = store.raw().prepare("SELECT sha256 FROM document")?;
        let v = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<_, _>>()?;
        drop(stmt);
        v
    };

    for path in files {
        let rel = path
            .strip_prefix(nl_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        let collection = path
            .strip_prefix(ussr_dir)
            .ok()
            .and_then(|r| r.components().next())
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .filter(|c| !c.is_empty())
            .unwrap_or_else(|| "unclassified".to_string());
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let kind = if ext == "pdf" { "book" } else { "article" };
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        // lib_ussr is a Soviet corpus with romanized filenames: language
        // follows the collection (translations dir is English), not the
        // filename charset.
        let sha = sha256_file(&path)?;
        if doc_sha.contains(&sha) {
            stats.duplicates_document += 1;
            insert_triage(
                store.raw(),
                &rel,
                "duplicate",
                "R_dedupe_document",
                None,
                Some(&sha),
            )?;
            continue;
        }
        let inserted = stacks_core::library::insert_document(
            store.raw(),
            &LibraryDocument {
                sha256: sha.clone(),
                family: "lib_ussr".to_string(),
                kind: kind.to_string(),
                title: Some(path.file_stem().unwrap().to_string_lossy().to_string()),
                authors: None,
                year: None,
                language: Some(if collection == "translations" {
                    "en".to_string()
                } else {
                    "ru".to_string()
                }),
                pages: None,
                source_url: None,
                download_url: None,
                path: rel.clone(),
                location_root: ROOT.to_string(),
                bytes: std::fs::metadata(&path).ok().map(|m| m.len() as i64),
                retrieved_at: None,
                text_layer_path: if ext == "txt" {
                    Some(rel.clone())
                } else {
                    None
                },
                collection: Some(collection.clone()),
                original_language: Some("ru".to_string()),
                transliterated_title: Some(stem),
                soviet_stratum: Some("lib_ussr".to_string()),
            },
        )?;
        if inserted {
            stats.imported += 1;
            *stats.by_collection.entry(collection).or_default() += 1;
            insert_triage(
                store.raw(),
                &rel,
                "imported",
                "R_lib_ussr",
                None,
                Some(&sha),
            )?;
        } else {
            stats.reimport_skipped += 1;
        }
    }
    Ok(stats)
}
