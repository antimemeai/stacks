# Extraction verification — sciencemadness

55 recipes: **37 pass, 18 warn, 0 fail**; 421 coverage warnings.

Matching rules: located text is the printed locator pages widened by one page each side (printed pages straddle PDF pages). Numbers match after: unicode minus → `-`; all whitespace removed (covers OCR `1 000`); degree signs stripped; lowercasing; a decimal-comma variant is tried in parallel (formula-adjacent commas like `NaBF4,50` are preserved in the primary variant). Spelled-out numerals one–twelve match against a whitespace-collapsed word variant. All matches require token boundaries (no `0` inside `101`, no `one` inside `done`). Series time coordinates are extraction-side structure and are not asserted literally; prose durations are asserted via `duration`. Formulas compare with subscript digits folded to ASCII and hydrate dots unified.

Interpretation: **FAIL** = a structured value absent from the source (fabrication fence; exit code 1). **WARN** = formula not literally in the page text — almost always OCR subscript mangling repaired during extraction; queued for human review, not auto-fail. **coverage** = prose quantity on the located pages matched by no structured field; dominated by *neighbouring* recipes that share the widened pages, so it is a recall signal, not an error count.

| recipe | locator | verdict | fails | warns | coverage |
|---|---|---|---|---|---|
| sciencemadness:brauer:bf3-m1 | pp. 219-220 | pass | 0 | 0 | 14 |
| sciencemadness:brauer:bf3-m2 | p. 220 | pass | 0 | 0 | 14 |
| sciencemadness:brauer:bf3-m3 | pp. 220-221 | WARN | 0 | 1 | 19 |
| sciencemadness:brauer:hbf4 | pp. 221-222 | pass | 0 | 0 | 20 |
| sciencemadness:brauer:nabf4 | p. 222 | pass | 0 | 0 | 9 |
| sciencemadness:brauer:kbf4 | p. 223 | pass | 0 | 0 | 8 |
| sciencemadness:brauer:kbf3oh | pp. 223-224 | pass | 0 | 0 | 10 |
| sciencemadness:brauer:nobf4 | pp. 224-225 | pass | 0 | 0 | 10 |
| sciencemadness:brauer:alf3 | p. 225 | WARN | 0 | 2 | 7 |
| sciencemadness:brauer:alf3-3h2o | pp. 225-226 | WARN | 0 | 1 | 7 |
| sciencemadness:brauer:nh4-3alf6 | p. 226 | WARN | 0 | 2 | 6 |
| sciencemadness:brauer:nh4alf4 | p. 227 | WARN | 0 | 2 | 4 |
| sciencemadness:brauer:gaf3 | pp. 227-228 | WARN | 0 | 1 | 6 |
| sciencemadness:brauer:nh4-3gaf6 | p. 228 | WARN | 0 | 1 | 4 |
| sciencemadness:brauer:inf3-m2 | p. 229 | pass | 0 | 0 | 2 |
| sciencemadness:brauer:tlf | p. 230 | WARN | 0 | 2 | 3 |
| sciencemadness:brauer:tlf3 | pp. 230-231 | WARN | 0 | 2 | 3 |
| sciencemadness:brauer:bef2 | pp. 231-232 | pass | 0 | 0 | 6 |
| sciencemadness:brauer:nh4-2bef4 | p. 232 | pass | 0 | 0 | 5 |
| sciencemadness:brauer:mgf2 | pp. 232-233 | pass | 0 | 0 | 5 |
| sciencemadness:brauer:caf2 | pp. 233-234 | pass | 0 | 0 | 3 |
| sciencemadness:brauer:srf2 | p. 234 | pass | 0 | 0 | 4 |
| sciencemadness:brauer:baf2 | p. 234 | WARN | 0 | 1 | 5 |
| sciencemadness:brauer:lif | p. 235 | pass | 0 | 0 | 6 |
| sciencemadness:brauer:naf | pp. 235-236 | pass | 0 | 0 | 9 |
| sciencemadness:brauer:kf-m1 | p. 236 | pass | 0 | 0 | 7 |
| sciencemadness:brauer:khf2 | p. 237 | pass | 0 | 0 | 9 |
| sciencemadness:brauer:cuf2-m2 | p. 239 | pass | 0 | 0 | 9 |
| sciencemadness:brauer:agf2-m1 | pp. 241-242 | pass | 0 | 0 | 10 |
| sciencemadness:brauer:scf3 | pp. 245-246 | pass | 0 | 0 | 6 |
| sciencemadness:brauer:yf3 | p. 246 | pass | 0 | 0 | 5 |
| sciencemadness:brauer:cef3 | p. 247 | pass | 0 | 0 | 5 |
| sciencemadness:brauer:cef4 | pp. 247-248 | pass | 0 | 0 | 6 |
| sciencemadness:brauer:euf2 | p. 248 | pass | 0 | 0 | 4 |
| sciencemadness:brauer:tif4 | pp. 250-251 | pass | 0 | 0 | 11 |
| sciencemadness:brauer:vf3 | p. 252 | WARN | 0 | 1 | 9 |
| sciencemadness:brauer:vf4 | pp. 252-253 | WARN | 0 | 1 | 11 |
| sciencemadness:brauer:vf5 | p. 253 | pass | 0 | 0 | 10 |
| sciencemadness:brauer:nbf5-m1 | p. 254 | pass | 0 | 0 | 7 |
| sciencemadness:brauer:k2taf7 | p. 256 | pass | 0 | 0 | 5 |
| sciencemadness:brauer:crf2 | pp. 256-257 | pass | 0 | 0 | 8 |
| sciencemadness:brauer:crf3 | p. 257 | WARN | 0 | 1 | 11 |
| sciencemadness:brauer:crf4 | p. 258 | WARN | 0 | 1 | 8 |
| sciencemadness:brauer:uf4 | pp. 261-262 | WARN | 0 | 1 | 4 |
| sciencemadness:brauer:mnf3 | p. 263 | pass | 0 | 0 | 4 |
| sciencemadness:brauer:cof3-m1 | p. 268 | pass | 0 | 0 | 6 |
| sciencemadness:brauer:nif2 | p. 269 | WARN | 0 | 1 | 9 |
| sciencemadness:brauer:znf2 | pp. 242-243 | WARN | 0 | 1 | 14 |
| sciencemadness:brauer:hfg2-m2 | p. 245 | pass | 0 | 0 | 5 |
| sciencemadness:brauer:mof6 | pp. 259-260 | pass | 0 | 0 | 9 |
| sciencemadness:brauer:kif6 | p. 238 | pass | 0 | 0 | 8 |
| sciencemadness:brauer:ag2f | pp. 239-240 | pass | 0 | 0 | 9 |
| sciencemadness:brauer:hg2f2 | p. 244 | pass | 0 | 0 | 6 |
| sciencemadness:brauer:tif3 | pp. 248-250 | WARN | 0 | 1 | 7 |
| sciencemadness:brauer:zrf4 | p. 251 | pass | 0 | 0 | 10 |

