//! Wave F sources: Materials Project 2025-09-25 snapshot, COD CIF stream,
//! OQMD MySQL dump, TOP4040 npz inventory. Each plugs into the wave
//! framework (`crate::wave`).

use std::io::BufRead;
use std::path::Path;

use stacks_core::materials::*;

use crate::wave::{batched, gz_jsonl_stream, Outcome, WaveCtx, WaveSource, WaveStat};
use crate::ImportError;

fn prop(
    key: &str,
    collection: &str,
    kind: &str,
    value: Option<f64>,
    unit: Option<&str>,
    text: Option<String>,
) -> Property {
    Property {
        id: 0,
        external_key: key.to_string(),
        collection: collection.to_string(),
        kind: kind.to_string(),
        value,
        unit: unit.map(str::to_string),
        text_value: text,
        extra: None,
    }
}

fn f64v(v: &serde_json::Value) -> Option<f64> {
    v.as_f64()
}

fn strv(v: &serde_json::Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

/// Minimal entry for a material that a property collection references before
/// (or without) summary.
fn stub_entry(rec: &serde_json::Value, id: &str, imported_at: &str) -> MaterialEntry {
    MaterialEntry {
        external_key: format!("mp:{id}"),
        source: "mp".to_string(),
        source_id: id.to_string(),
        formula: strv(&rec["formula_pretty"]),
        elements: rec["elements"].as_array().map(|a| {
            a.iter()
                .filter_map(|e| e.as_str().map(str::to_string))
                .collect()
        }),
        nsites: rec["nsites"].as_i64(),
        spacegroup: strv(&rec["symmetry"]["symbol"]),
        spacegroup_number: rec["symmetry"]["number"].as_i64(),
        crystal_system: strv(&rec["symmetry"]["crystal_system"]),
        cell: None,
        structure: None,
        density: f64v(&rec["density"]),
        description: None,
        reference_path: None,
        imported_at: imported_at.to_string(),
    }
}

fn emit_props(
    conn: &rusqlite::Connection,
    rec: &serde_json::Value,
    key: &str,
    collection: &str,
    specs: &[(&str, &str, Option<&str>)],
) -> Result<u64, ImportError> {
    let mut n = 0;
    for (field, kind, unit) in specs {
        let v = &rec[*field];
        let inserted = match (f64v(v), strv(v)) {
            (Some(x), _) => {
                insert_property(conn, &prop(key, collection, kind, Some(x), *unit, None))?
            }
            (None, Some(s)) => insert_property(
                conn,
                &prop(key, collection, kind, None, None, Some(s.to_string())),
            )?,
            _ => false,
        };
        n += inserted as u64;
    }
    Ok(n)
}

// ---------- MP summary ----------

pub struct MpSummary {
    pub dir: String,
}

impl WaveSource for MpSummary {
    fn name(&self) -> &str {
        "mp_summary"
    }

    fn run(&self, store: &mut MaterialsStore, ctx: &WaveCtx) -> Result<WaveStat, ImportError> {
        let mut stat = WaveStat::default();
        let mut stream = gz_jsonl_stream(Path::new(&self.dir))?;
        batched(store, &mut stream, 2000, &mut stat, |conn, rec| {
            let id = strv(&rec["material_id"]).unwrap_or_default();
            if id.is_empty() {
                return Ok(Outcome::Rejected);
            }
            let key = format!("mp:{id}");
            let mut e = stub_entry(rec, &id, &ctx.imported_at);
            let lattice = &rec["structure"]["lattice"];
            if lattice.is_object() {
                e.cell = Some(serde_json::json!({
                    "a": lattice["a"], "b": lattice["b"], "c": lattice["c"],
                    "alpha": lattice["alpha"], "beta": lattice["beta"], "gamma": lattice["gamma"],
                    "volume": lattice["volume"],
                }));
            }
            let s = serde_json::to_string(&rec["structure"])?;
            if s.len() < 65536 {
                e.structure = Some(rec["structure"].clone());
            }
            let inserted = insert_entry(conn, &e)?;
            if inserted {
                for spec in [
                    ("band_gap", "band_gap", Some("eV")),
                    ("energy_above_hull", "energy_above_hull", Some("eV/atom")),
                    (
                        "formation_energy_per_atom",
                        "formation_energy_per_atom",
                        Some("eV/atom"),
                    ),
                    ("energy_per_atom", "energy_per_atom", Some("eV/atom")),
                    ("density", "density", Some("g/cm3")),
                    ("volume", "volume", Some("other:Å^3")),
                    ("total_magnetization", "total_magnetization", Some("µB")),
                    ("cbm", "cbm", Some("eV")),
                    ("vbm", "vbm", Some("eV")),
                    ("efermi", "efermi", Some("eV")),
                    (
                        "weighted_surface_energy",
                        "weighted_surface_energy",
                        Some("other:eV/ang^2"),
                    ),
                    (
                        "weighted_work_function",
                        "weighted_work_function",
                        Some("eV"),
                    ),
                ] {
                    emit_props(conn, rec, &key, "summary", &[spec])?;
                }
                for (field, kind) in [
                    ("is_stable", "is_stable"),
                    ("is_gap_direct", "is_gap_direct"),
                    ("is_metal", "is_metal"),
                    ("is_magnetic", "is_magnetic"),
                    ("theoretical", "theoretical"),
                    ("deprecated", "deprecated"),
                ] {
                    if let Some(b) = rec[field].as_bool() {
                        insert_property(
                            conn,
                            &prop(&key, "summary", kind, None, None, Some(b.to_string())),
                        )?;
                    }
                }
                for (field, kind) in [("ordering", "ordering")] {
                    if let Some(s) = strv(&rec[field]) {
                        insert_property(conn, &prop(&key, "summary", kind, None, None, Some(s)))?;
                    }
                }
            }
            Ok(if inserted {
                Outcome::Inserted
            } else {
                Outcome::Skipped
            })
        })
        .map(|_| stat)
    }
}

// ---------- MP robocrys (descriptions → FTS) ----------

pub struct MpRobocrys {
    pub dir: String,
}

impl WaveSource for MpRobocrys {
    fn name(&self) -> &str {
        "mp_robocrys"
    }

    fn run(&self, store: &mut MaterialsStore, ctx: &WaveCtx) -> Result<WaveStat, ImportError> {
        let mut stat = WaveStat::default();
        let mut stream = gz_jsonl_stream(Path::new(&self.dir))?;
        batched(store, &mut stream, 2000, &mut stat, |conn, rec| {
            let id = strv(&rec["material_id"]).unwrap_or_default();
            if id.is_empty() {
                return Ok(Outcome::Rejected);
            }
            let key = format!("mp:{id}");
            let desc = strv(&rec["description"]);
            let inserted = insert_entry(conn, &stub_entry(rec, &id, &ctx.imported_at))?;
            if let Some(d) = desc {
                conn.execute(
                    "UPDATE material_entry SET description = ?1 WHERE external_key = ?2",
                    [&d, &key],
                )?;
                if inserted {
                    return Ok(Outcome::Inserted);
                }
            }
            Ok(if inserted {
                Outcome::Inserted
            } else {
                Outcome::Skipped
            })
        })
        .map(|_| stat)
    }
}

// ---------- MP molecules ----------

pub struct MpMolecules {
    pub dir: String,
}

impl WaveSource for MpMolecules {
    fn name(&self) -> &str {
        "mp_molecules"
    }

    fn run(&self, store: &mut MaterialsStore, ctx: &WaveCtx) -> Result<WaveStat, ImportError> {
        let mut stat = WaveStat::default();
        let mut stream = gz_jsonl_stream(Path::new(&self.dir))?;
        batched(store, &mut stream, 2000, &mut stat, |conn, rec| {
            let id = strv(&rec["molecule_id"]).unwrap_or_default();
            if id.is_empty() {
                return Ok(Outcome::Rejected);
            }
            let key = format!("mp-mol:{id}");
            let formula = strv(&rec["formula_alphabetical"]).map(|f| f.replace(' ', ""));
            let e = MaterialEntry {
                external_key: key.clone(),
                source: "mp".to_string(),
                source_id: id.clone(),
                formula,
                elements: rec["elements"].as_array().map(|a| {
                    a.iter()
                        .filter_map(|e| e.as_str().map(str::to_string))
                        .collect()
                }),
                nsites: rec["natoms"].as_i64(),
                spacegroup: strv(&rec["symmetry"]["point_group"]),
                spacegroup_number: None,
                crystal_system: None,
                cell: None,
                structure: None,
                density: None,
                description: None,
                reference_path: None,
                imported_at: ctx.imported_at.clone(),
            };
            let inserted = insert_entry(conn, &e)?;
            if inserted {
                emit_props(
                    conn,
                    rec,
                    &key,
                    "molecules",
                    &[
                        ("charge", "charge", Some("1")),
                        ("spin_multiplicity", "spin_multiplicity", Some("1")),
                        ("natoms", "natoms", Some("1")),
                    ],
                )?;
                for (field, kind) in [("inchi", "inchi"), ("inchi_key", "inchikey")] {
                    if let Some(s) = strv(&rec[field]) {
                        insert_property(conn, &prop(&key, "molecules", kind, None, None, Some(s)))?;
                    }
                }
            }
            Ok(if inserted {
                Outcome::Inserted
            } else {
                Outcome::Skipped
            })
        })
        .map(|_| stat)
    }
}

// ---------- MP property collections (thermo, electronic-structure, …) ----------

pub struct MpProps {
    pub dir: String,
    pub collection: &'static str,
    pub id_field: &'static str,
    pub specs: &'static [(&'static str, &'static str, Option<&'static str>)],
    pub text_fields: &'static [(&'static str, &'static str)],
}

