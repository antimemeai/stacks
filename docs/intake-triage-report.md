# Intake triage + lib_ussr import — report (Wave G)

Date: 2026-09-22. NL strictly read-only. Command:
`stacks-import intake /home/patrick/neurotic_library data/library.db`.
Schema: library.db migrations L3 (document `collection`, kind +`archive`,
`intake_triage` audit table) and L4 (kind +`article`) and L5 (Soviet fields
on documents). Every triage decision is a row in `intake_triage`
(path, decision, rule, reason, sha256) — the auditable artifact.

## Counts

**intake/** (18,912 files walked, sha256-hashed for dedupe):

| decision | files | rule |
|---|---|---|
| imported | 6,287 new documents | R_doc_* (pdf/txt/doc/md/…) |
| duplicate (document) | 8,578 | R_dedupe_document (entrance_bay mirror, staging dirs, prior tranches) |
| duplicate (paper) | 7 | R_dedupe_paper (already catalog papers) |
| personal | 3,852 | R_personal_path/R_personal_name — **flagged, not imported** |
| skipped (web scrape) | 103 | R_web (html?…/js/css/ds_store) |
| skipped (media) | 74 | R_media_ext (jpg/png/mov/…) |
| archive (recorded unopened) | 11 | R_archive_ext (zip/tar/…) |

The personal category is mostly the entrance_bay/school drop-zone dirs; per
discretion the report lists **counts only**, not contents.

**lib_ussr/** (281 files): all imported — kind book (171 pdf) / article (110
txt), original_language='ru', soviet_stratum='lib_ussr', transliterated_title
from the romanized filename stem, collection = topic dir. The 'translations'
dir is empty, so all 281 are language='ru'.

Documents total in library.db after the wave: **7,048** (469 matdattmp +
6,287 intake + 281 lib_ussr + 11 archives counted within intake).

## Teeth evidence (red→green)

- (a) personal rule: `PERSONAL_COMPONENTS` neutered → a `letters/` fixture
  PDF was imported (stats.personal 1 ≠ 2 — fail); restored → personal files
  produce triage rows and zero document rows.
- (b) idempotency: fixture re-run → 0 imported, 0 archives, 7
  reimport_skipped. At scale: second full run walked 18,912 files and
  inserted nothing (triage table makes re-runs cheap — no re-hashing).
- (c) unknown-param 400: `?bogus=1` → `{"error":{"code":"invalid_query",…}}`,
  live-verified.
- Hard-won honest bug: **INSERT OR IGNORE makes CHECK violations silent
  skips** — a lib_ussr `.txt` with kind='article' vanished without error
  (kind list lacked 'article'). Caught by the fixture count assertion;
  fixed by kind extension (migration L4) + a schema comment recording the
  lesson. Also: filenames are romanized in lib_ussr, so language follows
  the collection, not the filename charset (first pass wrongly marked all
  281 as 'en'; rebuilt).

## API verification (live)

`?family=lib_ussr` → book/ru rows with soviet_stratum + collection;
`?collection=Paladin Press Collection` → book rows; `/documents/status` →
documents=7,048, triage_decisions + by_collection; unknown-param → 400.
Schemas: LibraryDocument includes collection/original_language/
transliterated_title/soviet_stratum.

## Timings

First run: 28 min (sha256 of ~19k files incl. multi-GB PDFs, one tx per
file). lib_ussr-only re-runs: ~8 s. Intake re-runs: seconds (triage-table
short-circuit).
