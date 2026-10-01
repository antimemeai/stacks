# stacks roadmap — 2026-09-29

Where this is going, in order. Not a spec; update as reality moves.

## Layout (agreed)

- `/opt/stacks/` — the code: this repo, deployed as the intake/setup tooling.
- `/srv/stacks/` — the data, owned by a `stacks` user/group:
  - `db/` — live SQLite catalogs (library.db, materials.db, stacks.db)
  - `corpus/` — processed documents (harvest_extract, brauer, asm-hb-v18, incoming library)
  - `papers/` — curated reference PDFs + MANIFEST
  - `datasets/` — consolidated datasets
  - `intake/` — inbound spool the intake pipeline works from
  - `cold/` — packed archives (quarantine-archives, shard bags)

## Storage engine: SQLite, on purpose

- SQLite stays the stacks store by design, not default. Single-writer
  discipline: one intake/enrichment process writes, everything else reads.
- Hygiene the setup tooling must encode: WAL mode, `synchronous=NORMAL`,
  `busy_timeout`, checkpoint policy, `VACUUM INTO`/`.backup` snapshots,
  integrity checks.
- Periodic exports (Parquet/JSONL into `cold/`) as engine-independent
  insurance.

## Ecosystem context (fuzzy on purpose)

- This box will eventually host heavier formal infrastructure: PostgreSQL
  (likely +TimescaleDB) for sensor/telemetry streams from synthesis
  processes — that is NOT stacks' problem, but it changes the neighborhood.
- Once Postgres exists, it also becomes the warm spare: back the stacks
  SQLite DBs into it during lulls. Doubles as rehearsal if stacks ever
  ports engines. Unhurried.
- Multiple lab servers eventually; messaging (MQTT/NATS) is the seam,
  later. Blob/object layer only when artifact volume demands it.

## Sequence

1. [x] Pack quarantine reference clones (2026-09-29, `cold/quarantine-archives`)
2. [x] Stop stray stacks-api process; checkpoint + integrity-check the DBs
      (2026-09-29: all three `ok`; 19G materials WAL folded in, freed ~19G)
3. [x] Move corpora to `/srv/stacks/` (2026-09-29: db/ corpus/ papers/ cold/;
      repo keeps compat symlinks data→db, corpus, papers until the /opt cutover)
4. [ ] Transition intake logic to `/opt/stacks/`; sort out config/paths
5. [ ] Intake pipeline v2 (design agreed 2026-09-29):
      - `inproc` subcommand: explicit invocation (later an API), moves a
        file/dir/archive into `/srv/stacks/intake/` and enqueues each item
        in a new `inproc_queue` table. No watchers, no daemons.
      - Intake drains the queue per item, per-item transactions, resumable:
        extract (text layer, DOI/title regexes) → enrich → classify →
        stamp in + move file spool → corpus.
      - Bonafides fast path: sha256 match in paper/paper_enrichment with
        complete NL metadata (non-junk title, DOI or arXiv id, openalex
        topics with subfield, s2 tldr, OA status) → stamp straight in.
      - Enrichment = NL's chain, ported: CrossRef → OpenAlex → S2 →
        Unpaywall, with graceful fallback: batch-by-DOI (OpenAlex OR
        filters, ~50/req) → individual DOI → arXiv API → title search →
        DLQ. Rate-limited, retry-capped (max ~3 attempts, NL rule).
      - Category is the bare minimum for entry. Anything unclassifiable
        (no text layer, no identifier, no match, junk title) goes to a DLQ
        with its reason; `dlq_review` lets a human or model assert a
        category/doctype and re-enter the item at classification.
      - Papers vs documents decided at classify time: bibliographic match
        → paper (rich schema + chunks + embeddings, fastembed
        all-MiniLM-L6-v2, same model NL used); else document — but still
        requires a category or DLQ.
      - Migrate neuroticd state/journal (only record of 10k prior intake
        attempts) before reprocessing the NL intake pile. DONE (2026-09-29):
        schema v8 `nl_ledger` + `stacks-import migrate-ledger` (state map +
        journal summary + wave_staging lineage rows). Drain prechecks
        consult nl_ledger/paper/document before any API call: NL-exhausted
        files (api_failed, or pending_api with 3+ journal events) go
        straight to DLQ, legacy-enriched papers stamp without re-spend, and
        wave_staging duplicates cost zero API calls (sha256 fast path).
      - Secrets: S2 key lives at /srv/stacks/post_it.txt (640, patrick:stacks)
        as a stopgap; clients resolve env `S2_API_KEY` → post_it.txt →
        unauthenticated. Replace with something smart later (user's words).
      - NL tree stays read-only until hash-verified at destination.
6. [ ] Then: APIs and interfaces, for humans and bots.

## Parking lot

- Unify the tooling into one `stacks` CLI (subcommands: inproc, drain,
  status, dlq, ...; one binary in /opt/stacks on PATH) instead of the
  current stacks-import grab-bag. Do this before the API work so the API
  and CLI share one command surface.
- inproc-drain needs progress heartbeat logging (currently only a final
  JSON summary; monitoring = inproc-status against the DB).
- Move the fastembed model cache to /srv/stacks (default still points at
  ~/neurotic_library/.fastembed_cache).
- Sweep the 9,329 content-duplicate files left in NL intake after the
  drain proves out; then enqueue lib/ + lib_ussr/ + datasets/ PDFs.
- Kernel update pending reboot (from gh install, 2026-09-29).
- **Local enrichment leg (user-blessed 2026-10-01):** add a scimag-local
  enrichment step to the drain before the API ladder. Local holdings:
  82.5M DOIs (Sci-Hub 2020 list) + 64.2M-row scimag parquet
  (title/authors/year/journal/ISSN/MD5) + ISSN→OpenAlex 4-level
  classification (journal_classification.parquet, 34.6M papers covered).
  Fuzzy title match → DOI + journal → subfield, no network. Targets the
  enrichment-exhausted DLQ backlog (junk-title PDFs). Build the lookup as
  an indexed sqlite/duckdb sidecar table, not in library.db.
- Inventory debt (user 2026-10-01): we don't have a good map of what
  exists outside stacks; interim crawl report at
  docs/external-inventory-2026-10-01.md until stacks itself is the
  system of knowledge.

## Guardrails

- Forensics originals remain sacrosanct per ~/AGENTS.md — nothing in this
  roadmap touches them.
- neurotic_library is frozen (sunset 2026-09-22); treat as read-only source.
- No live-DB file copies; snapshots via SQLite backup API only.
