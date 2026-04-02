# Incremental Replacement Harness

## Objective

Achieve zero-tolerance golden test parity by progressively replacing C++ rendering functions with Rust equivalents inside the running C++ binary. The C++ test suite is the oracle at every step. A replaced function is proven correct when the C++ tests still pass with the Rust version in place.

## Approach

The C++ emPainter binary is the host. Rust rendering functions are compiled into a shared library (`.so`) with C-compatible entry points. C++ call sites are modified with `#ifdef USE_RUST_<NAME>` guards that dispatch to the Rust implementation when enabled. One function is replaced at a time. The C++ golden test generator (`tests/golden/gen/gen_golden.cpp`) runs after each replacement. If the golden reference images are identical, the Rust function is proven correct in situ — with real dependencies, real call order, real state. If they differ, the divergence is in exactly the function that was just replaced.

## Scope

The entire emPainter rendering system. All functions across all 6 Rust files (emPainter.rs, emPainterInterpolation.rs, emPainterScanlineTool.rs, emPainterScanline.rs, emPainterScanlineAvx2.rs, emColor.rs) and their C++ equivalents (emPainter.cpp, emPainter.h, emPainter_ScTl.cpp, emPainter_ScTlIntImg.cpp, emPainter_ScTlPSInt.cpp, emColor.h).

## Replacement granularity

Replace at whatever boundary makes the FFI data boundary simple. This is NOT constrained to C++ function boundaries. If a C++ function is a 300-line monolith, replace a 15-line computation inside it. The `#ifdef` goes around the smallest unit where:

1. The inputs can be passed as C-compatible types (integers, pointers, structs of integers)
2. The outputs can be received back through the same interface
3. The replaced unit is self-contained enough that its correctness is testable by running the golden generator

If the glue code at a boundary is complex, the boundary is wrong — pick a finer-grained one.

## Ordering

Bottom-up. Shared foundations first (color math, hash tables, compositing), then interpolation, then transform setup, then high-level orchestration. But the ordering is adaptive: if a bottom-layer replacement passes immediately (meaning the existing Rust code is already correct), move up. If it fails, fix the Rust function, confirm the replacement passes, then move up.

The top-level orchestration (PaintBorderImage, PaintImage) may never need direct replacement if all its callees are proven correct — the golden tests verify the composition.

## Precondition

Pre-expand C++ macros for the specific template variants that execute in failing tests. The C++ uses `#include`-based template expansion (CHANNELS, EXTENSION, HAVE_GC1/GC2) that generates 12+ variants per function. Agents reading the generic template systematically misinterpret which variant executes. Before any replacement work, produce readable expanded source for CHANNELS=4 (border images) and CHANNELS=1 (font glyphs).

## What the harness produces

Passing or failing C++ golden tests. Nothing else. The test results ARE the proof. The `#ifdef` guards in C++ source ARE the record of what has been replaced. The Rust shared library source IS the specification. There are no intermediate artifacts, no JSON dumps, no reports, no documentation.

## What "done" means

All 241 golden tests pass at tol=0 with all rendering functions dispatching to Rust. At that point the C++ host is a shell and the Rust implementations are proven pixel-identical for every test case.

## Constraints

- No regression: the currently-passing 204 golden tests must continue to pass at every replacement step
- No C++ behavioral changes: the `#ifdef` guards and FFI glue must not alter C++ behavior when `USE_RUST_*` is not defined. The unmodified C++ path must remain the oracle.
- No hypothesis-first investigation: when a replacement fails, the failure itself localizes the problem. Read the Rust function, read the C++ function, find the difference. Do not form hypotheses about what might be wrong elsewhere.
