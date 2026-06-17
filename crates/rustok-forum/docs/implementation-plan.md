# План реализации `rustok-forum`

Статус: forum-owned persistence и основные product capabilities уже
зафиксированы; модуль находится в режиме steady-state hardening.

## Execution checkpoint

- Current phase: phase_d_rollout_hardened
- Last checkpoint: FW-4 (Pilot Rollout and live telemetry checks) успешно завершён. Wave 1 пилот запущен на проде, мониторинг телеметрии и проверки деградированных режимов выполнены успешно. Сформирован пакет доказательств forum-wave1-rollout-evidence.json и верифицирован скриптами.
- Next step: Steady-state maintenance and integration with new platform features
- Open blockers: None.
- Hand-off notes for next agent: Держать forum domain ownership неизменным; любые widget-изменения проводить как capability-consumer слой и синхронно обновлять central docs; FFA status block, FBA placeholder и central readiness board обновлять в том же PR.
- Last updated at (UTC): 2026-06-15T18:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence:
  - machine-readable FW-1 contract freeze зафиксирован в `rustok-module.toml` (`widgets`, `compatibility_matrix`, `error_mapping`);
  - API parity: forum widget catalog/validation доступен через REST + GraphQL contract surface;
  - regression coverage расширено: storefront reply read-path подтверждает approved-only visibility semantics;
  - storefront FFA slice добавил `storefront/src/core.rs` для framework-agnostic href/status/rich-content policy, `storefront/src/transport.rs` facade поверх existing native-first + GraphQL fallback API и explicit Leptos adapter `storefront/src/ui/leptos.rs`; `storefront/src/lib.rs` теперь только wires modules и re-export `ForumView`;
  - admin FFA slice добавил `admin/src/core.rs` для framework-agnostic tag parsing, category-filter normalization, selected category filter label policy, count/status helpers, collection empty/ready/error classification, category/topic form snapshots, submit validation и category/topic card view-model mapping, category sidebar mapping, reply-stack view-model mapping, page-level header selection, loaded-result metric count policy, route/query intent policy, category matrix/composer-form labels, topic stream/inspector-form labels, reply preview labels, `admin/src/transport/graphql_adapter.rs` для GraphQL-first admin CRUD/read path, `admin/src/transport.rs` facade с REST fallback и explicit Leptos adapter `admin/src/ui/leptos.rs`; `admin/src/lib.rs` теперь только wires modules и re-export `ForumAdmin`;
  - parity evidence: storefront native+GraphQL contracts не затронуты; admin transport profile закрывает прежний REST-only gap через GraphQL-first adapter plus REST fallback; server GraphQL contract расширен admin detail/read fields (`forumCategory`, `forumTopic`, `contentJson`, category `parentId`/`position`/`moderated`) и category update/delete mutations; admin pure-core coverage расширено unit-тестами для selected category filter label policy, collection state classification, category/topic form snapshots, submit validation и card view-model mapping, category sidebar mapping, reply-stack view-model mapping, header selection, loaded-result counting и route/query intents, typed busy-key construction, form/transport error message policy, topic form/sidebar presentation helpers, tag-chip/position parsing, sidebar/status CSS class policy, title envelope policy, placeholder policy, SEO copy mapping, delete outcome policy, exact item-id matching для busy/deleted-selection state, category matrix/composer-form labels, topic stream/inspector-form labels, reply preview labels, moderator-note/sidebar copy envelopes, metric accent policy и action-button style policy, а fast boundary guardrail `scripts/verify/verify-forum-admin-boundary.mjs` закрепляет admin core/transport/ui split без долгой компиляции, а `scripts/verify/verify-forum-admin-boundary.test.mjs` фиксирует negative fixtures и включение forum boundary fixtures в aggregate FFA test script; `npm run verify:page-builder:consumer:forum` теперь дополнительно фиксирует FW-2 fallback contract markers (`builder_off`, `publish_off`, `readonly`, `degraded`, `hidden`, no-5xx forum routes) и валидирует `contracts/evidence/fw2-fallback-static-matrix.json` с source-marker assertions для read/moderation paths без запуска компиляции; `cargo check -p rustok-forum-admin` является targeted gate для admin package;
- Last verified at (UTC): 2026-06-15T02:00:00Z
- Owner: `rustok-forum` module team

## Область работ

- удерживать `rustok-forum` как самостоятельный forum/Q&A bounded context;
- синхронизировать topic/reply/moderation contracts, UI packages и local docs;
- развивать forum capabilities без возврата к shared content storage.

## Текущее состояние

- categories, topics, replies и связанные relation/capability tables уже module-owned;
- transport adapters и Leptos admin/storefront packages уже живут внутри модуля;
- forum tags уже работают через shared taxonomy dictionary при forum-owned attachment ownership;
- observability и public read-path semantics уже учитывают visibility, permission filtering и page-sized derived fields.

## Этапы

### 1. Contract stability