## sciencemadness:brauer:bf3-m1 (pp. 219-220)
- coverage: `220°C` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field
- coverage: `500°C` — prose quantity matched no structured field
- coverage: `2 mm` — prose quantity matched no structured field
- coverage: `300 g.` — prose quantity matched no structured field
- coverage: `4,50 g.` — prose quantity matched no structured field
- coverage: `300 ml.` — prose quantity matched no structured field
- coverage: `196°C` — prose quantity matched no structured field
- coverage: `180°C` — prose quantity matched no structured field
- coverage: `135°C` — prose quantity matched no structured field
- coverage: `40 g.` — prose quantity matched no structured field
- coverage: `8 g.` — prose quantity matched no structured field
- coverage: `120 ml.` — prose quantity matched no structured field
- coverage: `270°C` — prose quantity matched no structured field

## sciencemadness:brauer:bf3-m2 (p. 220)
- coverage: `500°C` — prose quantity matched no structured field
- coverage: `2 mm` — prose quantity matched no structured field
- coverage: `80 g.` — prose quantity matched no structured field
- coverage: `30 g.` — prose quantity matched no structured field
- coverage: `600°C` — prose quantity matched no structured field
- coverage: `17 g.` — prose quantity matched no structured field
- coverage: `4,50 g.` — prose quantity matched no structured field
- coverage: `196°C` — prose quantity matched no structured field
- coverage: `180°C` — prose quantity matched no structured field
- coverage: `135°C` — prose quantity matched no structured field
- coverage: `40 g.` — prose quantity matched no structured field
- coverage: `8 g.` — prose quantity matched no structured field
- coverage: `120 ml.` — prose quantity matched no structured field
- coverage: `270°C` — prose quantity matched no structured field

