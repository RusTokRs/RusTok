# rustok-ai-athanor

## Purpose

`rustok-ai-athanor` is the first-party adapter between the RusToK AI retrieval
contract and the Athanor library. It keeps Athanor's embedded store and Tantivy
index behind `rustok_ai::RagRetrievalPort`.

## Responsibilities

- Build an Athanor-backed RAG provider from an explicit `RuntimeComposition`.
- Run bounded Basic RAG searches through Athanor's canonical project search.
- Expand ranked entity IDs into source-referenced atoms from the latest snapshot.
- Reject vector retrieval explicitly until Athanor's Phase 9 vector adapter is available.

This is a support/capability crate, not a tenant-toggleable module and not a
second storage implementation. SurrealDB and Tantivy remain owned by Athanor.

## Entry points

- `AthanorRagAdapter`
- `AthanorRagConfig`
- `ATHANOR_SOURCE_ID`

Enable the `athanor` feature in the host that embeds the library. Add
`athanor-surreal` when the Athanor project is configured for its embedded
SurrealDB store; the default Athanor runtime remains JSONL-compatible.

## Documentation

- [Module documentation](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Platform documentation map](../../docs/index.md)
