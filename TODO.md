# TODO

- [x] Make `build.rs` conditional on `db` feature — avoid unnecessary ~27MB download for `types`-only consumers (partially addressed: `serde` is now optional, `build.rs` still runs but only downloads DB for `db` feature)
- [ ] Split Cloudflare Worker into separate repo (no submodule, depend on `wilayah` from crates.io)
