# External Inventory — data/resources OUTSIDE stacks management

Date: 2026-10-01. Scope: everything on this machine not already managed by
stacks (`/srv/stacks` corpus/db/intake, migrated NL literature library +
embeddings, chem-recipes, materials.db, registered NL datasets). Interim map
until stacks can answer "what do we have" itself.

Rules honored: read-only survey; forensic images/backups listed only
(~/AGENTS.md — sacrosanct originals); personal material recorded as
name + size + file count only. Venvs/caches/.git noted in one line each.

## /srv outside /srv/stacks

`/srv` contains only `stacks/` — nothing external there.

## /home/patrick — top-level loose files

| Path | Size | What | Status |
|---|---|---|---|
| ~/oxide.zip | 49G | zip snapshot of the oxide project (Sep 29) | archive |
| ~/owfjs.zip | 2.3G | large zip, content unknown (Sep 30) | unknown |
| ~/RECOVERY-RESUME.md, AGENTS.md, shell rc files | <50K | notes/config | active |

## Forensics (SACROSANCT — originals never opened; listing only)

2.5T total. Protection register: `forensics/maintenance/kimi-retirement-2026-09-20/protected-originals.jsonl` (17,906 inodes). See `forensics/AGENTS.md`, `forensics/ORIGINALS.md`, `forensics/recovery-lab/RESUME.md`.

| Path | Size | What | Status |
|---|---|---|---|
| recovery-lab/ | 1.4T | main case workbench: cold-storage/ 452G (11 compressed read-only gather archives), vault-derived-storage/ 254G, viewer-v2/ 22G (active viewer data: catalog/chronology/presentation sqlite 9.6+6.0+2.6G), content-vault 7G, gather 4.1G | active (paused per AGENTS.md 2026-09-19) |
| prev/ | 480G | prior recovery round: bwd_rescue 331G, omm.img 150G (ORIGINAL images) | frozen |
| minipc-rescue/ | 477G | minipc-sda.img 477G (ORIGINAL image) + carve logs/checksums | frozen |
| old-mbp-2015.raw | 128G | ORIGINAL MacBook raw image | frozen |
| recovery-audit/2026-09-12 | 3.9G | audit snapshot (omm.snapshot.sqlite 353M) | archive |
| maintenance/ | 2.7G | ~25 maintenance receipt dirs (compression waves, protection register, handoffs) | archive/active ledger |
| recovery-tools/ | 1.5G | reusable Go/Python forensic toolkit (incl. sqlite-dissect test corpus) | active |
| recovery-viewer/ | 550M | viewer app source; served by systemd user unit recovery-viewer-v2 | active service |
| systemrescue-13.01-amd64.iso (+sha256) | 1.3G | rescue boot ISO | archive |
| carve-ubuntu[.1-7] | ~460K | carve output dirs (small) | frozen |
| photorec-mbp[.1/.2], photorec.log/.ses, undelete-mbp | small | PhotoRec runs/sessions for MBP | frozen |
| handoffs/, operator-notes/, searchkit-mac-handoff-2026-09-15(.tar) | ~1.3M | notes and Mac handoff bundle | archive |
| RESUME.md, README.md, ORIGINALS.md, AGENTS.md | small | case documentation | active |

## neurotic_library (frozen sunset source, 1.2T — partially inventoried)

Migrated to stacks (one-liners only): catalog/ (9.3M), lib/ (24G literature),
lib_ussr/ (1.4G), embeddings/ (2.5G, 182 per-field sqlite vector DBs),
datasets registered subset (chem-recipes 8.3G incl. recipes.duckdb 1.8G).
Everything below is NOT yet migrated.

