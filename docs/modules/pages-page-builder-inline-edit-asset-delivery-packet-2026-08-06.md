# Pages / Page Builder Inline Edit Asset Delivery Packet

Date: 2026-08-06  
Status: `source-ready / execution-pending`

## Scope

This packet closes the source boundary between the authenticated Pages authoring route and the JS/WASM files it references. It does not claim that the client, server, HTTP route or browser flow was built or executed.

The slice adds:

- a Pages-owned, feature-gated Axum asset router;
- compile-time embedding of the generated bootstrap, JavaScript module and WebAssembly module;
- exact MIME, cache-validator and same-origin resource policy responses;
- an exact-lock client artifact builder;
- a build orchestrator that prepares the client artifacts before compiling the server binary;
- release workflow and production Docker build integration that use the same orchestrator.

## Ownership

Pages remains the HTTP owner because the existing Pages module router is already composed by the host from `rustok-module.toml`. The host does not add a parallel static-file policy or inspect Pages documents.

Page Builder/Fly remains the document and command authority. The delivered files contain executable adapter code only; they do not contain a tenant, user, page, revision, grant, authorization proof or signing secret.

The authenticated route, grant verification and document-only save owner introduced by the preceding packets remain unchanged.

## Opt-in feature graph

```text
rustok-pages/inline-edit-assets
rustok-server/pages-inline-edit-assets
```

`rustok-server/pages-inline-edit-assets` composes:

```text
rustok-server/pages-inline-edit
rustok-pages/inline-edit-assets
```

Neither feature is enabled by the default server or anonymous storefront profiles.

## Fixed asset contract

```text
/assets/pages-inline-edit-bootstrap.js
/assets/pages-inline-edit/rustok_storefront.js
/assets/pages-inline-edit/rustok_storefront_bg.wasm
```

The generated files are binary-embedded into the server with `include_bytes!`. Building the asset profile without preparing all three files fails at compilation instead of silently deploying an incomplete authoring surface. Release workflow integration uses the same orchestrator, while execution evidence remains pending.

The release archive and runtime image can therefore remain binary-only; there is no runtime dependency on `target/site` or another writable/static filesystem directory.

## HTTP contract

JavaScript responses use:

```text
Content-Type: text/javascript; charset=utf-8
```

The WebAssembly response uses:

```text
Content-Type: application/wasm
```

All three responses use:

```text
Cache-Control: public, max-age=0, must-revalidate
ETag: "<full SHA-256>"
Cross-Origin-Resource-Policy: same-origin
```

The fixed paths are intentionally revalidated rather than treated as immutable fingerprinted URLs. Exact and weak `If-None-Match` values produce `304 Not Modified`.

The global security middleware continues to add `nosniff` and the authoring document continues to use the existing nonce-backed CSP. The files are public code assets, but only the authenticated authoring document references the bootstrap.

## Exact-lock client builder

`apps/storefront/scripts/build-pages-inline-edit-client.mjs` now:

1. reads the single `wasm-bindgen` package version from the workspace `Cargo.lock`;
2. exposes `--print-wasm-bindgen-version` for build orchestration;
3. requires the installed CLI to report that exact version;
4. invokes Cargo with `--locked` and only `pages-inline-edit-hydrate`;
5. respects an external `CARGO_TARGET_DIR` when locating the compiled WASM input;
6. writes the generated JS/WASM pair to a staging directory;
7. validates both generated files before atomically replacing the delivery directory;
8. copies and validates the fixed bootstrap module.

## Server build orchestration

`scripts/build/build-pages-inline-edit-server.sh` is the source owner for a binary with embedded authoring assets.

It:

1. resolves the exact `wasm-bindgen-cli` version from `Cargo.lock`;
2. installs or verifies that exact CLI under an isolated tool root;
3. ensures the `wasm32-unknown-unknown` target exists;
4. builds the dedicated client artifacts;
5. checks all three fixed files are non-empty;
6. builds `rustok-server` with `--locked --features pages-inline-edit-assets`;
7. verifies the selected debug/release binary exists.

The script was added as reproducible source only and was not run.

## Deliberate limits

This slice does not:

- enable the asset profile by default;
- change the anonymous or canonical public Pages route;
- add an admin launch link;
- modify database, GraphQL, REST mutation, publish, rollback or event schemas;
- change Pages document persistence;
- claim that the release workflow integration or production Docker build has executed;
- claim a generated client artifact, embedded binary, HTTP response or browser result;
- promote FFA or FBA.

## Source evidence

- `crates/rustok-pages/Cargo.toml`
- `crates/rustok-pages/src/http.rs`
- `crates/rustok-pages/src/http/inline_edit_assets.rs`
- `apps/server/Cargo.toml`
- `apps/storefront/scripts/build-pages-inline-edit-client.mjs`
- `scripts/build/build-pages-inline-edit-server.sh`
- `crates/rustok-pages/contracts/evidence/pages-inline-edit-asset-delivery-source.json`
- `crates/rustok-pages/scripts/verify/verify-pages-inline-edit-asset-delivery.mjs`

## Next cursor

1. add the admin-owned launch link under a matching opt-in admin feature;
2. run and retain static, Cargo, WASM, binary-embedding and anonymous dependency-graph evidence;
3. observe same-origin `200`/`304`, MIME, ETag, CORP and CSP behavior;
4. execute authenticated browser edit/save/reload and stale/replay/expiry failure cases.

## Validation status

No tests, static verifiers, formatting, Cargo commands, WASM builds, client builders, server builders, HTTP hosts, asset requests, browsers, dependency graphs, workflows or CI were run by the implementation agent.

Suggested commands, intentionally not run:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-inline-edit-asset-delivery.mjs
node apps/storefront/scripts/build-pages-inline-edit-client.mjs --print-wasm-bindgen-version
bash scripts/build/build-pages-inline-edit-server.sh
cargo test -p rustok-pages --features inline-edit-assets --all-targets -- --nocapture
cargo check -p rustok-server --features pages-inline-edit-assets
node crates/rustok-pages/scripts/verify/verify-pages-authenticated-authoring-route.mjs
node crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-graph.mjs
node crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-ssr-delivery.mjs
```

Execution evidence remains pending.