impl WaveSource for MpProps {
    fn name(&self) -> &str {
        self.collection
    }

    fn run(&self, store: &mut MaterialsStore, ctx: &WaveCtx) -> Result<WaveStat, ImportError> {
        let mut stat = WaveStat::default();
        let mut stream = gz_jsonl_stream(Path::new(&self.dir))?;
        batched(store, &mut stream, 2000, &mut stat, |conn, rec| {
            let id = strv(&rec[self.id_field]).unwrap_or_default();
            if id.is_empty() {
                return Ok(Outcome::Rejected);
            }
            let key = format!("mp:{id}");
            // entry must exist for the FK; minimal stub if summary missed it
            let entry_new = insert_entry(conn, &stub_entry(rec, &id, &ctx.imported_at))?;
            let mut new_rows = emit_props(conn, rec, &key, self.collection, self.specs)?;
            for (field, kind) in self.text_fields {
                let inserted = if let Some(s) = strv(&rec[*field]) {
                    insert_property(
                        conn,
                        &prop(&key, self.collection, kind, None, None, Some(s)),
                    )?
                } else if let Some(b) = rec[*field].as_bool() {
                    insert_property(
                        conn,
                        &prop(&key, self.collection, kind, None, None, Some(b.to_string())),
                    )?
                } else {
                    false
                };
                new_rows += inserted as u64;
            }
            Ok(if entry_new || new_rows > 0 {
                Outcome::Inserted
            } else {
                Outcome::Skipped
            })
        })
        .map(|_| stat)
    }
}

