# rustok-ai-athanor / CRATE_API

## Public surface

### Constants & Configuration
- `ATHANOR_SOURCE_ID`: stable source identifier for Athanor RAG providers.
- `AthanorRagConfig`: configuration descriptor (`root`, `tenant_id`, `max_atoms`).

### Adapters & Providers
- `AthanorRagAdapter`: first-party RAG adapter implementing `RagRetrievalPort` and `RagIngestionPort`.

## Platform Capability Bindings
- **RAG Retrieval & Ingestion Seam**: Exposes Athanor embedded knowledge search and chunk ingestion to `rustok-ai`.
- **Determinism & Snapshot Publication**: Publishes deterministic RAG chunks into canonical snapshot stores.
- **Fail-Closed Vector Boundary**: Vector retrieval remains fail-closed until Phase 9 vector adapter availability.

## Domain Collaborations
- **`rustok-ai`**: Core AI orchestration consuming RAG retrieval and ingestion ports.
- **`athanor-core`**: Snapshot store and knowledge publication engine.
- **`athanor-app`**: Project search composition and text extraction.

## Minimum Contract Set

### Input DTOs/Commands
- `AthanorRagConfig`: initialization parameters (`root`, `tenant_id`, `max_atoms`).
- `RagSearchRequest`: search query parameters and candidate limits.
- `RagIngestRequest`: chunk ingestion batch definition.
- `RagExpandRequest`: candidate expansion and atom resolution request.

### Domain Invariants
- **Non-Swallowed Errors**: Snapshot storage, search, or ingestion failures must return typed `RagError` variants.
- **Tenant Scope Isolation**: All RAG operations are strictly scoped to the configured `tenant_id`.
- **Zero Capacity Prohibition**: `max_atoms` must be non-zero during configuration validation.

### Events / Outbox Side Effects
- Publishes atomic snapshot batches and knowledge entities to persistent Athanor storage.

### Errors / Failure Codes
- `RagError`: domain error types (e.g. `InvalidLimit`, `StorageError`, `SearchError`).

