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
    domains: &'static [&'static str],
    description: &'static str,
}

const NL: &str = "/home/patrick/neurotic_library";
const HOME: &str = "/home/patrick";

const ENTRIES: &[Entry] = &[
    Entry {
        name: "movement",
        path: "datasets/movement",
        root: NL,
        sha256_status: "none",
        status: "registered",
        status_note: None,
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
        domains: &["formal-methods", "lean"],
        description: "Lean theorem-prover related content (mathlib/formalization material). Landing zone; not yet triaged.",
    },
    Entry {
        name: "entrance_bay/osti_curated",
        path: "datasets/entrance_bay/osti_curated",
        root: NL,
        sha256_status: "none",
        status: "registered",
        status_note: None,
        domains: &["government", "osti"],
        description: "Curated OSTI (US DOE Office of Scientific and Technical Information) content. Landing zone; not yet triaged.",
    },
    Entry {
        name: "entrance_bay/p2tp",
        path: "datasets/entrance_bay/p2tp",
        root: NL,
        sha256_status: "none",
        status: "registered",
        status_note: None,
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
        domains: &["actoprotectors", "patents"],
        description: "Actoprotector stub in entrance_bay. The actual 4,530-PDF RU-heavy patent corpus was extracted from move_staging/harvest.zip into stacks/corpus/harvest_extract (extraction queue).",
    },
    Entry {
        name: "move_staging/harvest.zip",
        path: "move_staging/harvest.zip",
        root: HOME,
        sha256_status: "none",
        status: "preserved_original",
        status_note: Some("extracted to ~/stacks/corpus/harvest_extract for the actoprotector wave; original zip preserved"),
        domains: &["actoprotectors", "patents"],
        description: "17 GB harvest.zip: 4,530 multilingual (RU-heavy) patent PDFs, the actoprotector corpus. Canonical transport original — never delete.",
    },
    Entry {
        name: "move_staging/move_harvest_1.zip",
        path: "move_staging/move_harvest_1.zip",
        root: HOME,
        sha256_status: "none",
        status: "preserved_original",
        status_note: Some("129 GB superset transport; tranche2.zip is contained in it"),
        domains: &["transport"],
        description: "129 GB move_harvest_1.zip: the big transport bundle from the move_staging era. Canonical original — never delete.",
    },
    Entry {
        name: "move_staging/tranche2.zip",
        path: "move_staging/tranche2.zip",
        root: HOME,
        sha256_status: "none",
        status: "preserved_original",
        status_note: Some("44 GB; subset of move_harvest_1.zip"),
        domains: &["transport"],
        description: "44 GB tranche2.zip: second tranche of the move_staging transport, contained in move_harvest_1.zip. Canonical original — never delete.",
    },
    Entry {
        name: "move_staging/move.zip",
        path: "move_staging/move.zip",
        root: HOME,
        sha256_status: "none",
        status: "preserved_original",
        status_note: None,
        domains: &["transport"],
        description: "8.3 GB move.zip: first transport bundle of the move_staging era. Canonical original — never delete.",
    },
    Entry {
        name: "staging/entrance_bay zips",
        path: "staging/entrance_bay",
        root: HOME,
        sha256_status: "none",
        status: "preserved_original",
        status_note: Some("25 transport originals incl. datasheets.zip; content now lives under NL datasets/entrance_bay"),
        domains: &["transport"],
        description: "25 transport zips from the entrance_bay acquisition era (actoprotectors, arxiv, datasheets, ddoracl, eduftp, fabrica, hfro_*, …). Canonical originals of content now living in NL datasets/entrance_bay. Never delete.",
    },
    Entry {
        name: "yumd spine (live)",
        path: "yum/spine.duckdb",
        root: HOME,
        sha256_status: "none",
        status: "queryable",
        status_note: Some("served live by yumd"),
        domains: &["food", "recipes"],
        description: "Live DuckDB spine of the yum recipe corpus, extracted from NL datasets/yum and served by the yumd service.",
    },
    Entry {
        name: "stacks.db",
        path: "stacks/data/stacks.db",
        root: HOME,
        sha256_status: "none",
        status: "queryable",
        status_note: Some("self-registration"),
        domains: &["chemistry", "synthesis"],
        description: "The stacks recipe system of record: 2.76M recipes (bootstrap corpus + Brauer pilot + matdattmp wave). Served read-only by stacks-api.",
    },
    Entry {
        name: "library.db",
        path: "stacks/data/library.db",
        root: HOME,
        sha256_status: "none",
        status: "queryable",
        status_note: Some("self-registration"),
        domains: &["documents", "library"],
        description: "The stacks library database: 9,106 catalog papers + enrichments, 357,527 chunks with embeddings (FTS + dense search), 7,048 registered documents, intake triage audit, and this dataset registry.",
    },
    Entry {
        name: "materials.db",
        path: "stacks/data/materials.db",
        root: HOME,
        sha256_status: "none",
        status: "queryable",
        status_note: Some("self-registration"),
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
        domains: &["acquisition-bay", "materials", "chemistry"],
        description: "The matdattmp acquisition bay: 469 document payloads (historical books, government reports, patents, theses, modern papers) plus the modern structured corpora (raccuglia, RAPID, ZeoSyn, sol-gel, MOF, ceder 2020, precursor genome, GPSS, A-Lab). Content imported wave E; payloads remain in place.",
    },
    Entry {
        name: "corpus/harvest_extract",
        path: "stacks/corpus/harvest_extract",
        root: HOME,
        sha256_status: "none",
        status: "extraction_queue",
        status_note: Some("extraction wave (c) target per design 001"),
        domains: &["actoprotectors", "patents"],
        description: "4,530 actoprotector patent PDFs extracted from move_staging/harvest.zip. Queued for LLM extraction as extraction wave (c) of the initial campaign.",
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
            registered_at: "2026-09-22T00:00:00Z".to_string(),
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
