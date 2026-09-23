# Extraction verification — sciencemadness

145 recipes: **71 pass, 74 warn, 0 fail**; 1656 coverage warnings.

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
| sciencemadness:brauer:uf4 | pp. 261-262 | WARN | 0 | 1 | 7 |
| sciencemadness:brauer:mnf3 | p. 263 | pass | 0 | 0 | 4 |
| sciencemadness:brauer:cof3-m1 | p. 268 | pass | 0 | 0 | 6 |
| sciencemadness:brauer:nif2 | p. 269 | WARN | 0 | 1 | 9 |
| sciencemadness:brauer:znf2 | pp. 242-243 | WARN | 0 | 1 | 14 |
| sciencemadness:brauer:hfg2-m2 | p. 245 | pass | 0 | 0 | 5 |
| sciencemadness:brauer:mof6 | pp. 259-260 | WARN | 0 | 1 | 12 |
| sciencemadness:brauer:kif6 | p. 238 | pass | 0 | 0 | 8 |
| sciencemadness:brauer:ag2f | pp. 239-240 | pass | 0 | 0 | 9 |
| sciencemadness:brauer:hg2f2 | p. 244 | pass | 0 | 0 | 6 |
| sciencemadness:brauer:tif3 | pp. 248-250 | WARN | 0 | 1 | 7 |
| sciencemadness:brauer:zrf4 | p. 251 | pass | 0 | 0 | 10 |
| sciencemadness:brauer:hcl-m1 | pp. 280-281 | WARN | 0 | 2 | 6 |
| sciencemadness:brauer:hi-m2 | p. 287 | WARN | 0 | 1 | 3 |
| sciencemadness:brauer:ki | p. 290 | pass | 0 | 0 | 13 |
| sciencemadness:brauer:ki3-h2o | p. 294 | pass | 0 | 0 | 10 |
| sciencemadness:brauer:csbrcl2-m1 | pp. 294-295 | pass | 0 | 0 | 11 |
| sciencemadness:brauer:kicl2-m2 | pp. 295-296 | WARN | 0 | 1 | 15 |
| sciencemadness:brauer:csicl2 | p. 296 | WARN | 0 | 1 | 7 |
| sciencemadness:brauer:kibr2 | pp. 296-297 | pass | 0 | 0 | 10 |
| sciencemadness:brauer:csibr2-m1 | p. 297 | pass | 0 | 0 | 8 |
| sciencemadness:brauer:csibr2-m2 | p. 297 | pass | 0 | 0 | 6 |
| sciencemadness:brauer:hicl4-4h2o | p. 299 | WARN | 0 | 3 | 11 |
| sciencemadness:brauer:cl2o-m2 | p. 300 | WARN | 0 | 2 | 18 |
| sciencemadness:brauer:hclo-m2 | p. 309 | WARN | 0 | 2 | 27 |
| sciencemadness:brauer:nh4clo3-m2 | p. 314 | WARN | 0 | 2 | 13 |
| sciencemadness:brauer:nh4clo3-m3 | p. 314 | WARN | 0 | 1 | 13 |
| sciencemadness:brauer:hio3-m3 | p. 318 | WARN | 0 | 1 | 16 |
| sciencemadness:brauer:hclo4-m2 | p. 319 | WARN | 0 | 2 | 9 |
| sciencemadness:brauer:perchlorato-anhydrous | p. 320 | WARN | 0 | 2 | 15 |
| sciencemadness:brauer:perchlorato-anhydrous-m2 | p. 320 | WARN | 0 | 2 | 15 |
| sciencemadness:brauer:no2clo4-m2 | p. 321 | WARN | 0 | 2 | 17 |
| sciencemadness:brauer:kio4 | p. 325 | WARN | 0 | 3 | 36 |
| sciencemadness:brauer:cl2-m2 | pp. 272-273 | WARN | 0 | 1 | 6 |
| sciencemadness:brauer:br2-m1 | p. 275 | pass | 0 | 0 | 9 |
| sciencemadness:brauer:br2-m2 | pp. 275-276 | WARN | 0 | 3 | 9 |
| sciencemadness:brauer:br2-8h2o | p. 276 | pass | 0 | 0 | 3 |
| sciencemadness:brauer:hcl-m2 | p. 281 | WARN | 0 | 3 | 9 |
| sciencemadness:brauer:hi-m1 | pp. 286-287 | WARN | 0 | 1 | 10 |
| sciencemadness:brauer:icl | pp. 290-291 | pass | 0 | 0 | 14 |
| sciencemadness:brauer:cl2o6 | pp. 303-304 | WARN | 0 | 1 | 26 |
| sciencemadness:brauer:cl2o7-m3 | pp. 305-306 | WARN | 0 | 3 | 25 |
| sciencemadness:brauer:hclo-m1 | pp. 308-309 | WARN | 0 | 1 | 33 |
| sciencemadness:brauer:naclo2-3h2o | p. 312 | pass | 0 | 0 | 11 |
| sciencemadness:brauer:hclo3 | pp. 312-313 | WARN | 0 | 1 | 14 |
| sciencemadness:brauer:hbro3 | pp. 315-316 | pass | 0 | 0 | 19 |
| sciencemadness:brauer:hio3-m2 | pp. 316-317 | pass | 0 | 0 | 20 |
| sciencemadness:brauer:hclo4-m1 | pp. 318-319 | WARN | 0 | 2 | 15 |
| sciencemadness:brauer:perchlorato-hydrous | p. 320 | WARN | 0 | 2 | 19 |
| sciencemadness:brauer:o2-m1 | pp. 333-334 | pass | 0 | 0 | 7 |
| sciencemadness:brauer:h2s2-distill | p. 351 | pass | 0 | 0 | 10 |
| sciencemadness:brauer:na2s2-m2 | p. 362 | pass | 0 | 0 | 6 |
| sciencemadness:brauer:nh4-2s5 | p. 370 | pass | 0 | 0 | 12 |
| sciencemadness:brauer:scl2-m1 | pp. 370-371 | WARN | 0 | 2 | 10 |
| sciencemadness:brauer:cl2-m4 | p. 273 | pass | 0 | 0 | 5 |
| sciencemadness:brauer:cl2o-m1 | pp. 299-300 | WARN | 0 | 1 | 17 |
| sciencemadness:brauer:clo2-m1 | p. 301 | WARN | 0 | 2 | 13 |
| sciencemadness:brauer:clo2-m2 | pp. 301-302 | WARN | 0 | 1 | 11 |
| sciencemadness:brauer:naclo-5h2o | p. 309 | WARN | 0 | 1 | 24 |
| sciencemadness:brauer:baclo3-h2o | p. 315 | pass | 0 | 0 | 11 |
| sciencemadness:brauer:noclo4 | pp. 320-321 | WARN | 0 | 2 | 24 |
| sciencemadness:brauer:ba3h4io6-m1 | pp. 325-326 | WARN | 0 | 1 | 42 |
| sciencemadness:brauer:clno3 | pp. 326-327 | WARN | 0 | 3 | 29 |
| sciencemadness:brauer:i2o4 | p. 333 | WARN | 0 | 1 | 5 |
| sciencemadness:brauer:h2s-m1 | p. 344 | WARN | 0 | 2 | 15 |
| sciencemadness:brauer:h2s-m2 | pp. 344-346 | pass | 0 | 0 | 27 |
| sciencemadness:brauer:h2s-m3 | p. 346 | WARN | 0 | 2 | 18 |
| sciencemadness:brauer:h2s2-h2s3-crack | pp. 350-351 | pass | 0 | 0 | 9 |
| sciencemadness:brauer:h2s4 | pp. 354-355 | WARN | 0 | 2 | 18 |
| sciencemadness:brauer:nh4hs | p. 357 | WARN | 0 | 1 | 14 |
| sciencemadness:brauer:nahs | pp. 357-358 | WARN | 0 | 2 | 15 |
| sciencemadness:brauer:na2s-m1 | pp. 358-359 | pass | 0 | 0 | 11 |
| sciencemadness:brauer:k2s | pp. 360-361 | pass | 0 | 0 | 10 |
| sciencemadness:brauer:na2s2-m1 | pp. 361-362 | pass | 0 | 0 | 10 |
| sciencemadness:brauer:k2s4-m1 | pp. 362-366 | pass | 0 | 0 | 19 |
| sciencemadness:brauer:socl2-m1 | pp. 382-383 | WARN | 0 | 2 | 20 |
| sciencemadness:brauer:socl2-m2 | p. 383 | WARN | 0 | 3 | 7 |
| sciencemadness:brauer:sobr2-m3 | p. 388 | WARN | 0 | 1 | 16 |
| sciencemadness:brauer:h2so5 | pp. 388-389 | WARN | 0 | 1 | 20 |
| sciencemadness:brauer:k2s2o8 | p. 392 | pass | 0 | 0 | 19 |
| sciencemadness:brauer:na2s2o4-2h2o | p. 394 | WARN | 0 | 1 | 18 |
| sciencemadness:brauer:k2s3o6-m1 | p. 398 | WARN | 0 | 2 | 29 |
| sciencemadness:brauer:nohso4 | p. 406 | pass | 0 | 0 | 14 |
| sciencemadness:brauer:s4n4 | pp. 406-407 | WARN | 0 | 4 | 12 |
| sciencemadness:brauer:agf-m1 | pp. 240-241 | pass | 0 | 0 | 14 |
| sciencemadness:brauer:agf-pure | p. 240 | pass | 0 | 0 | 9 |
| sciencemadness:brauer:k2nbf7 | p. 255 | pass | 0 | 0 | 7 |
| sciencemadness:brauer:mnf2 | pp. 262-263 | pass | 0 | 0 | 11 |
| sciencemadness:brauer:k2mnf6 | p. 264 | WARN | 0 | 2 | 5 |
| sciencemadness:brauer:fef2 | p. 266 | WARN | 0 | 1 | 4 |
| sciencemadness:brauer:fef3 | pp. 266-267 | WARN | 0 | 1 | 7 |
| sciencemadness:brauer:cof2 | p. 267 | pass | 0 | 0 | 3 |
| sciencemadness:brauer:cof3-m2 | p. 268 | pass | 0 | 0 | 5 |
| sciencemadness:brauer:k2nif6 | p. 269 | WARN | 0 | 1 | 9 |
| sciencemadness:brauer:taf5 | pp. 255-256 | pass | 0 | 0 | 8 |
| sciencemadness:brauer:ref6 | pp. 264-265 | pass | 0 | 0 | 6 |
| sciencemadness:brauer:kbrf4 | pp. 237-238 | WARN | 0 | 1 | 11 |
| sciencemadness:brauer:uf6 | p. 262 | pass | 0 | 0 | 9 |
| sciencemadness:brauer:wf6 | p. 260 | WARN | 0 | 1 | 7 |
| sciencemadness:brauer:h2sx-crude | pp. 346-347 | WARN | 0 | 4 | 11 |
| sciencemadness:brauer:na2s4-m1 | pp. 365-366 | WARN | 0 | 1 | 6 |
| sciencemadness:brauer:cl2o7-m1 | pp. 304-305 | WARN | 0 | 2 | 29 |

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
- coverage: `2.3°C` — prose quantity matched no structured field
- coverage: `17.5°C` — prose quantity matched no structured field
- coverage: `15°C` — prose quantity matched no structured field
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
- WARN [formula] `MoOF4` — formula not found in page text after normalization (step 3069835); likely OCR-mangled in source — human review
- coverage: `500°C` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `100°C` — prose quantity matched no structured field
- coverage: `70 ml.` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `15 mm` — prose quantity matched no structured field
- coverage: `2.3°C` — prose quantity matched no structured field
- coverage: `17.5°C` — prose quantity matched no structured field
- coverage: `15°C` — prose quantity matched no structured field
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

