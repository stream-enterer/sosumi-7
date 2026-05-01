# B-013-dialog-cells-emstocks — Design

**Bucket:** B-013-dialog-cells-emstocks
**Pattern (post-correction):** P-004-rc-shim-accessor-present (was P-005-rc-shim-no-accessor; corrected by this brainstorm — see Audit-data corrections)
**Scope:** `emstocks` (4 rows, all in `crates/emstocks/src/emStocksListBox.rs`)
**Cited decisions:** D-002-rc-shim-policy (rule 1, convert — trigger side), D-006-subscribe-shape (per-dialog first-Cycle init), D-004-stocks-application-strategy (mechanical application across all in-bucket rows)
**Prereq buckets:** none
**New global decisions:** none

---

## 1. Audit-data corrections

The bucket sketch's framing was wrong. The C++ source uses signal-subscribe + sync-result-read (`AddWakeUpSignal(Dialog->GetFinishSignal())` + `IsSignaled(...) + Dialog->GetResult()`), not post-finish member-field assignment. The Rust port's `finish_signal: SignalId` accessor on `emDialog` (`crates/emcore/src/emDialog.rs:55`) is already present, so accessor-status is not missing.

Per-row triage:

| ID | C++ pattern (verified) | Rust today | D-002 rule | Disposition |
|---|---|---|---|---|
| emStocksListBox-189 | `AddWakeUpSignal(CutStocksDialog->GetFinishSignal())` (cpp:189) + Cycle: `IsSignaled(...) + GetResult()` (cpp:656-657) | `set_on_finish` writes `cut_stocks_result: Rc<Cell<Option<DialogResult>>>`; Cycle polls `cell.take()` as trigger | **Rule 1 — convert (trigger side)** | Subscribe to `dialog.finish_signal`; replace `cell.take()` polling with `IsSignaled(dialog.finish_signal)` as the trigger; cell stays as delivery buffer |
| emStocksListBox-287 | Same shape, Paste (cpp:287, 662-663) | Same shape, `paste_stocks_result` | **Rule 1 — convert (trigger side)** | Same |
| emStocksListBox-356 | Same shape, Delete (cpp:356, 668-669) | Same shape, `delete_stocks_result` | **Rule 1 — convert (trigger side)** | Same |
| emStocksListBox-443 | Same shape, Interest (cpp:443, 674-676) | Same shape, `interest_result` | **Rule 1 — convert (trigger side)** | Same |

Audit-data updates (working-memory session):
- All 4 rows: `pattern_id` P-005 → P-004; `accessor_status` missing → present.
- D-002 "Affects" line: P-004 +4, P-005 -4.
- B-013 reconciliation log entry: third audit-data accessor-status correction (cf. B-006/B-007/B-008 gap-blocked fixes). The audit's automated heuristic missed the inherited/composed `finish_signal` accessor on `emDialog`. Pattern is established but not promoted to a decision — known audit-data-quality issue, not a design choice.

## 2. Pattern split: trigger vs delivery

The drift in B-013 separates into two halves:

**Trigger half (rule-1, converted by this design).** C++ uses `IsSignaled(GetFinishSignal())` to wake the consumer when the dialog finishes. Rust today uses `cell.take().is_some()` polling — observable drift. Fix: subscribe to `dialog.finish_signal` and use `IsSignaled` as the trigger.

**Delivery half (idiom adaptation, no annotation).** C++ reads the result via the synchronous `Dialog->GetResult()` member-field accessor. Rust's `emDialog` does not expose a synchronous post-show result reader (the post-show state lives in App's dialog registry; only `App::mutate_dialog_by_id`-style deferred mutation is currently exposed). The Rust port delivers the result via the `set_on_finish` callback writing into a `pub(crate) Rc<Cell<Option<DialogResult>>>` field on `emStocksListBox`. The cell is below the observable surface — its role is to bridge `DialogPrivateEngine::Cycle`'s scope (where `DlgPanel.finalized_result` is in scope) to the consumer panel's scope. The observable timing is identical to C++:

