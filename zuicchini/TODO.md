# Deferred Items

Tracked here so they don't get forgotten. Sourced from EMCORE_FEATURE_CONTRACT.md.

## View Animators

- [ ] `SwipingViewAnimator` — touch-drag with spring physics and momentum (needs touch input infrastructure)
- [ ] `MagneticViewAnimator` — snaps view to "best" panel alignment (needs working UI for tuning)

## Widgets

- [ ] `FileSelectionBox` — file browser (only if game needs file open/save)
- [ ] `FileDialog` — wraps FileSelectionBox in a dialog window
- [ ] `CoreConfigPanel` — core settings editor (needs config system fully working)
- [ ] `ErrorPanel` — simple error text display (small effort, useful for debugging)

## Structural Refactors

- [x] Restrict PanelData field visibility — make computed fields (`enabled`, `pending_notices`) and tree-managed fields (`parent`, `first_child`, etc.) non-public after the fix pass settles their access patterns

## Rendering

- [ ] Multi-threaded tile rasterization — parallelize independent dirty tiles across threads (benchmark-driven, threading boundary is well-defined)
- [ ] Sub-pixel AA for non-text operations — route rects/gradients/images through polygon rasterizer with Fixed12 edge coverage; add axis-aligned rect fast path; fix coverage rounding in `make_span` (edge seams) and clamp minimum coverage for thin rects. See `.workflow/dialectic/convergence_ledger.md` for full analysis
- [ ] Glyph content sub-pixel positioning — edge coverage solves rect wiggle but glyphs remain pixel-snapped inside their bounding box. Investigate SDF rendering, boundary-only bilinear, or accept integer glyph positioning as C++ Eagle Mode does
- [ ] 4K paint profiling — `bench_interaction 3840 2160` to check if paint exceeds 16ms budget; if so, scanline rasterizer needs optimization
- [ ] Glyph rasterization cost under complex panel trees — single TestPanel is cheap, but multiple panels with diverse text sizes may stress the glyph cache LRU eviction path

## Font System Follow-ups

- [x] Hinted rasterization — skrifa's `HintingInstance` requires per-size instances; currently using `DrawSettings::unhinted`. Add hinting for crisper text at small sizes (no API changes needed)
- [ ] Thread FontCache through PanelBehavior/PanelCtx — when widgets start implementing `PanelBehavior::preferred_size` via the trait (not just inherent methods), the trait signature and PanelCtx need `&FontCache`
- [ ] Variable font weight selection — Inter Variable is embedded but always renders at default weight; expose weight axis via `skrifa::instance::Location`
- [x] Text scroll in TextField — `scroll_x` updated in `paint()` to keep cursor visible
- [ ] i18n shaping verification — rustybuzz handles Arabic/Devanagari/CJK but needs testing with actual multilingual text
