# Runbook: SEO index replay/repair operations

## When to use

- backlog in `seo_index_deliveries` grows in `failed`/`dead_letter`;
- replay timeline is stuck and `replay_mode` does not move forward;
- after a tenant rollback/migration, safe forward-only repair/replay steps are needed.

## Operational order (tenant-safe)

1. Capture the current status: `seoIndexDeliveryStatus` or `GET /api/seo/index/tracking`.
2. If there are `failed`/`dead_letter` entries, first run `repair_only` (`runSeoIndexRepairReplay` with `replayHistorical=false`).
3. For historical backfill, run `repair + historical replay` (`replayHistorical=true`).
4. Re-running replay is now idempotent: already sent historical transitions are not duplicated.
5. Verify the cursor timeline: expect forward-only progression (`not_started -> repair_only -> replay_requested -> replaying -> replay_completed`) without backward transitions.

## Troubleshooting

- **`PERMISSION_DENIED`**: the operator needs `seo:manage`.
- **`BAD_USER_INPUT`**: check `target_type` (`content|product`) and `limit` (`1..500`).
- **`dead_letter` remains after replay**: run `repair_only`, then re-check the health of the index consumer/outbox relay.
- **Repeated replay returns `replayed_count=0`**: this is expected with dedup (no new historical transitions).

## Verification evidence (last batch)

- `cargo test -p rustok-seo services::events::tests::historical_replay_deduplicates_repeat_runs` *(added)*
- `cargo test -p rustok-seo services::events::tests::historical_replay_retries_failed_delivery_without_duplicate_rows` *(added)*
- `cargo test -p rustok-seo services::events::tests::index_delivery_flow_has_transport_parity_for_memory_and_streaming_levels` *(added)*
- `cargo test -p rustok-seo-render --lib` *(extended with snapshot parity tests)*