## sciencemadness:brauer:bf3-m3 (pp. 220-221)
- WARN [formula] `HSO3F` — formula not found in page text after normalization (step 3069742); likely OCR-mangled in source — human review
- coverage: `300°C` — prose quantity matched no structured field
- coverage: `500°C` — prose quantity matched no structured field
- coverage: `2 mm` — prose quantity matched no structured field
- coverage: `80 g.` — prose quantity matched no structured field
- coverage: `30 g.` — prose quantity matched no structured field
- coverage: `600°C` — prose quantity matched no structured field
- coverage: `17 g.` — prose quantity matched no structured field
- coverage: `300 g.` — prose quantity matched no structured field
- coverage: `4,50 g.` — prose quantity matched no structured field
- coverage: `300 ml.` — prose quantity matched no structured field
- coverage: `196°C` — prose quantity matched no structured field
- coverage: `180°C` — prose quantity matched no structured field
- coverage: `40 g.` — prose quantity matched no structured field
- coverage: `8 g.` — prose quantity matched no structured field
- coverage: `120 ml.` — prose quantity matched no structured field
- coverage: `270°C` — prose quantity matched no structured field
- coverage: `6.2 g.` — prose quantity matched no structured field
- coverage: `25 g.` — prose quantity matched no structured field
- coverage: `5.3 g` — prose quantity matched no structured field

## sciencemadness:brauer:hbf4 (pp. 221-222)
- coverage: `17 g.` — prose quantity matched no structured field
- coverage: `300 g.` — prose quantity matched no structured field
- coverage: `4,50 g.` — prose quantity matched no structured field
- coverage: `300 ml.` — prose quantity matched no structured field
- coverage: `196°C` — prose quantity matched no structured field
- coverage: `180°C` — prose quantity matched no structured field
- coverage: `135°C` — prose quantity matched no structured field
- coverage: `40 g.` — prose quantity matched no structured field
- coverage: `8 g.` — prose quantity matched no structured field
- coverage: `120 ml.` — prose quantity matched no structured field
- coverage: `270°C` — prose quantity matched no structured field
- coverage: `6.2 g.` — prose quantity matched no structured field
- coverage: `25 g.` — prose quantity matched no structured field
- coverage: `5.3 g` — prose quantity matched no structured field
- coverage: `6.2 g.` — prose quantity matched no structured field
- coverage: `25 g.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `250 ml.` — prose quantity matched no structured field
- coverage: `40 g.` — prose quantity matched no structured field
- coverage: `120°C` — prose quantity matched no structured field

## sciencemadness:brauer:nabf4 (p. 222)
- coverage: `135°C` — prose quantity matched no structured field
- coverage: `40 g.` — prose quantity matched no structured field
- coverage: `8 g.` — prose quantity matched no structured field
- coverage: `120 ml.` — prose quantity matched no structured field
- coverage: `270°C` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `250 ml.` — prose quantity matched no structured field
- coverage: `40 g.` — prose quantity matched no structured field
- coverage: `120°C` — prose quantity matched no structured field

## sciencemadness:brauer:kbf4 (p. 223)
- coverage: `5.3 g` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `250 ml.` — prose quantity matched no structured field
- coverage: `40 g.` — prose quantity matched no structured field
- coverage: `120°C` — prose quantity matched no structured field
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `0.01 mm` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field

## sciencemadness:brauer:kbf3oh (pp. 223-224)
- coverage: `6.2 g.` — prose quantity matched no structured field
- coverage: `25 g.` — prose quantity matched no structured field
- coverage: `5.3 g` — prose quantity matched no structured field
- coverage: `6.2 g.` — prose quantity matched no structured field
- coverage: `25 g.` — prose quantity matched no structured field
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `0.01 mm` — prose quantity matched no structured field
- coverage: `25°C` — prose quantity matched no structured field
- coverage: `24
hours` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field

## sciencemadness:brauer:nobf4 (pp. 224-225)
- coverage: `6.2 g.` — prose quantity matched no structured field
- coverage: `25 g.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `40 g.` — prose quantity matched no structured field
- coverage: `120°C` — prose quantity matched no structured field
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `25°C` — prose quantity matched no structured field
- coverage: `24
hours` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `105°C` — prose quantity matched no structured field

