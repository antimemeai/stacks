# Extraction wave (b) — report

Date: 2026-09-22. Scale-out of the proven Brauer pilot pipeline. Sources
read-only (NL sacrosanct); work files under `~/stacks/corpus/` (gitignored).
All records loaded through the same loader path (stacks-core types,
`stacks-import pilot-load`), external keys `sciencemadness:<book>:<slug>` /
`asm-hb-v18:<slug>`, status=draft, extraction_method=llm_extracted,
confidence per recipe, provenance with sha256 + locator + extractor_version.

## Totals

| source | recipes | steps | step_materials | quantified materials | steps w/ temperature |
|---|---|---|---|---|---|
| Brauer pilot (wave a) | 55 | — | — | — | — |
| Brauer wave b (halogens + sulfur chapters) | 90 | — | — | — | — |
| **Brauer total** | **145** | 288 | 536 | 152 | 98 |
| ASM Handbook Vol 18 | 4 | 11 | 9 | 0 | 6 |

Brauer coverage now: the fluoride chapter complete (pilot), plus the
chlorine/bromine/iodine chapter and the sulfur chapter's sulfide/sulfane/
chlorosulfane/oxy-acid sections. Brauer has ~590 further candidates
(sulfur chapter tail, selenium/tellurium, nitrogen, phosphorus, metals
sections) — deliberately not exhausted this run.

## Confidence distributions

Brauer (145): 0.72–0.78 ×9; 0.80 ×20; 0.82 ×17; 0.83 ×5; 0.85 ×51;
0.86 ×3; 0.87 ×20; 0.88 ×15; 0.90 ×5. ASM (4): 0.55–0.7 (handbook protocol
descriptions, honestly discounted).

## Deterministic verification (`stacks-import verify-extraction`)

- **Brauer: 145 recipes — 71 pass / 74 warn / 0 FAIL** (exit 0).
  docs/brauer-verify-report.{md,json}.
- **ASM: 4 recipes — 4 pass / 0 warn / 0 FAIL** (exit 0).
  docs/asm-verify-report.{md,json}.
- WARN clusters (145 Brauer): 74 formula warns, all the OCR-mangle family
  from the pilot (subscript/1-l/g-2 mangling repaired during extraction).
  Coverage warns 1,656 — dominated by neighbouring sections on widened
  pages plus equation weight numbers, same character as the pilot.

## Verifier changes this wave (the wall it climbed)

1. **Spaced-digit page numbers**: OCR prints some headers as "2 7 3" — the
   page index now squashes whitespace before parsing (both probe windows).
2. **Probe window 3→6 lines**: section-start pages carry the number deeper.
3. **"half" word**: `half an hour` is a literal 0.5 duration — added to the
   numeral table (now f64).
4. **Cross-reference locators**: parse cap +3 → +4 (a method defined by
   reference to another page's procedure gets a wider printed range).

## Extraction-side lessons (what the verifier caught this time)

- **I reintroduced the derived-midpoint bug** from the pilot fix: 8 records
  had range midpoints as nominal values (e.g. 55 g for "50-60 g", 27.5 mm
  for "20-35 mm"). Fixed to range-max nominal per the established
  convention, re-imported, re-verified.
- **Printed-word volumes**: Brauer writes "four liters"/"one liter" for
  volumes; my mL conversions (4000 mL) were unverifiable. Data fixed to
  liter quantities with the printed values; the verifier's word table
  covers them.
- **A file-overwrite incident**: batch 1's JSONL was overwritten by batch 2
  (same filename); the 15 records survived only in the DB. Recovered by
  dumping them from the DB into `extracted-wave-b1-recovered.jsonl`
  (exercising the model's JSON round-trip), fixing the two midpoint bugs in
  them, re-importing. All loader rejections this wave were my own format
  drift (`other:` shapes), caught at the boundary — the fixer script now
  normalizes before load.
- **Provenance hygiene**: the delete/reload cycles left 52 orphaned
  provenance rows (recipe deletion doesn't cascade to provenance — the FK
  points the other way). Cleaned manually; the loader or a delete helper
  should handle this. Also retagged 90 wave-b provenance rows from the
  loader's default `pilot-1` to `wave-b` (the CLI default source profile
  masked the wave identity; the generalized CLI now takes dataset/book/path/
  sha256 args).

## ASM Handbook Vol 18: honest yield assessment

From the olmocr parquet (`matsci_stuff/ocr_parquet`, row
`15a06c26…`, 5.4 MB text → 1,127 pseudo-pages for verification). **This is
a handbook, not a recipe book**: overwhelmingly review prose (test methods
discussed without parameters, formulation practice without quantities).
Genuinely procedural content found by scanning for quantity markers
(g/L, wt%, "immersed", "heated to"): 4 records — TiN coating/wear testing
(modified ASTM G65), CVD/PVD coating temperature regimes, carbon-carbon
aircraft brake manufacture (CVI + char + graphitization, real parameter
ranges), cemented-carbide sintering (dewax 400-500C, sinter 1300-1600C,
HIP <100 MPa). Confidences 0.55-0.7. **Stopped early per the brief rather
than forcing review prose into recipes.** The 3 knowledge-note candidates
(nanoindentation protocol, dye-penetrant crack detection, detergent
formulation practice) were dropped: procedure-adjacent but not
recipe-shaped, and the loader's target-material requirement would have
meant fabricating materials.

## Cost and scaling (updated from the pilot)

- Brauer wave b: 90 recipes in ~3 chapters at ~2-4 page-reads per batch and
  one JSONL write per batch; throughput ~25-50 recipes/hour of agent time,
  comparable to the pilot (~50/hr incl. fixes). Per-book calibration held:
  the segmenter and boundary rules needed zero changes within Brauer; the
  verifier needed the spaced-digit/probe-window fixes — a per-OCR-engine
  cost, now paid once.
- The dominant error class is my own JSONL shape drift, caught 100% at the
  loader boundary; the normalize-before-load fixer should move into the
  loader (or be killed by generating from the served JSON Schemas).
- ASM confirms the brief's warning; the recommendation stands: recipe
  extraction waves should stay on recipe corpora (Brauer completion,
  Schlessinger, Inorganic Preparations) and treat handbooks as
  reference-text for FTS/semantic search, not extraction feedstock.

## Remainder

~500 Brauer candidates (Se/Te chapter, nitrogen, phosphorus, metals
sections), Mellor + the other OCR'd sciencemadness books, ASM vols 5 and 19
in the same parquet set (same shape, likely same low yield), then
extraction wave (c) — the actoprotector corpus.
