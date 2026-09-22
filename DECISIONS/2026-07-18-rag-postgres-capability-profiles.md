# Athanor-owned RAG data plane

- Status: Accepted
- Date: 2026-07-18

## Context

`rustok-ai` already owns the AI runtime and exposes Rig-backed embeddings and
reranking entrypoints. Its persisted control plane is PostgreSQL-backed, but
the AI-owned RAG source, chunk, embedding and retrieval schema is not yet
implemented. PostgreSQL is already the platform baseline; the current Search
module also uses PostgreSQL text and trigram indexes without making Search the
owner of AI knowledge retrieval.

Athanor is part of the AI infrastructure and is embedded as a library. Its
SurrealDB and Tantivy components ship with the Athanor module, providing the
structural/vector and lexical data planes in-process. Adding a separate
PostgreSQL extension would complicate installation, module composition and
support without adding a required capability to the RusToK + Athanor stack.

## Proposed direction

Keep the RAG control plane and contracts in `rustok-ai`, with embedded Athanor
as the canonical AI data-plane provider behind one AI-owned retrieval port:

1. **Basic RAG** — AI-owned source references, versions, citations and
   structure-aware retrieval through Athanor's document/atom graph, with
   Tantivy lexical retrieval and metadata filters for all indexed sources.
   Optional Rig reranking may improve the result set.
2. **Semantic RAG** — the same source/chunk/citation model plus Rig embeddings
   and an Athanor vector-index adapter. Athanor currently exposes the
   `EmbeddingProvider` and `VectorIndex` core ports, while the concrete vector
   implementation remains a planned Athanor Phase 9 slice. Tantivy lexical
   retrieval and structural expansion are the available v0.1 path.

The vector-specific storage is an Athanor AI-infrastructure capability to be
implemented behind its core ports. Core RAG contracts remain provider-neutral
and do not add PostgreSQL vector types or extension migrations.

## Enablement and installation

The operator/admin surface may expose Athanor capability status and optional
module installation, but it must first probe the embedded capability and
report:

- Athanor embedded SurrealDB/Tantivy capability and its contract/version;
- concrete Athanor vector-index capability status;
- migration/backfill state.

When an optional Athanor module is missing, the UI should offer an
operator-facing module installation or explicit instructions. When the vector
adapter becomes available, an explicit setup and idempotent backfill create
or synchronize its derived indexes.

## Ownership boundaries

- `rustok-ai` owns RAG source/version/retrieval/citation contracts and access
  policy. Athanor owns physical canonical atoms, chunks, embeddings and
  derived indexes; no duplicate PostgreSQL RAG tables are introduced.
- `rustok-storage` owns physical files; RAG stores only safe file references.
- `rustok-search` remains the owner of product/search indexing and `pg_trgm`;
  RAG must not reuse Search tables as its source of truth.
- Athanor may provide ingestion and document-normalization tools, while Rig
  remains the AI runtime/embedding seam.
- Domain modules and marketplace data enter RAG through explicit projections,
  ports or events, never through direct cross-module table access.

## Athanor AI data-plane boundary

Athanor is an embedded AI-infrastructure data plane. Its SurrealDB and Tantivy
indexes remain the source of truth for Athanor-owned documents, chunks,
embeddings, vector/lexical indexes, connector state and ingestion jobs.
`rustok-ai` owns the public RAG contract,
tenant/access policy, provider selection and citations; it must not join
Athanor tables directly or treat the private schema as its own RAG schema.

The integration flow is an explicit projection:

```text
Athanor document/revision
        -> Athanor ingestion/index pipeline
        -> AI source reference and content digest
        -> embedded Athanor retrieval (Tantivy + SurrealDB)
        -> bounded context with citations
```

The AI source record stores the external Athanor identity and revision. The
retrieval provider owns the physical chunks/embeddings, but every provider
must return the same bounded context/citation contract and must not leak its
database schema into `rustok-ai`.

Athanor's structure-aware path may address a document, section, block or atom
by stable identity and return its parent path, related nodes, source revision
and access metadata. RAG context assembly should preserve that structure
instead of flattening every source into anonymous text chunks. Vector search
is then a recall aid for natural-language queries, not the definition of
semantic understanding itself.

## Athanor vector capability (planned)

Vector search is reserved as an Athanor-owned AI capability. The AI
orchestrator will select a typed operation rather than exposing arbitrary
SurrealDB access to the model:

- `index_document_revision` — normalize a source revision and build/update its
  atom and embedding records;
- `vector_search` — return ranked atom/document candidates under tenant,
  source and access filters;
- `expand_structure` — load the relevant parent path, related atoms and
  bounded neighbouring context;
- `build_citations` — return stable source/revision/atom citations for the
  generated context.

Rig remains the embedding/model seam. Once implemented, Athanor will own the
SurrealDB vector index and structural expansion, while `rustok-ai` owns
retrieval strategy, policy, budgets, tool orchestration and the final context
envelope passed to the model.
Future external retrieval providers may be added only through the same typed
port; PostgreSQL vector extensions are not part of the current RusToK plan.

## Open release decision

Version `0.1` includes the Basic RAG foundation with Athanor structural and
Tantivy lexical retrieval. The semantic vector profile follows after Athanor's
concrete vector adapter is implemented. Additional Athanor modules may add
parsers, connectors, embedding providers or retrieval strategies without
changing the `rustok-ai` contract.
