# Extraction verification — asm-hb-v18

4 recipes: **4 pass, 0 warn, 0 fail**; 26 coverage warnings.

Matching rules: located text is the printed locator pages widened by one page each side (printed pages straddle PDF pages). Numbers match after: unicode minus → `-`; all whitespace removed (covers OCR `1 000`); degree signs stripped; lowercasing; a decimal-comma variant is tried in parallel (formula-adjacent commas like `NaBF4,50` are preserved in the primary variant). Spelled-out numerals one–twelve match against a whitespace-collapsed word variant. All matches require token boundaries (no `0` inside `101`, no `one` inside `done`). Series time coordinates are extraction-side structure and are not asserted literally; prose durations are asserted via `duration`. Formulas compare with subscript digits folded to ASCII and hydrate dots unified.

Interpretation: **FAIL** = a structured value absent from the source (fabrication fence; exit code 1). **WARN** = formula not literally in the page text — almost always OCR subscript mangling repaired during extraction; queued for human review, not auto-fail. **coverage** = prose quantity on the located pages matched by no structured field; dominated by *neighbouring* recipes that share the widened pages, so it is a recall signal, not an error count.

| recipe | locator | verdict | fails | warns | coverage |
|---|---|---|---|---|---|
| asm-hb-v18:asm-hb-v18:g65-tin | p. 910 | pass | 0 | 0 | 9 |
| asm-hb-v18:asm-hb-v18:carbon-carbon-brakes | p. 714 | pass | 0 | 0 | 3 |
| asm-hb-v18:asm-hb-v18:cemented-carbide-sintering | p. 976 | pass | 0 | 0 | 7 |
| asm-hb-v18:asm-hb-v18:tin-coating-wear | p. 976 | pass | 0 | 0 | 7 |

## asm-hb-v18:asm-hb-v18:g65-tin (p. 910)
- coverage: `566 °C` — prose quantity matched no structured field
- coverage: `140 °C` — prose quantity matched no structured field
- coverage: `20 min.` — prose quantity matched no structured field
- coverage: `600 °C` — prose quantity matched no structured field
- coverage: `6 h` — prose quantity matched no structured field
- coverage: `900 °C` — prose quantity matched no structured field
- coverage: `800 °C` — prose quantity matched no structured field
- coverage: `550 °C` — prose quantity matched no structured field
- coverage: `150 mm` — prose quantity matched no structured field

## asm-hb-v18:asm-hb-v18:carbon-carbon-brakes (p. 714)
- coverage: `6385C` — prose quantity matched no structured field
- coverage: `6302C` — prose quantity matched no structured field
- coverage: `1.85 g` — prose quantity matched no structured field

## asm-hb-v18:asm-hb-v18:cemented-carbide-sintering (p. 976)
- coverage: `1500 °C` — prose quantity matched no structured field
- coverage: `415 °C` — prose quantity matched no structured field
- coverage: `1000 °C` — prose quantity matched no structured field
- coverage: `83.0 HR` — prose quantity matched no structured field
- coverage: `93.0 HR` — prose quantity matched no structured field
- coverage: `14 g` — prose quantity matched no structured field
- coverage: `19 mm` — prose quantity matched no structured field

## asm-hb-v18:asm-hb-v18:tin-coating-wear (p. 976)
- coverage: `1500 °C` — prose quantity matched no structured field
- coverage: `415 °C` — prose quantity matched no structured field
- coverage: `1600 °C` — prose quantity matched no structured field
- coverage: `83.0 HR` — prose quantity matched no structured field
- coverage: `93.0 HR` — prose quantity matched no structured field
- coverage: `14 g` — prose quantity matched no structured field
- coverage: `19 mm` — prose quantity matched no structured field
