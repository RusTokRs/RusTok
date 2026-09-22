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

Document chunking and publication preparation are provider-neutral in
`rustok-ai`: `chunk_document` produces stable source/revision/ordinal ids and
bounded UTF-8 offsets, while `RagIngestionCoordinator` calls the provider-owned
`RagIngestionPort`. `AthanorRagAdapter` now publishes chunks as Athanor-owned
canonical entities through the atomic snapshot boundary, preserves existing
canonical objects, replaces the prior revision for the same tenant/source
document, and restores source metadata and byte ranges during expansion.
Embeddings and vector indexes remain Athanor-owned follow-up work.

## FFA/FBA readiness

- FFA status: `not_started` — this support adapter owns no UI surface.
- FBA status: `boundary_ready` (`domain_support_adapter`).
- Storage/index ownership remains in Athanor; RusToK must not add a parallel
  Postgres vector store or duplicate Tantivy integration.

## Next results

1. Add embedding batch handoff once Athanor exposes a concrete embedding
   provider and vector-index composition.
2. Extend source/revision replacement evidence to the future embedding/index
   side effects without weakening tenant isolation.
3. Implement semantic retrieval only after Athanor's Phase 9 vector adapter
   is available; keep the `Vector` strategy fail-closed until then.

## Verification

- `cargo test -p rustok-ai-athanor --lib`
- `cargo check -p rustok-ai-athanor --features athanor`
