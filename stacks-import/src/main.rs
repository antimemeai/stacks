use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use stacks_core::Store;
use stacks_import::ImportStats;

fn print_stats(stats: &ImportStats) {
    for (source, s) in &stats.per_source {
        println!(
            "{source}: seen={} inserted={} skipped_existing={} materials(+{}/={}) steps={} step_materials={} provenance={} atm(mapped/other/absent)={}/{}/{} amounts_dropped={}",
            s.recipes_seen,
            s.recipes_inserted,
            s.recipes_skipped_existing,
            s.materials_created,
            s.materials_reused,
            s.steps_inserted,
            s.step_materials_inserted,
            s.provenance_inserted,
            s.atmosphere_mapped,
            s.atmosphere_other,
            s.atmosphere_absent,
            s.amounts_dropped,
        );
        if !s.unmapped_operation_tokens.is_empty() {
            println!("  unmapped operation tokens (top 25):");
            let mut toks: Vec<_> = s.unmapped_operation_tokens.iter().collect();
            toks.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
            for (tok, c) in toks.into_iter().take(25) {
                println!("    {tok}: {c}");
            }
        }
        if !s.dropped_time_units.is_empty() {
            println!("  dropped time units: {:?}", s.dropped_time_units);
        }
    }
}

fn pilot_load(jsonl: &str, db: &str, args: &[String]) -> ExitCode {
    let mut store = match Store::open(db) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("open store: {e}");
            return ExitCode::FAILURE;
        }
    };
    // Defaults: the Brauer pilot source; override with 4 extra args:
    // pilot-load <jsonl> <db> <dataset> <book> <source-path> <sha256>
    let source = if args.len() == 8 {
        stacks_import::ExtractSource {
            source_dataset: args[4].clone(),
            book: args[5].clone(),
            path: args[6].clone(),
            sha256: args[7].clone(),
            extractor_version: "kimi-agent wave-b".to_string(),
        }
    } else {
        stacks_import::ExtractSource {
            source_dataset: "sciencemadness".to_string(),
            book: "brauer".to_string(),
            path: "/home/patrick/neurotic_library/datasets/sciencemadness/brauer_ocr.pdf"
                .to_string(),
            sha256: "77ac5c1c37524a91136f402ec97fcea374e2ff85f29f43d35daa73be3c16272f".to_string(),
            extractor_version: "kimi-agent pilot-1".to_string(),
        }
    };
    let created_at = "2026-09-21T00:00:00Z";
    match stacks_import::load_extracted(
        &mut store,
        std::path::Path::new(jsonl),
        &source,
        created_at,
    ) {
        Ok(stats) => {
            println!(
                "pilot-load: inserted={} skipped_existing={} rejected={}",
                stats.inserted,
                stats.skipped_existing,
                stats.rejected.len()
            );
            for (slug, reason) in &stats.rejected {
                println!("  rejected {slug}: {reason}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("pilot-load failed: {e}");
            ExitCode::FAILURE
        }
    }
}

fn verify_cmd(args: &[String]) -> ExitCode {
    if args.len() != 6 {
        eprintln!(
            "usage: stacks-import verify-extraction <db> <source_dataset> <pages-dir> <out-prefix>"
        );
        eprintln!("writes <out-prefix>.json and <out-prefix>.md; exit 1 on any FAIL");
        return ExitCode::FAILURE;
    }
    let store = match Store::open_read_only(&args[2]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("open store: {e}");
            return ExitCode::FAILURE;
        }
    };
    let report = match stacks_import::verify::verify_extraction(
        &store,
        &args[3],
        std::path::Path::new(&args[4]),
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("verify failed: {e}");
            return ExitCode::FAILURE;
        }
    };
    let json_path = format!("{}.json", args[5]);
    let md_path = format!("{}.md", args[5]);
    let write = (|| -> std::io::Result<()> {
        std::fs::write(&json_path, serde_json::to_string_pretty(&report)?)?;
        std::fs::write(&md_path, stacks_import::verify::render_markdown(&report))?;
        Ok(())
    })();
    if let Err(e) = write {
        eprintln!("write report: {e}");
        return ExitCode::FAILURE;
    }
    println!(
        "verify-extraction {}: {} recipes — {} pass, {} warn, {} fail, {} coverage warns",
        report.source_dataset,
        report.recipes,
        report.pass,
        report.warn,
        report.fail,
        report.coverage_warns
    );
    println!("reports: {json_path}, {md_path}");
    if report.fail > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn library_import_cmd(args: &[String]) -> ExitCode {
    if args.len() != 5 {
        eprintln!("usage: stacks-import library-import <export-dir> <embeddings-dir> <library-db>");
        return ExitCode::FAILURE;
    }
    let mut store = match stacks_core::library::LibraryStore::open(&args[4]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("open library db: {e}");
            return ExitCode::FAILURE;
        }
    };
    let t0 = std::time::Instant::now();
    let result = stacks_import::library_import::import_library(
        &mut store,
        std::path::Path::new(&args[2]),
        std::path::Path::new(&args[3]),
        "2026-09-22T00:00:00Z",
    );
    match result {
        Ok(stats) => {
            if let Err(e) = stacks_import::library_import::rebuild_fts(&store) {
                eprintln!("fts rebuild: {e}");
                return ExitCode::FAILURE;
            }
            println!("{}", serde_json::to_string_pretty(&stats).unwrap());
            println!("library-import done in {:?}", t0.elapsed());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("library-import failed: {e}");
            ExitCode::FAILURE
        }
    }
}