| Step | C++ | Rust (this design) |
|---|---|---|
| Dialog finalizes result | `DlgPanel.FinishState=2; Result=...` | `dlg.finalized_result = Some(...)` |
| Result observable | `Dialog->GetResult()` | (cell filled by on_finish in same Cycle body) |
| Subscriber notified | `Signal(FinishSignal)` → `IsSignaled(...)` true on next slice | `sched.fire(finish_signal)` → consumer's `IsSignaled(dialog.finish_signal)` true on next slice |
| Subscriber reads result | `Dialog->GetResult()` | `cell.take()` |

`on_finish` runs in the same `DialogPrivateEngine::Cycle` body that calls `fire(finish_signal)` (cf. `emDialog.rs:976-986` and the comment chain at line 456: "fire(finish_signal) → invoke on_finish sequence"). The cell is therefore guaranteed to be populated by the time any subscriber observes `IsSignaled` true on the following slice.

Per Port Ideology, the cell is idiom adaptation forced by emDialog's callback-only post-show API — but the absence of `emDialog::GetResult()` is itself a project-internal architectural choice (App owns dialogs in a registry; handle-side state goes pending-only). Idiom adaptation forced by a project-internal ownership choice is NOT a valid forced-category framing for `DIVERGED:`. The cell field is annotated only by a one-line informational `//` comment explaining its role; no `DIVERGED:` block.

## 3. Implementation

### 3.1 `emStocksListBox` field additions

Add 4 single-byte flags for per-dialog first-Cycle-init:

```rust
pub(crate) cut_subscribed: bool,
pub(crate) paste_subscribed: bool,
pub(crate) delete_subscribed: bool,
pub(crate) interest_subscribed: bool,
```

Initialize to `false` in `new()`.

The 4 existing `Rc<Cell<Option<DialogResult>>>` fields (`cut_stocks_result`, `paste_stocks_result`, `delete_stocks_result`, `interest_result`) **stay**. Add a single one-line comment above the cluster (lines 53-57) noting their role as delivery buffers from `set_on_finish` callbacks.

### 3.2 Mutator-creation sites — minimal changes

**Signatures unchanged.** `CutStocks`, `PasteStocks`, `DeleteStocks`, `SetInterest` keep their `<C: ConstructCtx>` generic signature. No connect call here — the connect moves to `Cycle` per D-006 first-Cycle-init.

**Per mutator, in the cancel-old-dialog branch (Finding I-2):** before pushing the close action onto `pending_actions`, **explicitly disconnect the old dialog's `finish_signal`** from the parent engine, then clear the subscribed flag and the result cell. Symmetric with the confirmed-branch disconnect in §3.3 — without this, the parent engine retains a live `(old_finish_signal → parent_engine)` connection until the old SignalId is reaped by the dialog teardown path, a slow leak. Concretely:

```rust
// Cancel-old-dialog branch (per mutator):
if let Some(old) = self.cut_stocks_dialog.as_ref() {
    if self.cut_subscribed {
        ectx.disconnect(old.finish_signal, ectx.id());
    }
}
self.cut_subscribed = false;
self.cut_stocks_result.set(None);
// ... existing pending_actions push to close the old dialog ...
```

The new dialog will re-subscribe on its first Cycle observation per §3.3.

**Per mutator, after `self.<dlg>_dialog = Some(dialog);`:** add `self.<dlg>_subscribed = false;` (defensive — covers the no-prior-dialog case where the cancel-old branch did not run).

The `set_on_finish` closure that writes the cell **stays unchanged**. The `dialog.show(cc)` call stays.

### 3.3 `Cycle` — signature and per-dialog block

**Signature change:** `pub fn Cycle<C: emcore::emEngineCtx::ConstructCtx>(&mut self, cc: &mut C, ...)` → `pub fn Cycle(&mut self, ectx: &mut emcore::emEngineCtx::EngineCtx<'_>, ...)`. Required because `IsSignaled` and `connect`/`disconnect` are not on `ConstructCtx`. The single production caller (`emStocksFilePanel.rs:380`) already passes `ectx: &mut EngineCtx<'_>` — no caller change needed. Tests at `emStocksListBox.rs:1171,1185,1201,1318` only invoke the four mutators (with `ask=false`, no Cycle invocation under dialog state); they remain unchanged.

