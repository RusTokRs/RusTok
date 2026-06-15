# SEO operations runbook

Этот runbook фиксирует D9 baseline для production SEO Suite. Он дополняет `replay-repair-runbook.md` и покрывает три частых operational-сценария без изменения API-контрактов.

## Когда использовать

- backlog в `seo_event_deliveries` или `seo_index_deliveries` перестал уменьшаться;
- sitemap/robots или storefront metadata отстают от owner-module changes;
- оператору нужно безопасно запустить repair/replay без повторной публикации всех SEO entities.

## 1. SEO event backlog stuck

1. Проверить, что tenant module enabled и rollout flags не выключены для tenant-а.
2. Снять delivery summary через GraphQL/REST control-plane (`seoIndexDeliveryStatus` или `/api/seo/index/tracking`).
3. Сгруппировать failures по `last_error`, `target_kind`, `status` и retry counter.
4. Если есть transient transport errors — перезапустить consumer/worker и дождаться bounded retry.
5. Если есть deterministic validation/config errors — остановить replay, исправить root cause и только затем запускать repair.

### Stop criteria

- растёт `dead_letter` быстрее, чем `retry` переходит в `sent`;
- один idempotency key создаёт больше одного фактического state transition;
- tenant/module gating даёт `PERMISSION_DENIED` или `NOT_FOUND` для оператора без ожидаемой причины.

## 2. Partial indexing failures

1. Отфильтровать `seo_index_deliveries` по tenant, `target_kind` и failed/dead-letter status.
2. Проверить cursor: high-water mark не должен откатываться назад.
3. Запустить `repair_only` для ограниченного target scope и лимита `1..500`.
4. После repair сверить, что failed count уменьшается, а cursor остаётся forward-only.
5. Для повторяющихся dead-letter items открыть owner-module data issue вместо force replay.

### Rollback / containment

- Не удалять delivery rows вручную.
- Не сбрасывать cursor назад.
- Для остановки blast radius отключать tenant rollout flag, а не менять transport contract.

## 3. Replay / reindex procedure

1. Начинать с `repair_only`.
2. Использовать `repair+historical_replay` только после подтверждения, что repair не закрывает gap.
3. Держать replay mode forward-only: `not_started -> repair_only -> replay_requested -> replaying -> replay_completed`.
4. Для каждого запуска фиксировать: tenant, target scope, limit, operator, command/surface и итоговые counters.
5. После replay выполнить storefront parity smoke: runtime page context, robots/sitemap source и non-home metadata routes.

## Evidence checklist

- Команда/поверхность запуска записана в issue/PR.
- Есть before/after counters по delivery statuses.
- Есть sample `last_error` для оставшихся failed/dead-letter rows.
- Зафиксировано, что GraphQL и REST возвращают совместимые semantic error codes.
- Для Next host подтверждён fallback reason (`module_disabled`, `not_found`, `permission_denied`, `transport_failure`) вместо blanket failure.

## Live incident evidence template (D9)

Use this block when D8 live backend/host evidence is available. Do not mark D9 live evidence complete from static review alone.

- Incident / drill id:
- Tenant and enabled SEO flags:
- Surface used: GraphQL / REST / Next Admin / runbook CLI
- Command, CI job, or operator action:
- Before counters: `pending`, `sent`, `retry`, `failed`, `dead_letter`
- After counters: `pending`, `sent`, `retry`, `failed`, `dead_letter`
- Sample `last_error` for remaining failed/dead-letter rows:
- Cursor transition observed:
- Semantic error parity sample (`BAD_USER_INPUT`, `PERMISSION_DENIED`, `NOT_FOUND`, transport):
- Storefront smoke evidence: runtime page context, robots/sitemap source, product/blog metadata route
- Stop criteria triggered: yes/no; if yes, remediation owner and rollback/containment action
