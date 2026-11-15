# Round 7 — color lens

**Score: 7/10** — color is recognizably close, but the halo is still materially too neutral; 10 means indistinguishable from the reference.

| Check | Measured / evidence | Target | Result |
|---|---|---|---|
| Hot core | RGB **(172,235,239)**; R/G **0.732** (from final measurement; render max is (173,236,239)) | Ref deep-measure core ≈(171,239,238), R/G ≈0.72; no white | **PASS** — near-exact hue; slightly dimmer green, no white plateau (0 px ≥230 in the explicit measurement). |
| Halo hue | R/G **0.783** | **0.57–0.65** | **FAIL** — visibly too neutral/gray; strongest color mismatch. |
| End dots | B−G **+40/+40** | Ref ends +26…+30 or more blue than green | **PASS** — steel-blue bias is present and no longer neon. |
| Body | Target RGB **(18,23,33)**; final measurement lists that body target, but no isolated body-pixel RGB was provided | (18,23,33) | **UNVERIFIED** — visible body reads near target on navy; do not infer an exact pass from the composite. |
| Rim / crease / specular | Visible subtle edge and interior shading in render; reference deep-measure finds **no** bright keyline/specular or bottom line | Soft dark crease; no bright rim/specular | **PASS, restrained** — current edge is subdued; avoid adding bright glass accents. |
| Tail | Current render visibly transitions teal to blue; end-dot B−G +40 | Ref tail B/G rises to 1.14–1.42, end B−G ≥+26 | **PASS** — blue is controlled and within the measured end-dot floor. |

## Top fixes by visual impact

1. **Tint halo much more teal before compositing**: lower halo R/G from 0.783 into 0.57–0.65, while preserving the (172,235,239) core. This is the clearest remaining color difference.
2. **Verify body RGB from interior-only pixels**, excluding shadow, edge AA, and glow; target (18,23,33). If it misses, adjust body shading without introducing a bright keyline.
3. **Keep tail hue symmetric and steel-blue**; current +40 B−G is safely above the reference's +26…+30 floor, so avoid further blue amplification.

**Already right:** the hot core is teal rather than white, and the end dots have a restrained blue bias instead of the prior neon cast.