## sciencemadness:brauer:alf3 (p. 225)
- WARN [formula] `(NH4)3AlF6` — formula not found in page text after normalization (step 3069756); likely OCR-mangled in source — human review
- WARN [formula] `AlF3` — formula not found in page text after normalization (step 3069756); likely OCR-mangled in source — human review
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `0.01 mm` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `25°C` — prose quantity matched no structured field
- coverage: `24
hours` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `105°C` — prose quantity matched no structured field

## sciencemadness:brauer:alf3-3h2o (pp. 225-226)
- WARN [formula] `AlF3·3H2O` — formula not found in page text after normalization (step 3069758); likely OCR-mangled in source — human review
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `0.01 mm` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `105°C` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field
- coverage: `350°C` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field

## sciencemadness:brauer:nh4-3alf6 (p. 226)
- WARN [formula] `Al(OH)3` — formula not found in page text after normalization (step 3069759); likely OCR-mangled in source — human review
- WARN [formula] `(NH4)3AlF6` — formula not found in page text after normalization (step 3069760); likely OCR-mangled in source — human review
- coverage: `25°C` — prose quantity matched no structured field
- coverage: `24
hours` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field
- coverage: `350°C` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field

## sciencemadness:brauer:nh4alf4 (p. 227)
- WARN [formula] `(NH4)3AlF6` — formula not found in page text after normalization (step 3069761); likely OCR-mangled in source — human review
- WARN [formula] `NH4AlF4` — formula not found in page text after normalization (step 3069761); likely OCR-mangled in source — human review
- coverage: `105°C` — prose quantity matched no structured field
- coverage: `350°C` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field
- coverage: `6 g.` — prose quantity matched no structured field

## sciencemadness:brauer:gaf3 (pp. 227-228)
- WARN [formula] `(NH4)3GaF6` — formula not found in page text after normalization (step 3069762); likely OCR-mangled in source — human review
- coverage: `105°C` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field
- coverage: `350°C` — prose quantity matched no structured field
- coverage: `6 g.` — prose quantity matched no structured field
- coverage: `500°C` — prose quantity matched no structured field
- coverage: `6 g.` — prose quantity matched no structured field

## sciencemadness:brauer:nh4-3gaf6 (p. 228)
- WARN [formula] `(NH4)3GaF6` — formula not found in page text after normalization (step 3069764); likely OCR-mangled in source — human review
- coverage: `300°C` — prose quantity matched no structured field
- coverage: `350°C` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field
- coverage: `500°C` — prose quantity matched no structured field

## sciencemadness:brauer:inf3-m2 (p. 229)
- coverage: `6 g.` — prose quantity matched no structured field
- coverage: `6 g.` — prose quantity matched no structured field

## sciencemadness:brauer:tlf (p. 230)
- WARN [formula] `Tl2CO3` — formula not found in page text after normalization (step 3069767); likely OCR-mangled in source — human review
- WARN [formula] `TlF` — formula not found in page text after normalization (step 3069768); likely OCR-mangled in source — human review
- coverage: `500°C` — prose quantity matched no structured field
- coverage: `6 g.` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field

## sciencemadness:brauer:tlf3 (pp. 230-231)
- WARN [formula] `Tl2O3` — formula not found in page text after normalization (step 3069769); likely OCR-mangled in source — human review
- WARN [formula] `TlF3` — formula not found in page text after normalization (step 3069769); likely OCR-mangled in source — human review
- coverage: `500°C` — prose quantity matched no structured field
- coverage: `6 g.` — prose quantity matched no structured field
- coverage: `105°C` — prose quantity matched no structured field

## sciencemadness:brauer:bef2 (pp. 231-232)
- coverage: `300°C` — prose quantity matched no structured field
- coverage: `105°C` — prose quantity matched no structured field
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field

## sciencemadness:brauer:nh4-2bef4 (p. 232)
- coverage: `300°C` — prose quantity matched no structured field
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field