## sciencemadness:brauer:hcl-m1 (pp. 280-281)
- WARN [formula] `HCl` — formula not found in page text after normalization (step 4521858); likely OCR-mangled in source — human review
- WARN [formula] `HCl` — formula not found in page text after normalization (step 4521859); likely OCR-mangled in source — human review
- coverage: `15 mm` — prose quantity matched no structured field
- coverage: `5 mm` — prose quantity matched no structured field
- coverage: `207°C` — prose quantity matched no structured field
- coverage: `15°C` — prose quantity matched no structured field
- coverage: `0.3 mm` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field

## sciencemadness:brauer:hi-m2 (p. 287)
- WARN [formula] `P2O5` — formula not found in page text after normalization (step 4521860); likely OCR-mangled in source — human review
- coverage: `78°C` — prose quantity matched no structured field
- coverage: `3 g.` — prose quantity matched no structured field
- coverage: `7 ml.` — prose quantity matched no structured field

## sciencemadness:brauer:ki (p. 290)
- coverage: `127°C` — prose quantity matched no structured field
- coverage: `760 mm` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `280 ml.` — prose quantity matched no structured field
- coverage: `600 ml.` — prose quantity matched no structured field
- coverage: `680°C` — prose quantity matched no structured field
- coverage: `300 ml.` — prose quantity matched no structured field
- coverage: `300 ml.` — prose quantity matched no structured field
- coverage: `468 g.` — prose quantity matched no structured field
- coverage: `1674 g.` — prose quantity matched no structured field
- coverage: `837 g.` — prose quantity matched no structured field
- coverage: `24 hours` — prose quantity matched no structured field
- coverage: `1070 g.` — prose quantity matched no structured field

## sciencemadness:brauer:ki3-h2o (p. 294)
- coverage: `500 g.` — prose quantity matched no structured field
- coverage: `250 g.` — prose quantity matched no structured field
- coverage: `250 ml.` — prose quantity matched no structured field
- coverage: `950 ml.` — prose quantity matched no structured field
- coverage: `45 minutes` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `16.9 g.` — prose quantity matched no structured field
- coverage: `85 ml.` — prose quantity matched no structured field
- coverage: `8 g.` — prose quantity matched no structured field
- coverage: `45 ml.` — prose quantity matched no structured field

## sciencemadness:brauer:csbrcl2-m1 (pp. 294-295)
- coverage: `500 g.` — prose quantity matched no structured field
- coverage: `250 g.` — prose quantity matched no structured field
- coverage: `250 ml.` — prose quantity matched no structured field
- coverage: `950 ml.` — prose quantity matched no structured field
- coverage: `45 minutes` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `45 ml.` — prose quantity matched no structured field
- coverage: `16.8 g.` — prose quantity matched no structured field
- coverage: `170 ml.` — prose quantity matched no structured field
- coverage: `2.7 g.` — prose quantity matched no structured field

## sciencemadness:brauer:kicl2-m2 (pp. 295-296)
- WARN [formula] `KICl2` — formula not found in page text after normalization (step 4521868); likely OCR-mangled in source — human review
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `16.9 g.` — prose quantity matched no structured field
- coverage: `85 ml.` — prose quantity matched no structured field
- coverage: `8 g.` — prose quantity matched no structured field
- coverage: `45 ml.` — prose quantity matched no structured field
- coverage: `16.8 g.` — prose quantity matched no structured field
- coverage: `170 ml.` — prose quantity matched no structured field
- coverage: `2.7 g.` — prose quantity matched no structured field
- coverage: `2
h` — prose quantity matched no structured field
- coverage: `26 g.` — prose quantity matched no structured field
- coverage: `17 g.` — prose quantity matched no structured field
- coverage: `21.3 g.` — prose quantity matched no structured field
- coverage: `213 ml.` — prose quantity matched no structured field
- coverage: `12.7 g.` — prose quantity matched no structured field
- coverage: `8 g.` — prose quantity matched no structured field

## sciencemadness:brauer:csicl2 (p. 296)
- WARN [formula] `H2O` — formula not found in page text after normalization (step 4521869); likely OCR-mangled in source — human review
- coverage: `2
h` — prose quantity matched no structured field
- coverage: `26 g.` — prose quantity matched no structured field
- coverage: `17 g.` — prose quantity matched no structured field
- coverage: `21.3 g.` — prose quantity matched no structured field
- coverage: `213 ml.` — prose quantity matched no structured field
- coverage: `12.7 g.` — prose quantity matched no structured field
- coverage: `8 g.` — prose quantity matched no structured field

## sciencemadness:brauer:kibr2 (pp. 296-297)
- coverage: `16.8 g.` — prose quantity matched no structured field
- coverage: `170 ml.` — prose quantity matched no structured field
- coverage: `2.7 g.` — prose quantity matched no structured field
- coverage: `2
h` — prose quantity matched no structured field
- coverage: `26 g.` — prose quantity matched no structured field
- coverage: `17 g.` — prose quantity matched no structured field
- coverage: `21.3 g.` — prose quantity matched no structured field
- coverage: `213 ml.` — prose quantity matched no structured field
- coverage: `12.7 g.` — prose quantity matched no structured field
- coverage: `8 g.` — prose quantity matched no structured field

## sciencemadness:brauer:csibr2-m1 (p. 297)
- coverage: `16.8 g.` — prose quantity matched no structured field
- coverage: `170 ml.` — prose quantity matched no structured field
- coverage: `2.7 g.` — prose quantity matched no structured field
- coverage: `2
h` — prose quantity matched no structured field
- coverage: `21.3 g.` — prose quantity matched no structured field
- coverage: `213 ml.` — prose quantity matched no structured field
- coverage: `12.7 g.` — prose quantity matched no structured field
- coverage: `8 g.` — prose quantity matched no structured field

## sciencemadness:brauer:csibr2-m2 (p. 297)
- coverage: `16.8 g.` — prose quantity matched no structured field
- coverage: `170 ml.` — prose quantity matched no structured field
- coverage: `2.7 g.` — prose quantity matched no structured field
- coverage: `2
h` — prose quantity matched no structured field
- coverage: `26 g.` — prose quantity matched no structured field
- coverage: `17 g.` — prose quantity matched no structured field

## sciencemadness:brauer:hicl4-4h2o (p. 299)
- WARN [formula] `ICl3` — formula not found in page text after normalization (step 4521876); likely OCR-mangled in source — human review
- WARN [formula] `HCl` — formula not found in page text after normalization (step 4521876); likely OCR-mangled in source — human review
- WARN [formula] `HICl4·4H2O` — formula not found in page text after normalization (step 4521876); likely OCR-mangled in source — human review
- coverage: `350 mm` — prose quantity matched no structured field
- coverage: `12 mm` — prose quantity matched no structured field
- coverage: `600 mm` — prose quantity matched no structured field
- coverage: `15 g.` — prose quantity matched no structured field
- coverage: `6 hours` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `3.55 g.` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `12 g.` — prose quantity matched no structured field
- coverage: `10.82 g.` — prose quantity matched no structured field
- coverage: `1.5 hours` — prose quantity matched no structured field

## sciencemadness:brauer:cl2o-m2 (p. 300)
- WARN [formula] `CCl4` — formula not found in page text after normalization (step 4521877); likely OCR-mangled in source — human review
- WARN [formula] `Cl2O` — formula not found in page text after normalization (step 4521878); likely OCR-mangled in source — human review
- coverage: `20 g.` — prose quantity matched no structured field
- coverage: `6.9 ml.` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `350 mm` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `600 mm` — prose quantity matched no structured field
- coverage: `15 g.` — prose quantity matched no structured field
- coverage: `6 hours` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `10.82 g.` — prose quantity matched no structured field
- coverage: `20 g.` — prose quantity matched no structured field
- coverage: `60 g.` — prose quantity matched no structured field
- coverage: `122 g.` — prose quantity matched no structured field
- coverage: `108 g.` — prose quantity matched no structured field
- coverage: `57 ml.` — prose quantity matched no structured field
- coverage: `400 ml.` — prose quantity matched no structured field
- coverage: `78°C` — prose quantity matched no structured field
- coverage: `110°C` — prose quantity matched no structured field

