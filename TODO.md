# TODO

- [ ] Make `build.rs` conditional on `db` feature — avoid unnecessary ~27MB download for `types`-only consumers
- [ ] Split Cloudflare Worker into separate repo (no submodule, depend on `wilayah` from crates.io)
