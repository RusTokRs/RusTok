# `rustok-manifest` Contract and Architecture

## Purpose

`rustok-manifest` is the canonical owner of declarative manifest modeling, deserialization, and pure contract verification for RusToK. It prevents semantic drift between compile-time codegen (`build.rs`), preflight CLI validation (`xtask`), and host runtime validation.

## Responsibility Zone

- **Canonical models**: Types corresponding exactly to `modules.toml` schema version 2 and `rustok-module.toml`.
- **Pure validation**: Deterministic in-memory checks for slugs, UI classifications, and topological acyclicity of `depends_on`.
- **Deterministic snapshot**: SHA-256 state hashing with BTreeMap-sorted key ordering.
- **Dependency isolation**: Zero database, async-runtime, or HTTP framework dependencies.

## Integration

- Used by `apps/server/build.rs` to eliminate custom TOML structs.
- Used by `xtask` to replace local `manifest_io.rs` models.
- Integrated via Cargo workspace dependencies: `rustok-manifest = { path = "crates/libs/rustok-manifest" }`.

## Verification

- Pure Rust unit tests in `crates/libs/rustok-manifest/tests/manifest_tests.rs`.
- Scoped compilation verification: `cargo check -p rustok-manifest`.
- Integration tests parsing the live repository manifests: `cargo test -p rustok-manifest`.

## Related Documents

- [Manifest-Layer Contracts](../../../docs/modules/manifest.md)
- [Architecture Quick Reference](../../../ARCHITECTURE.md)
- [ADR: Shared Platform Manifest Foundation Library](../../../DECISIONS/2026-10-15-rustok-manifest-foundation-library.md)
