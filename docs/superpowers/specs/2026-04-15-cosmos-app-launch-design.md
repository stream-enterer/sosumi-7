# Cosmos Application Launch Design

**Date:** 2026-04-15
**Status:** Approved

## Goal

Make eaglemode-rs launch a fully functional cosmos application — a faithful clone of C++ Eagle Mode's cosmos view — with only the already-ported plugins (emfileman, emstocks). Audit every emmain file against its C++ counterpart and fix all divergences.

## Success Criteria

- Window opens with starfield background and 3 cosmos items (Home, Root, Stocks1) with borders and titles
- Zooming into FS items shows file manager panels via emfileman
- Stocks item shows stocks panel via emstocks
- Control panel with bookmarks, buttons (New Window, Fullscreen, Reload, Close, Quit)
- Autoplay functional
- Keyboard shortcuts (F4, F5, F11, Escape) working
- All golden tests pass except the 4 pre-existing failures
- No new clippy warnings

## Cosmos Items Included

| Item | VcItem Config | Content File | Plugin |
|------|--------------|--------------|--------|
| Home | Home.emVcItem | Home.emFileLink | emfileman (emFileLinkFpPlugin → emDirEntryPanel) |
| Root | Root.emVcItem | Root.emFileLink | emfileman (emFileLinkFpPlugin → emDirEntryPanel) |
| Stocks1 | Stocks1.emVcItem | Stocks1.emStocks | emstocks (emStocksFpPlugin) |

## Out of Scope

Chess, Clock, Fractal, Mines, Netwalk, OSM, image viewers, PDF, text viewer — no Rust plugin crates exist for these.

## Approach

Audit-driven, bottom-up verification. The code is already substantially ported — the full panel hierarchy, plugin system, config loading, and rendering pipeline are implemented. This is primarily an audit-and-fix task.

1. Audit each emmain file against its C++ counterpart, method-by-method
2. Audit plugin wiring (emFpPlugin loading, symbol resolution, panel creation)
3. Audit config file loading (VcItem and FpPlugin parsing)
4. Fix all issues found — faithful C++ port, no "improvements"
5. Run golden tests continuously — no new failures at any point

## File Audit Matrix

### emmain crate

| Rust file | C++ file | Scope |
|-----------|----------|-------|
| emMain.rs | emMain.cpp | Entry point, IPC server, NewWindow/ReloadFiles |
| emMainWindow.rs | emMainWindow.cpp | Window creation, shortcuts, StartupEngine state machine |
| emMainPanel.rs | emMainPanel.cpp | Control/content split, slider, staged creation |
| emMainContentPanel.rs | emMainContentPanel.cpp | Cosmos panel instantiation, eagle logo rendering |
| emMainControlPanel.rs | emMainControlPanel.cpp | Button creation, bookmark panel, layout |
| emVirtualCosmos.rs | emVirtualCosmos.cpp | VcItem loading, model, item panel, cosmos panel |
| emStarFieldPanel.rs | emStarFieldPanel.cpp | Star rendering, recursive depth |
| emBookmarks.rs | emBookmarks.cpp | Model loading, panel rendering, navigation |
| emAutoplay.rs | emAutoplay.cpp | Animation engine, goal management |
| emMainConfig.rs | emMainConfig.cpp | Config model |

### Plugin wiring

| File | Scope |
|------|-------|
| emcore/emFpPlugin.rs | Plugin loading, CreateFilePanel routing |
| emfileman/emDirFpPlugin.rs | Dir plugin function, panel creation |
| emfileman/emFileLinkFpPlugin.rs | FileLink plugin function |
| emstocks/emStocksFpPlugin.rs | Stocks plugin function |
| emcore/emInstallInfo.rs | Path resolution (EM_DIR, config dirs) |

Each audit produces: **matches C++** or **specific fix with line references**.

## Config File Strategy

Ship defaults + load user overrides (matching C++ behavior).

**Default configs** (already exist in `etc/`):
- `etc/emMain/VcItems/` — Home.emVcItem, Root.emVcItem, Stocks1.emVcItem
- `etc/emMain/VcItemFiles/` — Home.emFileLink, Root.emFileLink
- `etc/emCore/FpPlugins/` — emDir, emDirStat, emFileLink, emStocks

**Resolution order** (via existing `emGetConfigDirOverloadable()`):
1. User config: `~/.eaglemode/config/<prj>/`
2. Host config: `$EM_DIR/etc/<prj>/`

No new config files or resolution code needed.

## Testing Strategy

Golden tests are the authority.

**Verification gates:**
1. Before any code changes: run full golden suite, confirm exactly 4 failures (baseline)
2. After each audit/fix file: run golden suite, confirm no new failures
3. After all fixes complete: full suite + clippy + nextest

**No new golden tests** for the cosmos itself — it integrates already-tested components.

**Regression risk:** Changes to Paint methods, layout math, or input handling could break golden tests. Mitigation: small targeted fixes, test after each file.

## What Will NOT Be Done

- Rewrite working code for style
- Add features beyond C++ parity
- Touch rendering pipeline internals
- Port new plugins
- Add error handling beyond what C++ has