// ---------- MP electrodes (battery_id keyed) ----------

pub struct MpElectrodes {
    pub dir: String,
    pub collection: &'static str,
}

impl WaveSource for MpElectrodes {
    fn name(&self) -> &str {
        self.collection
    }

    fn run(&self, store: &mut MaterialsStore, ctx: &WaveCtx) -> Result<WaveStat, ImportError> {
        let mut stat = WaveStat::default();
        let mut stream = gz_jsonl_stream(Path::new(&self.dir))?;
        batched(store, &mut stream, 2000, &mut stat, |conn, rec| {
            let id = strv(&rec["battery_id"]).unwrap_or_default();
            if id.is_empty() {
                return Ok(Outcome::Rejected);
            }
            let key = format!("mp-bat:{id}");
            let formula = strv(&rec["framework_formula"]).or_else(|| strv(&rec["battery_formula"]));
            let e = MaterialEntry {
                external_key: key.clone(),
                source: "mp".to_string(),
                source_id: id.clone(),
                formula,
                elements: rec["elements"].as_array().map(|a| {
                    a.iter()
                        .filter_map(|e| e.as_str().map(str::to_string))
                        .collect()
                }),
                nsites: None,
                spacegroup: None,
                spacegroup_number: None,
                crystal_system: None,
                cell: None,
                structure: None,
                density: None,
                description: None,
                reference_path: None,
                imported_at: ctx.imported_at.clone(),
            };
            let inserted = insert_entry(conn, &e)?;
            if inserted {
                emit_props(
                    conn,
                    rec,
                    &key,
                    self.collection,
                    &[
                        ("average_voltage", "average_voltage", Some("V")),
                        ("max_voltage_step", "max_voltage_step", Some("V")),
                        ("capacity_grav", "capacity_grav", Some("mA·h/g")),
                        ("capacity_vol", "capacity_vol", Some("other:mA·h/cm^3")),
                        ("energy_grav", "energy_grav", Some("other:W·h/kg")),
                        ("energy_vol", "energy_vol", Some("other:W·h/L")),
                        ("max_delta_volume", "max_delta_volume", Some("%")),
                        ("num_steps", "num_steps", Some("1")),
                        ("fracA_charge", "fracA_charge", Some("1")),
                        ("stability_charge", "stability_charge", Some("eV/atom")),
                        (
                            "stability_discharge",
                            "stability_discharge",
                            Some("eV/atom"),
                        ),
                    ],
                )?;
                if let Some(s) = strv(&rec["working_ion"]) {
                    insert_property(
                        conn,
                        &prop(&key, self.collection, "working_ion", None, None, Some(s)),
                    )?;
                }
            }
            Ok(if inserted {
                Outcome::Inserted
            } else {
                Outcome::Skipped
            })
        })
        .map(|_| stat)
    }
}

