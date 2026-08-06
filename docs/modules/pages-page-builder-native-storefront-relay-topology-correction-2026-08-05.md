# Pages / Page Builder Native Storefront Relay Topology Correction

Date: 2026-08-05
Status: source-audit-corrected / production-listener-acknowledgement-gap-open / execution-pending
Scope: continuity evidence boundary between the retained synchronous test target and the production server event topology

## Topology correction

The retained continuity harness is still useful, but its earlier description combined two different delivery topologies.

The test topology is:

```text
OutboxRelay
  → custom ContinuityTarget
  → real PageCacheInvalidationEventHandler
  → test-target acknowledgement
  → sys_events marked dispatched
```

The production server topology is:

```text
OutboxRelay
  → configured server relay target
  → local/remote transport acceptance
  → listener_bus
  → asynchronous module EventDispatcher
  → Pages event listener
```

The test uses the real relay and real Pages handler, but it does not instantiate the production relay target, listener bus or module dispatcher.

## Corrected claim

The continuity packet proves all of the following at source level:

- reviewed Page Builder publication writes the expected durable Pages lifecycle events;
- the real Pages invalidation handler maps `NodeUpdated` and `NodePublished` to the expected scopes;
- a synchronous target can keep relay acknowledgement behind successful handler completion;
- generation rotation changes the registered native storefront composite key;
- the old key remains physically present while the new generation-bound key is used;
- the new response points to the same reviewed immutable artifact.

It does not prove that the production module listener has completed when the production outbox row becomes `dispatched`.

## Production listener acknowledgement gap

In the current production source, the relay acknowledges after its configured transport target succeeds. Module listeners consume the separate `listener_bus` through `EventDispatcher`, which filters handlers with `EventHandler::handles` and executes matching handlers asynchronously.

A process crash or listener failure after transport acceptance but before Pages invalidation completion is therefore outside the retained continuity evidence. Existing in-memory dispatcher retries do not turn the already-dispatched outbox row back into a durable pending row.

This is a source-topology observation, not executed failure evidence.

## Required implementation decision

The next production slice must select one owner model:

### Option A — synchronous idempotent relay gate

Wrap the production relay target with a Pages invalidation transport that:

1. recognizes Pages lifecycle events;
2. serializes and deduplicates work by stable event UUID;
3. runs the real Pages invalidation runtime before downstream transport acceptance;
4. commits dedupe state only after invalidation succeeds;
5. prevents the module listener from rotating the same event a second time.

This makes relay retry/acknowledgement directly own generation rotation.

### Option B — durable listener receipt

Keep module-listener ownership, but persist a durable consumer receipt and defer or reconcile outbox acknowledgement until the Pages listener succeeds. The receipt must be idempotent and survive process restart.

This keeps listener ownership but requires a broader durable-consumer protocol.

## Recommendation

Use Option A for Pages cache generation rotation. The work is bounded, idempotent, server-local and already expressed as a typed invalidation request/receipt. Other asynchronous module listeners can continue to consume the downstream event after the generation gate succeeds.

The implementation must explicitly prevent duplicate rotation by the existing Pages module listener under both outbox delivery profiles.

## Source changes in this correction

- corrected the continuity evidence format and status;
- recorded that a custom synchronous relay target is used;
- recorded that the production server target and module dispatcher are not used by the harness;
- corrected the continuity packet and canonical parity plan;
- made the verifier inspect the real server transport/listener topology;
- normalized a duplicated SSR guard in the native storefront adapter;
- made no production behavior change.

## Evidence boundary

No tests, Cargo commands, formatting, verifiers, server processes, databases, workflows or CI were run. All execution fields remain empty or false.

FFA/FBA promotion remains blocked.
