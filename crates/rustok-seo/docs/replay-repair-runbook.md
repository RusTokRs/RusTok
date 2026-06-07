# Runbook: SEO index replay/repair operations

## Когда использовать

- backlog в `seo_index_deliveries` растёт в `failed`/`dead_letter`;
- replay timeline застрял и `replay_mode` не двигается вперёд;
- после tenant rollback/миграции нужны безопасные forward-only repair/replay шаги.

## Операционный порядок (tenant-safe)

1. Снять текущий статус: `seoIndexDeliveryStatus` или `GET /api/seo/index/tracking`.
2. Если есть `failed`/`dead_letter`, сначала выполнить `repair_only` (`runSeoIndexRepairReplay` с `replayHistorical=false`).
3. Для исторического backfill выполнить `repair + historical replay` (`replayHistorical=true`).
4. Повторный запуск replay теперь idempotent: уже отправленные historical transitions не дублируются.
5. Проверить cursor timeline: ожидается forward-only progression (`not_started -> repair_only -> replay_requested -> replaying -> replay_completed`) без backward transitions.

## Troubleshooting

- **`PERMISSION_DENIED`**: оператору нужен `seo:manage`.
- **`BAD_USER_INPUT`**: проверить `target_type` (`content|product`) и `limit` (`1..500`).
- **`dead_letter` остаётся после replay**: выполнить `repair_only`, затем повторно проверить health index consumer/outbox relay.
- **Повторный replay возвращает `replayed_count=0`**: это ожидаемо при dedup (новых historical transitions нет).

## Verification evidence (последний batch)

- `cargo test -p rustok-seo services::events::tests::historical_replay_deduplicates_repeat_runs` *(added)*
- `cargo test -p rustok-seo services::events::tests::historical_replay_retries_failed_delivery_without_duplicate_rows` *(added)*
- `cargo test -p rustok-seo services::events::tests::index_delivery_flow_has_transport_parity_for_memory_and_streaming_levels` *(added)*
- `cargo test -p rustok-seo-render --lib` *(extended with snapshot parity tests)*