## sciencemadness:brauer:hclo-m2 (p. 309)
- WARN [formula] `Cl2O` — formula not found in page text after normalization (step 4521881); likely OCR-mangled in source — human review
- WARN [formula] `HClO` — formula not found in page text after normalization (step 4521881); likely OCR-mangled in source — human review
- coverage: `70°C` — prose quantity matched no structured field
- coverage: `110°C` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `100°C` — prose quantity matched no structured field
- coverage: `240°C` — prose quantity matched no structured field
- coverage: `50 g.` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `35 g.` — prose quantity matched no structured field
- coverage: `43.7 g.` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `5°C` — prose quantity matched no structured field
- coverage: `438 g.` — prose quantity matched no structured field
- coverage: `306 ml.` — prose quantity matched no structured field
- coverage: `314 g.` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `175 g.` — prose quantity matched no structured field
- coverage: `8°C` — prose quantity matched no structured field
- coverage: `8°C` — prose quantity matched no structured field
- coverage: `180 g.` — prose quantity matched no structured field
- coverage: `5°C` — prose quantity matched no structured field
- coverage: `60 g.` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `50°C` — prose quantity matched no structured field
- coverage: `5°C` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `55 g.` — prose quantity matched no structured field

## sciencemadness:brauer:nh4clo3-m2 (p. 314)
- WARN [formula] `HClO3` — formula not found in page text after normalization (step 4521885); likely OCR-mangled in source — human review
- WARN [formula] `NH4ClO3` — formula not found in page text after normalization (step 4521886); likely OCR-mangled in source — human review
- coverage: `53.3 ml.` — prose quantity matched no structured field
- coverage: `53.3 ml.` — prose quantity matched no structured field
- coverage: `660 ml.` — prose quantity matched no structured field
- coverage: `122.6 g.` — prose quantity matched no structured field
- coverage: `70g.` — prose quantity matched no structured field
- coverage: `350 ml.` — prose quantity matched no structured field
- coverage: `160 g.` — prose quantity matched no structured field
- coverage: `160 ml.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `15 ml.` — prose quantity matched no structured field
- coverage: `27.6 g.` — prose quantity matched no structured field
- coverage: `275 ml.` — prose quantity matched no structured field
- coverage: `7.75 g.` — prose quantity matched no structured field

## sciencemadness:brauer:nh4clo3-m3 (p. 314)
- WARN [formula] `NH4ClO3` — formula not found in page text after normalization (step 4521888); likely OCR-mangled in source — human review
- coverage: `53.3 ml.` — prose quantity matched no structured field
- coverage: `53.3 ml.` — prose quantity matched no structured field
- coverage: `660 ml.` — prose quantity matched no structured field
- coverage: `122.6 g.` — prose quantity matched no structured field
- coverage: `70g.` — prose quantity matched no structured field
- coverage: `350 ml.` — prose quantity matched no structured field
- coverage: `160 g.` — prose quantity matched no structured field
- coverage: `160 ml.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `15 ml.` — prose quantity matched no structured field
- coverage: `27.6 g.` — prose quantity matched no structured field
- coverage: `275 ml.` — prose quantity matched no structured field
- coverage: `7.75 g.` — prose quantity matched no structured field

## sciencemadness:brauer:hio3-m3 (p. 318)
- WARN [formula] `HClO3` — formula not found in page text after normalization (step 4521889); likely OCR-mangled in source — human review
- coverage: `80°C` — prose quantity matched no structured field
- coverage: `50 g.` — prose quantity matched no structured field
- coverage: `70 C` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `25 ml.` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `180°C` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `25 g.` — prose quantity matched no structured field
- coverage: `90°C` — prose quantity matched no structured field
- coverage: `160°C` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `160°C` — prose quantity matched no structured field
- coverage: `30 mm` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field

## sciencemadness:brauer:hclo4-m2 (p. 319)
- WARN [formula] `HClO4` — formula not found in page text after normalization (step 4521891); likely OCR-mangled in source — human review
- WARN [formula] `HClO4` — formula not found in page text after normalization (step 4521891); likely OCR-mangled in source — human review
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `10 mm` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `10 mm` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field

## sciencemadness:brauer:perchlorato-anhydrous (p. 320)
- WARN [formula] `Mg(ClO4)2·6H2O` — formula not found in page text after normalization (step 4521892); likely OCR-mangled in source — human review
- WARN [formula] `Mg(ClO4)2` — formula not found in page text after normalization (step 4521892); likely OCR-mangled in source — human review
- coverage: `25 g.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `20 mm` — prose quantity matched no structured field
- coverage: `90°C` — prose quantity matched no structured field
- coverage: `160°C` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `20 mm` — prose quantity matched no structured field
- coverage: `160°C` — prose quantity matched no structured field
- coverage: `30 mm` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `142°C` — prose quantity matched no structured field
- coverage: `16 g.` — prose quantity matched no structured field
- coverage: `53 g.` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field

## sciencemadness:brauer:perchlorato-anhydrous-m2 (p. 320)
- WARN [formula] `MgCO3` — formula not found in page text after normalization (step 4521893); likely OCR-mangled in source — human review
- WARN [formula] `Mg(ClO4)2` — formula not found in page text after normalization (step 4521894); likely OCR-mangled in source — human review
- coverage: `25 g.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `20 mm` — prose quantity matched no structured field
- coverage: `90°C` — prose quantity matched no structured field
- coverage: `160°C` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `20 mm` — prose quantity matched no structured field
- coverage: `160°C` — prose quantity matched no structured field
- coverage: `30 mm` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `142°C` — prose quantity matched no structured field
- coverage: `16 g.` — prose quantity matched no structured field
- coverage: `53 g.` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field

## sciencemadness:brauer:no2clo4-m2 (p. 321)
- WARN [formula] `ClO2` — formula not found in page text after normalization (step 4521898); likely OCR-mangled in source — human review
- WARN [formula] `NO2ClO4` — formula not found in page text after normalization (step 4521899); likely OCR-mangled in source — human review
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `10 mm` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `10 mm` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `142°C` — prose quantity matched no structured field
- coverage: `16 g.` — prose quantity matched no structured field
- coverage: `53 g.` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `75 ml.` — prose quantity matched no structured field
- coverage: `200 ml.` — prose quantity matched no structured field
- coverage: `70°C` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `70°C` — prose quantity matched no structured field
- coverage: `50°C` — prose quantity matched no structured field
- coverage: `46 g.` — prose quantity matched no structured field
- coverage: `52.9 g.` — prose quantity matched no structured field

## sciencemadness:brauer:kio4 (p. 325)
- WARN [formula] `I2` — formula not found in page text after normalization (step 4521900); likely OCR-mangled in source — human review
- WARN [formula] `KClO3` — formula not found in page text after normalization (step 4521900); likely OCR-mangled in source — human review
- WARN [formula] `Cl2` — formula not found in page text after normalization (step 4521900); likely OCR-mangled in source — human review
- coverage: `110°C` — prose quantity matched no structured field
- coverage: `225 g.` — prose quantity matched no structured field
- coverage: `231.8 g.` — prose quantity matched no structured field
- coverage: `156.1 g.` — prose quantity matched no structured field
- coverage: `40 g.` — prose quantity matched no structured field
- coverage: `1200 ml.` — prose quantity matched no structured field
- coverage: `213 g.` — prose quantity matched no structured field
- coverage: `170 g.` — prose quantity matched no structured field
- coverage: `15 minutes` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `50 g.` — prose quantity matched no structured field
- coverage: `264 g.` — prose quantity matched no structured field
- coverage: `80 C` — prose quantity matched no structured field
- coverage: `80 ml.` — prose quantity matched no structured field
- coverage: `2 ml.` — prose quantity matched no structured field
- coverage: `80°C` — prose quantity matched no structured field
- coverage: `45 minutes` — prose quantity matched no structured field
- coverage: `3H` — prose quantity matched no structured field
- coverage: `25 ml.` — prose quantity matched no structured field
- coverage: `85 g.` — prose quantity matched no structured field
- coverage: `213.9
h` — prose quantity matched no structured field
- coverage: `200 ml.` — prose quantity matched no structured field
- coverage: `55 ml.` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `110°C` — prose quantity matched no structured field
- coverage: `61 g.` — prose quantity matched no structured field
- coverage: `11 g.` — prose quantity matched no structured field
- coverage: `168.6 g.` — prose quantity matched no structured field
- coverage: `225 g.` — prose quantity matched no structured field
- coverage: `10 ml.` — prose quantity matched no structured field
- coverage: `425 g.` — prose quantity matched no structured field
- coverage: `2 hours` — prose quantity matched no structured field
- coverage: `330 g.` — prose quantity matched no structured field
- coverage: `181.2 g.` — prose quantity matched no structured field
- coverage: `88.4 g.` — prose quantity matched no structured field

## sciencemadness:brauer:cl2-m2 (pp. 272-273)
- WARN [formula] `HCl` — formula not found in page text after normalization (step 4521905); likely OCR-mangled in source — human review
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `78°C` — prose quantity matched no structured field
- coverage: `24 hours` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field

## sciencemadness:brauer:br2-m1 (p. 275)
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `8H` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `5°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `6°C` — prose quantity matched no structured field

## sciencemadness:brauer:br2-m2 (pp. 275-276)
- WARN [formula] `K2C2O4` — formula not found in page text after normalization (step 4521909); likely OCR-mangled in source — human review
- WARN [formula] `K2Cr2O7` — formula not found in page text after normalization (step 4521911); likely OCR-mangled in source — human review
- WARN [formula] `H2SO4` — formula not found in page text after normalization (step 4521911); likely OCR-mangled in source — human review
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `8H` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `5°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `6°C` — prose quantity matched no structured field

## sciencemadness:brauer:br2-8h2o (p. 276)
- coverage: `8H` — prose quantity matched no structured field
- coverage: `5°C` — prose quantity matched no structured field
- coverage: `6°C` — prose quantity matched no structured field

