//! Wave I: the dataset registry. Every data payload on the host, registered
//! in place (no copying), with curator-written descriptions and cheap
//! filesystem stats. Plus `datasets verify` (path-existence -> missing).

use std::collections::HashMap;
use std::path::Path;

use stacks_core::library::{Dataset, LibraryStore};

use crate::ImportError;

#[derive(Debug, Default, serde::Serialize)]
pub struct RegistryStats {
    pub registered: u64,
    pub updated: u64,
    pub verified_missing: Vec<String>,
}

/// One registry entry, curated. Stats are filled from the filesystem.
struct Entry {
    name: &'static str,
    path: &'static str,
    root: &'static str,
    sha256_status: &'static str,
    status: &'static str,
    status_note: Option<&'static str>,
    provenance: &'static str,
    domains: &'static [&'static str],
    description: &'static str,
}

const NL: &str = "/home/patrick/neurotic_library";
const HOME: &str = "/home/patrick";
const SRV_DB: &str = "/srv/stacks/db";
const SRV: &str = "/srv/stacks";

const ENTRIES: &[Entry] = &[
    Entry {
        name: "movement",
        path: "datasets/movement",
        root: NL,
        sha256_status: "none",
        status: "registered",
        status_note: None,
        provenance: "downloaded",
        domains: &["biomechanics", "mocap"],
        description: "176 GB of human-motion capture and biomechanics data (grasping/motion corpora, various capture formats). Acquired for the human-motion side of the lab; not yet wired into any pipeline. Survey: NL datasets census.",
    },
    Entry {
        name: "zero-to-cad",
        path: "datasets/zero-to-cad",
        root: NL,
        sha256_status: "none",
        status: "registered",
        status_note: None,
        provenance: "downloaded",
        domains: &["cad", "mechanical-design"],
        description: "326 GB Zero-to-CAD dataset: language-to-CAD generation training data (CadQuery programs + renders). Feedstock for a future CAD-generation pillar. Survey: NL datasets census.",
    },
    Entry {
        name: "sketchgraphs",
        path: "datasets/sketchgraphs",
        root: NL,
        sha256_status: "none",
        status: "registered",
        status_note: None,
        provenance: "downloaded",
        domains: &["cad", "geometry"],
        description: "63 GB SketchGraphs dataset: millions of CAD sketch constraint graphs (line/arc primitives with geometric constraints). Companion to zero-to-cad. Survey: NL datasets census.",
    },
    Entry {
        name: "gigacorpus",
        path: "datasets/gigacorpus",
        root: NL,
        sha256_status: "sidecars",
        status: "registered",
        status_note: Some("6 tar.zst members; acquired.tar.zst has a .sha256 sidecar"),
        provenance: "downloaded",
        domains: &["nlp", "pretraining"],
        description: "97 GB of pretraining-scale text corpora packed as tar.zst archives (acquired, corpus-acquisition, eval-case-candidates, meta-tuning, orthogonal). Integrity: sha256 sidecar present for acquired.tar.zst only. Survey: NL datasets census.",
    },
    Entry {
        name: "harvest",
        path: "datasets/harvest",
        root: NL,
        sha256_status: "none",
        status: "registered",
        status_note: None,
        provenance: "downloaded",
        domains: &["multilingual", "web-text"],
        description: "18 GB multilingual web harvest (RU-heavy) collected during the actoprotector/search campaigns. Survey: NL datasets census.",
    },
    Entry {
        name: "course-skill-atlas",
        path: "datasets/course-skill-atlas",
        root: NL,
        sha256_status: "none",
        status: "registered",
        status_note: None,
        provenance: "downloaded",
        domains: &["education", "skills"],
        description: "18 GB course/skill atlas data (course descriptions mapped to skill taxonomies). Survey: NL datasets census.",
    },
    Entry {
        name: "chembl",
        path: "datasets/chembl",
        root: NL,
        sha256_status: "none",
        status: "queryable",
        status_note: Some("ChEMBL bioactivity DB; already queryable in place (duckdb/sqlite)"),
        provenance: "downloaded",
        domains: &["chemistry", "bioactivity", "drug-discovery"],
        description: "47 GB ChEMBL mirror: bioactivity measurements, compounds, assays. Queryable in place; not migrated into stacks.db (bioactivity, not recipes). Survey: NL datasets census.",
    },
    Entry {
        name: "chem-recipes",
        path: "datasets/chem-recipes",
        root: NL,
        sha256_status: "none",
        status: "migrated",
        status_note: Some("imported into stacks.db as the bootstrap corpus (177k inorganic + 2.3M ORD); see docs/chem-recipes-import-report.md"),
        provenance: "downloaded",
        domains: &["chemistry", "synthesis"],
        description: "8.3 GB processed synthesis corpus (recipes.duckdb: 4 inorganic sources + ORD parquet mirror). MIGRATED: stacks.db is now the system of record for these recipes; the duckdb remains the source of the extraction.",
    },
    Entry {
        name: "materials-project",
        path: "datasets/materials-project",
        root: NL,
        sha256_status: "none",
        status: "migrated",
        status_note: Some("2025-09-25 snapshot imported into materials.db (wave F)"),
        provenance: "downloaded",
        domains: &["materials", "computational"],
        description: "Materials Project collection snapshots (11 dated dirs; latest 2025-09-25 loaded). See docs/materials-import-report.md for the skip list.",
    },
    Entry {
        name: "oqmd",
        path: "datasets/oqmd",
        root: NL,
        sha256_status: "none",
        status: "migrated",
        status_note: Some("v1.8 dump imported into materials.db (wave F)"),
        provenance: "downloaded",
        domains: &["materials", "computational"],
        description: "OQMD v1.8 (2026-02 dump): 1.4M DFT entries + formation energies. Migrated into materials.db.",
    },
    Entry {
        name: "cod",
        path: "datasets/cod",
        root: NL,
        sha256_status: "none",
        status: "migrated",
        status_note: Some("PARTIAL: cod-cifs-mysql.txz is corrupt mid-stream; 111,315 members parsed, ~390k lost pending re-acquisition"),
        provenance: "downloaded",
        domains: &["crystallography"],
        description: "Crystallography Open Database CIF archive (18 GB txz). Wave F streamed 111,309 clean entries into materials.db before hitting archive corruption. See docs/materials-import-report.md.",
    },
    Entry {
        name: "topology-top",
        path: "datasets/topology-top",
        root: NL,
        sha256_status: "none",
        status: "migrated",
        status_note: Some("inventory only: npz members are bare arrays with no labels (wave F)"),
        provenance: "downloaded",
        domains: &["materials", "topology"],
        description: "TOP4040 topological-materials archive: 10,000 npz files of bare float64 arrays (100x40x40) with no labels/metadata in the acquisition. Inventory registered in materials.db; labels pending re-survey.",
    },
    Entry {
        name: "yum",
        path: "datasets/yum",
        root: NL,
        sha256_status: "none",
        status: "migrated",
        status_note: Some("spine extracted and live in ~/yum/spine.duckdb (yumd service)"),
        provenance: "downloaded",
        domains: &["food", "recipes"],
        description: "58 GB recipe/cooking corpus. The spine (structured core) was extracted and is served by yumd from ~/yum/spine.duckdb; the bulk stays here. Survey: yum spine extraction notes.",
    },
    Entry {
        name: "sciencemadness",
        path: "datasets/sciencemadness",
        root: NL,
        sha256_status: "none",
        status: "queryable",
        status_note: Some("Brauer pilot extracted (55 recipes, verified); rest is the extraction-waves source"),
        provenance: "downloaded",
        domains: &["chemistry", "books", "ocr"],
        description: "Sciencemadness library mirror: Brauer/Mellor/Schlessinger and other preparative-chemistry books with OCR text layers. Extraction wave (a) proved the path (docs/brauer-pilot-audit.md, docs/brauer-verify-report.md); waves b/c pending.",
    },
    Entry {
        name: "entrance_bay/datasheets",
        path: "datasets/entrance_bay/datasheets",
        root: NL,
        sha256_status: "none",
        status: "registered",
        status_note: Some("equipment pillar feedstock (per design 001 non-goal now, pillar later)"),
        provenance: "downloaded",
        domains: &["equipment", "datasheets"],
        description: "~48k component/equipment datasheets (PDFs). Feedstock for the future equipment/inventory pillar; explicitly out of the recipe wave's scope.",
    },
    Entry {
        name: "entrance_bay/arxiv",
        path: "datasets/entrance_bay/arxiv",
        root: NL,
        sha256_status: "none",
        status: "registered",
        status_note: None,
        provenance: "downloaded",
        domains: &["preprints", "arxiv"],
        description: "52 GB arXiv bulk subset + arxiv_emb embeddings staging. Landing zone content from the entrance_bay transport era; not yet triaged into the library.",
    },
    Entry {
        name: "entrance_bay/mathnet",
        path: "datasets/entrance_bay/mathnet",
        root: NL,
        sha256_status: "none",
        status: "registered",
        status_note: None,
        provenance: "downloaded",
        domains: &["mathematics", "russian-journals"],
        description: "54 GB Math-Net.Ru mirror content (Russian mathematics journal archive). Landing zone; not yet triaged.",
    },
    Entry {
        name: "entrance_bay/jprs",
        path: "datasets/entrance_bay/jprs",
        root: NL,
        sha256_status: "none",
        status: "registered",
        status_note: None,
        provenance: "downloaded",
        domains: &["translations", "government"],
        description: "JPRS (Joint Publications Research Service) translations collection — Cold-War-era translated technical/scientific reports. Landing zone; not yet triaged.",
    },
    Entry {
        name: "entrance_bay/hfro_archiv+hfro_dat",
        path: "datasets/entrance_bay",
        root: NL,
        sha256_status: "none",
        status: "registered",
        status_note: Some("covers hfro_archiv/ and hfro_dat/"),
        provenance: "downloaded",
        domains: &["electronics", "hf-radio"],
        description: "HFRO (HF radio/electronics) archives: hfro_archiv and hfro_dat directories. Landing zone; not yet triaged.",
    },
    Entry {
        name: "entrance_bay/fabrica",
        path: "datasets/entrance_bay/fabrica",
        root: NL,
        sha256_status: "none",
        status: "registered",
        status_note: None,
        provenance: "downloaded",
        domains: &["books", "russian"],
        description: "Fabrica book collection (Russian technical/scientific books). Landing zone; not yet triaged.",
    },
    Entry {
        name: "entrance_bay/lean",
        path: "datasets/entrance_bay/lean",
        root: NL,
        sha256_status: "none",
        status: "registered",
        status_note: None,
        provenance: "downloaded",
        domains: &["formal-methods", "lean"],
        description: "Lean theorem-prover related content (mathlib/formalization material). Landing zone; not yet triaged.",
    },
    Entry {
        name: "entrance_bay/osti_curated",
        path: "datasets/entrance_bay/osti_curated",
        root: NL,
        sha256_status: "none",
        status: "registered",
        status_note: Some("Metadata catalog only; no dataset payloads were downloaded here and no content has been indexed. Related transport source: staging/entrance_bay/osti_curated.zip (not counted as a separate registry payload). Transport ZIP retired on 2026-09-24 after all payload destinations passed ZIP CRC32 / symlink-target verification; member receipts: /home/patrick/cleanup/reports/home-cleanup-2026-09-24/."),
        provenance: "downloaded",
        domains: &["government", "osti", "research-data", "metadata"],
        description: "Curated OSTI acquisition metadata: 75 OSTI Dataset records, 34 small HydraGNN/Xhem model configuration or card files, and the 208-record acquisition manifest. The downloaded model checkpoints and software archives are preserved in separately registered staging locations.",
    },
    Entry {
        name: "entrance_bay/p2tp",
        path: "datasets/entrance_bay/p2tp",
        root: NL,
        sha256_status: "none",
        status: "registered",
        status_note: None,
        provenance: "downloaded",
        domains: &["patents"],
        description: "P2TP (patents-to-product?) staging content. Landing zone; not yet triaged. Description uncertain — needs a survey pass.",
    },
    Entry {
        name: "entrance_bay/ddoracl",
        path: "datasets/entrance_bay/ddoracl",
        root: NL,
        sha256_status: "none",
        status: "registered",
        status_note: None,
        provenance: "downloaded",
        domains: &["misc"],
        description: "ddoracl staging content. Description uncertain — needs a survey pass.",
    },
    Entry {
        name: "entrance_bay/matsci_stuff",
        path: "datasets/entrance_bay/matsci_stuff",
        root: NL,
        sha256_status: "none",
        status: "registered",
        status_note: None,
        provenance: "downloaded",
        domains: &["materials"],
        description: "matsci_stuff landing-zone content (materials-science miscellany). Description uncertain — needs a survey pass.",
    },
    Entry {
        name: "entrance_bay/actoprotectors",
        path: "datasets/entrance_bay/actoprotectors",
        root: NL,
        sha256_status: "none",
        status: "registered",
        status_note: Some("stub; the real corpus is stacks/corpus/harvest_extract"),
        provenance: "downloaded",
        domains: &["actoprotectors", "patents"],
        description: "Actoprotector stub in entrance_bay. The actual 4,530-PDF RU-heavy patent corpus was extracted from move_staging/harvest.zip into stacks/corpus/harvest_extract (extraction queue).",
    },
    Entry {
        name: "yumd spine (live)",
        path: "yum/spine.duckdb",
        root: HOME,
        sha256_status: "none",
        status: "queryable",
        status_note: Some("served live by yumd"),
        provenance: "downloaded",
        domains: &["food", "recipes"],
        description: "Live DuckDB spine of the yum recipe corpus, extracted from NL datasets/yum and served by the yumd service.",
    },
    Entry {
        name: "stacks.db",
        path: "stacks.db",
        root: SRV_DB,
        sha256_status: "none",
        status: "queryable",
        status_note: Some("self-registration"),
        provenance: "downloaded",
        domains: &["chemistry", "synthesis"],
        description: "The stacks recipe system of record: 2.76M recipes (bootstrap corpus + Brauer pilot + matdattmp wave). Served read-only by stacks-api.",
    },
    Entry {
        name: "library.db",
        path: "library.db",
        root: SRV_DB,
        sha256_status: "none",
        status: "queryable",
        status_note: Some("self-registration"),
        provenance: "downloaded",
        domains: &["documents", "library"],
        description: "The stacks library database: 9,106 catalog papers + enrichments, 357,527 chunks with embeddings (FTS + dense search), 7,048 registered documents, intake triage audit, and this dataset registry.",
    },
    Entry {
        name: "materials.db",
        path: "materials.db",
        root: SRV_DB,
        sha256_status: "none",
        status: "queryable",
        status_note: Some("self-registration"),
        provenance: "downloaded",
        domains: &["materials", "computational"],
        description: "The stacks computational-materials database: 2.34M entries (MP 2025-09-25, OQMD v1.8, COD partial, TOP4040 inventory) with 11.4M unit-disciplined property rows. See docs/materials-import-report.md.",
    },
    Entry {
        name: "matdattmp",
        path: "matdattmp",
        root: HOME,
        sha256_status: "none",
        status: "migrated",
        status_note: Some("469 documents into library.db (wave E); 9 structured sources into stacks.db (wave E); see docs/matdattmp-import-report.md"),
        provenance: "downloaded",
        domains: &["acquisition-bay", "materials", "chemistry"],
        description: "The matdattmp acquisition bay: 469 document payloads (historical books, government reports, patents, theses, modern papers) plus the modern structured corpora (raccuglia, RAPID, ZeoSyn, sol-gel, MOF, ceder 2020, precursor genome, GPSS, A-Lab). Content imported wave E; payloads remain in place.",
    },
    Entry {
        name: "corpus/harvest_extract",
        path: "corpus/harvest_extract",
        root: SRV,
        sha256_status: "none",
        status: "extraction_queue",
        status_note: Some("extraction wave (c) target per design 001"),
        provenance: "downloaded",
        domains: &["actoprotectors", "patents"],
        description: "4,530 actoprotector patent PDFs extracted from move_staging/harvest.zip. Queued for LLM extraction as extraction wave (c) of the initial campaign.",
    },
    Entry {
        name: "entrance_bay/materials-project-dos-parquet-tars",
        path: "datasets/entrance_bay/materials-project-dos-parquet-tars",
        root: NL,
        sha256_status: "sidecars",
        status: "registered",
        status_note: Some("Nine supplied per-TAR SHA-256 values are recorded in bundle/per-TAR manifests and staging/entrance_bay_misc/materials-project-dos-parquet-tars/materials-project-dos-parquet-tars/SHA256SUMS; not rehashed here. Source transport ZIP preserved at /home/patrick/staging/entrance_bay/materials-project-dos-parquet-tars.zip and intentionally not separately registered to avoid duplicate logical-payload accounting. Embedded worker manifests record one source/conversion error (worker 23) despite completed archive transfer. Overlaps the existing broad entrance_bay/hfro_archiv+hfro_dat row. Transport ZIP retired on 2026-09-24 after all payload destinations passed ZIP CRC32 / symlink-target verification; member receipts: /home/patrick/cleanup/reports/home-cleanup-2026-09-24/."),
        provenance: "downloaded",
        domains: &["materials", "materials-project", "density-of-states", "electronic-structure", "parquet"],
        description: "Materials Project parsed density-of-states (DOS) columnar export, run 20260703T103002Z-dos-full-p24. Nine TAR containers package 110,784 Parquet shards across atom_dos, energy_grids, meta, object_index, pdos, spd_dos, structures, and total_dos, plus 48 JSON run/shard manifests. The 110,832 packaged objects are shards/metadata, not a material count. No stacks import or extracted query store was found; direct use requires read-only extraction/materialization from TAR.",
    },
    Entry {
        name: "entrance_bay/osti_curated/weights",
        path: "staging/entrance_bay_misc/osti_curated/osti_curated/weights",
        root: HOME,
        sha256_status: "none",
        status: "registered",
        status_note: Some("37,898,998,683 B in staging: HydraGNN_GFM_2026 36,475,562,324 B; HydraGNN_GFM_2024 1,263,881,407 B; Xhem-GPT-2 159,554,952 B. Checkpoints were not loaded, executed, hashed, or model-validated. Configuration/card copies reside with the separately registered metadata fragment; related source ZIP is not counted twice. Transport ZIP retired on 2026-09-24 after all payload destinations passed ZIP CRC32 / symlink-target verification; member receipts: /home/patrick/cleanup/reports/home-cleanup-2026-09-24/."),
        provenance: "downloaded",
        domains: &["government", "osti", "materials", "machine-learning", "chemistry"],
        description: "OSTI-curated machine-learning model assets: HydraGNN predictive graph-foundation-model checkpoint releases from 2024 and 2026, plus the LANL Xhem-GPT-2 chemical-language-model checkpoint and tokenizer. Readmes identify the 2024 release as a 15-model materials ensemble and the 2026 release as a 10-model follow-on; files were cataloged by metadata and headers only.",
    },
    Entry {
        name: "entrance_bay/osti_curated/software",
        path: "staging/entrance_bay_misc/osti_curated/osti_curated/software",
        root: HOME,
        sha256_status: "none",
        status: "registered",
        status_note: Some("7,792,892,006 B in staging. Tar member listings were sampled only; no extraction, execution, full integrity check, or hashing was performed. Related source ZIP is not counted as another payload. Transport ZIP retired on 2026-09-24 after all payload destinations passed ZIP CRC32 / symlink-target verification; member receipts: /home/patrick/cleanup/reports/home-cleanup-2026-09-24/."),
        provenance: "downloaded",
        domains: &["government", "osti", "scientific-software"],
        description: "OSTI-curated software source archives: 127 gzip-compressed tarballs, mostly named by OSTI software record identifiers (124 code-prefixed and 3 numeric-only). The acquisition manifest maps these records to upstream project targets; archives remain unextracted and unexecuted.",
    },
];