**Engine-identity note (Finding I-1).** In C++, `class emStocksListBox : public emListBox` (verified at `~/Projects/eaglemode-0.96.4/src/emStocks/emStocksListBox.h:29`) — the ListBox is its own `emEngine`, so `AddWakeUpSignal(...)` self-subscribes the ListBox engine. In the Rust port, `emStocksListBox` is **not** a registered engine; it is a plain member of `emStocksFilePanel`, whose `Cycle` (`crates/emstocks/src/emStocksFilePanel.rs:380`) calls `lb.Cycle(ectx, ...)` while still holding the FilePanel's `EngineCtx`. Therefore inside `lb.Cycle`, `ectx.id()` resolves to the **FilePanel** engine, not the ListBox. The `connect`/`disconnect` calls below subscribe the parent FilePanel engine to the dialog's `finish_signal`. This is observably correct — the FilePanel's `Cycle` is what reaches the polling block, so waking the FilePanel is exactly what causes the polling block to re-run on the next slice. The structural shift (composite host instead of inheritance) is preserved-design-intent of the Rust port; no `DIVERGED:` annotation, but the substitution of "subscribe the parent engine" for C++'s "subscribe self" is documented here so a reader of the snippet does not mistake it for a 1:1 mirror of `AddWakeUpSignal`.

**Per-dialog block** (Cut shown; Paste/Delete/Interest are mechanical clones):

```rust
// Poll cut dialog.
// Invariant: subscribe-then-check happens on the same Cycle slice; do not
// split this block across two methods or two slices, or a fire could be
// missed (cf. Adversarial Review Note 7).
if let Some(sig) = self.cut_stocks_dialog.as_ref().map(|d| d.finish_signal) {
    // `sig` is Copy (SignalId); the immutable borrow of self.cut_stocks_dialog
    // ends at the end of this let-binding, so the &mut self uses below compile.
    if !self.cut_subscribed {
        ectx.connect(sig, ectx.id()); // subscribes the parent FilePanel engine
        self.cut_subscribed = true;
    }
    if ectx.IsSignaled(sig) {
        let confirmed =
            self.cut_stocks_result.take() == Some(DialogResult::Ok);
        ectx.disconnect(sig, ectx.id());
        self.cut_stocks_dialog = None;
        self.cut_subscribed = false;
        if confirmed {
            self.CutStocks(ectx, rec, false); // ectx is ConstructCtx; OK
        }
        // else-side cleanup is per-mutator: see §3.3a for the Interest variant.
    } else {
        // Outer guard is Some(dialog); inner if/else distinguishes
        // "signaled (consume)" from "still pending (busy)".
        busy = true;
    }
}
```

**§3.3a — Interest-block cancel-side cleanup (Finding I-5).** The Interest block today (`crates/emstocks/src/emStocksListBox.rs:782-784`) clears `self.interest_to_set = None` whenever the dialog finishes with non-Ok. That semantics must be preserved verbatim. The Interest per-dialog block therefore reads:

```rust
if ectx.IsSignaled(sig) {
    let confirmed =
        self.interest_result.take() == Some(DialogResult::Ok);
    ectx.disconnect(sig, ectx.id());
    self.interest_dialog = None;
    self.interest_subscribed = false;
    if confirmed {
        if let Some(interest) = self.interest_to_set.take() {
            // existing SetInterest(ectx, rec, interest, false) call
        }
    } else {
        self.interest_to_set = None; // preserve cancel-side reset
    }
}
```