## sciencemadness:brauer:hcl-m2 (p. 281)
- WARN [formula] `HCl` — formula not found in page text after normalization (step 4521914); likely OCR-mangled in source — human review
- WARN [formula] `H2O` — formula not found in page text after normalization (step 4521914); likely OCR-mangled in source — human review
- WARN [formula] `HCl` — formula not found in page text after normalization (step 4521915); likely OCR-mangled in source — human review
- coverage: `200 ml.` — prose quantity matched no structured field
- coverage: `200 ml.` — prose quantity matched no structured field
- coverage: `200 ml.` — prose quantity matched no structured field
- coverage: `67.4 g.` — prose quantity matched no structured field
- coverage: `5 mm` — prose quantity matched no structured field
- coverage: `207°C` — prose quantity matched no structured field
- coverage: `15°C` — prose quantity matched no structured field
- coverage: `0.3 mm` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field

## sciencemadness:brauer:hi-m1 (pp. 286-287)
- WARN [formula] `CaI2` — formula not found in page text after normalization (step 4521917); likely OCR-mangled in source — human review
- coverage: `120 g.` — prose quantity matched no structured field
- coverage: `200 ml.` — prose quantity matched no structured field
- coverage: `90 ml.` — prose quantity matched no structured field
- coverage: `75 C` — prose quantity matched no structured field
- coverage: `740 mm` — prose quantity matched no structured field
- coverage: `126°C` — prose quantity matched no structured field
- coverage: `760 mm` — prose quantity matched no structured field
- coverage: `78°C` — prose quantity matched no structured field
- coverage: `3 g.` — prose quantity matched no structured field
- coverage: `7 ml.` — prose quantity matched no structured field

## sciencemadness:brauer:icl (pp. 290-291)
- coverage: `127°C` — prose quantity matched no structured field
- coverage: `760 mm` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `280 ml.` — prose quantity matched no structured field
- coverage: `600 ml.` — prose quantity matched no structured field
- coverage: `725°C` — prose quantity matched no structured field
- coverage: `680°C` — prose quantity matched no structured field
- coverage: `300 ml.` — prose quantity matched no structured field
- coverage: `300 ml.` — prose quantity matched no structured field
- coverage: `1674 g.` — prose quantity matched no structured field
- coverage: `45 C` — prose quantity matched no structured field
- coverage: `338.3 g.` — prose quantity matched no structured field
- coverage: `622 g.` — prose quantity matched no structured field
- coverage: `79°C` — prose quantity matched no structured field

## sciencemadness:brauer:cl2o6 (pp. 303-304)
- WARN [formula] `Cl2O6` — formula not found in page text after normalization (step 4521925); likely OCR-mangled in source — human review
- coverage: `10°C` — prose quantity matched no structured field
- coverage: `30 g.` — prose quantity matched no structured field
- coverage: `15 minutes` — prose quantity matched no structured field
- coverage: `5 ml.` — prose quantity matched no structured field
- coverage: `70°C` — prose quantity matched no structured field
- coverage: `25 C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `90°C` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `3 hours` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `30 g.` — prose quantity matched no structured field
- coverage: `90°C` — prose quantity matched no structured field
- coverage: `2 mm` — prose quantity matched no structured field
- coverage: `78°C` — prose quantity matched no structured field
- coverage: `90°C` — prose quantity matched no structured field
- coverage: `50 g.` — prose quantity matched no structured field
- coverage: `120 ml.` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `8.2 g.` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `75°C` — prose quantity matched no structured field
- coverage: `80°C` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `80°C` — prose quantity matched no structured field

## sciencemadness:brauer:cl2o7-m3 (pp. 305-306)
- WARN [formula] `CCl4` — formula not found in page text after normalization (step 4521929); likely OCR-mangled in source — human review
- WARN [formula] `HClO4` — formula not found in page text after normalization (step 4521929); likely OCR-mangled in source — human review
- WARN [formula] `Cl2O7` — formula not found in page text after normalization (step 4521930); likely OCR-mangled in source — human review
- coverage: `30 g.` — prose quantity matched no structured field
- coverage: `15 minutes` — prose quantity matched no structured field
- coverage: `5 ml.` — prose quantity matched no structured field
- coverage: `70°C` — prose quantity matched no structured field
- coverage: `25 C` — prose quantity matched no structured field
- coverage: `90°C` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `3 hours` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `30 g.` — prose quantity matched no structured field
- coverage: `90°C` — prose quantity matched no structured field
- coverage: `2 mm` — prose quantity matched no structured field
- coverage: `78°C` — prose quantity matched no structured field
- coverage: `90°C` — prose quantity matched no structured field
- coverage: `75°C` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `2 ml.` — prose quantity matched no structured field
- coverage: `1 g.` — prose quantity matched no structured field
- coverage: `30
minutes` — prose quantity matched no structured field
- coverage: `78°C` — prose quantity matched no structured field
- coverage: `7 hours` — prose quantity matched no structured field
- coverage: `40 C` — prose quantity matched no structured field
- coverage: `55°C` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field

## sciencemadness:brauer:hclo-m1 (pp. 308-309)
- WARN [formula] `HClO` — formula not found in page text after normalization (step 4521932); likely OCR-mangled in source — human review
- coverage: `40 C` — prose quantity matched no structured field
- coverage: `55°C` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `70°C` — prose quantity matched no structured field
- coverage: `110°C` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `100°C` — prose quantity matched no structured field
- coverage: `240°C` — prose quantity matched no structured field
- coverage: `50 g.` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `35 g.` — prose quantity matched no structured field
- coverage: `43.7 g.` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `5°C` — prose quantity matched no structured field
- coverage: `438 g.` — prose quantity matched no structured field
- coverage: `306 ml.` — prose quantity matched no structured field
- coverage: `3°C` — prose quantity matched no structured field
- coverage: `314 g.` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `175 g.` — prose quantity matched no structured field
- coverage: `3°C` — prose quantity matched no structured field
- coverage: `8°C` — prose quantity matched no structured field
- coverage: `8°C` — prose quantity matched no structured field
- coverage: `180 g.` — prose quantity matched no structured field
- coverage: `5°C` — prose quantity matched no structured field
- coverage: `60 g.` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `50°C` — prose quantity matched no structured field
- coverage: `5°C` — prose quantity matched no structured field
- coverage: `3°C` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `55 g.` — prose quantity matched no structured field

## sciencemadness:brauer:naclo2-3h2o (p. 312)
- coverage: `24.5 g.` — prose quantity matched no structured field
- coverage: `20 g.` — prose quantity matched no structured field
- coverage: `21.6 g.` — prose quantity matched no structured field
- coverage: `11.8 ml.` — prose quantity matched no structured field
- coverage: `80 ml.` — prose quantity matched no structured field
- coverage: `322 g.` — prose quantity matched no structured field
- coverage: `500 ml.` — prose quantity matched no structured field
- coverage: `98 g.` — prose quantity matched no structured field
- coverage: `53.3 ml.` — prose quantity matched no structured field
- coverage: `53.3 ml.` — prose quantity matched no structured field
- coverage: `660 ml.` — prose quantity matched no structured field

## sciencemadness:brauer:hclo3 (pp. 312-313)
- WARN [formula] `HClO3` — formula not found in page text after normalization (step 4521939); likely OCR-mangled in source — human review
- coverage: `24.5 g.` — prose quantity matched no structured field
- coverage: `20 g.` — prose quantity matched no structured field
- coverage: `21.6 g.` — prose quantity matched no structured field
- coverage: `11.8 ml.` — prose quantity matched no structured field
- coverage: `80 ml.` — prose quantity matched no structured field
- coverage: `200 ml.` — prose quantity matched no structured field
- coverage: `31.6 g.` — prose quantity matched no structured field
- coverage: `12 g.` — prose quantity matched no structured field
- coverage: `15.6 g.` — prose quantity matched no structured field
- coverage: `53.3 ml.` — prose quantity matched no structured field
- coverage: `53.3 ml.` — prose quantity matched no structured field
- coverage: `122.6 g.` — prose quantity matched no structured field
- coverage: `70g.` — prose quantity matched no structured field
- coverage: `350 ml.` — prose quantity matched no structured field

## sciencemadness:brauer:hbro3 (pp. 315-316)
- coverage: `122.6 g.` — prose quantity matched no structured field
- coverage: `70g.` — prose quantity matched no structured field
- coverage: `350 ml.` — prose quantity matched no structured field
- coverage: `160 g.` — prose quantity matched no structured field
- coverage: `160 ml.` — prose quantity matched no structured field
- coverage: `15 ml.` — prose quantity matched no structured field
- coverage: `334 g.` — prose quantity matched no structured field
- coverage: `700 ml.` — prose quantity matched no structured field
- coverage: `244 g.` — prose quantity matched no structured field
- coverage: `400 ml.` — prose quantity matched no structured field
- coverage: `80°C` — prose quantity matched no structured field
- coverage: `50 g.` — prose quantity matched no structured field
- coverage: `70 C` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `25 ml.` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `180°C` — prose quantity matched no structured field
- coverage: `68.55 g.` — prose quantity matched no structured field

## sciencemadness:brauer:hio3-m2 (pp. 316-317)
- coverage: `160 g.` — prose quantity matched no structured field
- coverage: `160 ml.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `15 ml.` — prose quantity matched no structured field
- coverage: `27.6 g.` — prose quantity matched no structured field
- coverage: `275 ml.` — prose quantity matched no structured field
- coverage: `7.75 g.` — prose quantity matched no structured field
- coverage: `334 g.` — prose quantity matched no structured field
- coverage: `700 ml.` — prose quantity matched no structured field
- coverage: `244 g.` — prose quantity matched no structured field
- coverage: `400 ml.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `80°C` — prose quantity matched no structured field
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `180°C` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `68.55 g.` — prose quantity matched no structured field
- coverage: `20 minutes` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field

## sciencemadness:brauer:hclo4-m1 (pp. 318-319)
- WARN [formula] `KClO4` — formula not found in page text after normalization (step 4521947); likely OCR-mangled in source — human review
- WARN [formula] `HClO4` — formula not found in page text after normalization (step 4521948); likely OCR-mangled in source — human review
- coverage: `80°C` — prose quantity matched no structured field
- coverage: `50 g.` — prose quantity matched no structured field
- coverage: `70 C` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `180°C` — prose quantity matched no structured field
- coverage: `68.55 g.` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `90°C` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `30 mm` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field

## sciencemadness:brauer:perchlorato-hydrous (p. 320)
- WARN [formula] `MgO` — formula not found in page text after normalization (step 4521949); likely OCR-mangled in source — human review
- WARN [formula] `Mg(ClO4)2·6H2O` — formula not found in page text after normalization (step 4521950); likely OCR-mangled in source — human review
- coverage: `25 g.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `20 mm` — prose quantity matched no structured field
- coverage: `90°C` — prose quantity matched no structured field
- coverage: `160°C` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `20 mm` — prose quantity matched no structured field
- coverage: `160°C` — prose quantity matched no structured field
- coverage: `30 mm` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `10 mm` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `10 mm` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `142°C` — prose quantity matched no structured field
- coverage: `16 g.` — prose quantity matched no structured field
- coverage: `53 g.` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field

