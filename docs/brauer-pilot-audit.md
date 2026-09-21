# Brauer pilot extraction — audit

> **Superseded spot-checks (2026-09-21):** the manual 10-recipe spot-check
> table below is retained for narrative value, but verification is now
> deterministic — `stacks-import verify-extraction` over all 55 recipes.
> Verdict: **37 pass / 18 warn / 0 fail** (23 formula WARNs, all OCR-mangle
> repairs queued for review; 421 coverage warnings, dominated by
> page-sharing neighbours). See `brauer-verify-report.md` / `.json`. The
> verifier caught 5 derived-midpoint values (e.g. nominal 17.5 g for a
> "15-20 g" range) which were corrected to range-max nominals.


Date: 2026-09-21. Extractor: `kimi-agent pilot-1` (the agent, reading the OCR
text layer). Source: `/home/patrick/neurotic_library/datasets/sciencemadness/brauer_ocr.pdf`
(sha256 `77ac5c1c…16272f`), Brauer, *Preparative Inorganic Chemistry*.
Batch: **55 procedures** from the contiguous "Fluorine Compounds" chapter
(W. Kwasnik, pp. 219–269) — every well-formed metal/nonmetal fluoride
preparation in that span, each numbered Brauer *Method* modeled as its own
recipe. Loaded into `data/stacks.db` via `stacks-import pilot-load`
(extraction JSONL → stacks-core types → store; all teeth apply at the
boundary). Loaded: 55 recipes, 100 steps, 180 step_materials; re-run is a
clean no-op (0 inserted / 55 skipped).

## Tools used

- PyMuPDF (`neurotic_venv`, read-only) for text extraction — poppler is not
  installed on this machine. 1,906 pages, only 5 without a text layer.
- `corpus/brauer/segment.py` (throwaway Python) for segmentation.
- Loader is Rust (`stacks-import pilot-load`), because that is where the
  teeth live.

## Segmentation stats

647 header candidates book-wide (name line + formula line heuristic),
evenly spread across the book's ~30 chapters — vs roughly 1,000–1,500
numbered methods in Brauer, so the segmenter undercounts but finds every
chapter. Early pages (front matter, apparatus tables) produce false
positives; the procedure chapters are clean. The pilot batch was chosen by
inspecting the candidate list for a *contiguous run of quantitative
sections* (the Kwasnik fluorides chapter: gram quantities, explicit
temperatures, one-line stoichiometric equations). 36 of the 55 records come
from pp. 219–258 alone. Locators were taken from the printed page numbers
in the OCR headers; every recipe's locator was verified against the text
during extraction (segmenter-assigned locators were *not* trusted blindly —
two segment entries were chapter-author headers, not procedures).

## Per-field fill rates (55 recipes / 100 steps / 180 materials)

- target material (formula kind): 55/55; provenance + locator: 55/55
- steps with temperature condition: 44/100 (rest: genuinely temperature-less
  aqueous preps, or vague "red heat" — left NULL, noted in step note)
- steps with duration: 13; with atmosphere: 33; with pressure: 2
- quantified step_materials: 24/180 (only where the prose states a number —
  no quantities invented)
- narrative (key source prose): 55/55

## Confidence distribution

| band | count | notes |
|---|---|---|
| 0.90 | 26 | stoichiometric preps with explicit quantities and temperatures (CaF2, KBF4, AgF2, CoF3, VF5, UF4, …) |
| 0.80–0.88 | 28 | solid but one vague element (missing duration, "excess HF", "red heat") |
| 0.72 | 1 | NaF — drying temperature unreadable in OCR ("0C"), left unrecorded |

Confidence is per-recipe, stored in `provenance.confidence`, method
`llm_extracted`, extractor_version `kimi-agent pilot-1`.

## Spot-check table (10 of 55)

Source prose (abridged) vs structured fields. Verify against the PDF.

**CaF2 (p. 233, conf 0.90)** — "40% HF added with constant agitation to a
suspension of 100 g. of CaCO3 in 100 ml. of boiling water… until evolution
of CO2 almost ceases. Filtered hot… dried at 300C."
→ step1 precipitate [CaCO3 100 g eq, H2O 100 mL eq, HF reactant];
step2 dry 300 °C eq. ✔ exact.

**BF3 Method I (pp. 219-220, conf 0.88)** — "80 g. of dried or, preferably,
melted KBF4 and 30 g. of B2O3 is heated to about 600°C in an inclined iron
tube… The yield is 17 g." → mix [KBF4 80 g, B2O3 30 g]; heat 600 °C approx;
product BF3 17 g. ✔ exact ("about" → approx).

**NaBF4 (p. 222, conf 0.90)** — "Boric acid (6.2 g.) … 25 g. of 40%
hydrofluoric acid… left standing for six hours at room temperature, then
cooled with ice, and 5.3 g of dry Na2CO3 is added… evaporated until
crystallization starts… dried under vacuum." → mix 6 h [H3BO3 6.2 g,
HF 25 g]; precipitate [Na2CO3 5.3 g]; dry (vacuum). ✔ exact.

