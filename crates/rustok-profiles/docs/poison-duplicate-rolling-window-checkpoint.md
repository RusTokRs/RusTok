# Profiles checkpoint: bounded physical DLQ duplicate rolling window

Status: **cross-cycle rolling-window state source-complete; scanner/cursor integration and runtime evidence pending**.

## Why this belongs in the Profiles improvement trail

Profiles never authorizes presentation from broker, receipt, metric, evidence, or duplicate-inspection state. However, downstream processing reliability must not hide a physical duplicate merely because its copies were observed in adjacent scan cycles.

The existing fixed scans classify one bounded result. `DlqDuplicateRollingWindow` adds bounded cross-cycle identity retention without exporting message identity or turning operational state into profile policy.

## Delivered source boundary

Owner source:

```text
crates/rustok-iggy/src/dlq_duplicate_rolling_window.rs
```

Public API:

```text
DlqDuplicateRollingWindowPolicy
DlqDuplicateRollingWindow
DlqDuplicateRollingWindowSnapshot
DlqDuplicateRollingWindowError
```

Machine contract:

```text
crates/rustok-iggy/contracts/evidence/
  dlq-duplicate-rolling-window-source.json
```

Verifier:

```text
scripts/verify/verify-iggy-dlq-duplicate-rolling-window.mjs
```

Owner guide:

```text
crates/rustok-iggy/docs/dlq-duplicate-rolling-window.md
```

## Bounded semantics

The caller explicitly supplies a positive cycle count and positive per-cycle observation bound. Their checked product cannot exceed 10,000, and no production default is provided.

One successful `push_cycle` represents one complete scan cycle. The state:

- detects an ordinary duplicate split across retained cycles;
- detects conflicting exact bytes for one deterministic ID split across retained cycles;
- rejects an oversized cycle without changing prior state;
- evicts only the oldest complete cycle;
- exposes only aggregate summary and retention counts.

## Truncation boundary

After an eviction, every snapshot reports:

```text
history_truncated = true
```

An older copy may have been removed, so a later retained summary may no longer show the original duplicate relationship. The snapshot is therefore not complete history, current-tail proof, or evidence that production retention is sufficient.

## Profiles authorization boundary

Profiles never authorizes visibility, `followers_only`, follow controls, search inclusion, storefront presentation, GraphQL output, or author cards from:

- rolling-window summary counts;
- retained or evicted cycle counts;
- `history_truncated`;
- scan cadence or cursor position;
- Iggy deployment mode;
- receipt state or retained evidence.

Privacy remains resolved through authoritative owner ports before localized and Media-backed presentation. Operational state remains operational only.

## Remaining integration

The source-complete state deliberately does not yet define:

1. how the Iggy scanner feeds one complete cycle without identifier export;
2. how each physical partition advances its cursor;
3. whether state is persisted or reset on restart;
4. how mode-aware server composition uses the rolling window;
5. external-Iggy cross-cycle runtime evidence;
6. identifier-free telemetry and health projection.

Tests, Cargo commands, formatters, source verifiers, broker scans, server composition, telemetry registration, and retained capture were not run by the implementation agent.