// ---------- COD (stream-parsed CIF headers) ----------

/// COD CIF headers, stream-parsed offline to JSONL (see data/materials/cod_extract.py).
/// Payloads referenced in place: reference_path = "<archive>::<member>".
/// parse_failed rows land in material_quarantine, never crash or fabricate.
pub struct Cod {
    pub jsonl: String,
    pub archive: String,
}

impl WaveSource for Cod {
    fn name(&self) -> &str {
        "cod"
    }

    fn run(&self, store: &mut MaterialsStore, ctx: &WaveCtx) -> Result<WaveStat, ImportError> {
        let mut stat = WaveStat::default();
        let file = std::fs::File::open(&self.jsonl)?;
        let reader = std::io::BufReader::with_capacity(1 << 20, file);
        let mut stream = reader.lines().map(|l| {
            l.map_err(ImportError::from).and_then(|s| {
                serde_json::from_str::<serde_json::Value>(&s).map_err(ImportError::from)
            })
        });
        batched(store, &mut stream, 5000, &mut stat, |conn, rec| {
            let cod_id = strv(&rec["cod_id"]).unwrap_or_default();
            if rec["parse_failed"].as_bool() == Some(true) {
                insert_material_quarantine(
                    conn,
                    "cod",
                    &cod_id,
                    "parse_failure",
                    rec["error"].as_str().or(rec["member"].as_str()),
                )?;
                return Ok(Outcome::Rejected);
            }
            let formula = strv(&rec["formula"]).map(|f| f.replace(' ', ""));
            let e = MaterialEntry {
                external_key: format!("cod:{cod_id}"),
                source: "cod".to_string(),
                source_id: cod_id.clone(),
                formula,
                elements: None,
                nsites: None,
                spacegroup: strv(&rec["spacegroup"]),
                spacegroup_number: rec["sg_number"].as_str().and_then(|s| s.parse().ok()),
                crystal_system: None,
                cell: Some(serde_json::json!({
                    "a": rec["a"], "b": rec["b"], "c": rec["c"],
                    "alpha": rec["alpha"], "beta": rec["beta"], "gamma": rec["gamma"],
                    "volume": rec["volume"],
                })),
                structure: None,
                density: None,
                description: None,
                reference_path: Some(format!(
                    "{}::{}",
                    self.archive,
                    strv(&rec["member"]).unwrap_or_default()
                )),
                imported_at: ctx.imported_at.clone(),
            };
            let inserted = insert_entry(conn, &e)?;
            Ok(if inserted {
                Outcome::Inserted
            } else {
                Outcome::Skipped
            })
        })
        .map(|_| stat)
    }
}

