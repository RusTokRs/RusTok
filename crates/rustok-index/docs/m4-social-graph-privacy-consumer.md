# M4 Social Graph privacy parity shadow

Date: 2026-07-30

Status: `source_complete_metrics_evidence_tooling_execution_pending`

Previous source-only Status: `source_complete_metrics_execution_pending`.

This slice provides the first production-shaped consumer of `SharedIndexQueryRuntime`
without granting an eventually consistent projection privacy authority. Social Graph remains
the sole decision source for notification block/mute policy; Index is queried only as a
comparison shadow.

The shadow is default-off and activates only with
`RUSTOK_SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_ENABLED=true`. It exists to collect operational
parity signals before any authoritative cutover is considered.

## Typed Index adapter

`IndexSocialGraphPrivacyReadPort` implements the existing
`SocialGraphPrivacyReadPort` contract. It receives only the neutral
`SharedIndexQueryRuntime`; it does not receive a database connection, construct
`PostgresIndexQueryPort`, or read `social_graph_relations`.

The adapter builds typed queries against the owner-published
`rustok-social-graph::relation@1` schema:

- block checks `relation_kind = block` and accepts a block in either direction;
- mute remains directional from notification recipient to actor;
- point follow remains directional and retains source-actor authorization;
- bounded follow batches retain the 100-target maximum, deduplicate input, use typed `In`,
  validate every projected UUID, and return deterministic sorted IDs.

Inactive relations are absent because the owner projection publishes an Index tombstone for
an inactive revision. Relation revision remains Index `source_version`; the adapter does not
invent a selectable revision field.

## Non-authoritative shadow and observation contract

`IndexShadowSocialGraphPrivacyReadPort` wraps the authoritative owner port plus the typed
Index projection adapter. Every method executes the owner read first. If that read fails, the
owner error is returned and no projection result can replace it. After owner success, the
shadow executes the equivalent Index read and always returns the owner result.

The shadow measures the original port call before entering the owner read and gives the
projected comparison only the remaining caller deadline budget. If the owner consumes the
budget or the projected query does not finish in the remaining time, the comparison becomes
the existing retryable `social_graph.index_privacy_unavailable` observation. The owner result
is still returned, so a non-authoritative Index comparison cannot extend notification policy
past the caller's declared budget.

The Social Graph crate classifies the comparison into a neutral
`IndexPrivacyShadowObservation`. The observation contains only a fixed operation, fixed
outcome, comparison duration, optional bounded failure code, and optional retryability. The
owner crate does not import `rustok-telemetry` and does not know about Prometheus.

Boolean outcomes are `match_positive`, `match_negative`, `false_negative`, and
`false_positive`. A `false_negative` means authoritative Social Graph returned true while
Index returned false; this is the critical negative-result safety signal for a future privacy
cutover.

Follow-batch IDs are compared as sets and classified as `match_batch_empty`,
`match_batch_nonempty`, `batch_missing`, `batch_extra`, or `batch_mixed`. `batch_missing`
means Index omitted at least one owner-confirmed target; `batch_extra` means Index returned
only additional targets; `batch_mixed` contains both directions of drift.

Index mismatch, missing tenant schema, storage failure, deadline exhaustion, or contract error
remains observational. It never authorizes, suppresses, widens, or otherwise changes
notification policy. Logs contain operation, fixed outcome, booleans/counts, bounded failure
code, retryability, and comparison duration. They contain no tenant, user, relation, entity,
payload, SQL, or storage details.

## Host-owned Prometheus adapter

`rustok-server` owns the adapter from `IndexPrivacyShadowObservation` to
`rustok-telemetry`. This keeps the Social Graph owner contract telemetry-neutral while using
the single Prometheus registry for the process.

The collector publishes five metric families:

- `rustok_social_graph_index_privacy_shadow_collector_started_timestamp_seconds` with no
  labels, used only to prove that a retained start/end window came from one collector epoch;
- `rustok_social_graph_index_privacy_shadow_observations_total{operation,outcome}`;
- `rustok_social_graph_index_privacy_shadow_failures_total{operation,error_code,retryable}`;
- `rustok_social_graph_index_privacy_shadow_comparison_duration_seconds{operation,outcome}`;
- `rustok_social_graph_index_privacy_shadow_last_observation_timestamp_seconds{operation,outcome}`.

