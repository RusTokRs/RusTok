# Profiles checkpoint: bounded physical DLQ duplicate rolling window

Status: **cross-cycle rolling-window state and moving scanner integration source-complete; server composition and runtime evidence pending**.

## Why this belongs in the Profiles improvement trail

Profiles never authorizes presentation from broker, receipt, metric, evidence, or duplicate-inspection state. However, downstream processing reliability must not hide a physical duplicate merely because its copies were observed in adjacent scan cycles.

The fixed scanners classify one bounded result. `DlqDuplicateRollingWindow` adds bounded cross-cycle identity retention, and the feature-gated moving scanner now supplies complete fair cycles with private process-local per-partition cursors. Neither exports message identity or turns operational state into profile policy.

## Delivered source boundary

Owner sources:

```text
crates/rustok-iggy/src/
  dlq_duplicate_rolling_window.rs
  dlq_duplicate_moving_window_scan.rs
```

Public rolling API:

```text
DlqDuplicateRollingWindowPolicy
DlqDuplicateRollingWindow
DlqDuplicateRollingWindowSnapshot
DlqDuplicateRollingWindowError
```

Moving integration API:

```text
IggyDlqDuplicateMovingWindowPolicy
IggyDlqDuplicateMovingWindowState
IggyDlqDuplicateMovingWindowSnapshot
IggyDlqDuplicateMovingWindowScanner
IggyDlqDuplicateMovingWindowError
```

Machine contracts:

```text
crates/rustok-iggy/contracts/evidence/
  dlq-duplicate-rolling-window-source.json
  dlq-duplicate-moving-window-scan-source.json
```

Verifiers:

```text
scripts/verify/verify-iggy-dlq-duplicate-rolling-window.mjs
scripts/verify/verify-iggy-dlq-duplicate-moving-window-scan.mjs
```

Owner guides:

```text
crates/rustok-iggy/docs/dlq-duplicate-rolling-window.md
crates/rustok-iggy/docs/dlq-duplicate-moving-window-scan.md
```

## Bounded semantics

The caller explicitly supplies a positive cycle count and positive per-cycle observation bound. Their checked product cannot exceed 10,000, and no production default is provided.

One successful `push_cycle` represents one complete scan cycle. The state:

- detects an ordinary duplicate split across retained cycles;
- detects conflicting exact bytes for one deterministic ID split across retained cycles;
- rejects an oversized cycle without changing prior state;
- evicts only the oldest complete cycle;
- exposes only aggregate summary and retention counts.

## Moving scanner integration

The moving scanner integration is source-complete.

Each selected partition has one private process-local per-partition cursor. Every cycle applies an equal bounded message budget, polls with explicit offsets and `auto_commit=false`, and advances all cursors only after every partition succeeds and the rolling state accepts the combined cycle.

An incomplete, invalid, or failed cycle preserves all cursors and the previous rolling snapshot. Public results expose only partition count, advanced-partition count, reset generation, and the existing identifier-free rolling snapshot.

Restart semantics are explicit reset:

- no cursor or observation persistence;
- new state starts from one reviewed initial offset;
- `reset_to_initial_offset()` rewinds every cursor and clears rolling history;
- no restart-safe progress, current-tail, or complete-history claim.

A persistent cursor owner remains separate work and is not implied.

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
- scan cadence, private cursor position, or reset generation;
- Iggy deployment mode;
- receipt state or retained evidence.

Privacy remains resolved through authoritative owner ports before localized and Media-backed presentation. Operational state remains operational only.

## Remaining integration

Moving scanner integration is source-complete. Remaining work is:

1. compose it as an explicit opt-in mode in the mode-aware server observer;
2. define reviewed configuration and fail-closed startup validation;
3. retain external-Iggy cross-cycle runtime evidence;
4. add a persistent cursor owner only if restart continuity is required;
5. define identifier-free telemetry and health projection.

Tests, Cargo commands, formatters, source verifiers, broker scans, server composition, telemetry registration, and retained capture were not run by the implementation agent.