## sciencemadness:brauer:mgf2 (pp. 232-233)
- coverage: `300°C` — prose quantity matched no structured field
- coverage: `105°C` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field

## sciencemadness:brauer:caf2 (pp. 233-234)
- coverage: `105°C` — prose quantity matched no structured field
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field

## sciencemadness:brauer:srf2 (p. 234)
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field

## sciencemadness:brauer:baf2 (p. 234)
- WARN [formula] `BaCO3` — formula not found in page text after normalization (step 3069779); likely OCR-mangled in source — human review
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field

## sciencemadness:brauer:lif (p. 235)
- coverage: `1418°C` — prose quantity matched no structured field
- coverage: `2500°C` — prose quantity matched no structured field
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `500°C` — prose quantity matched no structured field
- coverage: `46°C` — prose quantity matched no structured field

## sciencemadness:brauer:naf (pp. 235-236)
- coverage: `1418°C` — prose quantity matched no structured field
- coverage: `2500°C` — prose quantity matched no structured field
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `500°C` — prose quantity matched no structured field
- coverage: `46°C` — prose quantity matched no structured field
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `0.5 g.` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field

## sciencemadness:brauer:kf-m1 (p. 236)
- coverage: `1353°C` — prose quantity matched no structured field
- coverage: `2260°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `46°C` — prose quantity matched no structured field
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `0.5 g.` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field

## sciencemadness:brauer:khf2 (p. 237)
- coverage: `500°C` — prose quantity matched no structured field
- coverage: `46°C` — prose quantity matched no structured field
- coverage: `0.5 g.` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `1 g.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `5 mm` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field