## sciencemadness:brauer:o2-m1 (pp. 333-334)
- coverage: `6 g.` — prose quantity matched no structured field
- coverage: `20 g.` — prose quantity matched no structured field
- coverage: `6 days` — prose quantity matched no structured field
- coverage: `5g.` — prose quantity matched no structured field
- coverage: `20
mm` — prose quantity matched no structured field
- coverage: `350°C` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field

## sciencemadness:brauer:h2s2-distill (p. 351)
- coverage: `300 ml.` — prose quantity matched no structured field
- coverage: `110°C` — prose quantity matched no structured field
- coverage: `125°C` — prose quantity matched no structured field
- coverage: `20 minutes` — prose quantity matched no structured field
- coverage: `110°C` — prose quantity matched no structured field
- coverage: `25 ml.` — prose quantity matched no structured field
- coverage: `120 ml.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `42°C` — prose quantity matched no structured field
- coverage: `52°C` — prose quantity matched no structured field

## sciencemadness:brauer:na2s2-m2 (p. 362)
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `0.5 hour` — prose quantity matched no structured field
- coverage: `4 g.` — prose quantity matched no structured field
- coverage: `30 minutes` — prose quantity matched no structured field
- coverage: `80°C` — prose quantity matched no structured field
- coverage: `8 g.` — prose quantity matched no structured field

## sciencemadness:brauer:nh4-2s5 (p. 370)
- coverage: `10 hours` — prose quantity matched no structured field
- coverage: `80 g.` — prose quantity matched no structured field
- coverage: `200 ml.` — prose quantity matched no structured field
- coverage: `80 minutes` — prose quantity matched no structured field
- coverage: `35 g.` — prose quantity matched no structured field
- coverage: `200 g.` — prose quantity matched no structured field
- coverage: `0.1 g.` — prose quantity matched no structured field
- coverage: `0.5 hour` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `2 ml.` — prose quantity matched no structured field
- coverage: `62°C` — prose quantity matched no structured field
- coverage: `60°C` — prose quantity matched no structured field

## sciencemadness:brauer:scl2-m1 (pp. 370-371)
- WARN [formula] `PCl3` — formula not found in page text after normalization (step 4521993); likely OCR-mangled in source — human review
- WARN [formula] `SCl2` — formula not found in page text after normalization (step 4521993); likely OCR-mangled in source — human review
- coverage: `10 hours` — prose quantity matched no structured field
- coverage: `80 g.` — prose quantity matched no structured field
- coverage: `80 minutes` — prose quantity matched no structured field
- coverage: `35 g.` — prose quantity matched no structured field
- coverage: `62°C` — prose quantity matched no structured field
- coverage: `60°C` — prose quantity matched no structured field
- coverage: `80°C` — prose quantity matched no structured field
- coverage: `137°C` — prose quantity matched no structured field
- coverage: `12 mm` — prose quantity matched no structured field
- coverage: `30 C` — prose quantity matched no structured field

## sciencemadness:brauer:cl2-m4 (p. 273)
- coverage: `78°C` — prose quantity matched no structured field
- coverage: `24 hours` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field

## sciencemadness:brauer:cl2o-m1 (pp. 299-300)
- WARN [formula] `Cl2O` — formula not found in page text after normalization (step 4522032); likely OCR-mangled in source — human review
- coverage: `6.9 ml.` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `350 mm` — prose quantity matched no structured field
- coverage: `12 mm` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `3.55 g.` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `12 g.` — prose quantity matched no structured field
- coverage: `10.82 g.` — prose quantity matched no structured field
- coverage: `1.5 hours` — prose quantity matched no structured field
- coverage: `60 g.` — prose quantity matched no structured field
- coverage: `122 g.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `108 g.` — prose quantity matched no structured field
- coverage: `57 ml.` — prose quantity matched no structured field
- coverage: `78°C` — prose quantity matched no structured field
- coverage: `110°C` — prose quantity matched no structured field

## sciencemadness:brauer:clo2-m1 (p. 301)
- WARN [formula] `KClO3` — formula not found in page text after normalization (step 4522033); likely OCR-mangled in source — human review
- WARN [formula] `SiO2` — formula not found in page text after normalization (step 4522033); likely OCR-mangled in source — human review
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `3.55 g.` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `12 g.` — prose quantity matched no structured field
- coverage: `10.82 g.` — prose quantity matched no structured field
- coverage: `1.5 hours` — prose quantity matched no structured field
- coverage: `122 g.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `108 g.` — prose quantity matched no structured field
- coverage: `57 ml.` — prose quantity matched no structured field
- coverage: `400 ml.` — prose quantity matched no structured field
- coverage: `78°C` — prose quantity matched no structured field
- coverage: `110°C` — prose quantity matched no structured field

## sciencemadness:brauer:clo2-m2 (pp. 301-302)
- WARN [formula] `KClO3` — formula not found in page text after normalization (step 4522035); likely OCR-mangled in source — human review
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `3.55 g.` — prose quantity matched no structured field
- coverage: `12 g.` — prose quantity matched no structured field
- coverage: `10.82 g.` — prose quantity matched no structured field
- coverage: `1.5 hours` — prose quantity matched no structured field
- coverage: `20 g.` — prose quantity matched no structured field
- coverage: `60 g.` — prose quantity matched no structured field
- coverage: `57 ml.` — prose quantity matched no structured field
- coverage: `78°C` — prose quantity matched no structured field
- coverage: `110°C` — prose quantity matched no structured field
- coverage: `10°C` — prose quantity matched no structured field

## sciencemadness:brauer:naclo-5h2o (p. 309)
- WARN [formula] `NaClO·5H2O` — formula not found in page text after normalization (step 4522041); likely OCR-mangled in source — human review
- coverage: `70°C` — prose quantity matched no structured field
- coverage: `110°C` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `100°C` — prose quantity matched no structured field
- coverage: `240°C` — prose quantity matched no structured field
- coverage: `35 g.` — prose quantity matched no structured field
- coverage: `43.7 g.` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `5°C` — prose quantity matched no structured field
- coverage: `438 g.` — prose quantity matched no structured field
- coverage: `306 ml.` — prose quantity matched no structured field
- coverage: `314 g.` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `175 g.` — prose quantity matched no structured field
- coverage: `8°C` — prose quantity matched no structured field
- coverage: `8°C` — prose quantity matched no structured field
- coverage: `180 g.` — prose quantity matched no structured field
- coverage: `5°C` — prose quantity matched no structured field
- coverage: `60 g.` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `5°C` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `55 g.` — prose quantity matched no structured field

## sciencemadness:brauer:baclo3-h2o (p. 315)
- coverage: `122.6 g.` — prose quantity matched no structured field
- coverage: `70g.` — prose quantity matched no structured field
- coverage: `350 ml.` — prose quantity matched no structured field
- coverage: `160 g.` — prose quantity matched no structured field
- coverage: `160 ml.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `15 ml.` — prose quantity matched no structured field
- coverage: `27.6 g.` — prose quantity matched no structured field
- coverage: `275 ml.` — prose quantity matched no structured field
- coverage: `7.75 g.` — prose quantity matched no structured field
- coverage: `400 ml.` — prose quantity matched no structured field

## sciencemadness:brauer:noclo4 (pp. 320-321)
- WARN [formula] `NOClO4` — formula not found in page text after normalization (step 4522045); likely OCR-mangled in source — human review
- WARN [formula] `NOClO4` — formula not found in page text after normalization (step 4522046); likely OCR-mangled in source — human review
- coverage: `25 g.` — prose quantity matched no structured field
- coverage: `20 mm` — prose quantity matched no structured field
- coverage: `90°C` — prose quantity matched no structured field
- coverage: `160°C` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `20 mm` — prose quantity matched no structured field
- coverage: `160°C` — prose quantity matched no structured field
- coverage: `30 mm` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `10 mm` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `10 mm` — prose quantity matched no structured field
- coverage: `142°C` — prose quantity matched no structured field
- coverage: `53 g.` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `75 ml.` — prose quantity matched no structured field
- coverage: `200 ml.` — prose quantity matched no structured field
- coverage: `70°C` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `70°C` — prose quantity matched no structured field
- coverage: `50°C` — prose quantity matched no structured field
- coverage: `46 g.` — prose quantity matched no structured field
- coverage: `52.9 g.` — prose quantity matched no structured field

