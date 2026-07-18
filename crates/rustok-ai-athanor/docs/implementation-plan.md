# rustok-ai-athanor implementation plan

## Current state

The support crate provides `AthanorRagAdapter` behind the `athanor` feature
(`athanor-surreal` additionally enables Athanor's optional embedded SurrealDB
store).
It delegates lexical search to Athanor's public application API and expands
bounded canonical entities into `RagAtom` values with source locators and
relation references. Each adapter instance is bound to one tenant and project,
and rejects requests for another tenant before touching the Athanor store. The
adapter is pinned to the Athanor library revision
declared in the workspace dependency table.

## FFA/FBA readiness

- FFA status: `not_started` — this support adapter owns no UI surface.
- FBA status: `boundary_ready` (`domain_support_adapter`).
- Storage/index ownership remains in Athanor; RusToK must not add a parallel
  Postgres vector store or duplicate Tantivy integration.

## Next results

1. Verify the adapter against the user's updated Athanor revision and update
   the workspace pin when that revision is committed.
2. Add source/revision filtering evidence and a composed tenant policy before
   exposing the provider to runtime composition.
3. Implement semantic retrieval only after Athanor's Phase 9 vector adapter
   is available; keep the `Vector` strategy fail-closed until then.

## Verification

- `cargo test -p rustok-ai-athanor --lib`
- `cargo check -p rustok-ai-athanor --features athanor`
