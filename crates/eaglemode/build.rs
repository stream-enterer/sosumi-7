fn main() {
    // $ORIGIN/deps is searched FIRST so that per-package builds (`cargo run -p`)
    // always load the freshly compiled cdylib from target/{profile}/deps/.
    //
    // Background: `cargo run -p eaglemode` (per-package) writes plugin .so files
    // only to target/{profile}/deps/, never to target/{profile}/. A workspace
    // build (`cargo build`) copies them to both locations. If $ORIGIN came first,
    // any stale .so left in the profile root by a previous workspace build would
    // shadow the freshly compiled deps/ copy — the app would silently run old code.
    //
    // $ORIGIN is kept as a fallback so that installed / release builds (where .so
    // files are placed next to the binary) continue to work without deps/.
    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/deps:$ORIGIN");
}
