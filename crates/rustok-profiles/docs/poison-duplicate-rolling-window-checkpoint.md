# Profiles checkpoint: bounded physical DLQ duplicate rolling window

Status: **cross-cycle rolling state plus moving scanner and server integration source-complete; external runtime evidence pending**.

## Why this belongs in the Profiles improvement trail

Profiles never authorizes presentation from broker, receipt, metric, evidence, duplicate, cursor, or rolling state. Downstream reliability still must not hide a physical duplicate merely because copies were observed in adjacent advancing scan cycles.

```text
complete cross-cycle observations
  -> bounded rolling classification
  -> count-only alert summary
  -> identifier-free server snapshot
```

## Bounded semantics

`DlqDuplicateRollingWindowPolicy` requires explicit positive cycle and per-cycle observation bounds. Their checked product cannot exceed 10,000, and no production default is provided.

One `push_cycle` represents one complete cycle. The state:

- detects ordinary or conflicting copies split across retained cycles;
- rejects oversized input without changing prior state;
- evicts only the oldest complete cycle;
- exposes only aggregate summary and retention counts.

## Truncation boundary

After any eviction, every later snapshot reports `history_truncated = true`. An evicted copy can remove a relationship from the retained summary, so the result is never complete-history or current-tail evidence.

## Moving scanner and server composition

The moving scanner and server integration are source-complete.

Each selected partition owns one private process-local per-partition cursor. A complete equal-budget all-partition candidate is collected before mutation. Cursors and rolling state update together only after the combined cycle is accepted.

The server mode is explicit opt-in through `moving_window`. Reviewed fail-closed configuration supplies initial offset, per-partition cap, batch size, rolling cycle count, and per-cycle observation capacity. Moving results are reduced to the existing identifier-free duplicate summary before alert policy evaluation.

Failed moving cycles preserve connected process-local state. A replacement connection or process restart resets to the reviewed initial offset with empty rolling history. No restart-safe progress or current-tail claim is made.

## Profiles authorization boundary

Profiles never authorizes visibility, `followers_only`, follow controls, search inclusion, storefront presentation, GraphQL output, or author cards from:

- rolling summary or retention counts;
- `history_truncated`;
- private cursor position, advancement, or reset;
- moving observer mode or availability;
- Iggy deployment mode;
- receipt state or retained evidence.

Privacy remains resolved through authoritative owner ports before localized and Media-backed presentation.

## Remaining work

1. retain external-Iggy cross-cycle execution evidence;
2. review initial offset and reset frequency per deployment;
3. add persistent cursor ownership only if restart continuity is required;
4. retain server observer execution evidence;
5. add identifier-free telemetry and optional operational health.

Tests, Cargo commands, formatters, verifiers, broker scans, server composition execution, telemetry registration, and retained capture were not run by the implementation agent.
