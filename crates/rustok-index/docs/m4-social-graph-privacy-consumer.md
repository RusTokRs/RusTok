# M4 Social Graph privacy parity shadow

Date: 2026-07-30

Status: `source_complete_execution_pending`

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

- block checks `relation_kind = block` and accept a block in either direction;
- mute remains directional from notification recipient to actor;
- point follow remains directional and retains source-actor authorization;
- bounded follow batches retain the 100-target maximum, deduplicate input, use typed `In`,
  validate every projected UUID, and return deterministic sorted IDs.

Inactive relations are absent because the owner projection publishes an Index tombstone for
an inactive revision. Relation revision remains Index `source_version`; the adapter does not
invent a selectable revision field.

## Non-authoritative shadow

`IndexShadowSocialGraphPrivacyReadPort` wraps two ports:

- the authoritative owner `SocialGraphPrivacyReadPort`;
- the typed `IndexSocialGraphPrivacyReadPort` projection adapter.

Each method executes the owner read first. If that read fails, the existing owner error is
returned and no projection result can replace it. After owner success, the shadow executes
the equivalent Index read, records only bounded operation/result information, and always
returns the owner result.

Index mismatch, missing tenant schema, storage failure, or contract error is observational
only in this slice. It never authorizes, suppresses, widens, or otherwise changes notification
policy. Logs contain operation, booleans/counts, bounded stable error code, and retryability;
they contain no tenant, user, relation, entity, payload, SQL, or storage details.

## Server composition

The final server facade:

1. completes ordinary provider composition;
2. materializes `SharedIndexQueryRuntime` from the source-owned registry and host database;
3. parses the default-off shadow flag;
4. when disabled, publishes the unchanged owner-backed notification policy;
5. when enabled, requires the shared runtime and recomposes the policy with
   `IndexShadowSocialGraphPrivacyReadPort`;
6. preserves explicitly registered `NotificationBlockReadRuntime` and
   `NotificationMuteReadRuntime` overrides.

An invalid flag or missing runtime while shadow is enabled fails bootstrap. The shadow still
uses the authoritative owner result for every policy decision.

## Why authoritative cutover remains blocked

A successful Index query can still be stale without returning an error. Therefore an absent
projected block or mute cannot safely prove absence in authoritative Social Graph storage.
Schema readiness alone does not provide per-tenant catch-up, bounded lag, or negative-result
safety.

Before enabling the shadow or considering cutover, the owner must retain evidence for:

- current per-tenant projection watermark and bounded lag;
- replay/repair behavior after missing or corrupted projection state;
- positive and negative block/mute result parity;
- behavior during worker outage, reconnect, restart, and backlog catch-up;
- acceptable shadow latency and resource overhead;
- an explicit fail-closed freshness policy for negative projection results.

## Boundary

This slice does not:

- make Index authoritative for notification privacy;
- change the policy result based on Index mismatch or failure;
- activate the shadow by default;
- move Social Graph source authority or replay ownership into Index;
- change commands, revision checks, event publication, projection, or DLQ handling;
- use Index for revision-bearing `SocialGraphFollowReadPort` results;
- cut profile privacy, presentation authorization, GraphQL, storefront, or admin reads over;
- enable the Social Graph projection worker or notification candidate worker by default;
- claim PostgreSQL execution, live parity, freshness, latency, or retained evidence;
- add many-link aggregate ordering or another source schema.

## Owner validation

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
cargo test -p rustok-social-graph --features index index_privacy -- --nocapture
cargo test -p rustok-server host_materializes_index_query_runtime_after_source_registry -- --nocapture
cargo check -p rustok-social-graph --features index --all-targets
cargo check -p rustok-server --all-targets
node scripts/verify/verify-index-social-graph-privacy-consumer.mjs
node scripts/verify/verify-social-graph-notification-policy.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo xtask module validate index
cargo xtask module validate social_graph
```
