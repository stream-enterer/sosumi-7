# EGOPOL

Cargo workspace with two crates:
- `zuicchini/` — UI framework library (reimplementation of Eagle Mode's emCore in Rust)
- `egopol/` — game binary, depends on zuicchini via path

## Commands

```bash
cargo check --workspace
cargo test --workspace
cargo run -p egopol
```