Notes:
- `ectx.connect(sig, ectx.id())` mirrors C++ `AddWakeUpSignal(GetFinishSignal())`. The connect is deferred from the C++ "at dialog creation" site to the parent's first Cycle observation of `Some(dialog)` per D-006 wiring shape (justified by `DialogPrivateEngine::Cycle` taking ≥2 of its own cycles to reach `fire(finish_signal)` — cf. `emDialog.rs:850-860` — guaranteeing the parent gets at least one Cycle to land the connect before any fire could be observed).
- `ectx.IsSignaled(dialog.finish_signal)` mirrors C++ `IsSignaled(Dialog->GetFinishSignal())`.
- `self.cut_stocks_result.take()` reads the result delivered by the `on_finish` callback (which ran inside `DialogPrivateEngine::Cycle` at the same time that `fire(finish_signal)` was issued). The synchronous read at this point is sound by the dialog-cycle ordering described in §2.
- `ectx.disconnect(...)` cleans up the subscription before the dialog handle drops. Hygiene; the `SignalId` belongs to a dialog being torn down.
- `self.cut_subscribed = false;` resets the per-dialog flag so the next dialog (if any) starts fresh.
- The recursive `self.CutStocks(ectx, rec, false)` works because `EngineCtx` implements `ConstructCtx` (`emEngineCtx.rs:280`); no signature change to `CutStocks`.

Apply identically to Paste (`paste_stocks_*`), Delete (`delete_stocks_*`), and Interest (`interest_*`) blocks. The Interest block additionally reads `self.interest_to_set.take()` inside the `confirmed` branch as today.

The `else if self.<dlg>_dialog.is_some() { busy = true; }` branches in today's code collapse into the `else { busy = true; }` of the new shape.

### 3.4 No emcore changes

`emDialog` is not modified. No `result()` accessor added. No App-registry inspection path added. The trigger drift is closed; the delivery-buffer pattern is preserved as idiom adaptation.

## 4. Cited decisions (with rule applied per row)

- **D-002 rule 1** (convert): all 4 rows. Trigger side only — see §2 for the trigger/delivery split.
- **D-006**: per-dialog first-Cycle init for the `connect` call, adapted to lazily-created subscribables (4 per-dialog `subscribed: bool` flags rather than one panel-wide flag).
- **D-004**: mechanical application of the design across all 4 in-bucket rows.

D-001 not cited — `emDialog.finish_signal` is already `SignalId`; no flip needed.
D-007 not cited — no model accessor flips; mutators are synchronous already and `emDialog`'s own `fire(finish_signal)` is implemented.
D-008 not cited — no new model `SignalId` is allocated; the dialog's signal is allocated inside `emDialog::new` via the existing path.

## 5. Watch-list (not a decision)

**Candidate framework lift: synchronous post-show `emDialog::GetResult()`.** emDialog's port lacks a synchronous post-show result reader equivalent to C++ `Dialog->GetResult()`. The result is delivered exclusively via the `set_on_finish` callback, which forces every dialog consumer (this bucket plus emfileman, emmain, emFileDialog, etc.) to provide some delivery channel — typically an `Rc<Cell<...>>` or equivalent shared state — to bridge the dialog-engine scope to the consumer scope. Closing this gap would add `App::inspect_dialog_by_id<R>(id, |dlg| ...) -> R` (mirror of the existing `App::mutate_dialog_by_id`) plus a public `emDialog::result(&self) -> Option<DialogResult>` reader that consults App for post-show dialogs.

If this lift lands as a future bucket, B-013's residual cells become drop candidates: trigger-side already converted; delivery-side could be replaced with the new sync reader. Same shape as D-008's A3 watch-list note — promote when enough consumers accumulate the shim, not now.

**Scope tightening (Note 8).** This watch-list note is scoped to **confirmation-style dialogs** (Cut/Paste/Delete/Interest in B-013, plus analogous yes/no consumers) where the consumer needs a synchronous result read at the moment of signal observation. It does **not** extend to long-running dialogs that already drive their own Cycle (e.g. `emStocksFetchPricesDialog`) or to emFileDialog (cf. B-018 design line 65, which explicitly states the B-013 architectural concern does not generalize to emFileDialog). Avoids over-broad future scope.

## 6. Implementer's checklist

