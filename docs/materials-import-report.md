# Materials red carpet import — report (Wave F)

Date: 2026-09-22. Sources read-only under `neurotic_library/datasets/`.
Output: `data/materials.db` (sibling DB; stacks.db and library.db untouched).
Command: `stacks-import materials <mp-collections/2025-09-25> data/materials data/materials.db`.
Framework: `stacks-import/src/wave.rs` — `WaveSource` trait +
`batched()` stream driver + `WaveStat` report shape; per-source modules in
`materials_wave.rs` plug into it (this is the generalized scaffolding the
wave was meant to produce).

## Counts (materials.db)

| source | rows seen | entries | notes |
|---|---|---|---|
| MP summary | 210,579 | 210,579 | full per-material records; structure JSON when < 64 KB |
| MP robocrys | 400,580 | 0 (merged) | descriptions UPDATEd onto summary entries (200,290 with text) |
| MP thermo | 800,326 | — | 1M+ property rows (GGA/GGA+U thermo_types) |
| MP electronic-structure | 400,974 | — | band_gap/efermi per task |
| MP elasticity | 26,566 | — | |
| MP magnetism | 400,974 | — | |
| MP dielectric | 14,664 | — | |
| MP piezoelectric | 6,644 | — | |
| MP insertion-electrodes | 13,694 | 6,847 (mp-bat:) | |
| MP conversion-electrodes | 41,244 | 20,221 (mp-bat:) | |
| MP molecules | 1,900,874 | 404,658 (mp-mol:) | full load, charge/spin/inchikey |
| COD (cod-cifs-mysql.txz) | 111,315 parsed | 111,309 | **partial — see corruption note** |
| TOP4040 | 10,000 | 10,000 | inventory rows only — see note |
| OQMD (v1.8, 2026-02 dump) | 1,407,285 entries + 2,256,458 formation-energy rows | 1,407,285 | entries: delta_e/stability/prototype/ntypes/natoms; fe rows as oqmd_fe properties |

Totals: **2,344,054 material entries, ~11.4M property rows** (7,935,056 after
dedupe + 3.47M oqmd), FTS over formula+description (2,344,054 docs). Sources:
mp 815,460 / oqmd 1,407,285 / cod 111,309 / top 10,000.

## MP snapshot policy

Loaded **only** `collections/2025-09-25`. Skipped as superseded: 2022-10-28,
2023-11-01, 2024-02-29, 2024-11-14, 2024-12-18, 2025-02-12,
2025-02-12-post, 2025-02-12-post1, 2025-04-10, 2025-06-09 (9 older
snapshots). Non-priority collections in the snapshot skipped this wave
(materials/, oxi-states, chemenv, bonds, alloys*, absorption, provenance,
task-validation, jcesr, oxidation-states, reverse-pourbaix*) — large,
structure-heavy, and covered by summary's structure JSON for the common
case; revisit when structure search exists.

## Property vocabulary observed (top by count)

energy_above_hull / energy_per_atom / formation_energy_per_atom (1,000,813
each, eV/atom), band_gap (601,461 eV), is_stable/is_gap_direct/is_metal/
is_magnetic/ordering/thermo_type (bools+categories as text), total_magnetization
(µB), efermi (eV), bulk/shear_modulus (GPa), average_voltage (V),
capacity_grav (mA·h/g), e_ij_max (pC/N), debye_temperature (other:K),
e_total/n (1), charge/spin_multiplicity (molecules). Unit discipline:
closed vocabulary + `other:<unit>` escape hatch, CHECK-enforced (tooth a).

## COD corruption note (important)

`cod-cifs-mysql.txz` is **corrupt mid-stream**: `xz -t` fails with
"Compressed data is corrupt" after ~111k members (the 9xxxxxxx series +
part of 7xxxxxxx). xz has no resync; the source is sacrosanct so no repair
was attempted. **111,315 members parsed cleanly before the corruption
point** (of ~500k expected); 2 rows quarantined as parse_failure (one
garbled CIF, one truncation marker). The ~390k CIFs behind the corruption
are absent — this is the wave's one documented incomplete coverage, pending
a re-acquisition of the archive.

