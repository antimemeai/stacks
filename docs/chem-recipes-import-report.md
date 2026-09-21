# chem-recipes bootstrap import — validation report

Date: 2026-09-21. Importer: `stacks-import chem-recipes data/import data/stacks.db`
(JSONL extracted from `neurotic_library/datasets/chem-recipes/processed/recipes.duckdb`
with the duckdb CLI; no duckdb/parquet crates in the workspace).
Output: `/home/patrick/stacks/data/stacks.db` (3.2 GB, WAL, schema version 3).

## Counts: source vs imported

| source | rows in | recipes imported | steps | step_materials |
|---|---|---|---|---|
| ceder_solid_state | 19,488 | 19,488 | 98,247 | 63,226 |
| ceder_solution_based | 35,675 | 35,675 | 302,276 | 63,846 |
| lee_impurity_phase | 80,806 | 80,806 | 80,806 | 249,138 |
| materials_project | 41,300 | 41,300 | 277,496 | 236,136 |
| ord | 2,310,911 | 2,310,911 | 2,310,911 | 10,794,007 |
| **total** | **2,488,180** | **2,488,180** | **3,069,736** | **11,406,353** |

Materials: 2,097,670 total — 64,520 `formula`-kind, 2,033,150 `molecule`-kind.
Provenance: 2,488,180 rows, exactly one per recipe (all
`kind=dataset_import`, `extraction_method=structured`, `confidence=1.0`).
Invariants verified in the live DB: zero formula-kind materials without a
formula; zero recipes without a provenance row or external key.

## Idempotency / dedupe

- Recipes carry `external_key = <source>:<source_id>` (partial unique
  index). Re-running the full importer: **2,488,180 seen, 0 inserted, 18 s**.
- Materials carry `identity = formula:<f>` / `smiles:<s>` (partial unique
  index on `(kind, identity)`), get-or-created through an in-memory cache.
  ORD reuse: 11,071,624 attachments onto 2,033,150 molecule rows.
- ORD SMILES are **not canonicalized** — dedupe is by exact string, so the
  same molecule under different SMILES spellings lands as separate rows.
  Accepted for this slice; canonicalization belongs to a future
  molecule-identity pass.

## Field null-rates and coverage

Recipe-level (all 2,488,180): `target_material_id` NULL 0.4% (9,914
lee_impurity_phase rows have no target formula; 144 ORD rows have no
products); `narrative` NULL 75.2% (lee has no reaction strings; 1.79M ORD
rows have empty `reaction_smiles`); `synthesis_type` NULL 0.0%.

`step_material.quantity` NULL-rate is **100%** — see "amounts" below.

Atmosphere (inorganic only; ORD `atmosphere` is 100% NULL):
43,883 recipes carry an atmosphere string. 39,445 mapped to the closed enum
(air 32,370; n2 2,426; ar 2,290; o2 1,404; h2 955); 4,438 fell to
`Atmosphere::Other` (520 distinct strings) — the top values are **crucible
materials, not atmospheres**: `alumina` (1,087), `platinum` (603), `water`
(192), `pt`, `silica`, `corundum`, `quartz`. The source column evidently
mixes atmosphere with container/media; honest mapping is impossible without
deeper parsing, so these stay `other:` with the raw string. Composite values
(e.g. `air,alumina`) map the first recognized segment and keep the raw
string in `atmosphere_note`. `has_atmosphere_info=false` → NULL, always.

## Unit vocabulary observed (inorganic conditions)