| Path | Size | What | Status |
|---|---|---|---|
| datasets/zero-to-cad | 326G | CAD dataset, raw/ only (processed/ empty) | frozen raw |
| datasets/movement | 176G | movement dataset, raw/ only | frozen raw |
| datasets/entrance_bay | 159G | research bundle: mathnet 54G, arxiv 52G, materials-project-dos-parquet-tars 27G, datasheets 15G, hfro_archiv 7.9G, jprs 1.7G, fabrica 1.2G, lean 903M | frozen raw |
| datasets/gigacorpus | 97G | corpus tarballs: acquired.tar.zst 97G + orthogonal/meta-tuning/eval shards | frozen raw |
| datasets/materials-project | 73G | Materials Project dump | frozen raw |
| datasets/sketchgraphs | 63G | SketchGraphs CAD sketches | frozen raw |
| datasets/yum | 58G | culinary corpus source (see ~/yum derived spine) | frozen raw |
| datasets/chembl | 47G | ChEMBL 37: chembl_37.db sqlite 29G + chembl_37.duckdb 13G | frozen (queryable) |
| datasets/oqmd | 20G | OQMD materials DB dump | frozen raw |
| datasets/harvest | 18G | harvested collection | frozen raw |
| datasets/course-skill-atlas | 18G | course/skill atlas data | frozen raw |
| datasets/cod | 18G | Crystallography Open Database | frozen raw |
| datasets/topology-top | 2.9G | topology dataset | frozen raw |
| datasets/sciencemadness | 1.7G | sciencemadness forum scrape | frozen raw |
| datasets/zero-to-cod | 100K | tiny COD derivative | frozen |
| intake/ | 45G | unprocessed acquisition staging: wave_staging 16G, entrance_bay 13G, actoprotectors 4.1G, acquired 4G, school_mess 2.8G, "GUN PDF's" 2G, Paladin Press 1.4G, unclass_retry 1.1G, assorted book/manual bundles | frozen backlog |
| archive/ | 12G | research 9.3G, experiment_outputs 1.8G, laser_project 314M, li_articles_grab 48M, plans 7.2M, correspondence 3.1M, memories 356K, configs 184K | frozen archive (personal dirs counted, not opened) |
| metadata/ | 1.7G | doi.log 1.5G (~62.8M DOIs), _test_2M.parquet 160M, journal_classification.parquet 2.6M, scimag conversion/classification scripts + logs, ACADEMIC_TORRENTS_CATALOG.md | frozen, unique metadata |
| scripts/, tests/, tool_room/, quarantine/, seed_crystals/, pylons/, specs/, outposts/, operations/, logs/, intake2/, neurotic_materia/ | <15M | operational code/notes of the old NL system | frozen |
| coding_swarm_research_supply_crate/ (+ .zip 76K) | 192K | small research supply crate | frozen |
| docs/ | 224K | field guides, ADRs, cartography essays, vector-storage notes | frozen |
| Root docs: MATERIA.md, SHOPPING.md (172K), ERRATA.md, NOTES.md, DOCTRINE.md, CLAUDE.md, WAVE_CATALOG.md, EMBEDDING_INFRASTRUCTURE_REPORT.md, FUTURE_ACQUISITIONS.md, DEEP-RESEARCH.md, NEGATIVE_SPACE.md, OTHER_TECH_TREES.md, LIBRARY_CARD.md, AGENTS_START_HERE.md, LETTER_FROM_ARCONAUT..., README | ~400K | operational/doctrine docs | frozen |
| ca-certificate-do-db.crt | 4K | DigitalOcean DB CA cert | config |

## personal-archive (119G — counts only, contents not inspected)

| Path | Size | Files | Status |
|---|---|---|---|
| videos/ | 54G | 2,820 | archive |
| old-laptop/ (apotheosis 29G) | 29G | 143,969 | archive |
| home-loose-2026-09-19/ | 24G | 3 | archive |
| backups/ | 11G | 16,489 | archive |
| photos/ | 1.9G | 55,036 | archive |
| documents/ | 1.9G | 1,570 | archive |
| memoir/ | 36K | 1 | archive |

## forensics-adjacent / recovery staging dirs at home level

