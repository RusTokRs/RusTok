# Документация `rustok-rbac`

`rustok-rbac` — канонический модуль RBAC runtime в RusToK. Локальная
документация этого модуля должна жить внутри crate, а не расползаться по
`docs/architecture/*` или server-only заметкам.

## Назначение

- публиковать единый RBAC runtime contract для разрешения и проверки прав;
- держать permission policy/evaluator и интеграционные event contracts внутри модуля;
- удерживать `apps/server` в роли adapter/wiring слоя, а не второго RBAC runtime.

## Зона ответственности

- relation-based source of truth: `roles`, `permissions`, `user_roles`, `role_permissions`;
- `PermissionResolver`, `RuntimePermissionResolver`, policy/evaluator и Casbin-backed authorization flow;
- кросс-модульные event contracts для изменений role assignments;
- permission-aware runtime contracts и typed RBAC primitives в связке с `rustok-core`;
- отсутствие rollout-mode и shadow-runtime логики в live surface.

## Интеграция

- `apps/server` владеет только adapter/wiring слоем: store adapters, cache integration, transport extractors и observability;
- `rustok-core` остаётся владельцем typed primitives (`Permission`, `Resource`, `Action`, `SecurityContext`);
- live authorization идёт только через Casbin-backed evaluation, без relation-only/shadow parity path;
- новые public RBAC surfaces и event contracts требуют синхронизации module docs, server docs и verification plan.

## Наблюдаемость и release gates

Канонические runtime signals:

- `rustok_rbac_permission_cache_hits`
- `rustok_rbac_permission_cache_misses`
- `rustok_rbac_permission_checks_allowed`
- `rustok_rbac_permission_checks_denied`
- `rustok_rbac_claim_role_mismatch_total`
- `rustok_rbac_engine_decisions_casbin_total`
- `rustok_rbac_engine_eval_duration_ms_total`
- `rustok_rbac_engine_eval_duration_samples`

Release gates для изменений в модуле:

- обновить unit tests для изменённой доменной логики;
- проверить совместимость server adapters;
- синхронизировать `README.md`, local docs и verification docs;
- не возвращать rollout-mode или вторую live authorization path.

## Проверка

- `cargo xtask module validate rbac`
- `cargo xtask module test rbac`
- targeted tests для permission resolution, Casbin-backed decisions и integration events

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
- [Verification plan](../../../docs/verification/rbac-server-modules-verification-plan.md)
