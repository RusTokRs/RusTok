# M4 Social Graph privacy consumer cutover

Date: 2026-07-30

Status: `source_complete_execution_pending`

This slice provides the first authorized production consumer source for
`SharedIndexQueryRuntime`. It can move Social Graph block/mute reads used by notification
recipient policy from direct owner-table reads to the generic Index query port while keeping
Social Graph as the contract and replay owner.

Activation is intentionally default-off. The final host selects the Index-backed policy only
when `RUSTOK_SOCIAL_GRAPH_INDEX_PRIVACY_READS_ENABLED=true`. Before activation, the existing
authoritative owner read path remains in place. This avoids making an ordinary deployment
unavailable while the projection worker is still default-off and live readiness/freshness
evidence remains an owner action.

## Owner adapter

`IndexSocialGraphPrivacyReadPort` implements the existing
`SocialGraphPrivacyReadPort` contract. It receives only a clone of the neutral
`SharedIndexQueryRuntime`; it does not receive a database connection, construct
`PostgresIndexQueryPort`, or read `social_graph_relations`.

The adapter builds typed queries against the owner-published
`rustok-social-graph::relation@1` schema:

- block suppression checks `relation_kind = block` and accepts a block in either direction;
- mute suppression remains directional from notification recipient to actor;
- point follow reads remain directional and retain source-actor authorization;
- bounded follow batches retain the existing maximum of 100 targets, deduplicate input,
  query through `In`, validate every projected UUID, and return deterministic sorted IDs.

Inactive relations are absent because owner projection publishes an Index tombstone for an
inactive revision. The adapter therefore does not add an independent active-state field or
reinterpret source revision.

## Authorization and failure contract

Every method preserves `PortCallPolicy::read`, tenant parsing, self-relation rejection, and
the existing user/source actor rule for follow reads. Notification policy calls use the
existing bounded service actor and matching tenant context.

`SchemaNotReady` and Index storage failures map to retryable
`social_graph.index_privacy_unavailable`. Query-plan, compiler, decoder, backend, and
projected-result contract failures map to the non-retryable invariant code
`social_graph.index_privacy_contract_invalid`.

After activation, notification policy therefore uses retryable fail-closed behavior: it does
not authorize from missing or stale Index state, does not convert a failed block/mute read
into `Allow`, and does not silently fall back to Social Graph tables.

## Final host composition

The server facade keeps the existing provider bootstrap as an internal base step, then:

1. materializes the canonical PostgreSQL `SharedIndexQueryRuntime`;
2. parses the explicit default-off activation flag;
3. when disabled, preserves the authoritative owner policy from the base bootstrap;
4. when enabled, requires the shared runtime and recomposes
   `ServerNotificationRecipientPolicy` with `IndexSocialGraphPrivacyReadPort`;
5. preserves explicitly registered `NotificationBlockReadRuntime` or
   `NotificationMuteReadRuntime` overrides;
6. publishes only the selected final extension set.

An invalid activation value fails bootstrap. Once the flag is enabled, a missing shared
runtime also fails bootstrap and no owner-table fallback remains in that activated path.

## Boundary

This slice does not:

- activate Index privacy reads by default;
- claim that the default-off projection consumer is already healthy or caught up;
- move Social Graph source authority or replay ownership into Index;
- change relation commands, revision checks, event publication, projection, or DLQ handling;
- use Index for revision-bearing `SocialGraphFollowReadPort` results;
- cut profile privacy, presentation authorization, GraphQL, storefront, or admin reads over;
- enable the notification candidate worker by default;
- claim projection freshness, PostgreSQL execution evidence, or live equivalence;
- add many-link aggregate ordering or another source schema.

Activation requires retained projection readiness, lag, and result-parity evidence plus an
operator decision to set the flag on the consuming host.

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
