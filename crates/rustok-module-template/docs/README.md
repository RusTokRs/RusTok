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
The renderer validates this scenario before returning the file set.
