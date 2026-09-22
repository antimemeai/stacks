use std::fs;

use stacks_core::library::LibraryStore;
use stacks_import::intake_triage::{import_lib_ussr, triage_intake};
use tempfile::TempDir;

fn fixture_tree(tmp: &TempDir) -> std::path::PathBuf {
    let nl = tmp.path().to_path_buf();
    let intake = nl.join("intake");
    fs::create_dir_all(intake.join("letters")).unwrap();
    fs::create_dir_all(intake.join("school_mess")).unwrap();
    fs::create_dir_all(intake.join("Paladin Press Collection")).unwrap();
    fs::create_dir_all(intake.join("reference_papers")).unwrap();
    fs::write(
        intake.join("letters").join("letter_to_mom.pdf"),
        b"%PDF personal",
    )
    .unwrap();
    fs::write(intake.join("school_mess").join("hw1.pdf"), b"%PDF homework").unwrap();
    fs::write(
        intake.join("Paladin Press Collection").join("book1.pdf"),
        b"%PDF paladin book content",
    )
    .unwrap();
    fs::write(
        intake.join("reference_papers").join("2402.11814.pdf"),
        b"%PDF arxiv paper",
    )
    .unwrap();
    fs::write(intake.join("bundle.zip"), b"PKzipdata").unwrap();
    fs::write(intake.join("photo.jpg"), b"jpg").unwrap();
    fs::write(intake.join("page.html?c=s;o=a"), b"<html>").unwrap();
    let ussr = nl.join("lib_ussr");
    fs::create_dir_all(ussr.join("chemistry")).unwrap();
    fs::write(
        ussr.join("chemistry").join("Химия кремния.pdf"),
        b"%PDF soviet book",
    )
    .unwrap();
    fs::write(ussr.join("chemistry").join("notes.txt"), b"plain text").unwrap();
    nl
}

#[test]
fn personal_paths_flagged_not_imported_and_idempotent() {
    let tmp = TempDir::new().unwrap();
    let nl = fixture_tree(&tmp);
    let mut store = LibraryStore::open(tmp.path().join("library.db")).unwrap();

    let stats = triage_intake(&mut store, &nl.join("intake")).unwrap();
    assert_eq!(stats.walked, 7);
    assert_eq!(
        stats.personal, 2,
        "letters/ and school_mess/ must not import"
    );
    assert_eq!(stats.imported, 2, "paladin book + arxiv paper");
    assert_eq!(stats.archives, 1);
    assert_eq!(stats.skipped_media, 1);
    assert_eq!(stats.skipped_web, 1);

    // No personal row in document.
    let personal_docs: i64 = store
        .raw()
        .query_row(
            "SELECT COUNT(*) FROM document WHERE path LIKE '%letters%' OR path LIKE '%school_mess%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(personal_docs, 0);

    // Triage table records every decision with its rule.
    let decisions: Vec<(String, String)> = {
        let mut stmt = store
            .raw()
            .prepare("SELECT decision, rule FROM intake_triage ORDER BY path")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    assert_eq!(decisions.len(), 7);
    assert!(decisions
        .iter()
        .any(|(d, r)| d == "personal" && r == "R_personal_path"));

    // Paladin book is kind=book, arxiv is kind=paper, zip is kind=archive.
    let kinds: Vec<String> = {
        let mut stmt = store
            .raw()
            .prepare("SELECT kind FROM document ORDER BY path")
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    assert_eq!(kinds, ["book", "archive", "paper"]);

    // (b) re-run: zero new decisions, zero new documents.
    let stats2 = triage_intake(&mut store, &nl.join("intake")).unwrap();
    assert_eq!(stats2.imported, 0);
    assert_eq!(stats2.archives, 0);
    assert_eq!(stats2.personal, 0);
    assert_eq!(stats2.reimport_skipped, 7);
}

#[test]
fn lib_ussr_import_with_soviet_fields() {
    let tmp = TempDir::new().unwrap();
    let nl = fixture_tree(&tmp);
    let mut store = LibraryStore::open(tmp.path().join("library.db")).unwrap();
    let stats = import_lib_ussr(&mut store, &nl.join("lib_ussr")).unwrap();
    assert_eq!(stats.imported, 2);

    let (kind, lang, coll): (String, String, String) = store
        .raw()
        .query_row(
            "SELECT kind, language, collection FROM document WHERE path LIKE '%Химия%'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(kind, "book");
    assert_eq!(lang, "ru");
    assert_eq!(coll, "chemistry");

    let stats2 = import_lib_ussr(&mut store, &nl.join("lib_ussr")).unwrap();
    assert_eq!(stats2.imported, 0);
}