// ---------- TOP4040 (npz inventory) ----------

/// TOP4040: the acquisition contains only bare float arrays keyed by opaque
/// integer ids — no labels, formulas, or metadata (verified by reading
/// members). Imported as inventory rows with reference paths and a
/// payload_note; the missing-labels gap is documented in the report.
pub struct Top4040 {
    pub jsonl: String,
    pub archive: String,
}

impl WaveSource for Top4040 {
    fn name(&self) -> &str {
        "top4040"
    }

    fn run(&self, store: &mut MaterialsStore, ctx: &WaveCtx) -> Result<WaveStat, ImportError> {
        let mut stat = WaveStat::default();
        let file = std::fs::File::open(&self.jsonl)?;
        let reader = std::io::BufReader::new(file);
        let mut stream = reader.lines().map(|l| {
            l.map_err(ImportError::from).and_then(|s| {
                serde_json::from_str::<serde_json::Value>(&s).map_err(ImportError::from)
            })
        });
        batched(store, &mut stream, 5000, &mut stat, |conn, rec| {
            let id = strv(&rec["top_id"]).unwrap_or_default();
            let key = format!("top:{id}");
            let e = MaterialEntry {
                external_key: key.clone(),
                source: "top".to_string(),
                source_id: id.clone(),
                formula: None,
                elements: None,
                nsites: None,
                spacegroup: None,
                spacegroup_number: None,
                crystal_system: None,
                cell: None,
                structure: None,
                density: None,
                description: None,
                reference_path: Some(format!(
                    "{}::{}",
                    self.archive,
                    strv(&rec["member"]).unwrap_or_default()
                )),
                imported_at: ctx.imported_at.clone(),
            };
            let inserted = insert_entry(conn, &e)?;
            if inserted {
                insert_property(
                    conn,
                    &prop(
                        &key,
                        "top4040",
                        "payload_note",
                        None,
                        None,
                        Some(
                            "bare npz array payload; no labels/metadata in acquisition".to_string(),
                        ),
                    ),
                )?;
            }
            Ok(if inserted {
                Outcome::Inserted
            } else {
                Outcome::Skipped
            })
        })
        .map(|_| stat)
    }
}

// ---------- OQMD (MySQL dump, stream-parsed offline) ----------

/// OQMD v1.8: entries (id, path, label, duplicate_of_id, delta_e,
/// stability, composition_id, reference_id, prototype_id, ntypes, natoms)
/// and formation_energies (id, composition_id, entry_id, calculation_id,
/// description, fit_id, stability, delta_e), stream-parsed to JSONL.
pub struct Oqmd {
    pub entries_jsonl: String,
    pub fe_jsonl: String,
}

impl WaveSource for Oqmd {
    fn name(&self) -> &str {
        "oqmd"
    }

