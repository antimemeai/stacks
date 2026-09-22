# Library migration import — report

Date: 2026-09-22. One-shot migration of the frozen neurotic_library catalog
and chunk corpora into `/home/patrick/stacks/data/library.db` (sibling DB;
`stacks.db` untouched). Command:
`stacks-import library-import data/library-import <NL>/embeddings data/library.db`
(catalog parquet → JSONL via duckdb CLI, chunks read directly from the
embeddings sqlite files with plain rusqlite — the sqlite-vec extension is
not needed to read shadow tables). Full import: **72 s**. NL was strictly
read-only throughout.

## Counts

| entity | source | imported |
|---|---|---|
| papers | 9,106 (papers.parquet) | 9,106 |
| enrichments | 6,961 (enrichments.parquet — the design brief said 6,958; reality has 6,961) | 6,961 |
| movements | 20,538 | 20,538 |
| chunks | 357,527 (176 unique sqlite files) | 357,527 |
| embeddings | 549.2 MB of float32 blobs | 357,527 chunks with embedding (100%) |
| corpora | 177 subfields / 182 files | 175 corpora in DB |

- **Duplicate DBs**: 6 byte-identical (md5) pairs skipped: agronomy,
  anesthesiology, animal_science, aquatic_science, building_and_construction
  (all `..._161708` == `..._161620`), and `software_20260611_202759 ==
  software_20250611_2040`. The brief said 5 pairs; reality is 6. 176 unique
  files → 175 corpora because two same-named corpora from different stamps
  merged under one corpus name.
- **Idempotency** (sha256 PK, UNIQUE(corpus, chunk_id), movement row
  uniqueness): full second run inserted 0 papers / 0 chunks / 0 movements.

## Filename-join outcomes (chunk ↔ catalog)

- joined: 340,763 chunks (95.3%)
- **orphans** (filename matches no catalog row): 15,891 — top offenders are
  privacy/PL papers absent from the catalog: `DworkRoth2014_AlgorithmicFoundationsDP.pdf`
  (258 chunks), `Klabnik2019_RustOwnership.pdf` (176), `Nasr2023_ScalableExtraction.pdf`
  (148), `Chiesa2020_Marlin.pdf` (131), `BenSasson2018_STARK.pdf` (125).
- **filename collisions quarantined** (>1 candidate, disambiguation failed):
  873 chunks; top: `Bridging2025_KolmogorovDeepLearning.pdf` (79).
- **disambiguated**: 2,238 chunks where several catalog rows shared the
  filename but exactly one candidate's path contained the corpus subfield.
- Chunks are always imported (sha256 NULL when unresolved); the
  `chunk_quarantine` table records (corpus, chunk_id, filename, reason,
  candidate sha256s) — absence of the join is data, not deletion.

## Schema/design notes (deviations and findings)

1. **Dangling references kept verbatim**: the frozen source contains 1,089
   enrichments and 100 movements whose sha256 is absent from papers.parquet
   (contradicting the brief's "1:1"). `paper_enrichment.sha256` and
   `paper_movement.sha256` therefore carry **no FK** — history is preserved
   as-is; documented in the schema SQL.
2. **sqlite-vec slab layout** (verified against source, not documented
   anywhere we found): chunk ids are contiguous 1..N per file, vector
   ordinal == chunk id, vectors packed 1024 per slab blob (1,572,864 B) in
   `vec_chunks_vector_chunks00`; `vec_chunks_rowids.chunk_id/chunk_offset`
   are embedding-batch bookkeeping, NOT chunk references. Importer asserts
   contiguity per file and skips embeddings (with stats) if it ever breaks.
3. **FTS5 is external-content** (`content=chunk, content_rowid=rowid`) — no
   text duplication; index built once post-import via `'rebuild'` (the DB is
   append-only, so no triggers needed). 357,527 docs indexed; BM25 search
   smoke-tested (`solid state synthesis` → sensible hits).
4. **Movements applied**: `paper.path` reconciled to the latest `to_path`;
   movements themselves kept verbatim in `paper_movement`.
5. 1,197 papers with NULL subfield imported as-is, per brief.
6. `import_meta` records source path/mtime/imported_at/row_counts per run
   (2 rows currently — initial import + idempotency re-run).

## API (stacks-api, library.db opened read-only + query_only)

`GET /api/v1/papers` (query, subfield, year_min/year_max, has_doi, limit,
cursor), `GET /api/v1/papers/{sha256}` (+ enrichment + chunk_count),
`GET /api/v1/papers/{sha256}/chunks` (keyset on rowid),
`GET /api/v1/chunks/search` (FTS5 BM25; keyset on (rank, rowid) via
`bm25()` computed in a CTE — rank is not WHERE-able; cursor encodes
rank-bits:rowid), `GET /api/v1/library/status` (import_meta + counts +
per-corpus). Schemas registered: LibraryPaper, PaperEnrichment,
LibraryChunk. Live-verified against the real library.db (status, filtered
papers, detail, chunks, FTS search with snippets, unknown-param 400).
Semantic search (`/chunks/semantic`) intentionally absent — the embedding
blobs are in place for Stage D2; `chunk.has_embedding` flags them.

## Timing

duckdb export ~5 s; full import 72 s (2.5 GB read, 357,527 chunks + 549 MB
blobs, one transaction per corpus); FTS rebuild ~40 s; idempotent re-run
~75 s (re-reads embeddings for md5/dedupe, inserts nothing).

## Teeth evidence (red→green)

- (a) collision quarantine: with the disambiguation guard neutered (first
  candidate wins), the fixture test failed (`quarantined_filename_collision`
  0 vs expected 1 — the harddup chunk silently joined). Restored → green:
  unresolvable collision → sha256 NULL + quarantine row; corpus-path
  disambiguation resolves the resolvable one.
- (b) idempotency: with plain INSERT (no OR IGNORE), the second fixture run
  failed with `UNIQUE constraint failed: chunk.corpus, chunk.chunk_id`.
  Restored → green; also proven at scale on the real corpus (0 inserted).
- (c) API unknown-param rejection: with `reject_unknown_params` neutered,
  `?bogus=1` returned 200 with a full page; restored → 400 `invalid_query`
  with `details.allowed`.
- Fixture bugs that taught us something real: sqlite-vec's
  `vec_chunks_vector_chunks00` declares `rowid PRIMARY KEY` *without*
  INTEGER — a plain nullable column that sqlite-vec happens to fill; a naive
  fixture that omits it reads NULL (the fixture now populates it explicitly,
  matching reality).
