# Profiles checkpoint: poison publish/mark ambiguity

Status: **source-complete runtime evidence pending**.

## Why this belongs in the Profiles improvement trail

Profiles privacy and authorization remain owner-port concerns, but the Social Graph Index is one downstream consumer of privacy-relevant relationship facts. Its malformed-delivery terminalization must not silently lose a source offset or claim stronger broker guarantees than the infrastructure provides.

The combined PostgreSQL/Iggy ordering harness already establishes:

```text
claim -> exact DLQ publish -> published -> source ack -> acknowledged bookkeeping
```

This checkpoint adds the missing cross-system ambiguity boundary: the broker may accept the deterministic DLQ message immediately before the process stops and before PostgreSQL records `published`.

## Locked source scenarios

Machine contract:

```text
crates/rustok-social-graph/contracts/evidence/index-raw-poison-publish-mark-ambiguity-source.json
```

Harness:

```text
crates/rustok-social-graph/tests/index_raw_poison_publish_mark_ambiguity.rs
```

Verifier:

```text
scripts/verify/verify-social-graph-index-raw-poison-publish-mark-ambiguity.mjs
```

Two distinct external-Iggy modes are required.

### Dedup enabled

The first publisher succeeds at the broker, remains `publishing` in PostgreSQL, and stops without source acknowledgement. After natural lease expiry, a recovery publisher receives the same source delivery and retries the same deterministic broker message ID.

Required physical DLQ counts:

```text
0 -> 1 -> 1
```

### Dedup disabled

The same sequence is repeated against a broker that accepts duplicate message IDs.

Required physical DLQ counts:

```text
0 -> 1 -> 2
```

This negative case is important: PostgreSQL publisher fencing alone does not provide physical exactly-once across a successful broker write and a missing `mark_published` write.

## Production implications

The source contract preserves the current worker order:

```text
reserve_and_claim
IggyTransport::move_to_dlq
mark_raw_poison_published
acknowledge_decode_failure
best-effort mark_acknowledged
```

It also preserves these recovery rules:

- a live lease returns `Busy`;
- an expired lease can be reclaimed;
- the old publisher is fenced with `ClaimLost`;
- `published` redelivery is acknowledgement-only;
- source acknowledgement never precedes the durable terminal result.

Server-side message-ID deduplication is therefore a physical-duplicate mitigation for the publish/mark crash window, not a PostgreSQL/Iggy transaction.

## Profiles authorization boundary

No relationship, privacy, visibility, block, mute, follow, or friendship policy is inferred from:

- poison receipt state;
- Iggy deduplication state;
- DLQ message count;
- source acknowledgement state;
- retained evidence metadata.

Profiles presentation still consumes authorized owner-port results. Broker and receipt evidence only protects the reliability and auditability of downstream processing.

## Remaining evidence

The following work remains explicit:

1. execute both ambiguity scenarios against reviewed PostgreSQL and two separately configured external-Iggy instances;
2. retain current-commit evidence without database URLs, broker addresses, credentials, payloads, offsets, UUIDs, schema names, stream names, or raw logs;
3. review whether production dedup expiry and capacity can contain the maximum supported publish-to-recovery interval;
4. keep a documented operational response for dedup-disabled or dedup-window-exhausted duplicate DLQ entries;
5. separately evaluate multi-replica lease ownership and broker failover.

No canonical execution packet exists for this checkpoint. Tests and verifiers were not run by the implementation agent.