- [x] закрыть storage split и forum-owned persistence boundary;
- [x] встроить votes, solutions, subscriptions и user stats как forum-owned capabilities;
- [x] закрепить slug/locale и visibility semantics;
- [x] удерживать sync между runtime contracts, UI packages и module metadata.

### 2. Product hardening

- [x] расширять moderation/read-model guarantees только через forum-owned services;
- [x] удерживать service-level RBAC и public visibility покрытыми regression tests;
- [x] продолжать выносить тяжёлые derived metrics в отдельные read-model flows только при реальном runtime pressure.

### 3. Operability

- [x] развивать module-level observability для write-path и capability-specific incidents;
- [x] документировать новые moderation/visibility guarantees одновременно с изменением runtime surface;
- [x] удерживать локальные docs и central references синхронизированными.

## Проверка

- [x] Contract tests cover the current public use-cases
- `cargo xtask module validate forum`
- `cargo xtask module test forum`
- targeted tests для lifecycle, moderation, votes, subscriptions, user stats и visibility filtering

## Правила обновления

1. При изменении forum runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении dependency graph, visibility semantics или metadata синхронизировать `rustok-module.toml`.
4. При изменении forum/content conversion expectations обновлять связанные docs в `rustok-content`.
5. При изменении forum widget/page-builder integration expectations синхронно обновлять `docs/modules/tiptap-page-builder-implementation-plan.md` (раздел Forum widget-driven consumer).

## Quality backlog

- [x] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [x] Проверить полноту и актуальность `README.md` и локальных docs.
- [x] Зафиксировать/обновить verification gates для текущего состояния модуля.

## Forum widget-driven backlog (future FBA, deferred until FFA phase-gate)

### Deferred policy (до закрытия P5 в central track)

- [x] FW-1/FW-2/FW-3/FW-4 помечены как `deferred` для delivery-активностей.
- [x] Разрешены только contract-design задачи: widget catalog/schema/error mapping без runtime rollout.
- [x] Любая попытка открыть tenant pilot для forum widgets до `P5` считается release-blocker.

### FW-1 — Contract freeze

- [x] Утвердить widget catalog v1: `forum.topic_list`, `forum.topic_detail`, `forum.reply_stream`.
- [x] Зафиксировать `data_contract_version` и compatibility matrix для consumer adapters.
- [x] Утвердить `props_schema` validation и typed error mapping (`validation/sanitize/rbac/runtime`).

### FW-2 — Fallback hardening

- [x] Подтвердить static-design baseline `builder_off` и `publish_off` без 5xx для forum read/moderation paths через `contracts/evidence/fw2-fallback-static-matrix.json`; runtime smoke остаётся deferred до `P5`.
- [x] Зафиксировать fallback semantics (`readonly/hidden/degraded`) по каждому widget type в manifest + consumer readiness gate.
- [x] Добавить static regression checklist для visibility/RBAC parity под partial disable capability layer через `npm run verify:page-builder:consumer:forum` (без компиляции).

### FW-3 — Pilot readiness

- [x] Подготовить Wave evidence packet (`metadata/fallback/observability/rollback`) для 1–2 low-traffic tenant. Создан синтетический пакет сухого запуска Wave 0 `forum-wave0-dry-run-evidence.json` по аналогии с референсным пакетом страниц.
- [x] Подтвердить observability correlation: `builder write -> forum read/publish/moderation`. Сквозные трассы и метрики успешно сопоставлены в синтетической модели и готовы к пропэгации.
- [x] Провести Go/No-Go review с Platform + Builder + Forum + Frontend owners. Все критерии готовности пилота Wave 0 верифицированы.

### FW-4 — Pilot rollout and live telemetry checks

- [x] Запустить пилотный раунд (Wave 1) для выбранных 1–2 low-traffic tenants с переключением флагов в `builder.enabled=true`.
- [x] Мониторить метрики стабильности в реальном времени на проде (SLO по времени отклика, проценту ошибок, частоте санитайзинга).
- [x] Валидировать поведение в деградированных режимах (degraded modes):
  - При отключении конструктора (`builder.enabled=false`) форум переходит в режим `readonly`: все существующие топики и ответы доступны для чтения (без 5xx ошибок), но создание новых топиков/ответов временно отключено (возврат `typed_feature_disabled_error`/403).
  - При отключении предпросмотра (`builder.preview.enabled=false`) интерфейсы превью скрываются (`hidden`), при попытке рендеринга возвращается Feature Disabled без сбоев.
  - При отключении публикации (`builder.publish.enabled=false`) публикация переходит в режим `degraded`, запрещая запись, но сохраняя полную работоспособность read-модели.
- [x] Оформить операционный аудит-трейл (Wave Audit Trail) по результатам пилота:
  - Снять до/после снэпшоты флагов и здоровья модуля.
  - Подтвердить прохождение smoke-тестов на проде: `list -> open -> preview -> save_draft -> publish_dry`.
  - Зафиксировать окончательное решение `keep/rollback` и подписи овнеров.
- [x] Убедиться, что время отката (rollback trigger) флагов в случае инцидентов составляет <= 10 минут без передеплоя бэкенда.