## sciencemadness:brauer:ba3h4io6-m1 (pp. 325-326)
- WARN [formula] `Ba(OH)2` — formula not found in page text after normalization (step 4522048); likely OCR-mangled in source — human review
- coverage: `110°C` — prose quantity matched no structured field
- coverage: `231.8 g.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `156.1 g.` — prose quantity matched no structured field
- coverage: `40 g.` — prose quantity matched no structured field
- coverage: `1200 ml.` — prose quantity matched no structured field
- coverage: `213 g.` — prose quantity matched no structured field
- coverage: `170 g.` — prose quantity matched no structured field
- coverage: `15 minutes` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `50 g.` — prose quantity matched no structured field
- coverage: `264 g.` — prose quantity matched no structured field
- coverage: `80 C` — prose quantity matched no structured field
- coverage: `80 ml.` — prose quantity matched no structured field
- coverage: `80°C` — prose quantity matched no structured field
- coverage: `45 minutes` — prose quantity matched no structured field
- coverage: `3H` — prose quantity matched no structured field
- coverage: `25 ml.` — prose quantity matched no structured field
- coverage: `85 g.` — prose quantity matched no structured field
- coverage: `213.9
h` — prose quantity matched no structured field
- coverage: `200 ml.` — prose quantity matched no structured field
- coverage: `55 ml.` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `110°C` — prose quantity matched no structured field
- coverage: `61 g.` — prose quantity matched no structured field
- coverage: `11 g.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `135 g.` — prose quantity matched no structured field
- coverage: `168.6 g.` — prose quantity matched no structured field
- coverage: `195 g.` — prose quantity matched no structured field
- coverage: `178 g.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `181.2 g.` — prose quantity matched no structured field
- coverage: `88.4 g.` — prose quantity matched no structured field
- coverage: `5 ml.` — prose quantity matched no structured field
- coverage: `78°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `15 hours` — prose quantity matched no structured field
- coverage: `30°C` — prose quantity matched no structured field
- coverage: `90°C` — prose quantity matched no structured field

## sciencemadness:brauer:clno3 (pp. 326-327)
- WARN [formula] `Cl2O` — formula not found in page text after normalization (step 4522050); likely OCR-mangled in source — human review
- WARN [formula] `Cl2` — formula not found in page text after normalization (step 4522051); likely OCR-mangled in source — human review
- WARN [formula] `ClNO3` — formula not found in page text after normalization (step 4522052); likely OCR-mangled in source — human review
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `110°C` — prose quantity matched no structured field
- coverage: `61 g.` — prose quantity matched no structured field
- coverage: `11 g.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `135 g.` — prose quantity matched no structured field
- coverage: `168.6 g.` — prose quantity matched no structured field
- coverage: `195 g.` — prose quantity matched no structured field
- coverage: `178 g.` — prose quantity matched no structured field
- coverage: `225 g.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `10 ml.` — prose quantity matched no structured field
- coverage: `425 g.` — prose quantity matched no structured field
- coverage: `2 hours` — prose quantity matched no structured field
- coverage: `330 g.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `181.2 g.` — prose quantity matched no structured field
- coverage: `88.4 g.` — prose quantity matched no structured field
- coverage: `5 ml.` — prose quantity matched no structured field
- coverage: `78°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `90°C` — prose quantity matched no structured field
- coverage: `5H` — prose quantity matched no structured field
- coverage: `3 g.` — prose quantity matched no structured field
- coverage: `40 ml.` — prose quantity matched no structured field
- coverage: `10°C` — prose quantity matched no structured field
- coverage: `78°C` — prose quantity matched no structured field
- coverage: `140°C` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field

## sciencemadness:brauer:i2o4 (p. 333)
- WARN [formula] `CaO` — formula not found in page text after normalization (step 4522055); likely OCR-mangled in source — human review
- coverage: `6 g.` — prose quantity matched no structured field
- coverage: `20 g.` — prose quantity matched no structured field
- coverage: `6 days` — prose quantity matched no structured field
- coverage: `5g.` — prose quantity matched no structured field
- coverage: `20
mm` — prose quantity matched no structured field

## sciencemadness:brauer:h2s-m1 (p. 344)
- WARN [formula] `MgCl2` — formula not found in page text after normalization (step 4522056); likely OCR-mangled in source — human review
- WARN [formula] `H2O` — formula not found in page text after normalization (step 4522056); likely OCR-mangled in source — human review
- coverage: `7.2 g.` — prose quantity matched no structured field
- coverage: `6.4 g.` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `1.5 ml.` — prose quantity matched no structured field
- coverage: `10 ml.` — prose quantity matched no structured field
- coverage: `2.7
g.` — prose quantity matched no structured field
- coverage: `8 ml.` — prose quantity matched no structured field
- coverage: `5.5 g.` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `300 ml.` — prose quantity matched no structured field
- coverage: `10 ml.` — prose quantity matched no structured field
- coverage: `300 ml.` — prose quantity matched no structured field
- coverage: `24 hours` — prose quantity matched no structured field
- coverage: `600°C` — prose quantity matched no structured field
- coverage: `600°C` — prose quantity matched no structured field

## sciencemadness:brauer:h2s-m2 (pp. 344-346)
- coverage: `7.2 g.` — prose quantity matched no structured field
- coverage: `6.4 g.` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `1.5 ml.` — prose quantity matched no structured field
- coverage: `10 ml.` — prose quantity matched no structured field
- coverage: `2.7
g.` — prose quantity matched no structured field
- coverage: `8 ml.` — prose quantity matched no structured field
- coverage: `5.5 g.` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `300 ml.` — prose quantity matched no structured field
- coverage: `10 ml.` — prose quantity matched no structured field
- coverage: `300 ml.` — prose quantity matched no structured field
- coverage: `24 hours` — prose quantity matched no structured field
- coverage: `18 hours` — prose quantity matched no structured field
- coverage: `500 g.` — prose quantity matched no structured field
- coverage: `400 ml.` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `1.5 hours` — prose quantity matched no structured field
- coverage: `10°C` — prose quantity matched no structured field
- coverage: `5°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `260 g.` — prose quantity matched no structured field
- coverage: `160 ml.` — prose quantity matched no structured field
- coverage: `480 g.` — prose quantity matched no structured field
- coverage: `64 g.` — prose quantity matched no structured field
- coverage: `500 ml.` — prose quantity matched no structured field
- coverage: `32 ml.` — prose quantity matched no structured field

## sciencemadness:brauer:h2s-m3 (p. 346)
- WARN [formula] `Na2S·9H2O` — formula not found in page text after normalization (step 4522060); likely OCR-mangled in source — human review
- WARN [formula] `H3PO4` — formula not found in page text after normalization (step 4522060); likely OCR-mangled in source — human review
- coverage: `250 g.` — prose quantity matched no structured field
- coverage: `600°C` — prose quantity matched no structured field
- coverage: `600°C` — prose quantity matched no structured field
- coverage: `18 hours` — prose quantity matched no structured field
- coverage: `500 g.` — prose quantity matched no structured field
- coverage: `250 g.` — prose quantity matched no structured field
- coverage: `400 ml.` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `1.5 hours` — prose quantity matched no structured field
- coverage: `10°C` — prose quantity matched no structured field
- coverage: `5°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `260 g.` — prose quantity matched no structured field
- coverage: `160 ml.` — prose quantity matched no structured field
- coverage: `480 g.` — prose quantity matched no structured field
- coverage: `64 g.` — prose quantity matched no structured field
- coverage: `500 ml.` — prose quantity matched no structured field
- coverage: `32 ml.` — prose quantity matched no structured field