**AgF2 Method I (pp. 241-242, conf 0.90)** — "External cooling must be
provided at the start so that the temperature does not exceed 80°C… The
temperature is then gradually increased to 250°C… The yield is 95%."
→ heat, temperature min_max 80–250 °C (start ≤80, ramp to 250), atmosphere
other:F2. ✔ faithful; yield in narrative (no run-level yields in a
definition).

**EuF2 (p. 248, conf 0.88)** — "heated rapidly to 900°C in a high-velocity
stream of carefully purified hydrogen and then reduced at 1100°C over a
period of three hours." → heat, temperature **series** [(0 h, 900 °C),
(3 h, 1100 °C)], atmosphere h2, duration 3 h. ✔ exercises the series
variant.

**VF5 (p. 253, conf 0.88)** — "VF4 … gradually heated to 650°C in a stream
of dry N2. The exit gases are collected in a quartz trap at −78°C… The
yield is almost quantitative." → heat ≤650 °C, N2; materials VF4 precursor,
VF5 product, VF3 byproduct (disproportionation, from the equation). ✔
byproduct captured from the printed equation, not the prose.

**UF4 (pp. 261-262, conf 0.88)** — "dry oxygen is passed through the
apparatus for one hour, while the furnace is heated to 400°C. The oxygen
flow is then replaced with CF2Cl2 at a rate of one liter per hour. The
reaction starts as soon as the temperature reaches 400°C." → heat 400 °C /
1 h / O2; heat 400 °C / atmosphere other:CCl2F2. ✔ two-step gas switch
modeled faithfully.

**TlF3 (pp. 230-231, conf 0.88)** — "The reaction begins even at room
temperature… Toward the end of the fluorination the temperature is
increased to 300°C." → heat ≤300 °C, F2. ✔ conservative (lte operator).

**TiF3 (pp. 248-250, conf 0.80)** — three stages: hydrogenation 600–700 °C
(H2); fluorination "1:4 mixture of H2:HF for four to five hours. The HF
flow is first started at a temperature above 200°C"; vacuum sublimation
"four hours at 1000°C" at "10⁻³ to 10⁻⁴ mm". → 3 steps: heat 600–700 h2;
heat duration 4–5 h (range), temperature ≥200, atmosphere other:"H2+HF
(4:1)"; heat 1000 °C, 4 h, vacuum. ⚠ sublimation pressure left NULL — the
OCR's exponent notation (10⁻³–10⁻⁴ mm) did not survive as machine-checkable
numbers; noted in the step note instead. Honest gap.

**NaF (pp. 235-236, conf 0.72)** — "suction-filtered and dried in an oven
at 0°C." An oven at 0 °C is not credible — OCR mangle. → dry step with NO
temperature and a note saying so; confidence lowered. ⚔ example of the
OCR-noise handling policy: drop the field, keep the note, lower confidence.

## What broke

1. **My own first-draft JSONL failed the model boundary 4 times** (wrong
   serde shapes for `other:` roles and `Operation::Other{note}`) — the
   loader rejected them with actionable messages, and they were fixed
   before insert. This is the teeth working as designed; it also means the
   JSON Schema served at `/api/v1/schemas` is the contract future
   extractors must follow, and several shapes (externally-tagged `other`)
   are exactly the kind agents get wrong without reading the schema.
2. **Quantity/Conditions shape drift**: range operators as
   `"operator":"range"`+sibling keys, and spurious `"kind"` on bare
   Quantity objects, were normalized in a fix-up pass before load. The
   schemas now exist precisely to prevent this class of error.
3. **OCR subscripts**: `BSO3`, `F3`, `HgSO4`, `C1F3` — systematically
   3-for-2, g-for-2, 1-for-l. All formulas in the batch were normalized by
   chemical knowledge; none were loaded raw. This is the main per-book risk:
   a different OCR engine will have different systematic errors.
4. **"Red heat", "several hours"** — vague quantities left as
   notes/approximations (approx/range operators or NULL), never fabricated.
5. **HF/F2 atmospheres** are not in the closed `Atmosphere` enum →
   `other:"F2"`, `other:"HF"` with the raw string preserved. If extraction
   scales, `F2` and `HF` earn their place in the enum — the chemistry
   corpus will hit them constantly.

## Recommendation on scaling to the rest of sciencemadness

- **Proceed, per-book.** The segmenter heuristic (name line + formula line)
  is Brauer-shaped; Mellor and the patent PDFs will need their own
  boundary rules. Budget one calibration pass per book — the segmenter's
  job is only to bound candidate spans; the extractor verifies locators
  while reading, so segmenter precision can stay modest.
- The loader, envelope schema, and teeth are book-agnostic and should not
  change. Authoring JSONL by hand (as here) does not scale; next wave
  should still be LLM-authored but chunked mechanically from
  `segments.json`, with the agent extracting per-chunk and the loader
  rejecting on shape — the reject path proved itself in this pilot.
- Expect ~50–100 well-formed recipes/hour of agent extraction at this
  fidelity. Whole-Brauer (~1,000+ methods) is feasible; whole-library is a
  scheduling question, not a feasibility question.
- Add `F2`, `HF`, `Cl2` to `Atmosphere` (or accept `other:` noise); add
  OCR-mangled formula normalization to a shared pre-pass before any second
  book.