fn matdattmp_cmd(args: &[String]) -> ExitCode {
    if args.len() != 6 {
        eprintln!(
            "usage: stacks-import matdattmp <matdattmp-root> <workdir> <stacks-db> <library-db>"
        );
        return ExitCode::FAILURE;
    }
    let root = std::path::Path::new(&args[2]);
    let workdir = std::path::Path::new(&args[3]);
    let t0 = std::time::Instant::now();
    let mut out = serde_json::json!({});

    // Part A: documents into library.db
    let mut lib = match stacks_core::library::LibraryStore::open(&args[5]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("open library db: {e}");
            return ExitCode::FAILURE;
        }
    };
    match stacks_import::matdattmp::import_documents(
        &mut lib,
        &root.join("ledgers/manifest.jsonl"),
        root,
    ) {
        Ok(s) => out["documents"] = serde_json::to_value(&s).unwrap(),
        Err(e) => {
            eprintln!("documents failed: {e}");
            return ExitCode::FAILURE;
        }
    }
    drop(lib);

    // Part B: recipes into stacks.db
    let mut store = match Store::open(&args[4]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("open stacks db: {e}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = (|| -> Result<(), stacks_import::ImportError> {
        store.raw().pragma_update(None, "synchronous", "OFF")?;
        store.raw().pragma_update(None, "wal_autocheckpoint", 0)?;
        store.raw().pragma_update(None, "cache_size", -2_000_000)?;
        store.raw().pragma_update(None, "temp_store", "MEMORY")?;
        Ok(())
    })() {
        eprintln!("pragma: {e}");
        return ExitCode::FAILURE;
    }
    type ImporterFn =
        fn(
            &mut Store,
            &std::path::Path,
        ) -> Result<stacks_import::matdattmp::SourceStat, stacks_import::ImportError>;
    let importers: [(&str, ImporterFn); 8] = [
        ("raccuglia", stacks_import::matdattmp::import_raccuglia),
        ("rapid", stacks_import::matdattmp::import_rapid),
        ("zeosyn", stacks_import::matdattmp::import_zeosyn),
        ("solgel", stacks_import::matdattmp::import_solgel),
        ("mof", stacks_import::matdattmp::import_mof),
        ("ceder2", stacks_import::matdattmp::import_ceder2),
        (
            "precursor_genome",
            stacks_import::matdattmp::import_precursor_genome,
        ),
        ("gpss", stacks_import::matdattmp::import_gpss),
    ];
    for (name, f) in importers {
        let t = std::time::Instant::now();
        match f(&mut store, workdir) {
            Ok(s) => {
                eprintln!("{name} done in {:?}", t.elapsed());
                out[name] = serde_json::to_value(&s).unwrap();
            }
            Err(e) => {
                eprintln!("{name} failed: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    // A-Lab after (small)
    match stacks_import::matdattmp::import_alab(&mut store, workdir) {
        Ok(s) => out["alab"] = serde_json::to_value(&s).unwrap(),
        Err(e) => {
            eprintln!("alab failed: {e}");
            return ExitCode::FAILURE;
        }
    }
    if let Err(e) = store
        .raw()
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
    {
        eprintln!("checkpoint: {e}");
    }
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
    println!("matdattmp import done in {:?}", t0.elapsed());
    ExitCode::SUCCESS
}

fn materials_cmd(args: &[String]) -> ExitCode {
    if args.len() != 5 {
        eprintln!("usage: stacks-import materials <mp-collections-dir> <workdir> <materials-db>");
        eprintln!("workdir must contain cod.jsonl, top4040.jsonl, oqmd_*.jsonl");
        return ExitCode::FAILURE;
    }
    let mp = &args[2];
    let workdir = std::path::PathBuf::from(&args[3]);
    let mut store = match stacks_core::materials::MaterialsStore::open(&args[4]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("open materials db: {e}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = (|| -> Result<(), stacks_core::StoreError> {
        store.raw().pragma_update(None, "synchronous", "OFF")?;
        store.raw().pragma_update(None, "wal_autocheckpoint", 0)?;
        store.raw().pragma_update(None, "cache_size", -2_000_000)?;
        store.raw().pragma_update(None, "temp_store", "MEMORY")?;
        Ok(())
    })() {
        eprintln!("pragma: {e}");
        return ExitCode::FAILURE;
    }
    use stacks_import::materials_wave::*;
    use stacks_import::wave::{run_wave, WaveCtx, WaveSource};
    let ctx = WaveCtx {
        workdir: workdir.clone(),
        imported_at: "2026-09-22T00:00:00Z".to_string(),
    };
    let j = |c: &str| format!("{mp}/{c}");
    let sources: Vec<Box<dyn WaveSource>> = vec![
        Box::new(MpSummary { dir: j("summary") }),
        Box::new(MpRobocrys { dir: j("robocrys") }),
        Box::new(MpProps {
            dir: j("thermo"),
            collection: "thermo",
            id_field: "material_id",
            specs: &[
                ("energy_above_hull", "energy_above_hull", Some("eV/atom")),
                (
                    "formation_energy_per_atom",
                    "formation_energy_per_atom",
                    Some("eV/atom"),
                ),
                ("energy_per_atom", "energy_per_atom", Some("eV/atom")),
                (
                    "decomposition_enthalpy",
                    "decomposition_enthalpy",
                    Some("eV/atom"),
                ),
                (
                    "uncorrected_energy_per_atom",
                    "uncorrected_energy_per_atom",
                    Some("eV/atom"),
                ),
            ],
            text_fields: &[
                ("is_stable", "is_stable"),
                ("thermo_type", "thermo_type"),
                ("energy_type", "energy_type"),
            ],
        }),
        Box::new(MpProps {
            dir: j("electronic-structure"),
            collection: "electronic-structure",
            id_field: "material_id",
            specs: &[
                ("band_gap", "band_gap", Some("eV")),
                ("efermi", "efermi", Some("eV")),
            ],
            text_fields: &[
                ("is_gap_direct", "is_gap_direct"),
                ("is_metal", "is_metal"),
                ("magnetic_ordering", "magnetic_ordering"),
            ],
        }),
        Box::new(MpProps {
            dir: j("elasticity"),
            collection: "elasticity",
            id_field: "material_id",
            specs: &[
                ("bulk_modulus", "bulk_modulus", Some("GPa")),
                ("shear_modulus", "shear_modulus", Some("GPa")),
                ("homogeneous_poisson", "homogeneous_poisson", Some("1")),
                ("universal_anisotropy", "universal_anisotropy", Some("1")),
                ("debye_temperature", "debye_temperature", Some("other:K")),
                (
                    "thermal_conductivity",
                    "thermal_conductivity",
                    Some("other:W/mK"),
                ),
            ],
            text_fields: &[("state", "state")],
        }),
        Box::new(MpProps {
            dir: j("magnetism"),
            collection: "magnetism",
            id_field: "material_id",
            specs: &[
                ("total_magnetization", "total_magnetization", Some("µB")),
                (
                    "total_magnetization_normalized_formula_units",
                    "total_magnetization_per_fu",
                    Some("µB"),
                ),
                (
                    "total_magnetization_normalized_vol",
                    "total_magnetization_per_vol",
                    Some("µB"),
                ),
                ("num_magnetic_sites", "num_magnetic_sites", Some("1")),
            ],
            text_fields: &[("ordering", "ordering"), ("is_magnetic", "is_magnetic")],
        }),
        Box::new(MpProps {
            dir: j("dielectric"),
            collection: "dielectric",
            id_field: "material_id",
            specs: &[
                ("e_total", "e_total", Some("1")),
                ("e_electronic", "e_electronic", Some("1")),
                ("e_ionic", "e_ionic", Some("1")),
                ("n", "refractive_index", Some("1")),
            ],
            text_fields: &[],
        }),
        Box::new(MpProps {
            dir: j("piezoelectric"),
            collection: "piezoelectric",
            id_field: "material_id",
            specs: &[("e_ij_max", "e_ij_max", Some("pC/N"))],
            text_fields: &[],
        }),
        Box::new(MpElectrodes {
            dir: j("insertion-electrodes"),
            collection: "insertion-electrodes",
        }),
        Box::new(MpElectrodes {
            dir: j("conversion-electrodes"),
            collection: "conversion-electrodes",
        }),
        Box::new(MpMolecules {
            dir: j("molecules"),
        }),
        Box::new(Cod {
            jsonl: workdir.join("cod.jsonl").to_string_lossy().to_string(),
            archive: "/home/patrick/neurotic_library/datasets/cod/raw/cod-cifs-mysql.txz"
                .to_string(),
        }),
        Box::new(Top4040 {
            jsonl: workdir.join("top4040.jsonl").to_string_lossy().to_string(),
            archive: "/home/patrick/neurotic_library/datasets/topology-top/raw/TOP4040.zip"
                .to_string(),
        }),
    ];
    let oqmd_entries = workdir.join("oqmd_entries.jsonl");
    let sources: Vec<Box<dyn WaveSource>> = if oqmd_entries.exists() {
        let mut s = sources;
        s.push(Box::new(Oqmd {
            entries_jsonl: oqmd_entries.to_string_lossy().to_string(),
            fe_jsonl: workdir.join("oqmd_fe.jsonl").to_string_lossy().to_string(),
        }));
        s
    } else {
        sources
    };
    match run_wave(&mut store, &ctx, sources) {
        Ok(report) => {
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("wave failed: {e}");
            ExitCode::FAILURE
        }
    }
}

fn intake_cmd(args: &[String]) -> ExitCode {
    if args.len() != 4 {
        eprintln!("usage: stacks-import intake <nl-root> <library-db>  [DEPRECATED — use inproc/inproc-drain]");
        return ExitCode::FAILURE;
    }
    let nl = std::path::PathBuf::from(&args[2]);
    let mut store = match stacks_core::library::LibraryStore::open(&args[3]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("open library db: {e}");
            return ExitCode::FAILURE;
        }
    };
    let t0 = std::time::Instant::now();
    let mut out = serde_json::json!({});
    match stacks_import::intake_triage::triage_intake(&mut store, &nl.join("intake")) {
        Ok(s) => out["intake_triage"] = serde_json::to_value(&s).unwrap(),
        Err(e) => {
            eprintln!("intake triage failed: {e}");
            return ExitCode::FAILURE;
        }
    }
    match stacks_import::intake_triage::import_lib_ussr(&mut store, &nl.join("lib_ussr")) {
        Ok(s) => out["lib_ussr"] = serde_json::to_value(&s).unwrap(),
        Err(e) => {
            eprintln!("lib_ussr failed: {e}");
            return ExitCode::FAILURE;
        }
    }
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
    println!("intake+lib_ussr done in {:?}", t0.elapsed());
    ExitCode::SUCCESS
}

fn datasets_cmd(args: &[String]) -> ExitCode {
    // stacks-import datasets register <library-db> | datasets verify <library-db>
    if args.len() != 4 || !["register", "verify"].contains(&args[2].as_str()) {
        eprintln!("usage: stacks-import datasets <register|verify> <library-db>");
        return ExitCode::FAILURE;
    }
    let store = match stacks_core::library::LibraryStore::open(&args[3]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("open library db: {e}");
            return ExitCode::FAILURE;
        }
    };
    if args[2] == "register" {
        match stacks_import::datasets::register_datasets(&mut { store }) {
            Ok(s) => {
                println!("{}", serde_json::to_string_pretty(&s).unwrap());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("register failed: {e}");
                ExitCode::FAILURE
            }
        }
    } else {
        match stacks_import::datasets::verify_datasets(&store) {
            Ok(flipped) => {
                println!("verified; flips: {}", flipped.len());
                for f in &flipped {
                    println!("  {f}");
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("verify failed: {e}");
                ExitCode::FAILURE
            }
        }
    }
}

/// Parse `--flag value` pairs out of args[start..]; returns (flags, positionals).
fn parse_flags(args: &[String], start: usize) -> Result<(Vec<(String, String)>, Vec<String>), String> {
    let mut flags = Vec::new();
    let mut positional = Vec::new();
    let mut i = start;
    while i < args.len() {
        if let Some(name) = args[i].strip_prefix("--") {
            match args.get(i + 1) {
                // Boolean flag: no value, or followed by another flag.
                Some(v) if !v.starts_with("--") => {
                    flags.push((name.to_string(), v.clone()));
                    i += 2;
                }
                _ => {
                    flags.push((name.to_string(), String::new()));
                    i += 1;
                }
            }
        } else {
            positional.push(args[i].clone());
            i += 1;
        }
    }
    Ok((flags, positional))
}

fn flag<'a>(flags: &'a [(String, String)], name: &str, default: &'a str) -> &'a str {
    flags
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v.as_str())
        .unwrap_or(default)
}

fn open_library(path: &str) -> Result<stacks_core::library::LibraryStore, ExitCode> {
    stacks_core::library::LibraryStore::open(path).map_err(|e| {
        eprintln!("open library db: {e}");
        ExitCode::FAILURE
    })
}

fn inproc_cmd(args: &[String]) -> ExitCode {
    let (flags, sources) = match parse_flags(args, 2) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    if sources.is_empty() {
        eprintln!("usage: stacks-import inproc [--intake-dir DIR] [--library-db DB] <source-path>...");
        return ExitCode::FAILURE;
    }
    let intake_dir = flag(&flags, "intake-dir", stacks_import::inproc::DEFAULT_INTAKE_DIR);
    let library_db = flag(&flags, "library-db", stacks_import::inproc::DEFAULT_LIBRARY_DB);
    let mut store = match open_library(library_db) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let sources: Vec<std::path::PathBuf> = sources.iter().map(std::path::PathBuf::from).collect();
    match stacks_import::inproc::enqueue_sources(
        &mut store,
        &sources,
        std::path::Path::new(intake_dir),
    ) {
        Ok(stats) => {
            println!("{}", serde_json::to_string_pretty(&stats).unwrap());
            if stats.errors.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(e) => {
            eprintln!("inproc failed: {e}");
            ExitCode::FAILURE
        }
    }
}

fn inproc_drain_cmd(args: &[String]) -> ExitCode {
    let (flags, positional) = match parse_flags(args, 2) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    if !positional.is_empty() {
        eprintln!("usage: stacks-import inproc-drain [--library-db DB] [--corpus-dir DIR] [--batch-openalex] [--no-embed]");
        return ExitCode::FAILURE;
    }
    let batch_openalex = flags.iter().any(|(n, _)| n == "batch-openalex");
    let library_db = flag(&flags, "library-db", stacks_import::inproc::DEFAULT_LIBRARY_DB);
    let corpus_dir = flag(&flags, "corpus-dir", stacks_import::inproc::DEFAULT_CORPUS_DIR);
    let mut store = match open_library(library_db) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let client = stacks_import::enrich::HttpEnricher::new();
    let no_embed = flags.iter().any(|(n, _)| n == "no-embed");
    let opts = stacks_import::inproc::DrainOpts { batch_openalex, no_embed };
    match stacks_import::inproc::drain_queue(&mut store, std::path::Path::new(corpus_dir), &client, &opts) {
        Ok(stats) => {
            println!("{}", serde_json::to_string_pretty(&stats).unwrap());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("inproc-drain failed: {e}");
            ExitCode::FAILURE
        }
    }
}

fn dlq_cmd(args: &[String]) -> ExitCode {
    if args.len() < 3 {
        eprintln!("usage:");
        eprintln!("  stacks-import dlq list [--library-db DB]");
        eprintln!("  stacks-import dlq assert <queue-id> --kind paper|document [--category CAT] [--title T] --by WHO [--notes N] [--library-db DB]");
        return ExitCode::FAILURE;
    }
    match args[2].as_str() {
        "list" => {
            let (flags, positional) = match parse_flags(args, 3) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::FAILURE;
                }
            };
            if !positional.is_empty() {
                eprintln!("usage: stacks-import dlq list [--library-db DB]");
                return ExitCode::FAILURE;
            }
            let library_db = flag(&flags, "library-db", stacks_import::inproc::DEFAULT_LIBRARY_DB);
            let store = match open_library(library_db) {
                Ok(s) => s,
                Err(code) => return code,
            };
            match stacks_import::inproc::dlq_list(&store) {
                Ok(entries) => {
                    println!("{}", serde_json::to_string_pretty(&entries).unwrap());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("dlq list failed: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        "assert" => {
            let Some(queue_id) = args.get(3).and_then(|s| s.parse::<i64>().ok()) else {
                eprintln!("usage: stacks-import dlq assert <queue-id> --kind paper|document [--category CAT] [--title T] --by WHO [--notes N] [--library-db DB]");
                return ExitCode::FAILURE;
            };
            let (flags, positional) = match parse_flags(args, 4) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::FAILURE;
                }
            };
            let kind = flag(&flags, "kind", "");
            let by = flag(&flags, "by", "");
            if kind.is_empty() || by.is_empty() || !positional.is_empty() {
                eprintln!("usage: stacks-import dlq assert <queue-id> --kind paper|document [--category CAT] [--title T] --by WHO [--notes N] [--library-db DB]");
                return ExitCode::FAILURE;
            }
            let library_db = flag(&flags, "library-db", stacks_import::inproc::DEFAULT_LIBRARY_DB);
            let category = flags.iter().find(|(n, _)| n == "category").map(|(_, v)| v.as_str());
            let title = flags.iter().find(|(n, _)| n == "title").map(|(_, v)| v.as_str());
            let notes = flags.iter().find(|(n, _)| n == "notes").map(|(_, v)| v.as_str());
            let mut store = match open_library(library_db) {
                Ok(s) => s,
                Err(code) => return code,
            };
            match stacks_import::inproc::dlq_assert(&mut store, queue_id, kind, category, title, by, notes) {
                Ok(()) => {
                    println!("{}", serde_json::json!({"asserted": queue_id, "kind": kind, "by": by}));
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("dlq assert failed: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        other => {
            eprintln!("unknown dlq subcommand {other:?}; expected list|assert");
            ExitCode::FAILURE
        }
    }
}

fn inproc_status_cmd(args: &[String]) -> ExitCode {
    let (flags, positional) = match parse_flags(args, 2) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    if !positional.is_empty() {
        eprintln!("usage: stacks-import inproc-status [--library-db DB] [--json]");
        return ExitCode::FAILURE;
    }
    let library_db = flag(&flags, "library-db", stacks_import::inproc::DEFAULT_LIBRARY_DB);
    let json = flags.iter().any(|(n, _)| n == "json");
    // Read-only alongside the live drain: query_only + busy_timeout, no locks taken.
    let store = match stacks_core::library::LibraryStore::open_read_only(library_db) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("open library db: {e}");
            return ExitCode::FAILURE;
        }
    };
    match stacks_import::status::queue_status(&store) {
        Ok(status) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&status).unwrap());
            } else {
                print!("{}", stacks_import::status::render_human(&status));
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("inproc-status failed: {e}");
            ExitCode::FAILURE
        }
    }
}

