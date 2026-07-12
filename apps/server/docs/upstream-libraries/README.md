# Upstream snapshots for server core libraries

This directory captures **fresh documentation links** for active server
libraries. The pure Axum runtime uses only the active library snapshots listed
below as implementation references.

- Version source: `Cargo.lock`
- Snapshot date: `2026-07-02`
- Update: `make docs-sync-server-libs`
- Freshness check: `make docs-check-server-libs`
- Mode: `Snapshot contains current versions and direct links to docs.rs without downloading HTML.`

## Current versions and links

| Crate | Version (`Cargo.lock`) | Docs.rs crate page | Rustdoc index | Local metadata |
|---|---:|---|---|---|
| `axum` | `0.8.9` | [crate](https://docs.rs/crate/axum/0.8.9) | [rustdoc](https://docs.rs/axum/0.8.9/axum/) | `apps/server/docs/upstream-libraries/axum/metadata.json` |
| `sea-orm` | `1.1.20` | [crate](https://docs.rs/crate/sea-orm/1.1.20) | [rustdoc](https://docs.rs/sea-orm/1.1.20/sea_orm/) | `apps/server/docs/upstream-libraries/sea-orm/metadata.json` |
| `async-graphql` | `7.2.1` | [crate](https://docs.rs/crate/async-graphql/7.2.1) | [rustdoc](https://docs.rs/async-graphql/7.2.1/async_graphql/) | `apps/server/docs/upstream-libraries/async-graphql/metadata.json` |
| `tokio` | `1.52.3` | [crate](https://docs.rs/crate/tokio/1.52.3) | [rustdoc](https://docs.rs/tokio/1.52.3/tokio/) | `apps/server/docs/upstream-libraries/tokio/metadata.json` |
| `serde` | `1.0.228` | [crate](https://docs.rs/crate/serde/1.0.228) | [rustdoc](https://docs.rs/serde/1.0.228/serde/) | `apps/server/docs/upstream-libraries/serde/metadata.json` |
| `tracing` | `0.1.44` | [crate](https://docs.rs/crate/tracing/0.1.44) | [rustdoc](https://docs.rs/tracing/0.1.44/tracing/) | `apps/server/docs/upstream-libraries/tracing/metadata.json` |
| `utoipa` | `5.5.0` | [crate](https://docs.rs/crate/utoipa/5.5.0) | [rustdoc](https://docs.rs/utoipa/5.5.0/utoipa/) | `apps/server/docs/upstream-libraries/utoipa/metadata.json` |

To attempt downloading HTML copies from docs.rs use:

```bash
python3 scripts/server_library_docs_sync.py sync --download-html
```
