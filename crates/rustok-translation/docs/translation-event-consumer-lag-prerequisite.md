---
id: doc://crates/rustok-translation/docs/translation-event-consumer-lag-prerequisite.md
kind: implementation_handoff
language: en
status: in_progress
last_reviewed: 2026-09-03
---

# Translation event-consumer lag prerequisite

Status: **source prerequisite only / runtime evidence open**

Reviewed against `main@51d2147bd920c7c580c0eee47f376035e8d8b77a` (tree `d35f48df95c12644dbe6cac439e15d06476bc515`).

## What current main already proves

Translation workflow mutations publish sealed `TranslationWorkflowEvent` contracts through `TransactionalEventBus::publish_contract_in_tx`, so committed workflow changes have a durable outbox publication boundary. Translation also owns fixed-cardinality module metrics, including provider checkpoint freshness and elapsed checkpoint age.

Those module metrics are not broker lag. The checkpoint-age metric explicitly says it is not cursor distance, and Translation inventory checkpoints are owner-provider opaque cursors rather than broker consumer positions.

The shared Iggy/runtime infrastructure already provides the primitives required for a truthful implementation:

- `IggyConsumerPositionObserver` reads every topic partition plus committed consumer-group offsets and high-watermarks;
- incomplete snapshots fail closed instead of inventing a zero checkpoint;
- `rustok_runtime_consumer_lag{consumer, aggregation}` publishes exact `total` and `max` lag only together with position completeness;
- durable inbound consumers must reuse the host's already configured Iggy connector rather than creating a second broker lifecycle.

## Why the Phase 3 gate remains open

Current Translation composition does not define a Translation-specific durable inbound consumer, persistent consumer group, or explicit broker topic whose committed position represents Translation event processing. There is therefore no truthful consumer-position series to observe yet.

The central Phase 3 checkbox stays open. This prerequisite does not claim a runtime consumer, external-Iggy execution, restart/rebalance behavior, or measured lag.

## Required next runtime slice

Before closing the lag gate, implement a real durable Translation event consumer because a Translation-owned projection or recovery requirement needs one. Do not add a no-op consumer only to manufacture a lag metric.

The bounded runtime contract is:

1. name the durable consumer's actual responsibility, explicit topic, and persistent consumer group;
2. reuse the configured host Iggy transport/configuration and its lifecycle;
3. acknowledge a broker position only after the consumer's durable application succeeds;
4. observe the same group/topic with `IggyConsumerPositionObserver` across every partition;
5. publish the shared runtime position snapshot and exact `total`/`max` lag only when the snapshot is complete;
6. fail closed on missing group checkpoints, incoherent offsets, missing partitions, reconnect failure, or unavailable broker state;
7. retain executable broker evidence showing backlog growth, durable apply/ack progress, restart/reconnect, and lag convergence without fabricating state from timestamps or owner cursors.

## Forbidden substitutes

Do not satisfy the gate with Translation provider checkpoint age, a Translation target opaque cursor, event timestamp age, or one observed partition presented as a complete topic. These are useful signals for other purposes but they do not establish durable consumer-position lag.

## Scope of this prerequisite

This handoff adds only a source contract and verifier. It does not change runtime code, event schemas, topics, consumer groups, metrics, migrations, or the central plan checkbox. It intentionally does not overlap the active Settings or Forum UGC Translation prerequisites.