    fn run(&self, store: &mut MaterialsStore, ctx: &WaveCtx) -> Result<WaveStat, ImportError> {
        let mut stat = WaveStat::default();

        // formation energies first pass: entry_id -> best (min) delta_e rows
        // are emitted as properties in the entries pass; keep all rows in a
        // side map only for entries we see (entries.jsonl is the master list).
        let mut fe_by_entry: std::collections::HashMap<i64, Vec<(f64, f64, String, String)>> =
            std::collections::HashMap::new();
        if !self.fe_jsonl.is_empty() && std::path::Path::new(&self.fe_jsonl).exists() {
            let file = std::fs::File::open(&self.fe_jsonl)?;
            for line in std::io::BufReader::with_capacity(1 << 20, file).lines() {
                let row: Vec<serde_json::Value> = serde_json::from_str(&line?)?;
                let Some(entry_id) = row
                    .get(2)
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok())
                else {
                    continue;
                };
                let delta_e = row
                    .get(7)
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok());
                let stability = row
                    .get(6)
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok());
                let fit = row
                    .get(4)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let desc = row
                    .get(5)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if let (Some(de), Some(st)) = (delta_e, stability) {
                    fe_by_entry
                        .entry(entry_id)
                        .or_default()
                        .push((de, st, fit, desc));
                }
            }
        }

        let file = std::fs::File::open(&self.entries_jsonl)?;
        let mut stream = std::io::BufReader::with_capacity(1 << 20, file)
            .lines()
            .map(|l| {
                l.map_err(ImportError::from).and_then(|s| {
                    serde_json::from_str::<Vec<serde_json::Value>>(&s).map_err(ImportError::from)
                })
            });
        let fe_map = &fe_by_entry;
        batched(store, &mut stream, 5000, &mut stat, |conn, row| {
            let Some(id) = row.first().and_then(|v| v.as_str()) else {
                return Ok(Outcome::Rejected);
            };
            let formula = row
                .get(6)
                .and_then(|v| v.as_str())
                .map(|f| f.replace(' ', ""));
            let e = MaterialEntry {
                external_key: format!("oqmd:{id}"),
                source: "oqmd".to_string(),
                source_id: id.to_string(),
                formula,
                elements: None,
                nsites: row
                    .get(10)
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok()),
                spacegroup: None,
                spacegroup_number: None,
                crystal_system: None,
                cell: None,
                structure: None,
                density: None,
                description: None,
                reference_path: row.get(1).and_then(|v| v.as_str()).map(str::to_string),
                imported_at: ctx.imported_at.clone(),
            };
            let key = e.external_key.clone();
            let entry_new = insert_entry(conn, &e)?;
            let mut new_rows = 0u64;
            let mut emit = |kind: &str, value: Option<f64>| -> Result<(), ImportError> {
                if let Some(x) = value {
                    new_rows += insert_property(
                        conn,
                        &prop(&key, "oqmd", kind, Some(x), Some("eV/atom"), None),
                    )? as u64;
                }
                Ok(())
            };
            emit(
                "delta_e",
                row.get(4)
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok()),
            )?;
            emit(
                "stability",
                row.get(5)
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok()),
            )?;
            if let Some(p) = row.get(8).and_then(|v| v.as_str()) {
                new_rows += insert_property(
                    conn,
                    &prop(&key, "oqmd", "prototype", None, None, Some(p.to_string())),
                )? as u64;
            }
            if let Ok(id_i64) = id.parse::<i64>() {
                if let Some(rows) = fe_map.get(&id_i64) {
                    for (de, st, fit, desc) in rows {
                        let mut extra = serde_json::json!({"fit": fit, "description": desc});
                        new_rows += insert_property(
                            conn,
                            &Property {
                                id: 0,
                                external_key: key.clone(),
                                collection: "oqmd_fe".to_string(),
                                kind: "delta_e".to_string(),
                                value: Some(*de),
                                unit: Some("eV/atom".to_string()),
                                text_value: None,
                                extra: Some(extra.clone()),
                            },
                        )? as u64;
                        new_rows += insert_property(
                            conn,
                            &Property {
                                id: 0,
                                external_key: key.clone(),
                                collection: "oqmd_fe".to_string(),
                                kind: "stability".to_string(),
                                value: Some(*st),
                                unit: Some("eV/atom".to_string()),
                                text_value: None,
                                extra: Some(std::mem::take(&mut extra)),
                            },
                        )? as u64;
                    }
                }
            }
            Ok(if entry_new || new_rows > 0 {
                Outcome::Inserted
            } else {
                Outcome::Skipped
            })
        })
        .map(|_| stat)
    }
}
