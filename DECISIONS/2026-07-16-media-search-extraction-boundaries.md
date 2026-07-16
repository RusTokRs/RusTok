# ADR: Media and Search as Whole-Module Extraction Pilots

## Status

Proposed

## Context

RusToK is implementing Fluid Backend Architecture as a deployment-neutral
module boundary. The current target is a modular monolith; FBA must not turn
every crate into a network service. A future extraction must preserve the
module owner, public ports, tenant/security context, typed errors, events, and
observability without duplicating domain rules.

`rustok-media` and `rustok-search` are the first candidates for a complete
remote-boundary pilot because they have independent read/storage profiles and
can tolerate explicit degraded behavior. `rustok-search` already owns the
`SearchEngine` connector abstraction and the PostgreSQL baseline. Its planned
Meilisearch, Typesense, and Algolia connectors are internal implementations of
that abstraction; they are not consumer-facing services.

`rustok-index` remains the ingestion/read-model substrate. It must not be
merged into the search query contract or expose search-engine internals.

## Decision

Treat `media` and `search` as whole-module extraction candidates, not as sets
of microservices.

The default deployment remains an in-process modular monolith:

```text
apps/server -> rustok-media (in-process MediaAssetReadPort)
apps/server -> rustok-search (in-process SearchQueryPort/SearchSuggestionPort)
```

The pilot topology adds a remote adapter at the module boundary only:

```text
host/consumer -> gRPC client -> whole rustok-media service
host/consumer -> gRPC client -> whole rustok-search service
```

The remote service contains the complete owner module, including its services,
repositories, migrations, connector implementations, health checks, and
module-owned adapters. No consumer may select or call a search engine directly.

### Media boundary

`MediaAssetReadPort` remains the only cross-module read contract. The remote
media service owns media metadata, translation policy, storage handles, public
URL/proxy policy, and its database. Raw blobs are not returned through the
cross-module port. SEO and AI-media continue to consume descriptors and typed
degraded outcomes.

### Search boundary

`SearchQueryPort` and `SearchSuggestionPort` remain the only query contracts.
The search service owns:

- `SearchEngine` and `SearchConnectorDescriptor` implementations;
- PostgreSQL FTS baseline;
- future Meilisearch/Typesense/Algolia connector crates;
- ranking, dictionaries, query rules, facets, analytics, and fallback policy;
- canonical search result/error envelopes.

Consumers never receive engine credentials, connector names as execution
instructions, engine-specific DTOs, or direct URLs. Engine selection is a
search-service policy decision based on deployment and tenant configuration.

`rustok-search` continues to own `SearchIngestionHandler`, `search_documents`,
and projection writes. A remote Search deployment consumes canonical domain
events through the platform event transport. Optional enrichment from
`rustok-index` must use `IndexReadModelPort`; direct SQL access to
`index_product_categories` or `index_product_attribute_values` is forbidden in
the isolated profile. Query-time search and index/read-model enrichment remain
separate contracts:

```text
owner modules -> outbox/event transport -> search ingestion -> SearchEngine adapter
search ingestion -> optional IndexReadModelPort enrichment
storefront/admin -> SearchQueryPort -> normalized SearchResult
```

The first pilot keeps Index in the monolith and proves the remote Search query
and ingestion boundary. Query-time joins to Index tables must be replaced by
search-owned denormalized fields populated during ingestion. Moving Index to a
separate worker remains a later decision and requires its own replay/lag
evidence.

### Search connector execution model

The existing `SearchEngine` trait remains the internal query contract. Before
an external connector can be enabled, Search adds an owner-internal document
writer contract covering schema synchronization, upsert, delete, rebuild, and
health. `PgSearchEngine` and every connector crate implement the same query and
writer capabilities. The Search runtime selects one configured connector and
owns fallback policy; no connector is registered in `apps/server`.

The public service boundary is independent of connector selection:

- `SearchQueryPort` executes normalized queries;
- `SearchSuggestionPort` executes normalized suggestions;
- a new owner-owned ingestion/control port accepts canonical document changes,
  rebuild requests, and idempotency metadata;