`°C` 969,728; `h` 593,198; `min` 22,563; `d` 3,392. Nothing else. Source
time-unit strings (`h, hr, hrs, hour, hours, min, minutes, d, day`) all
normalized; zero drops. Recipe-level `time_min/max` assumed to be **hours**
(source docs don't say; spot values like 0.034–1000 are consistent with h).
Operation-level `time_units` are explicit and were mapped. ORD conditions
use °C (493k rows) and h (933k rows).

## Operation token → enum mapping

534,759 inorganic steps mapped to the closed enum; 134,079 rows (222
distinct tokens) fell to `other:<token>`. Mapping rule: the free-text gerund
`token` wins when recognized (it is more specific than `action_type`), else
`action_type` (HeatingOperation→heat, Mixing*/SolutionMixing→mix,
LiquidGrinding→grind, DryingOperation→dry), else `other`.

Top unmapped tokens (candidates for a future enum or mapping pass):
`prepared` 32,833; `pressed`/`pressing`/`pelletized`/`compacted` family
≈33k (shaping — no enum token exists); `cooled`/`cooling`/`cool` ≈28k (no
`cool` operation in the enum); `synthesized`/`synthesised` 16,175;
`centrifug*` ≈10k; `collected`, `quenched`, `separated`, `decanted`,
`rinsing`, `sieved`. These are vocabulary gaps in the Operation enum
(shaping/cooling/separation operations), not importer failures.

## Amounts: dropped, deliberately

`precursors.amount` is non-NULL only for materials_project (192,058 rows)
and the values are garbage (range −1.2e17 … +3.2e17, no units). No
`Quantity` was fabricated from them; they are dropped and counted
(`amounts_dropped=192,058`). Consequently every imported
`step_material.quantity` is NULL — unquantified steps, "absence is data".
Stoichiometry remains recoverable from `reaction_string` narratives and the
ORD SMILES strings.

## Structural decisions (documented per plan)

- **Status**: all imported recipes are `draft`. The corpus is
  auto-extracted reference data nobody has vetted for execution; `deployed`
  would claim a review that never happened. Provenance + confidence 1.0
  records *where they came from*; status records *whether we stand behind
  them as runnable*. Promotion is a per-recipe decision later.
- **Steps**: ORD has no operations, so each ORD recipe gets one synthetic
  step (`other:ord_reaction`) holding all role-tagged materials and the
  conditions. Inorganic recipes with zero operations but conditions or
  precursors get one `other:unspecified procedure` step (82,981 of them,
  mostly lee_impurity_phase). Precursors attach to the first step (the
  source has no precursor↔step links).
- **Recipe-level conditions** (temp/time min/max, atmosphere) are applied
  as defaults to every step that doesn't specify its own; ranges become
  `Temperature::MinMax` / `Operator::Range`, equal endpoints become scalars.
- **Names** are deterministic: `<target> (<source>:<id>)`; `version=1`;
  `external_key` is the idempotency fence.
- **mp_id** lands in `provenance.locator`; ORD `dataset_id` likewise.
- **`procedure_text` (ORD, 2.3M non-null) not imported** — free-text prose
  is exactly what the model is designed to replace, and it would dominate
  DB size. It stays in the source duckdb, joinable via `external_key`.

## Performance

- Extraction (duckdb → JSONL): ~1 min; 1.8 GB total (procedure_text excluded).
- Full fresh import: **25.9 min** for 2,488,180 recipes / 14.5M total rows
  (~1,600 recipes/s), with import-time pragmas `synchronous=OFF`,
  `wal_autocheckpoint=0`, 2 GB page cache, 4 GB mmap (justified: DB is
  rebuildable from source and import is idempotent; a TRUNCATE checkpoint
  runs at the end). Batched commits of 5,000 recipes.
- An earlier un-tuned run managed ~160–680 recipes/s; the pragmas plus
  `prepare_cached` and single-lookup get-or-create gave ~10–17×.
- Idempotent full re-scan (all rows present): 18 s.

## Schema mismatches discovered (docs vs reality)

1. `precursors.amount` documented-ish as an amount; actually unitless and
   out-of-range garbage in the only source that populates it.
2. `atmosphere` (recipes) mixes atmospheres with crucible/media strings;
   `has_atmosphere_info` does not imply parseable.
3. Empty string, not NULL, encodes missing `doi`, `reaction_smiles`,
   `atmosphere` in both recipes and ord_reactions.
4. ORD `atmosphere` is uniformly NULL despite the column existing.
5. Negative `time_min`/`temperature_min` outliers exist (−14 h; −268 °C);
   imported as-is where `min <= max` (cryo temps are real; negative
   durations are source noise kept for honesty).
6. `recipes.duckdb` needed no parquet fallback: `ord_reactions` already
   holds the full 2.3M rows.