## sciencemadness:brauer:cuf2-m2 (p. 239)
- coverage: `1 g.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `5 mm` — prose quantity matched no structured field
- coverage: `2 g.` — prose quantity matched no structured field
- coverage: `50°C` — prose quantity matched no structured field
- coverage: `20 g.` — prose quantity matched no structured field
- coverage: `48 hours` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field

## sciencemadness:brauer:agf2-m1 (pp. 241-242)
- coverage: `300°C` — prose quantity matched no structured field
- coverage: `0.5 g.` — prose quantity matched no structured field
- coverage: `60 minutes` — prose quantity matched no structured field
- coverage: `60°C` — prose quantity matched no structured field
- coverage: `44
h` — prose quantity matched no structured field
- coverage: `800°C` — prose quantity matched no structured field
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `150 g.` — prose quantity matched no structured field
- coverage: `60 ml.` — prose quantity matched no structured field
- coverage: `450 ml.` — prose quantity matched no structured field

## sciencemadness:brauer:scf3 (pp. 245-246)
- coverage: `50 g.` — prose quantity matched no structured field
- coverage: `3 hours` — prose quantity matched no structured field
- coverage: `245
c` — prose quantity matched no structured field
- coverage: `4.5 hours` — prose quantity matched no structured field
- coverage: `450°C` — prose quantity matched no structured field
- coverage: `500°C` — prose quantity matched no structured field

## sciencemadness:brauer:yf3 (p. 246)
- coverage: `245
c` — prose quantity matched no structured field
- coverage: `4.5 hours` — prose quantity matched no structured field
- coverage: `450°C` — prose quantity matched no structured field
- coverage: `180°C` — prose quantity matched no structured field
- coverage: `500°C` — prose quantity matched no structured field

## sciencemadness:brauer:cef3 (p. 247)
- coverage: `180°C` — prose quantity matched no structured field
- coverage: `500°C` — prose quantity matched no structured field
- coverage: `900°C` — prose quantity matched no structured field
- coverage: `1100°C` — prose quantity matched no structured field
- coverage: `700°C` — prose quantity matched no structured field

## sciencemadness:brauer:cef4 (pp. 247-248)
- coverage: `180°C` — prose quantity matched no structured field
- coverage: `900°C` — prose quantity matched no structured field
- coverage: `1100°C` — prose quantity matched no structured field
- coverage: `700°C` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field

## sciencemadness:brauer:euf2 (p. 248)
- coverage: `500°C` — prose quantity matched no structured field
- coverage: `700°C` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field

## sciencemadness:brauer:tif4 (pp. 250-251)
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `5 g.` — prose quantity matched no structured field
- coverage: `3 mm` — prose quantity matched no structured field
- coverage: `1000°C` — prose quantity matched no structured field
- coverage: `50 g.` — prose quantity matched no structured field
- coverage: `150 g.` — prose quantity matched no structured field
- coverage: `4 g.` — prose quantity matched no structured field
- coverage: `1.5 hours` — prose quantity matched no structured field
- coverage: `100°C` — prose quantity matched no structured field
- coverage: `40 g.` — prose quantity matched no structured field
- coverage: `130 g.` — prose quantity matched no structured field

## sciencemadness:brauer:vf3 (p. 252)
- WARN [formula] `VCl3` — formula not found in page text after normalization (step 3069811); likely OCR-mangled in source — human review
- coverage: `50 g.` — prose quantity matched no structured field
- coverage: `150 g.` — prose quantity matched no structured field
- coverage: `100°C` — prose quantity matched no structured field
- coverage: `40 g.` — prose quantity matched no structured field
- coverage: `130 g.` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `50°C` — prose quantity matched no structured field
- coverage: `650°C` — prose quantity matched no structured field
- coverage: `78°C` — prose quantity matched no structured field

## sciencemadness:brauer:vf4 (pp. 252-253)
- WARN [formula] `VCl4` — formula not found in page text after normalization (step 3069812); likely OCR-mangled in source — human review
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `150 g.` — prose quantity matched no structured field
- coverage: `4 g.` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `1.5 hours` — prose quantity matched no structured field
- coverage: `100°C` — prose quantity matched no structured field
- coverage: `40 g.` — prose quantity matched no structured field
- coverage: `130 g.` — prose quantity matched no structured field
- coverage: `650°C` — prose quantity matched no structured field
- coverage: `78°C` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field

## sciencemadness:brauer:vf5 (p. 253)
- coverage: `4 g.` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `1.5 hours` — prose quantity matched no structured field
- coverage: `100°C` — prose quantity matched no structured field
- coverage: `40 g.` — prose quantity matched no structured field
- coverage: `130 g.` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `50°C` — prose quantity matched no structured field
- coverage: `78°C` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field

## sciencemadness:brauer:nbf5-m1 (p. 254)
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `50°C` — prose quantity matched no structured field
- coverage: `650°C` — prose quantity matched no structured field
- coverage: `78°C` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field
- coverage: `30 g.` — prose quantity matched no structured field
- coverage: `60 g.` — prose quantity matched no structured field

## sciencemadness:brauer:k2taf7 (p. 256)
- coverage: `30 g.` — prose quantity matched no structured field
- coverage: `60 g.` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `600°C` — prose quantity matched no structured field
- coverage: `1200°C` — prose quantity matched no structured field

## sciencemadness:brauer:crf2 (pp. 256-257)
- coverage: `30 g.` — prose quantity matched no structured field
- coverage: `60 g.` — prose quantity matched no structured field
- coverage: `120°C` — prose quantity matched no structured field
- coverage: `600°C` — prose quantity matched no structured field
- coverage: `1200°C` — prose quantity matched no structured field
- coverage: `500°C` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `70 ml.` — prose quantity matched no structured field

## sciencemadness:brauer:crf3 (p. 257)
- WARN [formula] `CrF3` — formula not found in page text after normalization (step 3069822); likely OCR-mangled in source — human review
- coverage: `96.8°C` — prose quantity matched no structured field
- coverage: `229.5°C` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `120°C` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `1200°C` — prose quantity matched no structured field
- coverage: `500°C` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `100°C` — prose quantity matched no structured field
- coverage: `70 ml.` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field

## sciencemadness:brauer:crf4 (p. 258)
- WARN [formula] `CrF5` — formula not found in page text after normalization (step 3069823); likely OCR-mangled in source — human review
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `600°C` — prose quantity matched no structured field
- coverage: `1200°C` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `100°C` — prose quantity matched no structured field
- coverage: `70 ml.` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `15 mm` — prose quantity matched no structured field

## sciencemadness:brauer:uf4 (pp. 261-262)
- WARN [formula] `CCl2F2` — formula not found in page text after normalization (step 3069825); likely OCR-mangled in source — human review
- coverage: `600°C` — prose quantity matched no structured field
- coverage: `110°C` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field

## sciencemadness:brauer:mnf3 (p. 263)
- coverage: `600°C` — prose quantity matched no structured field
- coverage: `110°C` — prose quantity matched no structured field
- coverage: `130 c` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field

## sciencemadness:brauer:cof3-m1 (p. 268)
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `275°C` — prose quantity matched no structured field

## sciencemadness:brauer:nif2 (p. 269)
- WARN [formula] `NiF3` — formula not found in page text after normalization (step 3069828); likely OCR-mangled in source — human review
- coverage: `75°C` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field
- coverage: `275°C` — prose quantity matched no structured field
- coverage: `240°C` — prose quantity matched no structured field
- coverage: `196°C` — prose quantity matched no structured field
- coverage: `78°C` — prose quantity matched no structured field
- coverage: `196°C` — prose quantity matched no structured field
- coverage: `196°C` — prose quantity matched no structured field

## sciencemadness:brauer:znf2 (pp. 242-243)
- WARN [formula] `ZnCO3` — formula not found in page text after normalization (step 3069832); likely OCR-mangled in source — human review
- coverage: `0.5 g.` — prose quantity matched no structured field
- coverage: `60 minutes` — prose quantity matched no structured field
- coverage: `80°C` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `60°C` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `44
h` — prose quantity matched no structured field
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `150 g.` — prose quantity matched no structured field
- coverage: `60 ml.` — prose quantity matched no structured field
- coverage: `450 ml.` — prose quantity matched no structured field
- coverage: `50 g.` — prose quantity matched no structured field
- coverage: `3 hours` — prose quantity matched no structured field
- coverage: `150°C` — prose quantity matched no structured field

## sciencemadness:brauer:hfg2-m2 (p. 245)
- coverage: `50 g.` — prose quantity matched no structured field
- coverage: `3 hours` — prose quantity matched no structured field
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `245
c` — prose quantity matched no structured field
- coverage: `180°C` — prose quantity matched no structured field

## sciencemadness:brauer:mof6 (pp. 259-260)
- coverage: `500°C` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `100°C` — prose quantity matched no structured field
- coverage: `70 ml.` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `15 mm` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field

## sciencemadness:brauer:kif6 (p. 238)
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `0.5 g.` — prose quantity matched no structured field
- coverage: `1 g.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field
- coverage: `50°C` — prose quantity matched no structured field
- coverage: `48 hours` — prose quantity matched no structured field

