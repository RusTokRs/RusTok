# Pages / Page Builder Anonymous Storefront Graph Packet

Date: 2026-08-05  
Status: **source-ready / execution-pending**

## Purpose

Retain a fail-closed dependency-graph guard for the anonymous storefront boundary without changing Pages, Page Builder or host runtime behavior.

The packet addresses the source gap between two weaker claims:

1. the storefront manifests do not directly name an admin package;
2. the resolved anonymous SSR/CSR/hydrate dependency graph cannot reach an authoring package transitively.

The existing Pages UI boundary verifier retains the first claim. This packet adds the second.

## Six feature-resolved graphs

The verifier invokes `cargo metadata` for these exact profiles:

| Profile | Manifest | Features | Target |
| --- | --- | --- | --- |
| Pages storefront default | `crates/rustok-pages/storefront/Cargo.toml` | none | host |
| Pages storefront hydrate | `crates/rustok-pages/storefront/Cargo.toml` | `hydrate` | `wasm32-unknown-unknown` |
| Pages storefront SSR | `crates/rustok-pages/storefront/Cargo.toml` | `ssr` | host |
| Host storefront CSR | `apps/storefront/Cargo.toml` | `csr` | `wasm32-unknown-unknown` |
| Host storefront hydrate | `apps/storefront/Cargo.toml` | `hydrate` | `wasm32-unknown-unknown` |
| Host storefront SSR | `apps/storefront/Cargo.toml` | `ssr` | host |

Every command uses `--no-default-features`. Target filtering is explicit for browser profiles.

## Reachability policy

The verifier starts at the selected root package and walks the resolved Cargo graph. **Dev-dependencies are excluded** from reachability, while normal and build dependencies remain visible.

The following packages are forbidden at every depth:

- `rustok-pages-admin`;
- `rustok-page-builder-admin`;
- `rustok-admin`;
- `fly-browser`;
- `fly-ui`;
- `fly-leptos`.

The Pages storefront profiles must retain the read-only chain:

```text
rustok-pages-storefront
  → rustok-page-builder-storefront
  → rustok-page-builder
  → fly
```

The Pages SSR profile must additionally reach the Pages owner crate. The host SSR profile must reach both Pages storefront crates. Current host CSR/hydrate profiles leave the optional Pages module disabled; the Pages hydrate graph is therefore checked separately rather than pretending the host currently ships it.

## Source-tree guard

The verifier also scans Rust sources under:

- `crates/rustok-pages/storefront/src`;
- `crates/rustok-page-builder-storefront/src`;
- `apps/storefront/src`.

Imports and composition markers for Pages admin, Page Builder admin and Fly browser/editor surfaces are forbidden. This protects the package graph and the module entrypoints together.

## Boundaries

This packet does not:

- change Cargo dependencies or features;
- change Pages or Page Builder production code;
- change SSR, CSR or hydrate composition;
- build a binary, WASM module or JavaScript bundle;
- inspect compiled bundle bytes, source maps or chunk manifests;
- prove runtime authentication or inline-edit behavior;
- promote FFA or FBA status.

A passing graph verifier proves that the selected Cargo feature graphs cannot reach the named authoring packages through non-dev dependencies. It is not compiled bundle-byte evidence.

## Evidence

- verifier: `crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-graph.mjs`;
- machine evidence: `crates/rustok-pages/contracts/evidence/pages-anonymous-storefront-graph-source.json`;
- canonical cursor: `docs/modules/pages-page-builder-parity-continuation-plan.md`;
- Pages-local cursor: `crates/rustok-pages/docs/implementation-plan.md`.

The evidence execution list is empty and every validation flag remains false.

## Maintainer validation

Intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-graph.mjs
```

After the graph verifier passes, accepted compiled bundle artifact evidence remains pending for anonymous SSR, CSR and hydrate outputs.