## sciencemadness:brauer:h2s2-h2s3-crack (pp. 350-351)
- coverage: `300 ml.` — prose quantity matched no structured field
- coverage: `20 minutes` — prose quantity matched no structured field
- coverage: `25 ml.` — prose quantity matched no structured field
- coverage: `120 ml.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `1.5 hours` — prose quantity matched no structured field
- coverage: `42°C` — prose quantity matched no structured field
- coverage: `1.5 mm` — prose quantity matched no structured field
- coverage: `52°C` — prose quantity matched no structured field

## sciencemadness:brauer:h2s4 (pp. 354-355)
- WARN [formula] `Na2S·9H2O` — formula not found in page text after normalization (step 4522066); likely OCR-mangled in source — human review
- WARN [formula] `S2Cl2` — formula not found in page text after normalization (step 4522067); likely OCR-mangled in source — human review
- coverage: `3 mm` — prose quantity matched no structured field
- coverage: `75°C` — prose quantity matched no structured field
- coverage: `50°C` — prose quantity matched no structured field
- coverage: `15°C` — prose quantity matched no structured field
- coverage: `34.08
H` — prose quantity matched no structured field
- coverage: `4 H` — prose quantity matched no structured field
- coverage: `102.98
H` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `65°C` — prose quantity matched no structured field
- coverage: `10 g.` — prose quantity matched no structured field
- coverage: `55°C` — prose quantity matched no structured field
- coverage: `15 minutes` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `0.5 hour` — prose quantity matched no structured field
- coverage: `1.5 hours` — prose quantity matched no structured field
- coverage: `15 mm` — prose quantity matched no structured field
- coverage: `4 mm` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field

## sciencemadness:brauer:nh4hs (p. 357)
- WARN [formula] `C2H5OH` — formula not found in page text after normalization (step 4522069); likely OCR-mangled in source — human review
- coverage: `54°C` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `85°C` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `20 ml.` — prose quantity matched no structured field
- coverage: `2 g.` — prose quantity matched no structured field
- coverage: `40 ml.` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `110 ml.` — prose quantity matched no structured field
- coverage: `4.3 g.` — prose quantity matched no structured field

## sciencemadness:brauer:nahs (pp. 357-358)
- WARN [formula] `C2H5OH` — formula not found in page text after normalization (step 4522071); likely OCR-mangled in source — human review
- WARN [formula] `C2H5OH` — formula not found in page text after normalization (step 4522073); likely OCR-mangled in source — human review
- coverage: `54°C` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `85°C` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `20 ml.` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `0.5 hour` — prose quantity matched no structured field
- coverage: `500°C` — prose quantity matched no structured field

## sciencemadness:brauer:na2s-m1 (pp. 358-359)
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `20 ml.` — prose quantity matched no structured field
- coverage: `2 g.` — prose quantity matched no structured field
- coverage: `40 ml.` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `110 ml.` — prose quantity matched no structured field
- coverage: `4.3 g.` — prose quantity matched no structured field
- coverage: `14 days` — prose quantity matched no structured field
- coverage: `15°C` — prose quantity matched no structured field
- coverage: `35°C` — prose quantity matched no structured field
- coverage: `700°C` — prose quantity matched no structured field

## sciencemadness:brauer:k2s (pp. 360-361)
- coverage: `14 days` — prose quantity matched no structured field
- coverage: `15°C` — prose quantity matched no structured field
- coverage: `35°C` — prose quantity matched no structured field
- coverage: `700°C` — prose quantity matched no structured field
- coverage: `4 g.` — prose quantity matched no structured field
- coverage: `30 minutes` — prose quantity matched no structured field
- coverage: `80°C` — prose quantity matched no structured field
- coverage: `8 g.` — prose quantity matched no structured field
- coverage: `2.5 g.` — prose quantity matched no structured field
- coverage: `45 minutes` — prose quantity matched no structured field

## sciencemadness:brauer:na2s2-m1 (pp. 361-362)
- coverage: `14 days` — prose quantity matched no structured field
- coverage: `15°C` — prose quantity matched no structured field
- coverage: `35°C` — prose quantity matched no structured field
- coverage: `700°C` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `0.5 hour` — prose quantity matched no structured field
- coverage: `500°C` — prose quantity matched no structured field
- coverage: `2.5 g.` — prose quantity matched no structured field
- coverage: `500°C` — prose quantity matched no structured field
- coverage: `45 minutes` — prose quantity matched no structured field

## sciencemadness:brauer:k2s4-m1 (pp. 362-366)
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `0.5 hour` — prose quantity matched no structured field
- coverage: `4 g.` — prose quantity matched no structured field
- coverage: `30 minutes` — prose quantity matched no structured field
- coverage: `80°C` — prose quantity matched no structured field
- coverage: `8 g.` — prose quantity matched no structured field
- coverage: `2.5 g.` — prose quantity matched no structured field
- coverage: `45 minutes` — prose quantity matched no structured field
- coverage: `5.0 g.` — prose quantity matched no structured field
- coverage: `72 ml.` — prose quantity matched no structured field
- coverage: `4.1
g.` — prose quantity matched no structured field
- coverage: `30 minutes` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `2 g.` — prose quantity matched no structured field
- coverage: `2.00 g.` — prose quantity matched no structured field
- coverage: `4.17 g.` — prose quantity matched no structured field
- coverage: `5 ml.` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `6 g.` — prose quantity matched no structured field

## sciencemadness:brauer:socl2-m1 (pp. 382-383)
- WARN [formula] `SCl2` — formula not found in page text after normalization (step 4522084); likely OCR-mangled in source — human review
- WARN [formula] `SOCl2` — formula not found in page text after normalization (step 4522085); likely OCR-mangled in source — human review
- coverage: `16°C` — prose quantity matched no structured field
- coverage: `15 ml.` — prose quantity matched no structured field
- coverage: `15°C` — prose quantity matched no structured field
- coverage: `50°C` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `20 minutes` — prose quantity matched no structured field
- coverage: `10°C` — prose quantity matched no structured field
- coverage: `3 g` — prose quantity matched no structured field
- coverage: `15°C` — prose quantity matched no structured field
- coverage: `1 mm` — prose quantity matched no structured field
- coverage: `77°C` — prose quantity matched no structured field
- coverage: `77°C` — prose quantity matched no structured field
- coverage: `108°C` — prose quantity matched no structured field
- coverage: `30 g.` — prose quantity matched no structured field
- coverage: `1000 ml.` — prose quantity matched no structured field
- coverage: `30 minutes` — prose quantity matched no structured field
- coverage: `150 g.` — prose quantity matched no structured field
- coverage: `10 g.` — prose quantity matched no structured field
- coverage: `725 mm` — prose quantity matched no structured field
- coverage: `30 g.` — prose quantity matched no structured field

## sciencemadness:brauer:socl2-m2 (p. 383)
- WARN [formula] `PCl5` — formula not found in page text after normalization (step 4522086); likely OCR-mangled in source — human review
- WARN [formula] `SOCl2` — formula not found in page text after normalization (step 4522087); likely OCR-mangled in source — human review
- WARN [formula] `POCl3` — formula not found in page text after normalization (step 4522087); likely OCR-mangled in source — human review
- coverage: `77°C` — prose quantity matched no structured field
- coverage: `77°C` — prose quantity matched no structured field
- coverage: `108°C` — prose quantity matched no structured field
- coverage: `1000 ml.` — prose quantity matched no structured field
- coverage: `150 g.` — prose quantity matched no structured field
- coverage: `10 g.` — prose quantity matched no structured field
- coverage: `725 mm` — prose quantity matched no structured field

## sciencemadness:brauer:sobr2-m3 (p. 388)
- WARN [formula] `SOCl2` — formula not found in page text after normalization (step 4522088); likely OCR-mangled in source — human review
- coverage: `160°C` — prose quantity matched no structured field
- coverage: `153°C` — prose quantity matched no structured field
- coverage: `240 g.` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `12 hours` — prose quantity matched no structured field
- coverage: `62 mm` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `70°C` — prose quantity matched no structured field
- coverage: `48°C` — prose quantity matched no structured field
- coverage: `64 g.` — prose quantity matched no structured field
- coverage: `138 g.` — prose quantity matched no structured field
- coverage: `160 g.` — prose quantity matched no structured field
- coverage: `180
g.` — prose quantity matched no structured field
- coverage: `80°C` — prose quantity matched no structured field
- coverage: `49.5°C` — prose quantity matched no structured field
- coverage: `12 hours` — prose quantity matched no structured field

## sciencemadness:brauer:h2so5 (pp. 388-389)
- WARN [formula] `HSO3Cl` — formula not found in page text after normalization (step 4522090); likely OCR-mangled in source — human review
- coverage: `160°C` — prose quantity matched no structured field
- coverage: `153°C` — prose quantity matched no structured field
- coverage: `240 g.` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `62 mm` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `70°C` — prose quantity matched no structured field
- coverage: `20 mm` — prose quantity matched no structured field
- coverage: `48°C` — prose quantity matched no structured field
- coverage: `64 g.` — prose quantity matched no structured field
- coverage: `138 g.` — prose quantity matched no structured field
- coverage: `160 g.` — prose quantity matched no structured field
- coverage: `180
g.` — prose quantity matched no structured field
- coverage: `150 ml.` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `0.1 mm` — prose quantity matched no structured field
- coverage: `80°C` — prose quantity matched no structured field
- coverage: `49.5°C` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `150 ml.` — prose quantity matched no structured field

## sciencemadness:brauer:k2s2o8 (p. 392)
- coverage: `10°C` — prose quantity matched no structured field
- coverage: `76.3 g.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `6
hours` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `10 hours` — prose quantity matched no structured field
- coverage: `33 g.` — prose quantity matched no structured field
- coverage: `4 hours` — prose quantity matched no structured field
- coverage: `40 g.` — prose quantity matched no structured field
- coverage: `2H` — prose quantity matched no structured field
- coverage: `15 minutes` — prose quantity matched no structured field
- coverage: `30°C` — prose quantity matched no structured field
- coverage: `4 H` — prose quantity matched no structured field
- coverage: `10 g.` — prose quantity matched no structured field
- coverage: `60 ml.` — prose quantity matched no structured field
- coverage: `10.2 g.` — prose quantity matched no structured field
- coverage: `3.5 g.` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `2 H` — prose quantity matched no structured field

## sciencemadness:brauer:na2s2o4-2h2o (p. 394)
- WARN [formula] `Na2S2O4·2H2O` — formula not found in page text after normalization (step 4522097); likely OCR-mangled in source — human review
- coverage: `4 H` — prose quantity matched no structured field
- coverage: `10 g.` — prose quantity matched no structured field
- coverage: `60 ml.` — prose quantity matched no structured field
- coverage: `10.2 g.` — prose quantity matched no structured field
- coverage: `3.5 g.` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `2 H` — prose quantity matched no structured field
- coverage: `100 ml.` — prose quantity matched no structured field
- coverage: `750 ml.` — prose quantity matched no structured field
- coverage: `250 ml.` — prose quantity matched no structured field
- coverage: `270 g.` — prose quantity matched no structured field
- coverage: `470 g.` — prose quantity matched no structured field
- coverage: `60°C` — prose quantity matched no structured field
- coverage: `70°C` — prose quantity matched no structured field
- coverage: `2 H` — prose quantity matched no structured field
- coverage: `500 ml.` — prose quantity matched no structured field
- coverage: `80 g.` — prose quantity matched no structured field
- coverage: `2 g.` — prose quantity matched no structured field

