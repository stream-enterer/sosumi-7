# B-014-rc-shim-no-acc-misc — P-005 — rc-shim with missing accessor (misc)

**Pattern:** P-005-rc-shim-no-accessor
**Scope:** misc (emAutoplay, emVirtualCosmos)
**Row count:** 2
**Mechanical-vs-judgement:** judgement-heavy
**Cited decisions:** D-002-rc-shim-policy — governs whether each rc-shim consumer converts to signal-subscribe or stays as a load-bearing shim, and flags emAutoplay flags-passing as needing escalation.
**Prereq buckets:** none

## Pattern description

Same rc-shim consumer pattern as P-004 (consumer routes around the signal via `Rc<RefCell<>>` / `Rc<Cell<>>` shared state) but the upstream signal accessor is also missing, so the fix needs accessor design plus shim removal. In this bucket the two affected scopes are emAutoplay (a Rust-only panel with no direct C++ analogue for its flag-passing shape) and emVirtualCosmos (whose model exposes no signal at all).

## Rows

| ID | C++ site | Rust site | Accessor status | Notes |
|---|---|---|---|---|
| emAutoplay-1172 | src/emMain/emAutoplay.cpp:1172 | crates/emmain/src/emAutoplay.rs:104 | missing | emAutoplayViewModel has no GetProgressSignal accessor; C++ UpdateProgress (emAutoplay.cpp:1370+) unported. |
| emVirtualCosmos-575 | src/emMain/emVirtualCosmos.cpp:575 | crates/emmain/src/emVirtualCosmos.rs:821 | missing | Model exposes no GetChangeSignal and never fires one; panel reacts to NF_VIEWING_CHANGED instead. |

## C++ reference sites

- src/emMain/emAutoplay.cpp:1172
- src/emMain/emVirtualCosmos.cpp:575

## Open questions for the bucket-design brainstorm

- D-002 explicitly defers the emAutoplay flags-passing pattern (`AutoplayFlags { progress: Rc<Cell<f64>> }`): does it fall under rule 1 (convert) or rule 2 (keep as post-finish handoff)? emAutoplay has no C++ analogue so the rule needs adaptation — escalate to working-memory before bucket execution.
- For emAutoplay, should the missing `UpdateProgress` infrastructure (emAutoplay.cpp:1370+) be ported as part of this bucket, or split into a prerequisite porting bucket? (Echoes D-003 gap-blocked-fill-vs-stub framing even though this row is not gap-tagged.)
- For emVirtualCosmos, the C++ model has no signal either — is the Rust panel's NF_VIEWING_CHANGED path observably equivalent (in which case this row is a re-classification toward "no drift") or is there a still-missing C++-side change signal to mirror?
- If emVirtualCosmos turns out to be observably equivalent, does it leave this bucket entirely (annotation only) or does the rc-shim still need removal on Port-Ideology grounds?