1. `crates/emstocks/src/emStocksListBox.rs`:
   - Add 4 `pub(crate) <dlg>_subscribed: bool` fields to the struct.
   - Initialize them to `false` in `new()`.
   - Add a one-line `//` comment above the existing `*_result` cell cluster noting their role as delivery buffers from `on_finish`.
   - In each of the 4 ask=true mutator branches: in the cancel-old-dialog block, call `ectx.disconnect(old.finish_signal, ectx.id())` (guarded by the old `<dlg>_subscribed` flag) **before** pushing the close action, then set `self.<dlg>_subscribed = false;` next to `result.set(None)` (Finding I-2). After `self.<dlg>_dialog = Some(dialog);` set `self.<dlg>_subscribed = false;` defensively.
   - Change `Cycle` signature to `(&mut self, ectx: &mut emcore::emEngineCtx::EngineCtx<'_>, rec: &mut emStocksRec, config: &emStocksConfig) -> bool`.
   - Replace each of the 4 per-dialog blocks in `Cycle` with the connect+IsSignaled+disconnect shape from §3.3 (use the `let Some(sig) = ...map(|d| d.finish_signal)` borrow-extraction pattern shown).
   - **Interest block specifically:** preserve the existing `else { self.interest_to_set = None; }` cancel-side reset (§3.3a, Finding I-5). Cut/Paste/Delete have no analogous cancel-side state.
   - Confirm recursive `self.<Mutator>(ectx, ...)` calls still type-check (EngineCtx: ConstructCtx).

2. `crates/emstocks/src/emStocksFilePanel.rs:380` already passes `ectx`. No change.

3. Tests at `emStocksListBox.rs:1171,1185,1201,1318` use `ask=false` and never invoke `Cycle` under dialog state. No test changes required.

4. Verify with `cargo check` then `cargo clippy -- -D warnings` then `cargo-nextest ntr`.

5. Pre-commit hook expected to pass cleanly.

## 7. Reconciliation summary (for working-memory session)

Working-memory session reconciliation tasks on design return:
- `inventory-enriched.json`: rows 189/287/356/443 → `pattern_id` P-004, `accessor_status` present.
- `decisions.md` D-002 "Affects" line: P-004 +4, P-005 -4.
- B-013 bucket sketch: pattern P-005 → P-004; reframe description as rule-1 convert (trigger side) with cell-as-delivery-buffer note (idiom adaptation, not divergence); cite D-006 alongside D-002 and D-004.
- B-013 reconciliation log: third audit-data accessor-status correction (cf. B-006/B-007/B-008). Heuristic gap noted; not promoted.
- Watch-list note in `decisions.md` D-008 neighborhood (or new watch-list section): "emDialog post-show synchronous `GetResult()` candidate framework lift" — first sighting B-013, affects all dialog consumers.

No prereq edges introduced. No new D-### entries. B-013 status: pending → designed.

## Adversarial Review — 2026-05-01

### Summary
- Critical: 0 | Important: 3 | Minor: 3 | Notes: 2

### Findings

1. **[Important] [§3.3 / engine-identity for connect]** — Design claims `ectx.connect(dialog.finish_signal, ectx.id())` mirrors C++ `AddWakeUpSignal(...)`. **Subtle but correct, and worth documenting.** In C++ `class emStocksListBox : public emListBox` (header line 29) — the ListBox **is** its own `emEngine`, so `AddWakeUpSignal` self-subscribes the ListBox engine. In Rust, `emStocksListBox` is **not** a registered engine; it is a member of `emStocksFilePanel` whose `Cycle` (`emStocksFilePanel.rs:349-387`) calls `lb.Cycle(ectx, …)` (line 380). Therefore inside `lb.Cycle`, `ectx.id()` is the **FilePanel's** engine, not the ListBox's. This still produces correct observable behavior — FilePanel's Cycle is what reaches the polling block, so waking FilePanel is what we want — but the design's "mirrors AddWakeUpSignal" framing elides the structural difference. **Fix:** add an explicit one-paragraph note in §2 or §3.3 stating that the Rust port subscribes the *parent* engine (FilePanel) because the ListBox is not an independent engine, and cite the C++ vs Rust class hierarchy difference. This is preserved-design-intent (FilePanel as composite host) — annotate, don't change.

