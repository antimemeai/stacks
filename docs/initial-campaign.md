# Initial campaign — bring everything in scope into stacks

**Campaign complete 2026-09-22 (waves E–I all landed).** Open remainder:
extraction waves (sciencemadness scale-out, matsci OCR, actoprotectors),
COD archive re-acquisition (corrupt mid-stream at ~111k members), TOP4040
label survey, cross-source material identity (formula+spacegroup dedup),
equipment/inventory pillar, dataset storage tiering (blobs/streaming).

Decided 2026-09-22: all neurotic_library holdings in scope migrate into stacks
in one initial campaign. "Brought in" means stacks becomes the system of
record — cataloged, indexed, checksummed, queryable — not byte-copying bulk
payloads. NL is frozen (daemon terminated 2026-09-22) and remains untouched;
payloads are referenced in place (`location_root` discipline).

## Waves (serial; one builder at a time on the workspace)

- **E — matdattmp** (running): 470 ledgered documents → library.db;
  ~260k structured recipes (sol-gel 148k, MOF 47k, ZeoSyn 24k, RAPID 9k,
  Raccuglia 4.2k, Ceder version upgrade, Precursor Genome, GPSS/A-Lab) →
  stacks.db. Dark-reaction outcomes preserved as fractional scores.
- **F — materials red carpet:** MP latest snapshot (molecules, summary,
  property collections), COD CIFs, OQMD v1.8, TOP4040 → materials.db
  (new sibling DB). Generalize importer into wave framework here.
- **G — NL document remainder:** intake/ triage (15.5k PDFs; separate
  personal documents out — flagged, not imported), lib_ussr/ (281 files,
  Soviet metadata fields preserved) → library.db.
- **H — institutional memory:** NL archive/ + scripts/ + doctrine docs
  (AGENTS_START_HERE, MATERIA, DOCTRINE, CLAUDE.md, LIBRARY_CARD, docs/)
  imported as git history into this repo (e.g. under legacy/neurotic_library/).
- **I — dataset registry:** dataset table (path, size, format, sha256 where
  present, notes) covering movement, zero-to-cad, sketchgraphs, gigacorpus,
  harvest, course-skill-atlas, arxiv, mathnet, osti_curated, datasheets —
  registered in place; API discovery for agents.
- **J — yum images:** imagemap.duckdb wired into yumd detail pages (side
  quest, yum repo).

## Legacy import

Wave H landed: NL doctrine docs, scripts/ (467 files), and the archive/ text
corpus (946 files) now live under `legacy/neurotic_library/` in this repo
(commits 4dab3cc..9f1608c, 34 MB). See `legacy/neurotic_library/MANIFEST.md`
for the import record, skip list, and the arconaut.zip assessment (merits a
later git-history extraction).

## Explicitly out of scope (never import)

forensics/ (evidence), personal-archive/, projects-archive/recov-sd.zip,
staging transport zips and move_staging (canonical originals of NL content;
kept, not re-imported). projects-archive/arconaut.zip unexamined — assess
during wave H.

## Standing rules

Every wave: idempotent importer, provenance rows, teeth tests red-first,
report in docs/. Payloads never copied when referencing suffices.
