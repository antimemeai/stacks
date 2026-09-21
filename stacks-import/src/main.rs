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

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 || args[1] != "chem-recipes" {
        eprintln!("usage: stacks-import chem-recipes <jsonl-dir> <db-path>");
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
