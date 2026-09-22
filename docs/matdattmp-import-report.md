# matdattmp acquisition-bay import — report

Date: 2026-09-22. Source `/home/patrick/matdattmp` read-only throughout.
Command: `stacks-import matdattmp /home/patrick/matdattmp data/matdattmp data/stacks.db data/library.db`.
Exports staged in `data/matdattmp/` (duckdb CLI for CSV/xlsx, unzip/xz for
archives, python for pandas-split JSON and ledger flattening).

## Part A — documents (library.db, new `document` table, migration L2)

470 manifest rows → **469 documents** (one sha256 appears twice across
ledgers — INSERT OR IGNORE on the PK; the duplicate is the same payload).
Payloads referenced in place; `location_root` = `/home/patrick/matdattmp`
(a future move is one UPDATE). Companion text layers found for 184 rows.

| family | docs | | kind | docs |
|---|---|---|---|---|
| historical | 148 | | book | 192 |
| range-retries | 95 | | paper | 196 |
| modern | 91 | | report | 46 |
| government | 48 | | thesis | 23 |
| multilingual | 48 | | patent | 12 |
| resumed | 20 | | | |
| theses-patents | 13 | | | |
| reference | 6 | | | |

kind is inferred: path contains patent/thesis wins; government→report;
modern/resumed/range-retries→paper; rest→book. Manifest quirks handled:
`year` and `authors` arrive as strings, ints, or arrays; `page_count` may be
string or int.

## Part B — recipe sources (stacks.db; migrations 4–5 added
`recipe.outcome`/`outcome_score`, `provenance.note`, and the
`hydrothermal` operation token via CHECK-rebuild)

| source | in | inserted | skipped | rejected | notes |
|---|---|---|---|---|---|
| raccuglia_dark_reactions | 4,231 (3,955+276) | 4,231 | — | 0 | both SI CSVs; 232's outcome column is `outcome (actual)` — mapped |
| rapid_perovskites | 9,003 | 9,003 | — | 0 | 831 RunID_vial repeats (re-measurements) preserved via line-index keys |
| zeosyn | 30,165 xlsx rows | 23,962 | — | 6,203 blank separators | duckdb `read_xlsx` quirk: `stop_at_empty=false`, header=false, letter+1 columns |
| solgel_hackingmaterials | 148,024 | 148,024 | — | 1 | stray non-UTF-8 line; lossy decode; license_status=uncleared in every provenance note |
| mof_synthesis_condition | 46,701 | 46,701 | — | 0 | pandas-split JSON rehydrated to rows |
| ceder2 (2020-07-13) | 41,300 (31,782+9,518) | 36,398 | 4,902 content-dupes | 0 | dedupe vs existing `ceder_*` by sha256(doi, reaction_string) |
| precursor_genome | 1,035 | 1,035 | — | 0 | outcome categories → Outcome enum |
| gpss_352 | 352 | 352 | — | 0 | max phase weight fraction in outcome_score |
| alab_moesm3 + corrected | 57 + 57 | 114 | — | 0 | both provenance-tagged (see below) |

Total recipes in stacks.db after the wave: **2,758,055**.

**Intentional skips (verified, not imported)**: `SS_rxns_80806.json.gz`
contains exactly 80,806 records — count-identical to the already-imported
`lee_impurity_phase` corpus; skipped by policy and logged.
`solution-synthesis_dataset_2021-8-5.json` is **not present** in the
acquisition bay — nothing to skip; noted.

## Outcome distributions

- raccuglia: 4,231 rows with scores, range 1.0–4.0, **all integers in this
  export** (the SI publishes integer classes; the model stores REAL and the
  0.37 fixture tooth proves fractions survive exactly).
- rapid: 8,171 scored, 0.0–4.0 (crystalscore, real-valued).
- mof: 36,037 with yield% → outcome_score 0–1 (junk outlier max 8.1 from
  unparseable percent strings like "810"; noted, kept as published).
- zeosyn: percent_cryst/100 → 6,081 scored (source has values >100 — kept).
- gpss: max XRD phase weight fraction, 0.353–1.0.
- categorical outcomes (precursor_genome + alab): success 454, partial 127,
  failed 542.

## Unmapped operation tokens (landed as `other:<token>`)

- ceder2: `prepared` 28,790, `pressed` 13,421, `synthesized` 13,292,
  `cooled/cooling` 7,985, `pelletized` family ~1,300, `quenched` 1,599, …
  (same shaping/cooling enum gaps as the bootstrap wave).
- mof: `cool` 19,477, `prepare` 8,798, `wait` 7,699, `evaporate` 7,272,
  `remove` 3,978, `diffuse` 2,664, `purify` 893 (no enum tokens for these;
  `dissolve`/`crystallize` were remapped to dissolve/precipitate mid-wave).
- solgel: `deposition` 467, `other` 892, `pressing` 369.

## Teeth evidence (red→green)

- (a) outcome score: schema was right first time (REAL, no boolean) —
  negative proof: fixture with score 0.37 reads back exactly 0.37, and
  `outcome='sorta-worked'` via raw SQL fails the CHECK. Also hydrothermal
  token round-trips; 'autoclave' still CHECK-rejected (documented gap:
  autoclave maps to hydrothermal in these corpora).
- (b) idempotency: skip-guard neutered → re-run failed with
  `UNIQUE constraint failed: recipe.external_key`; restored → re-runs
  insert 0 (every matdattmp importer, fixture-level; and at scale: the final
  full run re-inserted only the intentionally rebuilt slices).
- (c) document sha256 == existing paper sha256: both rows coexist, paper
  row untouched (fixture test).
- (d) API unknown-param 400: covered by the same `reject_unknown_params`
  pattern teeth as the library endpoints, live-verified:
  `{"error":{"code":"invalid_query",...,"allowed":[...]}}`.
- Real bugs the wave caught: UNIQUE(name,version) forced line-index keys
  for raccuglia/rapid (duplicate titles/vials are real re-runs);
  run4 died mid-solgel on a blank line and the external-key idempotency
  made the resume exact (145,000 prior commits skipped, 3,024 tail rows
  inserted, total = 148,024 exactly).

## API verification (live, real DBs)

`GET /api/v1/documents?family=government` → DTIC silicate handbook row with
location_root/path; `?query=zeolite` → zeolite theses; detail by sha256 ✔;
`?bogus=1` → 400 invalid_query ✔; LibraryDocument schema served ✔.

## Deviations

1. `document` as a separate table (not generalizing `paper`) — different
   keyspace (manifest sha256 vs catalog sha256 can overlap, proven by tooth
   c), different shape; joins stay explicit.
2. RAPID per-row inorganic/acid identity is stock-solution level (the CSV
   carries mmol amounts, identity lives in inventory.csv categories) —
   imported as name-only stock materials; documented limitation.
3. ZeoSyn gel molar-ratio columns (70+) not imported into steps (they're
   composition-space features, not step quantities); noted for the
   analytics phase.
4. A-Lab: the acquired MOESM3 CSV already lacks the corrected-out
   Zn2Cr3FeO8 row, so both provenance variants carry identical content with
   distinct notes citing the original (41/58) and correction (36/57).
5. MOF/zeosyn rows without parseable procedure get a single
   hydrothermal/unspecified step so materials attach; sol-gel operations
   without units keep quantities out (no fabricated units).