Projection failures use the observation outcome `error` plus a failure counter. Error labels
are bounded to `social_graph.index_privacy_unavailable`,
`social_graph.index_privacy_contract_invalid`, or `other`. Tenant and user identifiers,
relation/entity IDs, payloads, SQL, and raw storage errors are forbidden labels.

Enabling the shadow requires successful collector registration. Metrics-disabled or
uninitialized telemetry fails shadow activation rather than silently running an unmeasured
evidence mode.

## Retained evidence capture

The owner first saves two ordinary Prometheus text scrapes around an explicit UTC window. The
repository capture tool reads those local files; it does not perform HTTP, start the server,
run Cargo, or connect to PostgreSQL.

`capture-social-graph-privacy-shadow.mjs` requires an explicit opt-in, an exact clean Git
commit, a stable run key, distinct start/end files, and a positive window of at most seven
days. It parses only the five approved shadow metric families, rejects unknown labels and
shadow-prefixed metrics, validates histogram/error consistency, rejects counter resets, and
requires the collector-start gauge to be identical in both snapshots.

The retained bundle contains exactly:

- `start.prom` — a deterministic canonical whitelist export of the approved shadow series;
- `end.prom` — the corresponding canonical end export;
- `capture.json` — descriptor published last with source identity, UTC window, hashes,
  computed deltas, bounded p95 bucket upper bounds, and the authority boundary.

The full process scrape is never copied into the bundle. Unrelated application metrics and
their labels are discarded before publication. The bundle is fresh/no-clobber and contains
only regular non-symlink files.

## Independent admission

`admit-social-graph-privacy-shadow.mjs` is a separate read-only review command. The reviewer
supplies repository, commit, and run key independently, plus explicit minimum-observation,
maximum-error-rate, and maximum-p95 thresholds.

Admission verifies the exact three-file inventory, rejects aliases/symlinks/extras, validates
all hashes and descriptor fields, reparses both retained `.prom` files, recomputes every delta,
and rechecks all bytes after review. The receipt is created outside the immutable bundle with
no-clobber semantics.

Integrity and policy are deliberately separate:

- `admitted: true` means the exact bundle passed integrity/provenance review;
- `policy_passed` records the reviewer policy result;
- `authoritative_cutover_authorized: false` is always retained.

A mismatch window can therefore be honestly admitted with `policy_passed: false`. Even a
window with `policy_passed: true` proves only that one bounded parity/latency window met the
supplied policy. It does not prove projection freshness, watermark/lag safety, repair behavior,
or authorize a privacy cutover.

The machine contract is
`crates/rustok-social-graph/contracts/social-graph-index-privacy-shadow-evidence.json`.
Capture contract: `social_graph_index_privacy_shadow_window_capture_v1`. Admission contract:
`social_graph_index_privacy_shadow_window_admission_v1`.

## Server composition

The final server facade:

1. completes ordinary provider composition;
2. materializes `SharedIndexQueryRuntime` from the source-owned registry and host database;
3. parses the default-off shadow flag;
4. when disabled, publishes the unchanged owner-backed notification policy;
5. when enabled, requires successful privacy-shadow metrics registration;
6. requires the shared query runtime and recomposes the policy with
   `IndexShadowSocialGraphPrivacyReadPort` plus the host-owned Prometheus adapter;
7. preserves explicitly registered `NotificationBlockReadRuntime` and
   `NotificationMuteReadRuntime` overrides.

An invalid flag, unavailable metrics registry, or missing runtime while shadow is enabled
fails bootstrap. The shadow still uses the authoritative owner result for every policy
decision.

## Why authoritative cutover remains blocked

A successful Index query can still be stale without returning an error. Therefore an absent
projected block or mute cannot safely prove absence in authoritative Social Graph storage.
Schema readiness and one passing parity window do not provide per-tenant catch-up, bounded
lag, or negative-result freshness safety.

Before enabling the shadow broadly or considering cutover, the owner must retain evidence for:

- current per-tenant projection watermark and bounded lag;
- replay/repair behavior after missing or corrupted projection state;
- positive and negative block/mute result parity;
- behavior during worker outage, reconnect, restart, and backlog catch-up;
- acceptable shadow latency and resource overhead;
- an explicit fail-closed freshness policy for negative projection results;
- one independently reviewed retained metric window from the merged source.

Authoritative cutover remains blocked after this tooling slice.

## Boundary

This slice does not:

- make Index authoritative for notification privacy;
- change the policy result based on Index mismatch, timeout, or failure;
- allow the projected comparison to exceed the remaining caller deadline budget;
- activate the shadow by default;
- move Social Graph source authority or replay ownership into Index;
- change commands, revision checks, event publication, projection, or DLQ handling;
- use Index for revision-bearing `SocialGraphFollowReadPort` results;
- cut profile privacy, presentation authorization, GraphQL, storefront, or admin reads over;
- enable the Social Graph projection worker or notification candidate worker by default;
- execute the shadow, capture a scrape, admit a bundle, or claim live evidence;
- define a universal production threshold or authorize an authoritative cutover;
- add many-link aggregate ordering or another source schema.

## Owner validation

Not run by the implementation agent, per maintainer instruction.

Suggested source checks:

```bash
cargo test -p rustok-telemetry social_graph_index_privacy_shadow_metrics -- --nocapture
cargo test -p rustok-social-graph --features index index_privacy -- --nocapture
cargo test -p rustok-server host_materializes_index_query_runtime_after_source_registry -- --nocapture
cargo check -p rustok-telemetry --all-targets
cargo check -p rustok-social-graph --features index --all-targets
cargo check -p rustok-server --all-targets
node --test scripts/evidence/social-graph-privacy-shadow-evidence.test.mjs
node scripts/verify/verify-social-graph-privacy-shadow-evidence.mjs
node scripts/verify/verify-index-social-graph-privacy-consumer.mjs
node scripts/verify/verify-social-graph-notification-policy.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo xtask module validate index
cargo xtask module validate social_graph
```

Owner capture example after saving two process metric scrapes:

```bash
SOCIAL_GRAPH_PRIVACY_SHADOW_ALLOW_CAPTURE=1 \
SOCIAL_GRAPH_PRIVACY_SHADOW_START_PROM=<start-scrape-file> \
SOCIAL_GRAPH_PRIVACY_SHADOW_END_PROM=<end-scrape-file> \
SOCIAL_GRAPH_PRIVACY_SHADOW_WINDOW_STARTED_AT=<utc-rfc3339> \
SOCIAL_GRAPH_PRIVACY_SHADOW_WINDOW_ENDED_AT=<utc-rfc3339> \
SOCIAL_GRAPH_PRIVACY_SHADOW_COMMIT=<40-char-merged-commit> \
SOCIAL_GRAPH_PRIVACY_SHADOW_RUN_KEY=<stable-run-key> \
  node scripts/evidence/capture-social-graph-privacy-shadow.mjs
```

Independent admission example:

```bash
SOCIAL_GRAPH_PRIVACY_SHADOW_ALLOW_ADMISSION=1 \
SOCIAL_GRAPH_PRIVACY_SHADOW_BUNDLE=<capture-root> \
SOCIAL_GRAPH_PRIVACY_SHADOW_EXPECTED_COMMIT=<40-char-merged-commit> \
SOCIAL_GRAPH_PRIVACY_SHADOW_EXPECTED_RUN_KEY=<stable-run-key> \
SOCIAL_GRAPH_PRIVACY_SHADOW_MIN_OBSERVATIONS=<reviewer-threshold> \
SOCIAL_GRAPH_PRIVACY_SHADOW_MAX_ERROR_RATE_BPS=<reviewer-threshold> \
SOCIAL_GRAPH_PRIVACY_SHADOW_MAX_P95_SECONDS=<reviewer-threshold> \
SOCIAL_GRAPH_PRIVACY_SHADOW_ADMISSION_OUTPUT=<existing-parent>/privacy-shadow-admission.json \
  node scripts/evidence/admit-social-graph-privacy-shadow.mjs
```