- GraphQL, Leptos server functions, and gRPC remain thin adapters over those
  owner contracts.

Connector SDK DTOs, credentials, index names, and schema APIs never cross the
public Search boundary.

### Media service completeness

`MediaAssetReadPort` is sufficient for current SEO/AI consumers but not for a
complete remote Media service. Before extraction, Media must publish an
owner-owned write/control contract for upload-session creation or streaming
upload, delete, translation updates, and cleanup/operator actions. Large binary
upload should use the Media-owned REST streaming or presigned-upload surface;
gRPC remains appropriate for metadata/control calls and must not buffer whole
blobs in generic port envelopes.

## Remote transport requirements

The shared transport layer must provide, for both pilots:

- versioned protobuf/gRPC envelopes mapped to the existing typed port DTOs;
- propagation of tenant, actor/security, locale, channel, correlation,
  causation, traceparent, deadline, and idempotency fields;
- typed mapping for timeout, unavailable, forbidden, not-found, validation,
  and invariant errors;
- capability and contract-version negotiation;
- bounded retry only for explicitly retryable errors;
- health/readiness and connector/provider diagnostics;
- redacted tracing and metrics for request, fallback, and degraded outcomes.

No remote adapter may introduce a second business implementation. In-process
and gRPC providers must run the same owner service/port path.

## Database isolation requirements

The pilot service must run with a separate database or schema and
separate credentials. Cross-module SQL and shared-table reads are forbidden.

Media migration requires an isolated media schema and storage configuration.
Search migration requires an isolated search schema or engine index and a
replayable ingestion stream. Cutover is atomic after shadow verification; no
consumer-facing dual query path or dual business write path is allowed.

## Phased delivery

1. **Contract hardening** — finalize port DTO/error matrices, topology metadata,
   Media write/control contracts, Search ingestion/control contracts,
   connector writer ownership, and event/inbox envelopes.
2. **Loopback transport** — implement generic gRPC server/client adapters and
   run conformance cases against an in-process-equivalent provider.
3. **Media isolated pilot** — separate process/database/storage, descriptor and
   fallback execution, tenant isolation, health, and restart evidence.
4. **Search isolation prerequisites** — remove query-time SQL reads of Index
   tables, populate search-owned denormalized fields during ingestion, and
   prove `IndexReadModelPort` enrichment where needed.
5. **Search isolated pilot** — separate query/ingestion service, internal
   connector selection, PostgreSQL baseline, normalized results, and fallback
   evidence.
6. **Optional Index worker split** — only after replay, lag, duplicate delivery,
   rebuild, and recovery evidence exists for Index independently of Search.
7. **Decision gate** — compare p95/p99 latency, CPU, DB load, error rate,
   operator cost, and recovery behavior against the in-process baseline.

## Non-goals

- Splitting `ai-content`, `ai-order`, `ai-product`, `ai-media`, or `ai-alloy`
  into separate services.
- Splitting `cart`, `pricing`, `inventory`, `customer`, and `order` before a
  measured checkout bottleneck and saga design exist.
- Adding gRPC adapters to every module by default.
- Replacing PostgreSQL search with an external engine implicitly.
- Promoting FBA readiness based on metadata without compiled or live evidence.

## Consequences

The platform gets two concrete end-to-end extraction pilots while preserving a
single deployable unit per module. Search connector development remains local
to `rustok-search`, so adding an engine does not multiply service boundaries.
The cost is a shared transport/conformance layer and explicit event replay
work before an isolated search/index deployment can be called production
ready.

## Evidence required for acceptance

- in-process and loopback gRPC conformance results for every published port;
- isolated database/schema and credential proof;
- tenant/security/deadline/idempotency propagation traces;
- media descriptor and degraded storage evidence;
- search query/suggestion and connector fallback evidence;
- index ingestion replay, duplicate, lag, rebuild, and recovery evidence;
- health, metrics, and restart/failure drill results.
