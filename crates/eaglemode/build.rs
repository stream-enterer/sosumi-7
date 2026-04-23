fn main() {
    // $ORIGIN        — finds .so files next to the binary (target/{profile}/).
    // $ORIGIN/deps   — finds plugin cdylibs where cargo places them when they
    //                  are listed as [dependencies] (target/{profile}/deps/).
    //                  Both paths are needed: workspace builds copy the final .so
    //                  to the profile root, but per-package builds (cargo run -p)
    //                  leave them only in deps/.
    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN:$ORIGIN/deps");
}