type DirStats = (Option<i64>, Option<i64>, Option<Vec<(String, u64)>>);

fn dir_stats(path: &Path) -> DirStats {
    if path.is_file() {
        let size = std::fs::metadata(path).ok().map(|m| m.len() as i64);
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".to_string());
        return (size, Some(1), Some(vec![(ext, 1)]));
    }
    if !path.is_dir() {
        return (None, None, None);
    }
    let mut bytes = 0i64;
    let mut files = 0i64;
    let mut exts: HashMap<String, u64> = HashMap::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    files += 1;
                    if let Ok(m) = entry.metadata() {
                        bytes += m.len() as i64;
                    }
                    let ext = p
                        .extension()
                        .map(|e| e.to_string_lossy().to_lowercase())
                        .unwrap_or_else(|| "(none)".to_string());
                    *exts.entry(ext).or_default() += 1;
                }
            }
        }
    }
    let mut exts: Vec<(String, u64)> = exts.into_iter().collect();
    exts.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    exts.truncate(5);
    (Some(bytes), Some(files), Some(exts))
}

/// Register all curated entries, computing fresh filesystem stats.
pub fn register_datasets(store: &mut LibraryStore) -> Result<RegistryStats, ImportError> {
    let mut stats = RegistryStats::default();
    // Stamped only on insert: the upsert conflict path preserves the
    // original "first registered" timestamp.
    let now = crate::inproc::now_utc();
    for e in ENTRIES {
        let full = Path::new(e.root).join(e.path);
        let (size_bytes, file_count, formats) = dir_stats(&full);
        let ds = Dataset {
            id: 0,
            name: e.name.to_string(),
            path: e.path.to_string(),
            location_root: e.root.to_string(),
            size_bytes,
            file_count,
            dominant_formats: formats,
            sha256_status: e.sha256_status.to_string(),
            description: e.description.to_string(),
            domains: e.domains.iter().map(|s| s.to_string()).collect(),
            status: e.status.to_string(),
            status_note: e.status_note.map(str::to_string),
            provenance: e.provenance.to_string(),
            registered_at: now.clone(),
        };
        if stacks_core::library::upsert_dataset(store.raw(), &ds)? {
            stats.registered += 1;
        }
    }
    Ok(stats)
}

/// `datasets verify`: flip vanished paths to status='missing'.
pub fn verify_datasets(store: &LibraryStore) -> Result<Vec<String>, ImportError> {
    let flipped = stacks_core::library::verify_datasets(store.raw())?;
    Ok(flipped)
}