## sciencemadness:brauer:ag2f (pp. 239-240)
- coverage: `1 g.` — prose quantity matched no structured field
- coverage: `5 mm` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field
- coverage: `0.5 g.` — prose quantity matched no structured field
- coverage: `60 minutes` — prose quantity matched no structured field
- coverage: `80°C` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field

## sciencemadness:brauer:hg2f2 (p. 244)
- coverage: `60 ml.` — prose quantity matched no structured field
- coverage: `450 ml.` — prose quantity matched no structured field
- coverage: `50 g.` — prose quantity matched no structured field
- coverage: `245
c` — prose quantity matched no structured field
- coverage: `4.5 hours` — prose quantity matched no structured field
- coverage: `450°C` — prose quantity matched no structured field

## sciencemadness:brauer:tif3 (pp. 248-250)
- WARN [formula] `TiH2` — formula not found in page text after normalization (step 3069844); likely OCR-mangled in source — human review
- coverage: `500°C` — prose quantity matched no structured field
- coverage: `900°C` — prose quantity matched no structured field
- coverage: `1100°C` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `3 mm` — prose quantity matched no structured field
- coverage: `50 g.` — prose quantity matched no structured field
- coverage: `150 g.` — prose quantity matched no structured field

## sciencemadness:brauer:zrf4 (p. 251)
- coverage: `5 g.` — prose quantity matched no structured field
- coverage: `3 mm` — prose quantity matched no structured field
- coverage: `1000°C` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `4 g.` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `1.5 hours` — prose quantity matched no structured field
- coverage: `100°C` — prose quantity matched no structured field
- coverage: `40 g.` — prose quantity matched no structured field
- coverage: `130 g.` — prose quantity matched no structured field
