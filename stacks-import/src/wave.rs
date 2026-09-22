//! Wave framework: shared batching / idempotency / report plumbing that
//! per-source modules plug into. A `WaveSource` supplies records; the
//! framework owns transactions, skip counting, and the report shape.

use std::io::BufRead;
use std::path::Path;
use std::time::Instant;

use stacks_core::materials::MaterialsStore;

use crate::ImportError;

/// Per-source outcome, serialized into the wave report.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct WaveStat {
    pub seen: u64,
    pub inserted: u64,
    pub skipped_existing: u64,
    pub rejected: u64,
    pub quarantined: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    pub seconds: f64,
}

/// One corpus source in a wave.
pub trait WaveSource {
    fn name(&self) -> &str;
    fn run(&self, store: &mut MaterialsStore, ctx: &WaveCtx) -> Result<WaveStat, ImportError>;
}

/// Shared context (work dirs, constants).
pub struct WaveCtx {
    pub workdir: std::path::PathBuf,
    pub imported_at: String,
}

/// Run a wave: every source, timed, aggregated; failures abort with the
/// source name attached.
pub fn run_wave(
    store: &mut MaterialsStore,
    ctx: &WaveCtx,
    sources: Vec<Box<dyn WaveSource>>,
) -> Result<serde_json::Value, ImportError> {
    let mut report = serde_json::Map::new();
    for source in sources {
        let t = Instant::now();
        let mut stat = source.run(store, ctx).map_err(|e| {
            ImportError::Io(std::io::Error::other(format!("{}: {e}", source.name())))
        })?;
        stat.seconds = t.elapsed().as_secs_f64();
        eprintln!(
            "{}: seen={} inserted={} skipped={} rejected={} quarantined={} in {:.1}s",
            source.name(),
            stat.seen,
            stat.inserted,
            stat.skipped_existing,
            stat.rejected,
            stat.quarantined,
            stat.seconds
        );
        report.insert(source.name().to_string(), serde_json::to_value(&stat)?);
    }
    Ok(serde_json::Value::Object(report))
}

/// What one record did.
pub enum Outcome {
    Inserted,
    Skipped,
    Rejected,
}

/// Batched-stream driver: read records, apply `insert` in transactions of
/// `batch` size. The closure returns an [`Outcome`]; the driver counts.
pub fn batched<T>(
    store: &mut MaterialsStore,
    stream: &mut dyn Iterator<Item = Result<T, ImportError>>,
    batch: usize,
    stat: &mut WaveStat,
    mut insert: impl FnMut(&rusqlite::Connection, &T) -> Result<Outcome, ImportError>,
) -> Result<(), ImportError> {
    loop {
        let n = store.with_transaction(|conn| {
            let mut n = 0;
            while n < batch {
                let Some(item) = stream.next() else { break };
                let item = item?;
                stat.seen += 1;
                match insert(conn, &item)? {
                    Outcome::Inserted => stat.inserted += 1,
                    Outcome::Skipped => stat.skipped_existing += 1,
                    Outcome::Rejected => stat.rejected += 1,
                }
                n += 1;
            }
            Ok::<_, ImportError>(n)
        })?;
        if n == 0 {
            break;
        }
    }
    Ok(())
}

/// Gzip-JSONL record stream over a directory of partitioned *.jsonl.gz
/// shards (Materials Project layout), reading each shard with flate2.
pub fn gz_jsonl_stream(
    dir: &Path,
) -> Result<Box<dyn Iterator<Item = Result<serde_json::Value, ImportError>>>, ImportError> {
    let mut files = Vec::new();
    walk(dir, &mut files)?;
    files.retain(|p| p.extension().is_some_and(|e| e == "gz"));
    files.sort();
    Ok(Box::new(files.into_iter().flat_map(|p| {
        match std::fs::File::open(&p).map(flate2::read::GzDecoder::new) {
            Ok(gz) => {
                let reader = std::io::BufReader::with_capacity(1 << 20, gz);
                reader
                    .lines()
                    .map(|l| {
                        l.map_err(ImportError::from)
                            .and_then(|s| serde_json::from_str(&s).map_err(ImportError::from))
                    })
                    .collect::<Vec<_>>()
            }
            Err(e) => vec![Err(ImportError::from(e))],
        }
        .into_iter()
    })))
}

fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            walk(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}
