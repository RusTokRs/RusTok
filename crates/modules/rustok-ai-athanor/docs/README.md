# Athanor AI adapter documentation

The adapter owns only the RusToK-facing boundary. Athanor owns project
configuration, indexing, canonical snapshots, SurrealDB storage and Tantivy
search. The adapter never exposes those implementation types through the AI
contracts.

The current integration is Basic RAG: structural expansion over canonical
entities, provider-owned chunk publication into canonical snapshots, and
lexical retrieval through Athanor's Tantivy-backed search. Vector retrieval
remains an explicit capability gap until Athanor Phase 9 ships a concrete
`VectorIndex` adapter.