## sciencemadness:brauer:k2s3o6-m1 (p. 398)
- WARN [formula] `SCl2` — formula not found in page text after normalization (step 4522099); likely OCR-mangled in source — human review
- WARN [formula] `KCl` — formula not found in page text after normalization (step 4522100); likely OCR-mangled in source — human review
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `30°C` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `160 g.` — prose quantity matched no structured field
- coverage: `30 minutes` — prose quantity matched no structured field
- coverage: `30 minutes` — prose quantity matched no structured field
- coverage: `75°C` — prose quantity matched no structured field
- coverage: `30 minutes` — prose quantity matched no structured field
- coverage: `300 ml.` — prose quantity matched no structured field
- coverage: `65°C` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `75 ml.` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `5°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `350 ml.` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `20 ml.` — prose quantity matched no structured field
- coverage: `200 ml.` — prose quantity matched no structured field
- coverage: `30°C` — prose quantity matched no structured field
- coverage: `2H` — prose quantity matched no structured field
- coverage: `750 ml.` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `75 g.` — prose quantity matched no structured field
- coverage: `500 ml.` — prose quantity matched no structured field

## sciencemadness:brauer:nohso4 (p. 406)
- coverage: `3 hours` — prose quantity matched no structured field
- coverage: `480 ml.` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `2 days` — prose quantity matched no structured field
- coverage: `14 days` — prose quantity matched no structured field
- coverage: `12 hours` — prose quantity matched no structured field
- coverage: `250 ml.` — prose quantity matched no structured field
- coverage: `50°C` — prose quantity matched no structured field
- coverage: `4 hours` — prose quantity matched no structured field
- coverage: `15
minutes` — prose quantity matched no structured field
- coverage: `750 ml.` — prose quantity matched no structured field
- coverage: `100°C` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `100°C` — prose quantity matched no structured field

## sciencemadness:brauer:s4n4 (pp. 406-407)
- WARN [formula] `CCl4` — formula not found in page text after normalization (step 4522104); likely OCR-mangled in source — human review
- WARN [formula] `S2Cl2` — formula not found in page text after normalization (step 4522104); likely OCR-mangled in source — human review
- WARN [formula] `Cl2` — formula not found in page text after normalization (step 4522104); likely OCR-mangled in source — human review
- WARN [formula] `H2O` — formula not found in page text after normalization (step 4522106); likely OCR-mangled in source — human review
- coverage: `480 ml.` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `2 days` — prose quantity matched no structured field
- coverage: `14 days` — prose quantity matched no structured field
- coverage: `12 hours` — prose quantity matched no structured field
- coverage: `5°C` — prose quantity matched no structured field
- coverage: `750 ml.` — prose quantity matched no structured field
- coverage: `110°C` — prose quantity matched no structured field
- coverage: `24 g.` — prose quantity matched no structured field
- coverage: `380 ml.` — prose quantity matched no structured field
- coverage: `110°C` — prose quantity matched no structured field
- coverage: `65°C` — prose quantity matched no structured field

## sciencemadness:brauer:agf-m1 (pp. 240-241)
- coverage: `400°C` — prose quantity matched no structured field
- coverage: `2 g.` — prose quantity matched no structured field
- coverage: `50°C` — prose quantity matched no structured field
- coverage: `20 g.` — prose quantity matched no structured field
- coverage: `48 hours` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field
- coverage: `0.5 g.` — prose quantity matched no structured field
- coverage: `60 minutes` — prose quantity matched no structured field
- coverage: `80°C` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `60°C` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `44
h` — prose quantity matched no structured field
- coverage: `800°C` — prose quantity matched no structured field

## sciencemadness:brauer:agf-pure (p. 240)
- coverage: `400°C` — prose quantity matched no structured field
- coverage: `2 g.` — prose quantity matched no structured field
- coverage: `50°C` — prose quantity matched no structured field
- coverage: `20 g.` — prose quantity matched no structured field
- coverage: `48 hours` — prose quantity matched no structured field
- coverage: `0.5 g.` — prose quantity matched no structured field
- coverage: `60 minutes` — prose quantity matched no structured field
- coverage: `80°C` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field

## sciencemadness:brauer:k2nbf7 (p. 255)
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `111.2°C` — prose quantity matched no structured field
- coverage: `19°C` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field
- coverage: `30 g.` — prose quantity matched no structured field
- coverage: `60 g.` — prose quantity matched no structured field
- coverage: `120°C` — prose quantity matched no structured field

## sciencemadness:brauer:mnf2 (pp. 262-263)
- coverage: `2.3°C` — prose quantity matched no structured field
- coverage: `17.5°C` — prose quantity matched no structured field
- coverage: `15°C` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field
- coverage: `600°C` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `130 c` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field

## sciencemadness:brauer:k2mnf6 (p. 264)
- WARN [formula] `KCl` — formula not found in page text after normalization (step 4522115); likely OCR-mangled in source — human review
- WARN [formula] `K2MnF6` — formula not found in page text after normalization (step 4522115); likely OCR-mangled in source — human review
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `130 c` — prose quantity matched no structured field
- coverage: `35 mm` — prose quantity matched no structured field
- coverage: `125°C` — prose quantity matched no structured field

## sciencemadness:brauer:fef2 (p. 266)
- WARN [formula] `FeCl2` — formula not found in page text after normalization (step 4522116); likely OCR-mangled in source — human review
- coverage: `35 mm` — prose quantity matched no structured field
- coverage: `125°C` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field

## sciencemadness:brauer:fef3 (pp. 266-267)
- WARN [formula] `FeCl3` — formula not found in page text after normalization (step 4522118); likely OCR-mangled in source — human review
- coverage: `35 mm` — prose quantity matched no structured field
- coverage: `125°C` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field
- coverage: `75°C` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field

## sciencemadness:brauer:cof2 (p. 267)
- coverage: `1000°C` — prose quantity matched no structured field
- coverage: `1000°C` — prose quantity matched no structured field
- coverage: `75°C` — prose quantity matched no structured field

## sciencemadness:brauer:cof3-m2 (p. 268)
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `75°C` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `275°C` — prose quantity matched no structured field

## sciencemadness:brauer:k2nif6 (p. 269)
- WARN [formula] `KCl` — formula not found in page text after normalization (step 4522123); likely OCR-mangled in source — human review
- coverage: `75°C` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `240°C` — prose quantity matched no structured field
- coverage: `196°C` — prose quantity matched no structured field
- coverage: `78°C` — prose quantity matched no structured field
- coverage: `196°C` — prose quantity matched no structured field
- coverage: `196°C` — prose quantity matched no structured field

## sciencemadness:brauer:taf5 (pp. 255-256)
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `111.2°C` — prose quantity matched no structured field
- coverage: `19°C` — prose quantity matched no structured field
- coverage: `300°C` — prose quantity matched no structured field
- coverage: `120°C` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `600°C` — prose quantity matched no structured field
- coverage: `1200°C` — prose quantity matched no structured field

## sciencemadness:brauer:ref6 (pp. 264-265)
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `130 c` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field
- coverage: `1000°C` — prose quantity matched no structured field
- coverage: `1000°C` — prose quantity matched no structured field

## sciencemadness:brauer:kbrf4 (pp. 237-238)
- WARN [formula] `KCl` — formula not found in page text after normalization (step 4522129); likely OCR-mangled in source — human review
- coverage: `500°C` — prose quantity matched no structured field
- coverage: `46°C` — prose quantity matched no structured field
- coverage: `150°C` — prose quantity matched no structured field
- coverage: `1 g.` — prose quantity matched no structured field
- coverage: `100 g.` — prose quantity matched no structured field
- coverage: `5 mm` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field
- coverage: `2 g.` — prose quantity matched no structured field
- coverage: `50°C` — prose quantity matched no structured field
- coverage: `48 hours` — prose quantity matched no structured field

## sciencemadness:brauer:uf6 (p. 262)
- coverage: `2.3°C` — prose quantity matched no structured field
- coverage: `17.5°C` — prose quantity matched no structured field
- coverage: `15°C` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field
- coverage: `110°C` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field
- coverage: `250°C` — prose quantity matched no structured field

## sciencemadness:brauer:wf6 (p. 260)
- WARN [formula] `WF6` — formula not found in page text after normalization (step 4522134); likely OCR-mangled in source — human review
- coverage: `15 mm` — prose quantity matched no structured field
- coverage: `2.3°C` — prose quantity matched no structured field
- coverage: `17.5°C` — prose quantity matched no structured field
- coverage: `15°C` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field
- coverage: `400°C` — prose quantity matched no structured field

## sciencemadness:brauer:h2sx-crude (pp. 346-347)
- WARN [formula] `Na2S·9H2O` — formula not found in page text after normalization (step 4522135); likely OCR-mangled in source — human review
- WARN [formula] `H2O` — formula not found in page text after normalization (step 4522135); likely OCR-mangled in source — human review
- WARN [formula] `H2O` — formula not found in page text after normalization (step 4522136); likely OCR-mangled in source — human review
- WARN [formula] `P2O5` — formula not found in page text after normalization (step 4522137); likely OCR-mangled in source — human review
- coverage: `600°C` — prose quantity matched no structured field
- coverage: `600°C` — prose quantity matched no structured field
- coverage: `18 hours` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `10°C` — prose quantity matched no structured field
- coverage: `5°C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `160 ml.` — prose quantity matched no structured field
- coverage: `480 g.` — prose quantity matched no structured field
- coverage: `64 g.` — prose quantity matched no structured field
- coverage: `32 ml.` — prose quantity matched no structured field

## sciencemadness:brauer:na2s4-m1 (pp. 365-366)
- WARN [formula] `C2H5OH` — formula not found in page text after normalization (step 4522138); likely OCR-mangled in source — human review
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `5.0 g.` — prose quantity matched no structured field
- coverage: `72 ml.` — prose quantity matched no structured field
- coverage: `4.1
g.` — prose quantity matched no structured field
- coverage: `30 minutes` — prose quantity matched no structured field
- coverage: `2.00 g.` — prose quantity matched no structured field

## sciencemadness:brauer:cl2o7-m1 (pp. 304-305)
- WARN [formula] `HClO4` — formula not found in page text after normalization (step 4522143); likely OCR-mangled in source — human review
- WARN [formula] `Cl2O7` — formula not found in page text after normalization (step 4522144); likely OCR-mangled in source — human review
- coverage: `10°C` — prose quantity matched no structured field
- coverage: `15 minutes` — prose quantity matched no structured field
- coverage: `70°C` — prose quantity matched no structured field
- coverage: `25 C` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `90°C` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `40°C` — prose quantity matched no structured field
- coverage: `3 hours` — prose quantity matched no structured field
- coverage: `200°C` — prose quantity matched no structured field
- coverage: `90°C` — prose quantity matched no structured field
- coverage: `2 mm` — prose quantity matched no structured field
- coverage: `78°C` — prose quantity matched no structured field
- coverage: `90°C` — prose quantity matched no structured field
- coverage: `50 g.` — prose quantity matched no structured field
- coverage: `120 ml.` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `8.2 g.` — prose quantity matched no structured field
- coverage: `0°C` — prose quantity matched no structured field
- coverage: `75°C` — prose quantity matched no structured field
- coverage: `80°C` — prose quantity matched no structured field
- coverage: `20°C` — prose quantity matched no structured field
- coverage: `80°C` — prose quantity matched no structured field
- coverage: `2 ml.` — prose quantity matched no structured field
- coverage: `1 g.` — prose quantity matched no structured field
- coverage: `50 ml.` — prose quantity matched no structured field
- coverage: `50°C` — prose quantity matched no structured field
- coverage: `78°C` — prose quantity matched no structured field
- coverage: `7 hours` — prose quantity matched no structured field
