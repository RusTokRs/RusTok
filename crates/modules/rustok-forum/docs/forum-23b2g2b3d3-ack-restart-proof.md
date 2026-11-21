# forum 23b2g2b3d3 ack restart proof

Status: source-ready / unvalidated.

This owner note records the source boundary checked by `verify-forum-search-versioned-invalidation-ack-restart-proof.mjs`. It is a maintainer handoff, not evidence of execution: tests, Cargo, formatting, and CI were not run by the implementation agent, and runtime execution or evidence admission remains maintainer-owned.

## Source contract

- Owner note for `forum 23b2g2b3d3 ack restart proof`.
- Verifier: `scripts/verify/verify-forum-search-versioned-invalidation-ack-restart-proof.mjs`.

## Maintainer boundary

- Source markers and contract paths below are retained for maintainer review only.
- No runtime result, migration application, or admission decision is claimed here.
- required marker: `source_ready_maintainer_execution_pending`
- required marker: FORUM-23B2G2B3D3
- required marker: crates/modules/rustok-forum/contracts/forum-search-versioned-invalidation-ack-restart-proof.json
- required marker: apps/server/tests/forum_versioned_invalidation_ack_restart_iggy.rs
- required marker: target/forum-search-versioned-invalidation-ack-restart-evidence.json
- required marker: rustok-search-forum-projection-v1
- required marker: same broker offset
- required marker: one durable `search_projection_inbox` row
- required marker: does not run the server-owned consumer loop
- required marker: No command above was run by the implementation agent