## TOP4040 note

The 10,000 npz members are **bare float64 arrays (100×40×40) with zero
metadata** — no labels, formulas, or topology classes inside (verified by
reading members). Imported as inventory rows (top:<id>, reference_path,
`payload_note` property). The topological labels the brief expected are not
in this acquisition; flagged for re-survey.

## OQMD note

The 21 GB MySQL dump was extracted via single-pass zcat|awk (~35 min)
into entries / formation_energies INSERT files, then tuple-parsed to JSONL
(1,407,285 + 2,256,458 rows). Skipped tables (documented, ~60): django
auth/session scaffolding, atoms (per-site rows — the compositions table
carries formulas), dos (90,328 rows), calculations*, fits*, publications*
(metadata beyond this wave), structures_species_set. OQMD formulas are
Hill-alphabetical ("F3La1" not "LaF3") — query accordingly. Lessons learned
the hard way: (1) a python char-wise tuple parser dies on multi-GB
extended-INSERT lines — zcat|awk at C speed is the right tool; (2)
importing the parser module re-ran the whole extraction (missing
`__main__` guard — guarded now). nsites column was initially mis-indexed;
the slice was rebuilt (FK cascade on properties works as designed).

## Idempotency

Fixtures: re-run inserts 0 for cod/summary sources (external_key PK +
property UNIQUE(collection, kind, value, text_value)). At scale, the first
full re-run exposed a real bug: **SQLite UNIQUE treats NULLs as distinct**,
so the table-level UNIQUE(external_key, collection, kind, value,
text_value) never deduped rows with NULL value or text_value — 10,351,191
duplicate property rows across two runs. Fixed by full dedupe + a
COALESCE expression index (`property_dedupe`, migration V2); the third
full re-run then inserted **0 rows across all 3,593,184 source rows**
(properties stable at 7,935,056). Note the semantics this creates:
property uniqueness is (entry, collection, kind, value, text) — two
distinct calculations with identical values collapse to one row; task ids
are not tracked this wave.

## Teeth evidence (red→green)

- (a) property unit CHECK: neutered → `INSERT … unit='furlong'` returned
  Ok(1); restored → ConstraintViolation. `other:K` escape hatch works.
- (b) idempotency: INSERT OR IGNORE removed → re-run failed
  `UNIQUE constraint failed: material_entry.source, material_entry.source_id`;
  restored → 0 inserted.
- (c) unknown-param 400: same pattern teeth, live-verified
  (`{"error":{"code":"invalid_query",...}}`).
- (d) garbled CIF: quarantine guard neutered → the bad row became a
  fabricated entry (2 vs 1); restored → lands in material_quarantine with
  reason+detail, zero entry rows.
- Live-verify catches: band_gap JOIN duplicated entries (same property kind
  across collections) — fixed with SELECT DISTINCT and re-verified live.

## Timings (this machine)

MP summary 222 s; robocrys 43 s; thermo 309 s; electronic-structure 88 s;
magnetism 67 s; elasticity 8 s; dielectric/piezo ~3 s; electrodes 86 s;
molecules ~4 min; COD JSONL import ~2 min (parse: 18 GB stream ~12 min
before corruption point); TOP4040 ~1 s; FTS rebuild 18 s. Total MP+COD+TOP
wave ≈ 19 min.

## Cross-source dedup note (deliberately deferred)

The same physical material exists across sources (mp-*, cod:*, oqmd:*)
under different identities. This wave keeps sources distinct and queryable
(external_key = <source>:<id>). Cross-source identity should key on
(formula normalized, spacegroup_number) with a match-confidence — MP
formula_pretty vs COD _chemical_formula_sum need normalization rules
(ordering, unicode). Defer until a material-identity pass; note that
mp-mol and mp-bat namespaces are already separate from mp: by design.
