# Dataset registry — report (Wave I, final wave of the initial campaign)

Date: 2026-09-22. Goal: agents can discover and reason about every data
payload on the host without moving bytes. Nothing copied; all registered in
place. Schema: `dataset` table in library.db (migration L6) — the library
DB is the catalog home. Descriptions are curator-written from the campaign
surveys (NL datasets census, entrance_bay transport history, wave E/F/G
imports); entries whose contents were never surveyed say so in the
description ("Description uncertain — needs a survey pass": p2tp, ddoracl,
matsci_stuff).

## Registered: 37 datasets, 1.83 TB accounted

| status | count | members |
|---|---|---|
| registered | 18 | movement (189 GB / 9,933 files), zero-to-cad, sketchgraphs, gigacorpus (sha256 **sidecars** honored: acquired.tar.zst.sha256), harvest, course-skill-atlas, sciencemadness, entrance_bay datasheets/arxiv/mathnet/jprs/hfro×2/fabrica/lean/osti_curated/p2tp/ddoracl/matsci_stuff/actoprotectors-stub |
| migrated | 7 | chem-recipes, materials-project, oqmd, cod (PARTIAL — corrupt archive, see materials-import-report), topology-top (inventory only), yum, matdattmp |
| queryable | 6 | chembl (in place), sciencemadness(also), yumd spine, stacks.db, library.db, materials.db |
| preserved_original | 5 | move_staging zips (move 8.3G, move_harvest_1 129G, tranche2 44G ⊂, harvest.zip 17G) + staging/entrance_bay 25 transport zips |
| extraction_queue | 1 | stacks/corpus/harvest_extract (4,530 actoprotector PDFs — extraction wave (c)) |
| missing | 0 | verify pass: all 37 paths exist |

Self-registration: stacks.db, library.db, materials.db are rows (agents
asking "where is X" should find the catalogs too).

## Verification

- `stacks-import datasets verify data/library.db` — 0 flips (all paths
  present). Teeth: fixture with a deleted path must flip to
  `status='missing'` (and back to `registered` when the path returns);
  red-observed with the flip neutered (verify produced `[]`, assertion
  failed), restored → green.
- Unknown-param 400 (conventions): `?bogus=1` →
  `{"error":{"code":"invalid_query",...}}` live-verified.
- Live API: `/api/v1/datasets?status=migrated`, `?domain=crystallography`,
  `/api/v1/datasets/cod` (full row incl. honest description),
  `/api/v1/datasets/status` (counts + 1.83 TB), `Dataset` schema served.
- Registration is idempotent by `name` (upsert; re-run updates stats in
  place and preserves `missing` flags across re-registration).

## Notes and honesty flags

- Sizes/file counts are freshly walked metadata (du-equivalent) — e.g.
  movement: 189 GB / 9,933 files (jpg-heavy). The 100s-of-GB blobs were
  *not* hashed; gigacorpus uses its existing sidecar.
- `cod` description and status carry the archive-corruption fact; the
  registry is where that wound stays visible.
- `matdattmp` path is relative to home (`matdattmp`, root `/home/patrick`);
  same for move_staging/staging/yum/stacks rows.
- entrance_bay bundles register as one row per logical holding
  (hfro_archiv+hfro_dat share a row; noted in status_note).

## Campaign closing note (for parent to append to docs/initial-campaign.md)

The initial campaign is complete: recipes (stacks.db, 2.76M), documents
(library.db, 7,048 + papers/chunks), computational materials
(materials.db, 2.34M), the LLM-extraction path proved and verified
(Brauer pilot), dense+hybrid semantic search parity-gated, and every data
payload on the host discoverable through this registry. Open remainder:
extraction waves b/c (matsci OCR, actoprotectors corpus), COD archive
re-acquisition, TOP4040 label re-survey, cross-source material identity,
equipment/inventory pillar from the datasheets feedstock.