| Path | Size | What | Status |
|---|---|---|---|
| staging/entrance_bay_misc | 52G | unsorted entrance_bay pull (datasheet-harvester, datasheets, ddoracl, eduftp, fabrica, hfro_*, lean, ...) | active backlog |
| staging/zip-leftovers-2026-10-01 | 344M | leftover zips | backlog |
| staging/ scripts + sort.log, entrance_bay/ (36K: TRANSPORT.md) | small | sorting scripts/log for the above | active |
| projects-archive/ | 154G | autopsy.zip 104G, recov-sd.zip 46G, arconaut.zip 3.7G, extract.zip 31M | archive |
| cleanup/quarantine | 1.4G | quarantined junk (incl. permission-denied root-home-vestige) | quarantine |
| cleanup/dupwork | 66M | duplicate-analysis workspace | archive |
| cleanup/reports | 21M | inventories, ingest logs, home-cleanup-2026-09-24 receipts | ledger |
| old_laptop/ | 158M | partial old-laptop rootfs copy (boot 129M, etc 30M; home empty) — not a mount | frozen fragment |
| supply_crate/ | 92G | materials-science supply crate: synthesis.zip 33G, matbench.zip 29G, reactions.tar.zst 21G, docs 3.6G, mlip 2.1G, thermo 1.4G, pipeline 1.3G, topo 1.1G, software 431M, data 247M, s4e 173M, toolmap 50M, fieldguide, elags/audit/logs zips, VASP_6.5.1 .rar 127M, manifest/handoff READMEs | archive (curated) |
| clones/bulk_extractor-2.1.1 (+tar.gz) | 189M | bulk_extractor source tree | tooling |

## Project dirs

| Path | Size | What | Status |
|---|---|---|---|
| oxide/ | 49G | materials/DFT campaign: archives/ 49G (oxide-campaign-20260924.tar.zst 43G, remainder tar 5.9G, reclamation dir 156M), runs 4.7M, scripts/notes/proposals/tests, potpaw_LDA/PBE tgz ~56M, vasp.6.5.1.tgz 72M, lab journal + catalog docs | active/archive mix |
| matdattmp/ | 22G | materials-data acquisitions: modern 9.5G, historical 9.5G, government 1.7G, multilingual 359M, theses 318M, reference 87M, patents 24M + ledgers, scripts, SHA256SUMS, MANIFEST/OBJECTIVES | active project |
| yum/ | 31G | culinary corpus + spine.duckdb 13G (8.8M recipes / 2.39M-entity ingredient spine, built 2026-07-05), images/ 18G, yumd/ Go web service source (89M incl. binary) | active project |
| enhance/ | 17M | scrubbed public DFIR toolkit (recovery-tools pkg ~350 modules, recovery-viewer, audit/batch scripts, SCRUB-REPORT) | active (publication-ready) |
| fans/ | 16M | fan-control web app (app.py, fans.service template, venv) | active utility |
| go/ | 4.7G | GOPATH: bin/golink + pkg cache | tooling (mostly cache) |
| nl-assessment-2026-09-23/ | 58M | one-shot NL catalog assessment: ASSESSMENT.md, before/after catalogs, paper candidates/watchlists, registry proposals (OSTI/DoS), survey + transport scripts | archive (snapshot) |

## Databases outside stacks (excluding venv/cache/.git)