fn migrate_ledger_cmd(args: &[String]) -> ExitCode {
    let Some(state) = args.get(2).filter(|a| !a.starts_with("--")).cloned() else {
        eprintln!("usage: stacks-import migrate-ledger <nl-state.json> [--journal <journal.jsonl>] [--nl-root <dir>] --library-db <path>");
        return ExitCode::FAILURE;
    };
    let (flags, positional) = match parse_flags(args, 3) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    if !positional.is_empty() {
        eprintln!("usage: stacks-import migrate-ledger <nl-state.json> [--journal <journal.jsonl>] [--nl-root <dir>] --library-db <path>");
        return ExitCode::FAILURE;
    }
    let library_db = flag(&flags, "library-db", stacks_import::inproc::DEFAULT_LIBRARY_DB);
    let journal = flags.iter().find(|(n, _)| n == "journal").map(|(_, v)| v.clone());
    let nl_root = flag(&flags, "nl-root", "/home/patrick/neurotic_library");
    let mut store = match open_library(library_db) {
        Ok(s) => s,
        Err(code) => return code,
    };
    match stacks_import::migrate_ledger::migrate_ledger(
        &mut store,
        std::path::Path::new(&state),
        journal.as_deref().map(std::path::Path::new),
        std::path::Path::new(nl_root),
    ) {
        Ok(stats) => {
            println!("{}", serde_json::to_string_pretty(&stats).unwrap());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("migrate-ledger failed: {e}");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 && args[1] == "inproc-status" {
        return inproc_status_cmd(&args);
    }
    if args.len() >= 2 && args[1] == "migrate-ledger" {
        return migrate_ledger_cmd(&args);
    }
    if args.len() >= 2 && args[1] == "inproc" {
        return inproc_cmd(&args);
    }
    if args.len() >= 2 && args[1] == "inproc-drain" {
        return inproc_drain_cmd(&args);
    }
    if args.len() >= 2 && args[1] == "dlq" {
        return dlq_cmd(&args);
    }
    if args.len() >= 2 && args[1] == "datasets" {
        return datasets_cmd(&args);
    }
    if args.len() >= 2 && args[1] == "intake" {
        return intake_cmd(&args);
    }
    if args.len() >= 2 && args[1] == "materials" {
        return materials_cmd(&args);
    }
    if args.len() >= 2 && args[1] == "matdattmp" {
        return matdattmp_cmd(&args);
    }
    if args.len() >= 2 && args[1] == "library-import" {
        return library_import_cmd(&args);
    }
    if args.len() >= 2 && args[1] == "verify-extraction" {
        return verify_cmd(&args);
    }
    if args.len() >= 4 && args[1] == "pilot-load" && (args.len() == 4 || args.len() == 8) {
        return pilot_load(&args[2], &args[3], &args);
    }
    if args.len() != 4 || args[1] != "chem-recipes" {
        eprintln!("usage:");
        eprintln!("  stacks-import chem-recipes <jsonl-dir> <db-path>");
        eprintln!("  stacks-import pilot-load <extracted.jsonl> <db-path> [<dataset> <book> <source-path> <sha256>]");
        return ExitCode::FAILURE;
    }
    let dir = PathBuf::from(&args[2]);
    let mut store = match Store::open(&args[3]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("open store: {e}");
            return ExitCode::FAILURE;
        }
    };
    // Import-time pragmas: the DB is rebuildable from the source corpus and
    // the import is idempotent, so trading crash durability for throughput
    // is safe here. A checkpoint runs when the import finishes.
    if let Err(e) = (|| -> Result<(), stacks_core::StoreError> {
        let conn = store.raw();
        conn.pragma_update(None, "synchronous", "OFF")?;
        conn.pragma_update(None, "wal_autocheckpoint", 0)?;
        conn.pragma_update(None, "cache_size", -2_000_000)?;
        conn.pragma_update(None, "mmap_size", 4_i64 << 30)?;
        conn.pragma_update(None, "temp_store", "MEMORY")?;
        Ok(())
    })() {
        eprintln!("pragma setup: {e}");
        return ExitCode::FAILURE;
    }
    let t0 = Instant::now();
    let result = (|| {
        let mut stats = stacks_import::import_inorganic(&mut store, &dir)?;
        eprintln!("inorganic done in {:?}; starting ORD", t0.elapsed());
        let ord = stacks_import::import_ord(&mut store, &dir)?;
        for (k, v) in ord.per_source {
            stats.per_source.insert(k, v);
        }
        Ok::<_, stacks_import::ImportError>(stats)
    })();
    match result {
        Ok(stats) => {
            if let Err(e) = store
                .raw()
                .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            {
                eprintln!("final checkpoint: {e}");
            }
            print_stats(&stats);
            let m = stats.merged();
            println!(
                "total: seen={} inserted={} skipped={} in {:?}",
                m.recipes_seen,
                m.recipes_inserted,
                m.recipes_skipped_existing,
                t0.elapsed()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("import failed: {e}");
            ExitCode::FAILURE
        }
    }
}
