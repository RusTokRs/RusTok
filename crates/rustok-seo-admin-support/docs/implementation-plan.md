# План реализации `rustok-seo-admin-support`

Статус: support crate стабилизирован как reusable owner-side SEO UI слой. Следующий execution wave синхронизирован с Phase D SEO-модуля (`D6`, `D8`, `D9`).

## Execution checkpoint

- Current phase: `phase_d6_admin_integration_alignment`
- Last checkpoint: План синхронизирован с текущим состоянием support crate и большим прогоном SEO Phase D; зафиксирован backlog observability/remediation widgets и transport parity для Next admin control-plane.
- Next step: Закрыть D6.1 — reusable event/delivery status widgets + diagnostics remediation hints для owner-module admin surfaces.
- Open blockers:
  - В этой VM отсутствует `cargo` в `PATH`, локальные проверки не запускались.
  - Для полной D6.1 observability/remediation UX всё ещё нужен owner-side UI wiring в `pages/product/blog/forum`.
- Hand-off notes for next agent:
  - Не переносить ownership entity screens в central SEO hub.
  - Поддерживать host-locale contract без package-local fallback chains.
  - Все новые UI виджеты должны работать одинаково для `pages/product/blog/forum`.
- Last updated at (UTC): 2026-05-30T12:00:00Z

## Цель

- не дублировать SEO panel logic в `pages`, `product`, `blog`, `forum` и будущих content-модулях;
- не превращать `rustok-seo-admin` в universal entity editor;
- держать reusable UI/tooling слой отдельно от SEO runtime и от owner-module screen ownership.

## Выполнено

- [x] создан support crate с root README и local docs;
- [x] вынесены shared GraphQL helper-ы для `seoMeta`, `upsertSeoMeta`, `publishSeoRevision`;
- [x] реализован `SeoEntityPanel` для owner-side entity editors;
- [x] реализован `SeoCapabilityNotice` для capability-slot сценариев;
- [x] встроены owner-side SEO panels в `rustok-pages/admin`, `rustok-product/admin`, `rustok-blog/admin`, `rustok-forum/admin`;
- [x] убран package-local locale override: support crate читает host effective locale, canonicalizes его и не держит editable locale field;
- [x] вынесены reusable snippet preview/recommendation/summary widgets;
- [x] raw `structured_data` textarea заменён на typed schema input contract с сохранением GraphQL write parity.

## Phase D backlog (SEO integration parity)

- [ ] **D6.1 — Observability/remediation widgets**
  - [ ] Добавить reusable cards для event delivery status (pending/sent/retry/failed) без жёсткой привязки к конкретному owner module layout.
  - [ ] Добавить remediation hints для diagnostics issue-кодов с явным action mapping (`open_entity_editor`, `open_bulk_job`, `run_reindex`).

- [ ] **D6.2 — Transport helpers parity**
  - [ ] Расширить shared transport layer под REST parity endpoints из SEO Batch D4 (diagnostics summary, bulk job detail/status, sitemap job detail).
  - [ ] Сохранить fallback на текущий GraphQL contract, пока rollout-флаг REST parity выключен.

- [ ] **D6.3 — UX consistency gates**
  - [ ] Выделить единый visual/state contract для loading/error/permission/empty состояний.
  - [ ] Привязать permission hints к canonical SEO permission model (`seo:read`, `seo:manage`).

- [ ] **D8 — Verification matrix**
  - [ ] Unit tests для scoring/remediation mapping и locale wiring.
  - [ ] Integration tests для transport fallback (GraphQL/REST) и error envelope mapping.
  - [ ] Snapshot/smoke tests для reusable cards в owner layouts.

- [ ] **D9 — Docs/DoD sync**
  - [ ] Обновить crate README/docs с operational guidance для owner modules.
  - [ ] Зафиксировать Definition of Done для reusable widget additions.

## Проверка

- `cargo check -p rustok-seo-admin-support --tests --config profile.dev.debug=0`
- `cargo check -p rustok-pages-admin --config profile.dev.debug=0`
- `cargo check -p rustok-product-admin --config profile.dev.debug=0`
- `cargo check -p rustok-blog-admin --config profile.dev.debug=0`
- `cargo check -p rustok-forum-admin --config profile.dev.debug=0`
- `npm --prefix apps/next-admin run lint && npm --prefix apps/next-admin run typecheck`

## Quality backlog

- [ ] Актуализировать test coverage для новых observability/remediation widgets.
- [ ] Поддерживать transport compatibility matrix GraphQL/REST.
- [ ] Синхронизировать docs после каждого D6/D8 инкремента.