| Path | Size | Content (best guess) |
|---|---|---|
| yum/spine.duckdb | 13G | culinary ingredient/recipe spine (see above) |
| neurotic_library/datasets/chembl/raw/.../chembl_37.db | 29G | ChEMBL 37 SQLite release |
| neurotic_library/datasets/chembl/chembl_37.duckdb | 13G | ChEMBL 37 DuckDB conversion |
| neurotic_library/datasets/chem-recipes/processed/recipes.duckdb | 1.8G | chem-recipes processed (registered in stacks) |
| neurotic_library/embeddings/*.sqlite (182 files) | 2.5G total | per-field literature embeddings (migrated to stacks) |
| neurotic_library/metadata/*.parquet | 163M | scimag 2M-sample test parquet + journal classification |
| forensics/recovery-lab/viewer-v2/data/{catalog,chronology,presentation}.sqlite | 18.2G | active recovery-viewer databases |
| forensics/recovery-lab/viewer-v2/data/field-indexes/*, source-indexes, name-search, chronology-initial | ~2.7G | viewer index DBs |
| forensics/recovery-lab/cold-storage/*/{index,browse}.sqlite (~20 files) | ~6G | frozen indexes inside compressed gather archives |
| forensics/recovery-lab/source-catalog/omm.sqlite; recovery-audit/.../omm.snapshot.sqlite | 353M each | OMM source catalog + snapshot |
| forensics/recovery-lab/viewer-v2/web-relationships-2026-09-17/*.sqlite (5) | ~560M | web-artifact relationship analysis |
| forensics/maintenance/{hidden-video-removal.../frames-before.sqlite, kimi-retirement.../files.sqlite} | 505M/36K | maintenance snapshot/census DBs |
| personal-archive (various small app DBs: game saves, browser, Xmind/draw.io) | <1M each | personal app leftovers — not inspected |
| forensics/recovery-tools/.../sqlite-dissect tests (~25 small .db) | <1M | tool test fixtures |

## Systemd user services (~/.config/systemd/user/)

- recovery-viewer-v2.service — recovery viewer v2, python3 server.py on forensics/recovery-lab/viewer-v2/case.json
- stacks-inproc-drain.service — stacks intake drain loop (stacks-managed)
- stacks-mcp.service — stacks MCP server, 127.0.0.1:8433 (stacks-managed)
- All three enabled in default.target.wants. fans/fans.service is a template file only, not installed as a user unit.

## Caches/toolchains (existence only)

.cache 6.1G, neurotic_venv 5.6G, .rustup 1.8G, .codex 1.2G, .cargo 324M,
.kimi-code 299M, .duckdb 56M, .npm 58M, .local 59M, .config 676K.
stacks/ working copy itself 49G (incl. target/ build) — managed, excluded.

## Highlights (10 most significant/surprising holdings)

1. neurotic_library/datasets/zero-to-cad (326G) and movement (176G) are raw-only — processed/ dirs empty; not registered anywhere.
2. entrance_bay exists in three places: NL datasets (159G), NL intake (13G), and ~/staging/entrance_bay_misc (52G) — overlap/dedup unresolved.
3. yum/spine.duckdb (13G) is a polished, documented standalone product DB (8.8M recipes, 2.39M-ingredient spine) living entirely outside stacks.
4. ChEMBL 37 sits in two full copies: 29G SQLite + 13G DuckDB.
5. forensics/recovery-lab is 1.4T — the single largest data mass after NL datasets, with 18G of live viewer SQLite DBs backed by a running user service.
6. projects-archive/autopsy.zip alone is 104G; recov-sd.zip 46G — both opaque zip archives, no manifest found at top level.
7. supply_crate (92G) is a fully manifested materials-science corpus (synthesis 33G, matbench 29G, reactions 21G) with per-bundle READMEs — ready-made for registration.
8. NL metadata/doi.log (1.5G, ~62.8M DOIs) + journal classification parquets are unique bibliographic metadata not in stacks.
9. matdattmp acquisitions (22G, modern/historical/government/theses/patents) have ledgers + SHA256SUMS — a second well-documented corpus outside stacks.
10. ~/oxide.zip (49G) duplicates the live oxide/ dir (49G) — and oxide/archives already holds 49G of tar.zst campaign snapshots; three generations coexist.

## Gaps

- cleanup/quarantine/junk/root-home-vestige: permission denied (root-owned); size uncounted.
- personal-archive contents: counts only by policy (privacy); no content inventory.
- Forensic images (minipc-sda.img, omm.img, old-mbp-2015.raw, prev/bwd_rescue) and vault payloads: listed only, never opened (sacrosanct per ~/AGENTS.md).
- owfjs.zip (2.3G): content unidentified without opening.
- Compressed archives (oxide campaign tars, gigacorpus tars, supply_crate bundles, projects-archive zips, NL cold-storage mounts): internal file counts not enumerated.
- Large DB schemas (spine.duckdb, chembl, viewer catalog) not peeked — sizes noted instead; small DBs were mostly personal-app or test fixtures, skipped.
- old_laptop/home is empty in the copied rootfs; the real home content appears to live under personal-archive/old-laptop instead.
- Entrance_bay subcollection overlaps (datasets vs intake vs staging) not reconciled byte-for-byte.
