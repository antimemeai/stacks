# Embedding parity gate — evidence

Date: 2026-09-22. Stage D2's hard gate: the Rust query-side embedding
pipeline must reproduce NL's Python pipeline bit-near-exactly before any
semantic retrieval ships.

## Pipelines under test

- **Python (reference)**: `/home/patrick/neurotic_library/.venv/bin/python`
  (frozen env, run read-only), sentence-transformers 3.4.1,
  `sentence-transformers/all-MiniLM-L6-v2` (Transformer → mean pool →
  Normalize; unit-normed 384-dim float32).
- **Rust**: fastembed 4.x (`fastembed-rs`), `EmbeddingModel::AllMiniLML6V2`,
  reading the already-cached ONNX model at
  `neurotic_library/.fastembed_cache/models--Qdrant--all-MiniLM-L6-v2-onnx`
  (model.onnx + tokenizer.json, read-only; `HF_HUB_OFFLINE=1`, zero
  downloads, zero writes into NL). fastembed worked out of the box; the
  `ort`+tokenizer fallback was not needed.
- Utility: `stacks-api/examples/parity_embed.rs` (`embed` / `brute` modes).

## Leg 1: per-vector parity (20 diverse strings)

Strings: chemistry prose, English nonsense, German, Chinese, a transliterated
Russian citation, a 1-char string, a near-empty string.

| metric | worst case | threshold | verdict |
|---|---|---|---|
| max abs diff per component | **2.508e-07** | < 1e-4 | PASS |
| cosine similarity | **1.00000000** (≥ 0.9999999 every string) | > 0.9999 | PASS |

Spot: string 0 ("zeolite catalysis…") max|Δ| = 1.53e-07; norms exactly
1.000000 on both sides.

## Leg 2: retrieval parity (sqlite-vec MATCH vs Rust brute force)

Copy of `embeddings/chemistry_20260611_202759.sqlite` at
`/tmp/parity/chem.sqlite` (copy lives outside NL; NL untouched). 5 queries
embedded with the Python reference; top-10 via NL venv `sqlite_vec`
(`WHERE embedding MATCH ? AND k = 10`) vs `brute_force_topk` over slabs
decoded in Rust (ordinal == chunk id, 1024 vectors/slab, LE f32).

| query | overlap | identical order |
|---|---|---|
| zeolite catalysis | 10/10 | yes |
| solid state synthesis of perovskites | 10/10 | yes |
| solvent extraction of rare earths | 10/10 | yes |
| hydrothermal crystal growth | 10/10 | yes |
| polymer electrolyte membrane degradation | 10/10 | yes |

**RETRIEVAL PARITY: PASS** — 10/10 overlap with identical ordering on all
five queries (no tie ambiguity observed).

Notes from the gate that corrected earlier assumptions:
- `vec_chunks` vec0 virtual table exposes only `embedding`; its rowid IS the
  chunk id (confirms the D1 slab-mapping conclusion).
- sqlite-vec MATCH on unit vectors: L2 ordering == cosine ordering
  (monotonic), consistent with Rust dot-product ranking.

## Service characteristics (library.db, 357,527 chunks, this machine)

- All-corpora dense, cold (loads + caches 549 MB of vectors): **3.47 s**
- All-corpora dense, warm: **~0.20 s**
- All-corpora hybrid (dense + FTS5 BM25 + RRF k=60), warm: **~0.19 s**
- Single-corpus (chemistry, 98.5k chunks) warm: **~4 ms**
- Embedding a query: ~10–20 ms (ONNX CPU)
- Vector cache: lazy per-corpus load, LRU cap 2 GB
  (`semantic::VectorCache`); no extension loading anywhere.

Live smell test (`zeolite catalysis`, k=5, all corpora): top-4 all from
`SolventFreeZeolites2021.pdf` (inorganic_chemistry), #5 an MXene catalysis
paper (materials_chemistry). Hybrid leg pulls in Sol-Gel Science and
Corma solid-acids chunks — RRF fusion behaving as expected.

## Teeth (red→green)

- (a) `query` missing/empty: validation neutered → endpoint returned 200
  with a full data page; restored → 400 `invalid_query`. Also k>50 → 400
  with `details.max=50`; unknown params rejected.
- (b) retrieval correctness fixture (axis-aligned unit vectors): query on
  axis 1 must return chunk 2. With chunk 2's embedding bytes zeroed, the
  positive assertion `top1 == 2` failed (winner became chunk 3) — the test
  provably detects wrong retrieval. Restored → green. RRF fused scores
  checked against the reference formula (1/(60+rank), 1-based) to 1e-12.
