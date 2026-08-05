# Pages / Page Builder Anonymous Storefront SSR Delivery Packet

Date: 2026-08-05
Status: source-ready / execution-pending

## Scope

This packet actualizes the anonymous Pages delivery boundary after the six-profile dependency-graph packet. It is the retained SSR-only anonymous Pages delivery contract.

The current public Pages host is SSR-only:

- `apps/storefront` is an `rlib` host library;
- the host `ssr` feature enables `rustok-pages-storefront/ssr`;
- host `csr` and `hydrate` do not enable the optional Pages storefront module;
- `render_document` emits server-rendered HTML and the shared stylesheet link;
- there is no executable client bootstrap, module script, module preload, WASM URL, `mount_to_body` or `hydrate_body` entrypoint for Pages in the current host.

JSON-LD structured-data scripts remain allowed because they are document metadata, not executable Pages authoring or hydration code.

## Retained source sequence

```text
anonymous request
  → apps/storefront SSR router
  → Leptos render-to-HTML
  → Pages storefront read-only composition when the Pages module is enabled
  → <!DOCTYPE html>
  → /assets/app.css
  → no executable client bootstrap
  → no Pages/Page Builder admin or Fly authoring surface
```

The focused regression is:

- `apps/storefront/tests/pages_anonymous_ssr_delivery.rs`.

It renders the public shell through `rustok_storefront::render_shell` and rejects executable client and authoring markers in the returned document.

## Source verifier

`crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-ssr-delivery.mjs` retains:

- the host feature boundary;
- `rlib` host shape;
- SSR document output markers;
- absence of client bootstrap entrypoints across `apps/storefront/src`;
- the focused runtime regression source;
- linkage to the existing feature-resolved anonymous dependency-graph evidence;
- the explicit artifact inspector contract;
- empty execution evidence and false validation flags.

## Explicit built artifact inspection

`crates/rustok-pages/scripts/verify/inspect-pages-anonymous-storefront-ssr-artifact.mjs` requires at least one explicit built artifact path. It never treats a missing artifact or a nonexistent client bundle as a passing bundle proof.

Before scanning bytes it runs:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-graph.mjs
```

For every supplied artifact it records:

- path;
- byte size;
- SHA-256;
- every matched forbidden authoring marker.

It writes a `pages_anonymous_storefront_ssr_artifact_execution_v1` JSON packet and fails if an artifact is missing, empty, not a regular file, or contains a Pages/Page Builder admin or Fly authoring marker.

Example maintainer sequence, intentionally not run here:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-ssr-delivery.mjs

CARGO_TARGET_DIR=target/pages-anonymous-storefront-ssr \
  cargo build -p rustok-storefront --no-default-features --features ssr --lib

node crates/rustok-pages/scripts/verify/inspect-pages-anonymous-storefront-ssr-artifact.mjs \
  --profile host-storefront-ssr \
  --artifact target/pages-anonymous-storefront-ssr/debug/deps/librustok_storefront-<hash>.rlib \
  --output /tmp/pages-anonymous-storefront-ssr-artifact.json

cargo test -p rustok-storefront --no-default-features --features ssr \
  --test pages_anonymous_ssr_delivery -- --nocapture
```

The exact hash-qualified artifact path must come from the maintainer build; this packet does not guess it.

## Client bundle boundary

Compiled CSR/hydrate Pages bundle evidence is not claimed because the current host does not mount Pages in those profiles and has no Pages client bootstrap target.

The client bundle gate reopens immediately if any of the following is introduced:

- host `csr` or `hydrate` enables `rustok-pages-storefront`;
- a Pages `wasm_bindgen(start)` entrypoint appears;
- `mount_to_body` or `hydrate_body` is added;
- the SSR document starts loading a module script, module preload or WASM asset;
- a deployable Pages client artifact target is introduced.

At that point, real WASM/JS/chunk/source-map artifact inspection becomes mandatory before FFA promotion.

## Boundaries

This slice does not:

- change Pages, Page Builder or storefront production behavior;
- add a client runtime or hydration path;
- change Cargo dependencies or feature definitions;
- alter migrations, schemas, DTOs, routes, cache keys, TTL or event delivery;
- touch optional Iggy infrastructure;
- claim source verifier, regression, Cargo metadata, build, artifact inspection, workflow or CI execution;
- promote FFA or FBA.

Execution evidence remains pending.
