# TODO

- [x] Add `AdminLevel`, `LocateMethod`, `Location` types + `locate()` function
- [x] Re-export new types + `locate()` wrapper + doc test in `src/lib.rs`
- [x] Add `GET /locate` endpoint to `examples/serve.rs`
- [ ] Run verification: `cargo test`, `cargo clippy`, `cargo publish --dry-run`
- [ ] Polygon-based `Contained` locate method — exact boundary containment instead of centroid proximity
