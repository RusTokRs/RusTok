# Документация `rustok-outbox`

`rustok-outbox` — core-модуль transactional event persistence и relay
infrastructure для event runtime платформы.

## Назначение

- публиковать канонический runtime entry type `OutboxModule`;
- держать write-side outbox contract и relay semantics вне host-слоя;
- давать платформе единый transactional publishing contract для событий.

## Зона ответственности

- `TransactionalEventBus` и atomic publish-with-transaction semantics;
- persistence в `sys_events` через transactional transport;
- relay, retry и DLQ semantics для event runtime;
- module-owned Leptos admin package `rustok-outbox-admin` с FFA-разделением `core/transport/ui` для read-only relay visibility.

## Политика relay, retry и DLQ

- Claim ownership хранится в `sys_events.claimed_by` и `sys_events.claimed_at`.
- PostgreSQL claim использует `FOR UPDATE SKIP LOCKED`; SQLite/test окружение использует guarded update fallback.
- `claim_ttl` определяет, когда зависший claim можно reclaim-ить.
- `RelayConfig.max_concurrency` ограничивает параллельный dispatch.
- Retry использует exponential backoff от `backoff_base` до `backoff_max`.
- Runtime `apps/server` выбирает `max_attempts` из `rustok.events.dlq.max_attempts`, если DLQ включён, иначе из `rustok.events.relay_retry_policy.max_attempts`.
- Временная ошибка оставляет событие в `pending`, увеличивает `retry_count`, пишет `last_error`, очищает claim и задаёт `next_attempt_at`.
- Терминальная ошибка переводит событие в `failed`/DLQ, сохраняет `last_error`, очищает claim и отражается в metrics/admin DLQ surface.

## Incident response

Primary owner для outbox/event delivery — Platform foundation on-call. Escalation path: владелец `crates/rustok-outbox`, затем владелец server runtime composition.

При росте backlog, retry или DLQ:

1. Проверить `/health/ready` и метрики `rustok_outbox_backlog_size`, `rustok_outbox_pending_lag_seconds`, `rustok_outbox_retries_total`, `rustok_outbox_dlq_total`.
2. Проверить состояние worker `worker:outbox_relay` и `rustok_runtime_worker_state{worker="outbox_relay"}`.
3. Для зависших claims сверить `claim_ttl`, `claimed_by`, `claimed_at` и дождаться reclaim либо выполнить штатную operator-процедуру replay/requeue.
4. Для DLQ не редактировать payload вручную: сначала классифицировать ошибку, подтвердить idempotency consumer-а и только затем запускать повторную доставку.
5. После rollback или requeue сохранить evidence: affected event ids, retry counts, DLQ count, health snapshot и итоговый статус downstream consumer-а.

## Интеграция

- используется `apps/server` для migrations, runtime relay bootstrap и event transport wiring;
- зависит от `rustok-core` для module contracts и event transport abstractions, а от `rustok-api` — для shared `PortContext`/`PortError` и write-policy primitives;
- может форвардить доставку в downstream transports вроде `rustok-iggy`, не владея provider-specific delivery semantics;
- остаётся `Core` module независимо от того, что часть bootstrap wiring живёт в host runtime.
- module-level `health()` возвращает `Degraded`, потому что без host `AppContext` модуль не может проверить `sys_events`, relay worker state, backlog, lag и DLQ; конкретные checks находятся в `/health/ready`.

## Проверка

- `cargo xtask module validate outbox`
- `cargo xtask module test outbox`
- `node scripts/verify/verify-outbox-admin-boundary.mjs`
- `node scripts/verify/verify-outbox-admin-boundary.test.mjs`
- `npm run verify:outbox:fba`
- targeted event-runtime tests для transactional publish, relay и backlog semantics

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Контракт manifest-слоя](../../../docs/modules/manifest.md)
