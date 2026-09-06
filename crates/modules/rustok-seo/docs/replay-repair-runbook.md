# Runbook: SEO index replay/repair operations

## When to use

- backlog in `seo_index_deliveries` grows in `failed`/`dead_letter`;
- replay timeline is stuck and `replay_mode` does not move forward;
- after a tenant rollback/migration, safe forward-only repair/replay steps are needed.

## Operational order (tenant-safe)

1. Capture the current status: `seoIndexDeliveryStatus` or `GET /api/seo/index/tracking`.
2. If there are `failed`/`dead_letter` entries, queue `repair_only` (`runSeoIndexRepairReplay` with `replayHistorical=false`). The mutation validates and persists the job; it no longer performs repair work in the request.
3. Ensure the server SEO background worker is enabled through `runtime.background_workers.seo_bulk_enabled`. The existing poller advances at most one bulk job, one sitemap phase, and one index repair/replay job per poll.
4. Re-check index tracking after the worker completes the queued job. For historical backfill, then queue `repair + historical replay` (`replayHistorical=true`).
5. Re-running replay is idempotent: already sent historical transitions are not duplicated.
6. Verify the cursor timeline: expect forward-only progression (`not_started -> repair_only -> replay_requested -> replaying -> replay_completed`) without backward transitions.

## Troubleshooting

- **Queued work does not progress**: confirm the host runs background workers and `runtime.background_workers.seo_bulk_enabled` is enabled.
- **`PERMISSION_DENIED`**: the operator needs `seo:manage`.
- **`BAD_USER_INPUT`**: check `target_type` (`content|product`) and `limit` (`1..500`).
- **`dead_letter` remains after replay**: queue `repair_only`, then re-check the health of the index consumer/outbox relay.
- **Repeated replay produces no new deliveries**: this is expected with dedup when there are no new historical transitions.

## Verification evidence (last batch)

- `cargo test -p rustok-seo services::events::tests::historical_replay_deduplicates_repeat_runs` *(added)*
- `cargo test -p rustok-seo services::events::tests::historical_replay_retries_failed_delivery_without_duplicate_rows` *(added)*
- `cargo test -p rustok-seo services::events::tests::index_delivery_flow_has_transport_parity_for_memory_and_streaming_levels` *(added)*
- `cargo test -p rustok-seo-render --lib` *(extended with snapshot parity tests)*
