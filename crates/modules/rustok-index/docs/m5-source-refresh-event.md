# M5 exact source refresh event worker

Status: `source_complete_owner_event_publication_and_runtime_wiring_pending`.

## Purpose

Some owner events should not copy a complete Index record into the transport payload. Product and
ProductVariant are the motivating examples: their canonical records include localized state,
relations, retained tombstones, and monotonic owner revisions that already belong to registered
`IndexSource` adapters.

`IndexSourceRefreshEventWorker` provides a generic bridge from one thin owner change notification to
one exact authoritative source load. It is independent of Product, any broker SDK, PostgreSQL, and
server lifecycle code.

## Delivery contract

`IndexSourceRefreshEventDelivery<T>` carries only:

- one registered event domain;
- one non-nil owner event UUID;
- one exact `EntityKey` containing tenant, schema, entity, and optional locale;
- one positive minimum owner source version;
- one opaque broker acknowledgement token.

The event UUID becomes the durable Index inbox identity. The token remains opaque to Index and is
used only after durable mutation completion.

The event payload must not contain a copied Index record. The registered owner source remains the
canonical authority for fields, links, deletion state, and source version.

## Processing order

The worker performs these steps in order:

1. resolve the exact immutable mutation-event route;
2. require the delivery schema to equal the route schema;
3. resolve the immutable source for that schema;
4. require the resolved source name to equal the route source name;
5. perform one bounded `IndexSourceLoadRequest` for exactly one key;
6. require exactly one returned upsert or tombstone mutation;
7. require the loaded source version to be at least the event's minimum owner version;
8. replace the replay-only mutation UUID with the broker event UUID;
9. durably apply the mutation through `IndexReplayMutationSink`;
10. acknowledge the broker token.

A later consumer may observe a source revision newer than the event minimum. That is valid and
convergent. A missing result or a revision below the event fence is not terminal and suppresses both
mutation apply and acknowledgement.

Applied, duplicate, and stale mutation outcomes are terminal only after the mutation sink has
returned successfully. An acknowledgement failure after durable apply is returned to the transport
owner; redelivery remains safe through the event UUID inbox identity and monotonic source version.

## Fail-closed boundaries

The worker rejects or leaves unacknowledged:

- unknown event domains;
- schema or source-route mismatches;
- missing replay sources;
- source contract/storage failures;
- empty exact loads;
- ambiguous exact loads;
- source revisions behind the owner event fence;
- mutation persistence failures.

It performs no SQL, starts no task, selects no broker, owns no retry loop, logs no acknowledgement
token, and exposes no public transport.

## Product follow-up

This slice intentionally does not add Product wire events or publish Product routes. The next owner
slice must define reviewed Product locale and ProductVariant refresh event families, update the
committed `rustok-events` release digests, publish exact owner revisions transactionally, and compose
a concrete consumer/acknowledger around this worker.

Legacy `ProductCreated`, `ProductUpdated`, `ProductDeleted`, `VariantCreated`, `VariantUpdated`, and
`VariantDeleted` root events are not sufficient because they do not carry the canonical
`index_revision` fence or exact locale/tombstone identity.

## Deliberate limits

This slice does not:

- add batch refresh events;
- register Product or ProductVariant production routes;
- modify the shared event wire schema or release digest artifact;
- start or configure an Iggy consumer;
- change retry, DLQ, lag, or poison handling;
- retain PostgreSQL or broker execution evidence;
- alter the separate concrete-repair evidence gate.

The primary implementation cursor remains `M6 - execute and admit concrete repair evidence`.

## Maintainer validation

```bash
cargo test -p rustok-index source_refresh_event --lib -- --nocapture
node scripts/verify/verify-index-source-refresh-event.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, database scenarios, workflows, or CI were run by
the implementation agent.
