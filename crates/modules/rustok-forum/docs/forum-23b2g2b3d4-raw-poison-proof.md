# forum 23b2g2b3d4 raw poison proof

Status: source-ready / unvalidated.

This owner note records the source boundary checked by `verify-forum-search-versioned-invalidation-raw-poison-proof.mjs`. It is a maintainer handoff, not evidence of execution: tests, Cargo, formatting, and CI were not run by the implementation agent, and runtime execution or evidence admission remains maintainer-owned.

## Source contract

- Owner note for `forum 23b2g2b3d4 raw poison proof`.
- Verifier: `scripts/verify/verify-forum-search-versioned-invalidation-raw-poison-proof.mjs`.

## Maintainer boundary

- Source markers and contract paths below are retained for maintainer review only.
- No runtime result, migration application, or admission decision is claimed here.
- required marker: `source_ready_maintainer_execution_pending`
- required marker: FORUM-23B2G2B3D4
- required marker: crates/modules/rustok-forum/contracts/forum-search-versioned-invalidation-raw-poison-proof.json
- required marker: apps/server/tests/forum_versioned_invalidation_raw_poison_iggy.rs
- required marker: target/forum-search-versioned-invalidation-raw-poison-evidence.json
- required marker: `rustok-search-forum-projection-v1`
- required marker: `AlreadyPublished`
- required marker: No second physical DLQ message may appear
- required marker: does not execute the server-owned Forum Search worker loop
- required marker: No command above was run by the implementation agent
