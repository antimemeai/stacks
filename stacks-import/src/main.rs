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

fn pilot_load(jsonl: &str, db: &str) -> ExitCode {
    let mut store = match Store::open(db) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("open store: {e}");
            return ExitCode::FAILURE;
        }
    };
    let source = stacks_import::ExtractSource {
        source_dataset: "sciencemadness".to_string(),
        book: "brauer".to_string(),
        path: "/home/patrick/neurotic_library/datasets/sciencemadness/brauer_ocr.pdf".to_string(),
        sha256: "77ac5c1c37524a91136f402ec97fcea374e2ff85f29f43d35daa73be3c16272f".to_string(),
        extractor_version: "kimi-agent pilot-1".to_string(),
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

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 4 && args[1] == "pilot-load" {
        return pilot_load(&args[2], &args[3]);
    }
    if args.len() != 4 || args[1] != "chem-recipes" {
        eprintln!("usage:");
        eprintln!("  stacks-import chem-recipes <jsonl-dir> <db-path>");
        eprintln!("  stacks-import pilot-load <extracted.jsonl> <db-path>");
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
