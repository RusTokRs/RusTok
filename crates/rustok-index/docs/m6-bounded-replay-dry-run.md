# M6 bounded replay dry-run

Status: `source_complete_locale_transport_execution_pending`

This slice adds an Index-owned, side-effect-free validation runtime over the exact immutable
`SharedIndexSourceRegistry` and `SharedIndexSchemaRegistry` used by production replay, binds that capability
behind the server-owned request-bound replay operator guard, and exposes bounded schema-wide or exact-locale
GraphQL Shadow commands through the authenticated confidential continuation boundary.

The shared continuation contract is locale-safe, so schema-wide and exact canonical-locale tokens cannot cross
scan scopes. The dry-run execution path now carries the same canonical locale identity through request validation,
source scanning, sealed continuation scope and GraphQL transport.

## Contract

`IndexReplayDryRunRequest` carries:

- one non-nil tenant;
- one exact registered schema;
- one explicit schema-wide (`None`) or exact canonical-locale (`Some(LocaleKey)`) scope;
- one optional source-owned continuation cursor;
- one source page limit already bounded by `IndexSourceScanRequest`;
- one invocation budget from 1 through 1024 pages.

`IndexReplayDryRunRequest::new` constructs the schema-wide scope. `IndexReplayDryRunRequest::for_locale` constructs
one exact canonical locale scope. Both delegate validation to the existing source-scan contract rather than
creating another locale representation.

`SharedIndexReplayDryRunRuntime::run` resolves source ownership and the registered schema from the immutable
registries before scanning. An exact-locale request fails closed with `LocaleScopeUnsupported` when the registered
schema has `LocaleMode::None`. Every actual source page is then constructed from the same request scope through
schema-wide `IndexSourceScanRequest::new` or exact-locale `IndexSourceScanRequest::for_locale`.

For every returned page the runtime verifies:

- the existing source-page tenant/schema/locale, size, key uniqueness, and cursor-progress contract;
- non-nil event UUIDs;
- page-local event UUID uniqueness before accepting the page;
- complete `SchemaRegistry::validate_mutation` validity for every upsert or delete.

The internal outcome reports the stable source name, completion or bounded yield, a resume cursor only when
unfinished, page/mutation/upsert/delete counts, and the maximum observed source version.

## Source-call boundary

Dry-run invokes `SharedIndexSourceRegistry::scan`; it does not keep or call a source adapter outside
the immutable registry. Product, ProductVariant, SalesChannel, and future production bridges that
register through the canonical `register_index_source` helper therefore inherit the existing
30-second source-call timeout and its bounded retryable `index_source_scan_timeout` failure.

Direct low-level `IndexSourceCatalog::register` usage remains available to isolated fixtures and
contract tests. It intentionally does not imply the production timeout policy.

## No-write boundary

The dry-run runtime owns no database connection and receives no mutation sink, job store,
checkpoint store, reconciliation progress store, scheduler, or task handle. It cannot write:

- `index_entities` or `index_links`;
- `index_inbox`;
- `index_jobs`;
- `index_checkpoints`;
- reconciliation progress or terminal state.

Materialization performs no source call and no database I/O. The capability is published only
when both immutable registries exist; an absent source registry publishes nothing, and a source
registry without its shared schema registry fails closed.

A yielded internal dry-run is resumed only by explicitly passing its returned opaque cursor into another bounded
request with the same schema-wide or exact-locale scope. No durable cursor is implied.

## Server-owned host guard

`IndexReplayOperatorRuntime` retains the already-materialized `SharedIndexReplayDryRunRuntime` beside the durable
`SharedIndexReplayRuntime` and exposes `IndexReplayOperatorRuntime::run_shadow`.

`run_shadow` reuses the same request-bound `modules:manage` authorization boundary as durable Full replay. The
exact request tenant must match the operator context before the side-effect-free source scan can start. The
operator publishes no database connection, source registry, scheduler, worker handle, job identifier, lease
control, cancellation state, or retry/requeue control to the caller.

Server composition fails closed if durable replay materializes but the expected dry-run capability is missing.
Retained server source evidence covers `modules:read` rejection and an authorized `modules:manage` empty-page
completion through the guarded Shadow method. That completion creates no durable replay job or checkpoint and
does not alter the Full runner.

## GraphQL Shadow transport

`runIndexReplayShadow` exposes one bounded schema-wide or exact-locale invocation through
`IndexReplayShadowTransportRuntime`.

The GraphQL preparation path derives tenant/actor from request context and requires effective `modules:manage`
**before** parsing untrusted schema identifiers, optional locale or continuation text. Locale is bounded and
canonicalized through the same `LocaleKey` parser used by durable Full replay. The server adapter repeats
exact-tenant authorization before opening the token or constructing `IndexReplayDryRunRequest`.

Caller input contains only module/entity/schema routing identity, one optional locale and one optional sealed
continuation token. Page limit and maximum pages remain server-owned at `100` and `8`. Source identity, raw cursor
JSON, job, checkpoint, lease, worker, cancellation and retry controls remain unavailable.

The adapter reuses the deployment `IndexSourceContinuationKeyringRuntime` and canonical frozen
`IndexSourceContinuationScope`. Schema-wide requests derive scope through `from_registry`; exact-locale requests
derive it through `for_locale`. Incoming tokens are authenticated, decrypted, expired and exact-scope checked
before the raw cursor is reconstructed. An outgoing raw cursor is sealed under that same scope before returning to
GraphQL. The transport-safe result contains only Complete/Yielded status, bounded counters and optional sealed
continuation; it omits internal source name and source version.

Schema-wide (`None`) and exact-locale (`Some(LocaleKey)`) tokens are mutually incompatible, and different canonical
locales cannot exchange tokens. The codec has one current unversioned envelope with no old-format decoder or
version-tagged claim family.

## Explicitly open

- execute/admit schema-wide and exact-locale Shadow GraphQL/source behavior plus continuation-key deployment evidence;
- persisted dry-run reports or comparison snapshots;
- cross-page duplicate event detection beyond one bounded page;
- mutation/checkpoint timing simulation and interruption;
- targeted mutation execution over `IndexSource::load`;
- cooperative cancellation while a source or validation future is already pending;
- configurable per-source or per-request timeout policy;
- retained production-source and PostgreSQL execution evidence.

Transport code calls the guarded server adapter/operator chain rather than treating
`SharedIndexReplayDryRunRuntime` as an authorized public endpoint. The existing GraphQL `runIndexReplay` command
remains durable Full replay and is unchanged by Shadow transport.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, source-adapter execution, PostgreSQL validation, workflows
and CI are maintainer-run. The implementation agent did not execute them.

Suggested commands:

```bash
cargo test -p rustok-index --test replay_dry_run_contract -- --nocapture
cargo test -p rustok-index replay_dry_run -- --nocapture
cargo check -p rustok-index --all-targets
node scripts/verify/verify-index-source-continuation.mjs
node scripts/verify/verify-index-replay-runtime-composition.mjs
node scripts/verify/verify-index-replay-shadow-host-dispatch.mjs
node scripts/verify/verify-index-replay-shadow-graphql-transport.mjs
```