2. **[Important] [§3.3 / cancel-old subscription leak]** — In the cancel-old-dialog branch (each mutator), the design adds `self.<dlg>_subscribed = false;` next to the existing `self.<dlg>_result.set(None);` (e.g. parallel to `emStocksListBox.rs:495-503`). But the old dialog's `finish_signal` is **still connected to the parent engine** at this point (subscribed last time first-Cycle observation ran). The design relies on "the old dialog's subscription is dropped when the dialog is closed via `pending_actions` and its `finish_signal` no longer fires." Two issues: (a) `app.close_dialog_by_id` finalizes via `Finish(NEGATIVE)` which causes `DialogPrivateEngine` to call `fire(finish_signal)` exactly once (`emDialog.rs:1086`), waking the parent engine *after* `_subscribed=false` and after `<dlg>_dialog = Some(new_dialog)` has overwritten the slot; the parent's polling block observes `Some(new_dialog)` with `subscribed=false`, runs `connect` for the **new** signal, then checks `IsSignaled(new_dialog.finish_signal)` (false). The stale fire on the *old* signal does not enter the new block because we re-read `dialog.finish_signal` from the new dialog. Benign in practice. (b) But the parent engine still holds a live `(old_finish_signal → parent_engine)` connection until the old SignalId is removed (`emDialog`'s drop / scheduler cleanup). If signals are not removed promptly, this is a slow leak. **Fix:** in the cancel-old branch, call `ectx.disconnect(old.finish_signal, ectx.id())` *before* pushing the close action. Symmetric with the "confirmed" branch's disconnect.

3. **[Important] [§3.3 / `else { busy = true }` collapse]** — Design says "the `else if self.<dlg>_dialog.is_some() { busy = true; }` branches collapse into the `else { busy = true; }` of the new shape." That collapse is correct **only if** the `else` is inside `if let Some(dialog) = self.<dlg>_dialog.as_ref()`. The proposed snippet has the `else` attached to `if ectx.IsSignaled(...)`, *inside* the outer `if let Some(dialog)`. So `else { busy = true }` only fires when the dialog exists but hasn't fired yet — exactly C++ shape. Confirmed correct. Risk: an implementer copying mechanically may flatten the structure differently. **Fix:** make the bracket structure explicit in §3.3 (one extra clarifying line, e.g. "outer guard is `Some(dialog)`; inner `if/else` distinguishes signaled vs not-yet").

4. **[Minor] [§3.3 / borrow checker for `dialog.finish_signal`]** — `if let Some(dialog) = self.<dlg>_dialog.as_ref()` borrows `self` immutably; later `self.<dlg>_dialog = None;` and `self.CutStocks(ectx, rec, false)` need `&mut self`. Implementer must extract `let sig = dialog.finish_signal;` (Copy) and drop the immutable borrow before mutation, or use `.as_ref().map(|d| d.finish_signal)` patterns. **Fix:** show the borrow-handling explicitly in the §3.3 snippet to prevent the implementer from hitting compiler errors and improvising structure.

5. **[Minor] [§3.3 / Interest result reset on Cancel]** — Existing code at `emStocksListBox.rs:782-784` resets `self.interest_to_set = None` when result is non-Ok. The design's per-dialog block snippet shows only the Cut shape; the implementer-checklist (§6 row 1.f) doesn't call out preserving the Interest-specific cancel-side cleanup. **Fix:** add an explicit bullet in §6 step 1 noting that the Interest block must keep the existing `else { self.interest_to_set = None; }` semantics (i.e. clear `interest_to_set` whenever the dialog finishes with non-Ok, not only on Ok-then-mutate).

6. **[Minor] [§3.3 / disconnect timing — engine-id under recursion]** — The `confirmed` branch disconnects, sets `<dlg>_dialog = None`, then calls `self.CutStocks(ectx, rec, false)`. If the recursive call (ask=false path) creates another dialog through some indirect route, no harm — but the disconnect uses `ectx.id()` which is the parent engine. Verify that `ectx.id()` is stable across the synchronous recursion (it is: `engine_id` field, `emEngineCtx.rs:245-247`). Note only — no fix needed.

7. **[Note] [Cross-reference B-018 latent gap]** — B-018's latent gap was a `CheckFinish` post-show *else-branch* missing `scheduler.connect`. B-013's analog is the *first-Cycle init* branch (`if !self.<dlg>_subscribed`). That branch is **always taken** before any IsSignaled check on the same Cycle slice — by construction, no else-branch can be skipped. So B-013 does not have a B-018-shaped gap. **However**, the inverse risk exists: if a future refactor splits `Cycle` into two methods such that one runs the first-Cycle init and another runs IsSignaled across a slice boundary, a fire could be missed. Document the invariant ("subscribe-then-check on the same slice") inline in §3.3 to harden against that future refactor.

