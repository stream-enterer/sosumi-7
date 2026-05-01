# Plugin Cdylib ABI Trap

## Symptom

Running `cargo run -p eaglemode` (or any per-package run) produces a panic of the form:

```
thread 'main' panicked at crates/emcore/src/emPanelTree.rs:756:45:
invalid SlotMap key used
...
   2: emcore::emPanelTree::PanelTree::remove
   3: <emMain::emMainWindow::ControlPanelBridge as emcore::emEngine::emEngine>::Cycle
...
free(): invalid pointer
```

triggered by user actions that exercise plugin-supplied panels (e.g. zoom-in/zoom-out in the file manager, opening a TestPanel cosmos entry).

The panic site, the garbage-looking `PanelId(<huge u32>v<huge u32>)`, and the trailing `free(): invalid pointer` are all **downstream** of the actual bug.

## Root cause

The eaglemode binary's plugin loader calls `dlopen()` on cdylib `.so` files at runtime (via `emFpPlugin`). Plugin behaviors are returned as `Box<dyn PanelBehavior>`. Calls into those behaviors dispatch through the trait's vtable.

Vtable layout is determined at *compile time* by the `PanelBehavior` trait definition in `emcore`. If a plugin cdylib was compiled against an **older** `PanelBehavior` (e.g., before a method was added or reordered), its vtable encodes the old layout. The eaglemode binary, compiled against the current trait, looks up methods by the *new* offsets. The dispatch lands on the wrong slot — possibly returning data that looks structurally valid (e.g., `Some(PanelId)` shape) but is actually the return of an unrelated method. From that point, every downstream consumer sees garbage.

Specifically, `<TestPanel as PanelBehavior>::CreateControlPanel`'s vtable slot would be filled with the address of *some other* method in the trait, returning whatever that method returns interpreted as `Option<PanelId>`.

## Why the trap exists

`crates/eaglemode/Cargo.toml` has a comment:

> Plugin cdylibs. Each plugin crate must be listed here so cargo builds it
> for all profiles (including --release). The binary finds the .so at runtime
> via RUNPATH=$ORIGIN:$ORIGIN/deps (set in build.rs). When adding a new plugin
> crate, add it here.

Plugins listed under `[dependencies]` are tracked by cargo: any change to `emcore`'s public surface invalidates them, forcing a rebuild on the next `cargo run`. Plugins that are *not* listed (or only listed under `[dev-dependencies]`) are **not tracked** by `cargo run -p eaglemode`. Their `.so` artifacts persist in `target/{profile}/` from whatever the last full-workspace build produced — possibly weeks or commits in the past — and continue to be `dlopen`'d by the running binary, with the old vtable layout.

The `emtest` crate (which compiles to `libemTestPanel.so`) was historically only in `[dev-dependencies]`. This made it a silent ABI bomb: any commit that touched the `PanelBehavior` trait would render the on-disk `libemTestPanel.so` ABI-incompatible until somebody happened to run a workspace-wide build. In the meantime, every developer running `cargo run -p eaglemode` would see seemingly-random panics whose backtraces all pointed at innocent code in `emcore`.

## Why this looked like a real bug

Multiple symptoms reinforced the false hypothesis that there was a logic defect in `create_control_panel_in` or in some plugin's `CreateControlPanel` impl:

1. The panic was **deterministic** — same user actions, same crash. (Because the plugin's `.so` was always equally stale.)
2. The garbage `PanelId` values **changed across runs**. (Because each ABI mismatch picked a different wrong slot, returning different unrelated method-return data.)
3. `free(): invalid pointer` after the panic looked like **classic heap corruption**. (It was — the ABI mismatch corrupted Box destruction.)
4. Adding `eprintln!` probes to plugin-side `CreateControlPanel` impls **did not fire**. (Because the plugin's `.so` was stale; the rebuilt source never made it into the loaded library.)
5. Earlier "fixes" (IsActive guards, etc.) were **applied to source files belonging to the plugin** — which then weren't rebuilt — so the fixes appeared to do nothing.

## How it was diagnosed

The diagnostic that broke the loop was a mechanical one:

```bash
ls -la target/debug/libemTestPanel.so crates/emtest/src/emTestPanel.rs
```

The `.so` was dated *before* the most recent edits to its source. That ruled out every "the code does X wrong" hypothesis in one step: the running binary did not contain the user's recent edits at all.

A `dladdr()` probe on the trait object's vtable pointer (no dispatch, just a loader-table lookup) had previously confirmed that the vtable lived in `libemTestPanel.so`. Combined with the timestamp mismatch, the conclusion was forced.

## The fix

Move `emtest` from `[dev-dependencies]` to `[dependencies]` in `crates/eaglemode/Cargo.toml`. This makes cargo rebuild it whenever any of its inputs (notably `emcore`) changes.

```toml
[dependencies]
emcore = { workspace = true }
emmain = { path = "../emmain" }
winit = { workspace = true }
emfileman = { path = "../emfileman" }
emstocks = { path = "../emstocks" }
emtest = { path = "../emtest" }    # ← was [dev-dependencies] only
```

A permanent invariant assert was also added to `create_control_panel_in` (`emPanelTree.rs`) — if a `CreateControlPanel` impl ever returns a `PanelId` that belongs to neither `target_tree` nor the content tree, the assert fires with a message pointing at this document. Future ABI drift will trip the assert immediately rather than producing the long downstream panic chain.

## Preventing recurrence

Three layers of defense, in order of cost:

1. **Cargo dependency listing (current fix).** Every new plugin cdylib MUST be added to `eaglemode`'s `[dependencies]`. The Cargo.toml comment already says this. Honor it.
2. **The audit assert in `create_control_panel_in`.** Catches a specific class of ABI drift symptom (out-of-tree returned `PanelId`). Not a general guarantee but a useful tripwire.
3. **CI workspace build.** If CI builds the workspace, ABI drift on `emcore`'s public surface is caught at PR time — even before runtime. The pre-commit hook already runs `cargo-nextest ntr` which builds the workspace, so this is implicitly in place; preserve it.

A more general future improvement would be a build-time check that the trait method count and offsets in every plugin cdylib match what `emcore` exports. That is out of scope for this investigation.

## Resolution commits

- Cargo.toml fix: see commit message containing `eaglemode/Cargo.toml: list emtest in [dependencies]`
- Audit assert: in `crates/emcore/src/emPanelTree.rs::create_control_panel_in`
- Investigation facts: `docs/debug/investigations/isactive-panic-facts.md`

## Lessons for future debugging

- When a `Box<dyn Trait>`-backed call produces garbage but the trait object's metadata (vtable pointer, data pointer) looks sane, **suspect the build, not the call**. dladdr the vtable and timestamp the `.so`.
- A panic message and backtrace that all live in trustworthy in-tree code, while no in-tree edits ever change behavior, is a tell that *the running code is not the in-tree code*.
- The `IsActive` guard fixes (commits b5dad89b and 22c11333) made on this branch did not fix the original panic — they could not have, because they live in plugin source that wasn't being rebuilt. They may still be valid C++-fidelity improvements; evaluate them on their own merits.
