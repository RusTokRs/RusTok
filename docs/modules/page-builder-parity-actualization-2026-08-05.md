# Page Builder / Pages Parity Actualization

Date: 2026-08-05
Status: current-source-overlay / execution-and-rollout-open

This overlay reconciles the Page Builder programme with current `main`. It supersedes stale open-checkbox wording in older broad plans where that wording conflicts with merged source. It does not convert source-ready work into executed evidence.

## Corrected source state

### Consumer properties

The typed metadata contribution is source-complete for the Pages reference consumer.

- `rustok.pages.metadata` has one registered six-field schema.
- Draft Pages workspaces mount the canonical `ConsumerPropertiesPanel` inside Fly.
- Published pages mount the same registered panel in a Pages-owned standalone surface.
- Published Fly authoring remains unmounted.
- `PageMetadataEditor` and its direct metadata transport write are removed.
- Metadata persistence remains Pages-owned and independently versioned from the Fly document.

Any older Phase 5 checkbox saying that consumer metadata still needs to move into typed property contributions is stale at source level. Executed conflict, dirty-Fly isolation and browser evidence remain open.

### Immutable rollback

Immutable rollback is source-complete.

- Pages has a separate idempotent rollback command and receipt.
- Rollback selects a prior exact publish manifest and verifies immutable artifacts.
- It replaces locale bindings, advances the page version and writes `NodeUpdated` plus `NodePublished` in the owner transaction.
- It never invokes current-document sanitization, runtime materialization or compilation.
- GraphQL, HTTP, OpenAPI and the typed Pages admin prepare/confirm control are connected.

Any older Phase 6 checkbox saying rollback still needs implementation is stale at source level. Database execution and accepted rollback evidence remain open.

### Cache and public readers

The event-driven cache boundary is source-connected.

- Pages owns route/page/artifact scopes, namespace generations and key shape.
- Publish and rollback emit durable lifecycle events instead of calling cache infrastructure inline.
- The handler validates event/correlation-bound generation receipts.
- Storefront and artifact readers authorize before lookup, use current generations, load verified owner data before fill and fail open on cache errors.

Recent source packets add progressively stronger evidence:

- PR #2955: event/correlation and generation miss/refill contract;
- PR #2971: PostgreSQL publish/rollback outbox/cache harness;
- PR #2974: durable relay failure and restart harness;
- PR #2979: public artifact HTTP cache harness;
- current slice: native storefront cache source contract.

The native storefront cache source contract is ready. It retains composite route/page/artifact generation keys, hit short-circuit, generation rotation, old-value reachability rules and cache-failure fallback through the same public Pages cache runtime.

### Status boundary

Source parity has advanced, but execution and rollout gates remain open.

- No new test, verifier, Cargo, database, HTTP, browser, workflow or CI execution is claimed here.
- No FFA/FBA promotion is made.
- Real native server-function execution, durable relay-to-storefront continuity, browser/runtime packets and observed tenant rollout remain required.

## Current next cursor

1. Execute the existing metadata conflict/isolation and published metadata browser packets.
2. Mount and execute the real Pages native storefront server-function route with trusted host context, database fixtures and `PagesCacheReadRuntime`.
3. Retain one exact-revision continuity packet from durable `NodePublished` relay delivery through generation rotation to native storefront miss/refill/hit.
4. Complete compilation, workflow, anonymous-bundle and tenant Wave evidence before promotion.