8. **[Note] [Watch-list / `emDialog::GetResult()`]** — §5 correctly identifies the framework-lift candidate. B-018 reconciliation explicitly states (line 65 of B-018 design) that the B-013 architectural concern does **not** generalize to emFileDialog. Confirm in B-013's reconciliation log that the watch-list note is scoped to confirmation-style dialogs (Cut/Paste/Delete/Interest) where the consumer needs a synchronous read on signal observation, not to long-running dialogs that already have their own Cycle (e.g. `emStocksFetchPricesDialog`). Avoids over-broad future scope.

### Recommended Pre-Implementation Actions

1. Amend §3.3: add a paragraph explicitly addressing engine-identity (Finding 1) — Rust subscribes the parent FilePanel engine because the ListBox is not an independent engine; cite C++ `class emStocksListBox : public emListBox` vs Rust composition.
2. Amend §3.3 / §6: add `ectx.disconnect(old.finish_signal, ectx.id())` to the cancel-old-dialog branch in each of the four mutators (Finding 2). Place it before the `pending_actions` push.
3. Amend §3.3 snippet: show borrow handling (`let sig = dialog.finish_signal; drop(...);`) and the precise outer/inner `if let Some / if/else` bracket structure (Findings 3 & 4).
4. Amend §6 step 1: add an explicit bullet noting that the Interest block must preserve the `interest_to_set = None;` reset on non-Ok results (Finding 5).
5. Add an inline invariant comment in the §3.3 snippet stating "subscribe-then-check happens on the same Cycle slice; do not split" (Finding 7).
6. Tighten §5 watch-list scope to confirmation-style dialogs (Finding 8).

No prereq buckets introduced. No global decisions changed. After amendments, B-013 is implementer-ready.

## Amendment Log — 2026-05-01

Folded Adversarial Review findings into the design body. Adversarial Review preserved verbatim above.

- **I-1 (engine identity):** Added explicit "Engine-identity note" paragraph at the top of §3.3 explaining that Rust subscribes the parent FilePanel engine — not self — because `emStocksListBox` is composed inside `emStocksFilePanel` rather than inheriting from `emListBox`/`emEngine` as in C++ (`~/Projects/eaglemode-0.96.4/src/emStocks/emStocksListBox.h:29` vs `crates/emstocks/src/emStocksFilePanel.rs:380`). Preserved-design-intent of the Rust composite host; not a `DIVERGED:`.
- **I-2 (cancel-old subscription leak):** Rewrote §3.2 cancel-old-dialog guidance to call `ectx.disconnect(old.finish_signal, ectx.id())` (guarded by old `_subscribed` flag) before the `pending_actions` close push. Symmetric with the confirmed-branch disconnect in §3.3. §6 checklist updated.
- **I-3 (bracket structure):** §3.3 snippet now contains an explicit comment distinguishing outer `Some(dialog)` guard from inner signaled/not-yet branches.
- **I-4 (borrow handling):** §3.3 snippet rewritten to extract `let Some(sig) = self.<dlg>_dialog.as_ref().map(|d| d.finish_signal)` so the immutable borrow drops before `&mut self` mutations.
- **I-5 (Interest cancel-side reset):** Added §3.3a showing the Interest-specific block preserving `self.interest_to_set = None;` on non-Ok. §6 checklist gained an explicit Interest-block bullet.
- **Note 7 (subscribe-then-check invariant):** Added an inline invariant comment in the §3.3 snippet ("subscribe-then-check happens on the same Cycle slice; do not split").
- **Note 8 (watch-list scope):** §5 tightened to confirmation-style dialogs only; explicitly excludes `emStocksFetchPricesDialog` and emFileDialog (cf. B-018 design line 65).
- **Minor 6 (engine-id stable under recursion):** No change required — finding self-resolves with citation in Adversarial Review; no design body change needed.

B-013 status: designed → dispatch-ready.
