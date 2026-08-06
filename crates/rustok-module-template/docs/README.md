# Rust module component template

The renderer is pure: it validates all input and source-manifest content before
returning an ordered file set, and it performs no filesystem or process work.
The owner-local CLI is responsible for create-new filesystem semantics and
lockfile generation.

The generated crate uses Rust edition 2024 and the exact workspace MSRV as a
pinned `rust-toolchain.toml` channel. Native Cargo targets `wasm32-wasip2`;
there is no `cargo-component` fallback. Guest bindings come only from the exact
`rustok-module-sdk` release embedded in the generated `Cargo.toml`.

`module-artifact.json` is an author declaration, not a publication descriptor.
It cannot contain `artifact_digest`. The build worker inserts the verified
component digest into the final descriptor after Component and WIT inspection.

`tests/sandbox-scenario.json` binds the example command input to an exact
`platform.events/publish` fixture, a typed topic/operation grant, bounded
sandbox limits, and the expected output. The guest publishes
`module.<slug>.executed` with `{topic, payload}` input, so the example passes the
same Events capability constraint validator as an admitted runtime execution.

## Index compatibility boundary

Every rendered project includes `docs/index-integration.md`. The guide records
that the generated project is a standalone Component Model guest and therefore
cannot directly depend on `rustok-index`, register host runtime extensions,
access PostgreSQL, or write Index-owned storage.

The current component ABI exposes no `platform.index` capability. The Events
example is intentionally not presented as an Index mutation contract. A future
standalone integration must be host-owned, versioned, capability-constrained,
and admitted before the template can generate executable Index-specific code.

Native in-repository modules use the separate host integration contract in
`crates/rustok-index/docs/module-source-integration.md`.

The renderer validates the sandbox scenario and renders the Index boundary
guide before returning the ordered file set. The static verifier additionally
rejects a direct `rustok-index` dependency or an invented `platform.index`
capability in the standalone template.
