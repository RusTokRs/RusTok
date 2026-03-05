# План миграции RBAC на relation-модель (source of truth)

- Дата: 2026-02-26
- Статус: In progress (execution in phases)
- Область: `apps/server`, `crates/rustok-core`, `crates/rustok-rbac`, миграции `apps/server/migration`
- Цель: перейти на модель, где права доступа вычисляются **только** через таблицы связей (`roles`, `permissions`, `user_roles`, `role_permissions`), и убрать зависимость авторизации от денормализованного `users.role`.

---

## 0.1 Корректировка курса (module-first update)

На момент выполнения фаз 2–5 значимая часть runtime RBAC была реализована в `apps/server::AuthService` как самый быстрый путь к безопасному rollout (`relation resolver`, `dual-read`, cache, observability).

Это позволило закрыть критичные migration-задачи, но создало архитектурный перекос: policy/use-case логика размазана по server-слою, а `crates/rustok-rbac` пока используется не как фактический центр RBAC-доменной логики.

С этого шага план официально корректируется:

1. **Rollout safety не останавливаем**: фазы data-migration/cutover (4–6) продолжаются по графику.
2. **Новые RBAC изменения делаем module-first**: policy/use-case ядро переносится в `crates/rustok-rbac`, а `apps/server` остаётся transport/infra adapter-слоем.
3. **Перенос инкрементальный (strangler pattern)**: без big-bang переписывания и без деградации текущего продового контроля доступа.

Критерий этой корректировки: после завершения migration-фаз RBAC policy-source находится в `crates/rustok-rbac`, а server не содержит дублирующей policy-логики.

---

## 0.2 MVP-рамка (анти-разрастание)

Чтобы migration не превратился в бесконечный рефакторинг, фиксируем **MVP cutover scope**.

### Входит в MVP (обязательно до relation-only)

1. **Data correctness:**
   - staging dry-run/backfill/rollback прогон,
   - инварианты `users_without_roles_total`, `orphan_user_roles_total`, `orphan_role_permissions_total` подтверждены отчётом.
2. **Cutover safety:**
   - dual-read baseline с `mismatch_delta == 0`,
   - shadow path без деградации (ошибки shadow не растут в окне наблюдения),
   - проверка decision-volume (`--min-decision-delta`) выполнена.
3. **Controlled switch:**
   - включение relation-only под feature-flag,
   - пост-переключательный мониторинг 401/403/latency.

### Не входит в MVP (post-MVP backlog)

- Дальнейшее расширение helper-скриптов и флагов, если это не блокирует cutover.
- Глубокий structural cleanup server-layer сверх необходимого для безопасного релиза.
- Полная «косметическая» переработка документации/чеклистов вне критичных runbook-обновлений.

Правило для следующих шагов: **один merge = один измеримый cutover-risk reduction**, без добавления новой платформенной сложности «на будущее».

---

## 0.3 Синхронизация с user/auth remediation plan

Чтобы не дублировать статусы и задачи между двумя roadmap-документами:

- `docs/architecture/api.md` (раздел «Auth lifecycle consistency и release-gate») является source-of-truth по user/auth parity (REST/GraphQL), reset-password session invalidation policy и rollout gate-верификации этих изменений.
- В этом RBAC-плане фиксируются только RBAC migration/cutover задачи; auth parity упоминается только как зависимость/предусловие readiness.
- Текущее согласование статуса: кодовые remediation-задачи user/auth закрыты, release-gate переведён в operational handoff (`scripts/auth_release_gate.sh --require-all-gates`).

---

## 0. Статус выполнения (progress tracker)

> Обновляется по мере мерджей в `apps/server`/`crates/*`.

- [x] **Фаза 0 — Архитектурное решение и подготовка (частично):**
  - План и архитектурные ссылки зафиксированы в `docs/architecture/*` и `docs/index.md`.
  - ADR про RBAC source-of-truth в relation-модели принят: `DECISIONS/2026-02-26-rbac-relation-source-of-truth-cutover.md`.
- [x] **Фаза 1 — Быстрые исправления консистентности (базовые пункты):**
  - user creation flows для `register/sign_up/create_user/accept_invite` уже заведены через назначение relation RBAC (`assign_role_permissions`).
  - `seed_user` (dev/test seed bootstrap) теперь также вызывает `assign_role_permissions` после создания пользователя.
  - parity reset-password/session invalidation закрыт по коду и документирован в `docs/architecture/api.md` (раздел «Auth lifecycle consistency и release-gate»), rollout verification выполняется операционно через `scripts/auth_release_gate.sh`.
- [x] **Фаза 2 — Единый Permission Resolver (завершено):**
  - В `rustok-rbac` стандартизирован модульный cross-module integration event contract для role-assignment изменений: добавлены `RbacRoleAssignmentEvent`, `RbacIntegrationEventKind` и стабильные event-type ключи `rbac.*` для единообразной публикации/подписки между модулями.
  - В `AuthService` добавлены tenant-aware методы `get_user_permissions / has_permission / has_any_permission / has_all_permissions`.
  - В `rustok-rbac` добавлен `permission_evaluator` (единый API итоговой policy-оценки allow/deny + missing permissions + denied reason), а `AuthService` теперь использует его как модульный policy-source вместо локальной сборки outcome.
  - В `rustok-rbac` добавлен контракт `PermissionResolver` + `PermissionResolution`; на промежуточном шаге использовался тонкий `ServerPermissionResolver` adapter в `apps/server` (инкрементальный strangler-этап).
  - В `PermissionResolver` добавлены default use-case методы `has_permission/has_any_permission/has_all_permissions`, использующие модульный evaluator; далее wiring runtime-резолва перенесён в модульный `RuntimePermissionResolver`, а в server оставлены только инфраструктурные адаптеры (DB/cache/role-assignment).
  - Кэшируемый relation-resolve унифицирован в `rustok-rbac` через `resolve_permissions_with_cache` и контракт `PermissionCache`; `apps/server` использует `MokaPermissionCache` как инфраструктурный адаптер, а логика определения cache hit/miss перенесена в модульный оркестратор.
  - В `rustok-rbac` добавлен модульный use-case слой `permission_authorizer` (authorize single/any/all), который собирает decision (`allowed/missing/denied_reason/cache_hit`) поверх `PermissionResolver`; `AuthService` использует этот API вместо локальной сборки policy-решения.
  - Алгоритм relation-resolve (цепочка `user_roles -> roles(tenant) -> permissions`) вынесен в `rustok-rbac::resolve_permissions_from_relations` через контракт `RelationPermissionStore`; в `apps/server` оставлен SeaORM adapter к этому контракту.
  - В `rustok-rbac` добавлен модульный runtime-resolver `RuntimePermissionResolver` (+ контракт `RoleAssignmentStore`), который объединяет relation-store + cache + role-assignment use-cases; `apps/server` теперь использует его как основной resolver и удалил локальный `ServerPermissionResolver`.
  - Public write-path RBAC операции (`assign_role_permissions`, `replace_user_role`) в `AuthService` переведены на вызов модульного `RuntimePermissionResolver`, чтобы server-слой оставался adapter-only и для assignment use-cases.
  - Реализованы tenant-scoping и deduplication; сохранена семантика `resource:manage` как wildcard.
  - Для cache-path в модульном resolver сохранён инвариант стабильного normalized output (dedup + сортировка), чтобы format результата не зависел от источника (cache или relation DB).
  - Переведена часть GraphQL-checks (users CRUD/read/list, alloy, content mutation/query) и RBAC extractors на relation-проверки.
  - RBAC extractors перестали держать локальную wildcard-логику и используют общий helper `rustok_rbac::has_effective_permission_in_set` (снижение дублирования policy-семантики).
  - `GraphQL update_user` теперь синхронно обновляет relation-модель (`user_roles`) через `replace_user_role`, чтобы legacy-role и relation RBAC не расходились.
  - Назначение relation-ролей/пермишенов переведено на conflict-safe idempotent upsert (`ON CONFLICT DO NOTHING`) для устойчивости к конкурентным операциям.
  - Поле `User.can` в GraphQL переведено с role-based (`users.role`) на tenant-aware relation-проверку через `AuthService::has_permission`.
  - В `AuthService` добавлено базовое observability resolver-решений: structured logs для latency и причин deny в `has_permission/has_any_permission/has_all_permissions`.
  - В `AuthService` добавлен in-memory permission cache (TTL 60s) с cache hit/miss observability и инвалидацией при изменении relation-ролей пользователя.
  - В `/metrics` добавлены счётчики resolver-кэша и decision outcomes: `rustok_rbac_permission_cache_hits`, `rustok_rbac_permission_cache_misses`, `rustok_rbac_permission_checks_allowed`, `rustok_rbac_permission_checks_denied`.
  - Добавлены latency-метрики resolver-проверок/lookup и breakdown denied reasons в `/metrics`.
- [x] **Фаза 3 — AuthContext и токены (завершено):**
  - `CurrentUser.permissions` теперь резолвятся из relation-модели, а не из `users.role`.
  - `AuthContext` в GraphQL больше не хранит `role` как policy-источник; security-context продолжает выводиться из relation-permissions.
  - В auth extractor добавлен shadow-control: warning-лог `rbac_claim_role_mismatch`, если role-claim в JWT расходится с ролью, выведенной из relation-permissions.
  - В `/metrics` добавлены счётчики `rustok_rbac_claim_role_mismatch_total` (только claim-vs-relation) и `rustok_rbac_decision_mismatch_total` (только relation-vs-legacy dual-read) для раздельной observability.
  - Role claim в JWT используется как display/debug claim и для shadow-observability, но не как policy source-of-truth.
- [~] **Фаза 4 — Миграция данных и защитные инварианты (в работе):**
  - Добавлена maintenance-задача `cleanup --args "rbac-report"` для отчёта по инвариантам (`users_without_roles`, `orphan_user_roles`, `orphan_role_permissions`).
  - Добавлена maintenance-задача `cleanup --args "target=rbac-backfill"` для idempotent backfill relation-RBAC по `users.role` в пределах tenant c повторной проверкой инвариантов после выполнения.
  - Для staged rollout в backfill добавлены safety-controls: `dry_run=true` (без изменений данных), `limit=<N>` (батчевый прогон) и `continue_on_error=true` (best-effort режим при частичных ошибках).
  - Backfill расширен исключениями для service/special-case аккаунтов (`exclude_user_ids`, `exclude_roles`) и добавлен rollback-путь `rbac-backfill-rollback` через snapshot-файл (`rollback_file` -> `source`) с role-targeted откатом (удаляется только роль из snapshot, а не все tenant-роли пользователя).
  - Добавлен helper-скрипт `scripts/rbac_relation_staging.sh` для staged rehearsal (pre-report, dry-run, optional apply, optional rollback, markdown-отчёт + таймстемпированные логи в `artifacts/rbac-staging`); helper формирует machine-readable JSON-инварианты через `cleanup target=rbac-report output=<file>` (pre/post apply/post rollback), добавляет в markdown-отчёт diff-таблицу инвариантов (`pre -> post`, delta) и поддерживает stop-the-line флаг `--fail-on-regression`; rollback-шаги требуют валидный snapshot (`--run-apply` в том же запуске или `--rollback-source=<file>`).
  - Для более строгого cutover-gate в helper добавлены флаги `--require-zero-post-apply` и `--require-zero-post-rollback`: при включении скрипт завершится с ошибкой, если любой из инвариантов (`users_without_roles_total`, `orphan_user_roles_total`, `orphan_role_permissions_total`) после соответствующего шага не равен `0`. Флаги валидируются по шагам выполнения: `--require-zero-post-apply` требует `--run-apply`, `--require-zero-post-rollback` требует `--run-rollback-apply`. Если ожидаемый JSON-отчёт шага отсутствует, strict-check также завершает прогон ошибкой (fail-fast).
  - Добавлены smoke-тесты helper-скрипта (`scripts/tests/rbac_relation_staging_test.sh`) с mock cargo-path для проверки rollback guardrails и snapshot-source сценариев.
- [~] **Фаза 5 — Dual-read и cutover (частично):**
  - В `AuthService` добавлен runtime shadow dual-read для `has_permission/has_any_permission/has_all_permissions` под env-флагом `RUSTOK_RBAC_AUTHZ_MODE=dual_read` (также поддерживаются алиасы `dual-read` и `dual`) (relation decision остаётся авторитативным).
  - Режим rollout-конфигурации (`RbacAuthzMode`: `relation_only`/`dual_read`) перенесён в `crates/rustok-rbac` как модульный контракт, `apps/server` использует его без локального enum-дублирования.
  - Для rollout-совместимости `RbacAuthzMode` поддерживает legacy toggle `RUSTOK_RBAC_RELATION_DUAL_READ_ENABLED` (aliases: `RBAC_RELATION_DUAL_READ_ENABLED`, `rbac_relation_dual_read_enabled`) (если `RUSTOK_RBAC_AUTHZ_MODE` не задан), чтобы staged cutover можно было выполнять без одномоментного переезда всех окружений на новый env-ключ.
  - Legacy-vs-relation shadow decision semantics вынесены в модульный `rustok-rbac::shadow_decision`; `apps/server` оставляет только загрузку legacy-ролей, метрики и logging/feature-flag orchestration.
  - Для server-adapter слоя shadow orchestration унифицирован через `ShadowCheck` + `compare_shadow_decision` (single/any/all без дублирования policy-веток в `AuthService`).
  - В `rustok-rbac` добавлен модульный dual-read orchestrator `evaluate_dual_read` (`DualReadOutcome`), поэтому `apps/server` больше не держит локальную decision-ветвистость для `skip/compare` и использует модульный исход dual-read-решения.
  - В warning-логе `rbac_decision_mismatch` добавлен стабильный тег `shadow_check` (`single|any|all`) для более точной сегментации dual-read mismatch в observability.
  - При расхождении relation-vs-legacy-role увеличивается `rustok_rbac_decision_mismatch_total` и пишется warning-лог `rbac_decision_mismatch`; ошибки самого shadow-path отдельно учитываются в `rustok_rbac_shadow_compare_failures_total`.
  - Для снижения накладных расходов dual-read legacy role lookup покрыт in-memory TTL-кэшем в `AuthService`.
  - Shadow-сравнение выполняется fail-open: ошибки shadow path логируются и не влияют на авторитативное relation-based allow/deny решение.
  - Добавлен cutover baseline helper `scripts/rbac_cutover_baseline.sh`: скрипт снимает серию `/metrics` snapshot'ов, считает deltas по `rustok_rbac_decision_mismatch_total`/`rustok_rbac_shadow_compare_failures_total`/`rustok_rbac_permission_checks_{allowed,denied}`, формирует markdown+json отчёт в `artifacts/rbac-cutover` и по умолчанию включает strict gate (`mismatch_delta == 0`) для dual-read окна наблюдения.
  - В baseline helper добавлены дополнительные stop-the-line guardrails: обязательный минимальный объём decision-трафика в окне (`--min-decision-delta`, default `1`) и fail-fast при обнаружении reset-а счётчиков (уменьшение counter-метрик между последовательными samples). Для аудита и post-mortem helper по умолчанию сохраняет raw `/metrics` snapshots по всем sample-точкам (`rbac_cutover_samples_*`), с опциональным отключением через `--no-save-samples`.
  - Для более строгого dual-read gate baseline helper теперь также по умолчанию требует `shadow_compare_failures_delta == 0`; для controlled troubleshooting-окон доступен явный override-флаг `--allow-shadow-failures`.
  - Добавлены тесты helper'а (`scripts/tests/rbac_cutover_baseline_test.sh` + smoke `scripts/test_rbac_cutover_baseline.sh`) с mocked curl-path (`RUSTOK_CURL_BIN`) для проверки mismatch-gate и генерации baseline артефактов.
  - Добавлен gate helper `scripts/rbac_cutover_gate.sh` для финальной проверки обязательных артефактов (timestamp-consistent staging bundle: pre/dry-run/apply/rollback/post-rollback + timestamp-consistent cutover baseline md/json + auth release gate report) и fail-fast в случае `gate_status != pass`, ненулевых `mismatch_delta/shadow_compare_failures_delta` или ненулевых post-rollback инвариантов (`users_without_roles_total`, `orphan_user_roles_total`, `orphan_role_permissions_total`); helper покрыт тестами (`scripts/tests/rbac_cutover_gate_test.sh` + smoke `scripts/test_rbac_cutover_gate.sh`).
  - Принят ADR финального relation-only cutover с обязательным rollback-gate и привязкой к auth parity release-gate: `DECISIONS/2026-03-05-rbac-relation-only-final-cutover-gate.md`.
- [ ] **Фаза 6 — Cleanup legacy-модели:** не начато.

### Что осталось приоритетно на ближайший шаг

1. Провести staged dry-run/backfill/rollback в staging и приложить отчёт по инвариантам (MVP-блокер №1, Фаза 4).
2. ✅ ADR по final cutover (`relation-only`) согласован: `DECISIONS/2026-03-05-rbac-relation-only-final-cutover-gate.md` (явный rollback-gate + связь с auth parity gate-артефактами).
3. Выполнить production dual-read окно наблюдения и зафиксировать baseline-отчёт без регрессий (MVP-блокер №3, Фаза 5).

### Post-MVP (выполнять только после relation-only стабилизации)

1. Зафиксировать module-first extraction backlog: выделить policy/use-case API в `crates/rustok-rbac` и описать server-adapter границы.
2. Планово пройти Фазу 6 cleanup legacy-модели и удалить временные fallback/feature flags.

### Исполнительный фокус (сейчас делаем только это)

До следующего milestone команда выполняет **только MVP-блокеры** из этого плана:

1. Staging rehearsal: dry-run/backfill/rollback + отчёт по инвариантам.
2. ADR final cutover (`relation-only`) с явным rollback-gate.
3. Production dual-read baseline window + решение go/no-go для relation-only.

Что **не делаем сейчас**: новые helper-флаги, расширение scope cleanup, дополнительный platform-refactor вне MVP-блокеров.

Критерий завершения текущего этапа: все три пункта выше закрыты и задокументированы артефактами (report/ADR/baseline).

### Итоговая проверка перед переходом к Фазе 4

- Фазы 1–3 считаются завершёнными при текущем состоянии runtime:
  - user lifecycle-потоки и dev seed назначают relation RBAC (`user_roles`/`role_permissions`) при создании/обновлении пользователя;
  - централизованные permission checks идут через `AuthService` resolver (tenant-aware, wildcard, cache + metrics);
  - `CurrentUser.permissions` и `AuthContext.permissions` резолвятся из relation-модели;
  - claim role mismatch учитывается только как observability/shadow-сигнал и не определяет access decision.

---

## 1. Контекст и проблема

Сейчас в коде одновременно существуют две модели:

1. Ролевая проверка по `users.role` (например, через `Rbac::has_permission(&auth.role, ...)`).
2. Relation RBAC через таблицы `user_roles` + `role_permissions` + `permissions`.

Это создаёт риск рассинхронизации: пользователь может иметь роль в `users`, но не иметь корректных связей в `user_roles`.

### 1.1 Исторические симптомы (закрыты в фазах 1–3)

- В `register`/`sign_up` назначение relation-RBAC ранее было неравномерным между user-flow.
- Часть GraphQL-проверок ранее опиралась на `auth.role` вместо relation-проверок.
- `CurrentUser` permissions ранее формировались от role-claim, а не relation-resolver.

Текущее состояние по этим пунктам зафиксировано в progress tracker выше: baseline-симптомы закрыты, фокус смещён на data migration/cutover/cleanup.

---

## 2. Целевая архитектура

## 2.1 Source of truth

Единственный источник прав:

- `roles` — роли в рамках tenant.
- `permissions` — атомарные разрешения (`resource`, `action`) в рамках tenant.
- `user_roles` — связи пользователь ↔ роль.
- `role_permissions` — связи роль ↔ разрешение.

## 2.2 Роль поля `users.role`

На переходный период `users.role` сохраняется как legacy/денормализованное поле (для совместимости API, UI и отчётности), но **не участвует в принятии решения доступа**.

## 2.3 Принцип принятия решения

Все runtime-checks авторизации должны идти через единый сервис резолва прав:

- `get_user_permissions(tenant_id, user_id)`
- `has_permission(tenant_id, user_id, permission)`
- `has_any_permission(...)`
- `has_all_permissions(...)`

---

## 3. План внедрения по фазам

## Фаза 0 — Архитектурное решение и подготовка

**Цель:** зафиксировать единый контракт до изменений кода.

Шаги:
1. Оформить ADR в `DECISIONS/`: "RBAC source of truth = relation model".
2. Зафиксировать rollout-стратегию: `dual-read -> relation-only -> cleanup`.
3. Обновить документацию:
   - `docs/architecture/rbac.md` — целевая модель enforcement.
   - `docs/index.md` — ссылка на этот план.

Критерии завершения:
- ADR принят.
- Архитектурные документы не противоречат коду/плану.

---

## Фаза 1 — Быстрые исправления консистентности

**Цель:** убрать текущие функциональные дыры до большой миграции.

Шаги:
1. Гарантировать назначение relation-RBAC при любом создании пользователя:
   - `create_user` (GraphQL),
   - `register` (REST),
   - `sign_up` (GraphQL),
   - invite-flow и сервисные сценарии seed/sync.
2. Привести reset-password flow к единому правилу инвалидирования сессий во всех API-каналах.
3. Добавить smoke-тесты на обязательное наличие `user_roles` после создания user.

Критерии завершения:
- Ни один публичный flow не создаёт пользователя без `user_roles`.
- После reset-password активные сессии отзываются одинаково во всех каналах.

---

## Фаза 2 — Единый Permission Resolver

**Цель:** отделить authorization policy от transport-слоёв (REST/GraphQL/extractors) и закрепить модуль `rustok-rbac` как целевой policy-host.

Шаги:
1. Ввести общий сервис/компонент `PermissionResolver` с tenant-aware API; базовый контракт уже вынесен в `crates/rustok-rbac`, а `apps/server` использует adapter-реализацию поверх существующего wiring.
2. Реализовать в нём:
   - чтение прав через relation-таблицы,
   - дедупликацию разрешений,
   - корректную обработку невалидных resource/action,
   - observability (latency, cache hit/miss, denied reason).
3. Перевести GraphQL mutations/queries с ролевых проверок на resolver.
4. Перевести REST permission checks/extractors на resolver.

Критерии завершения:
- В runtime-коде нет прямых `has_permission(&auth.role, ...)` в business-критичных точках.
- Все основные permission checks используют единый resolver.

---

## Фаза 3 — AuthContext и токены

**Цель:** убрать зависимость принятия решения от claim-роли.

Шаги:
1. В `CurrentUser` / `AuthContext` перестать вычислять permissions из `users.role`.
2. Использовать resolver для фактических permissions пользователя в tenant.
3. Оставить роль в JWT как display/debug claim (опционально), но не как источник truth.
4. Ввести инвалидацию permission-кэша при изменении связей ролей/прав.

Критерии завершения:
- Решение о доступе не зависит от role-claim в JWT.
- Изменение роли/прав начинает действовать в пределах целевого SLA (например, ≤ 60 сек с кэшем).

---

## Фаза 4 — Миграция данных и защитные инварианты

**Цель:** гарантировать полноту данных relation RBAC во всех tenant.

Шаги:
1. Backfill-процедура:
   - найти users без `user_roles`,
   - создать tenant-роль по `users.role` (если отсутствует),
   - создать связь `user_roles`,
   - добавить отсутствующие `role_permissions` для системных ролей.
2. Добавить проверки целостности в maintenance/job:
   - orphan users без ролей,
   - orphan `user_roles` без `roles`,
   - orphan `role_permissions` без `permissions`.
3. Подготовить rollback-скрипт (логически обратимый шаг).

Критерии завершения:
- `0` пользователей без ролей (кроме явно разрешённых service-аккаунтов, если такие есть).
- Консистентность подтверждена отчётом/метриками.

---

## Фаза 5 — Dual-read и cutover

**Цель:** безопасно переключиться в production без деградации доступа.

Шаги:
1. Включить dual-read режим (короткий период):
   - decision по relation,
   - параллельный shadow-check по legacy роли,
   - метрика расхождений `rbac_decision_mismatch_total`.
2. Снять baseline по mismatch и устранить источники.
3. Включить relation-only режим feature-флагом.
4. Наблюдать SLO (401/403 rate, latency, error budget).

Критерии завершения:
- mismatch стабильно равен 0 в целевом окне (например, 7 дней).
- relation-only работает без роста инцидентов.

---

## Фаза 6 — Cleanup legacy-модели и финализация module boundaries

**Цель:** убрать технический долг после стабилизации и закрыть архитектурный перенос RBAC в модуль.

Шаги:
1. Удалить из runtime-кода все legacy-пути авторизации по `users.role`.
2. Перевести `users.role` в read-only/derived статус на переходный релиз.
3. Подготовить миграцию на удаление `users.role` (или оставить как строго денормализованное отображаемое поле, если это обосновано).
4. Завершить перенос policy/use-case RBAC из `apps/server` в `crates/rustok-rbac`; в server оставить только adapter/integration слой (DB, cache, transport, wiring).
5. Обновить документацию, API-контракты и onboarding.

Критерии завершения:
- В коде отсутствуют decision-ветки по legacy-role.
- RBAC policy-source и публичный доменный API расположены в `crates/rustok-rbac`.
- Документация описывает только relation RBAC и актуальные module boundaries.

---

## 4. Delivery-план (по спринтам)

## Спринт A (foundation)

- Фаза 0 + Фаза 1.
- Частично Фаза 2 (скелет resolver + первые интеграции).
- Результат: консистентные user-flow и отсутствие новых дыр.

## Спринт B (core migration)

- Завершение Фазы 2 + Фаза 3.
- Начало Фазы 4 (backfill на staging).
- Результат: runtime checks централизованы.

## Спринт C (production cutover)

- Завершение Фазы 4 + Фаза 5.
- Подготовка Фазы 6.
- Результат: relation-only в production.

## Спринт D (debt payoff)

- Фаза 6 полностью.
- Результат: закрытие legacy, упрощение поддержки.

---

## 5. Тестовая стратегия

## 5.1 Обязательные интеграционные сценарии

1. User creation из всех входов приводит к валидным `user_roles`.
2. Проверка `users:list/read/update/manage` опирается на relation-права.
3. Изменение роли пользователя отражается в доступе после обновления контекста/кэша.
4. Reset-password во всех API-каналах отзывают сессии единообразно.
5. Tenant isolation: права пользователя tenant A не влияют на tenant B.

## 5.2 Регрессионные проверки

- Негативные проверки denied по всем критичным операциям.
- Проверка, что отсутствие `user_roles` приводит к отказу доступа (fail-closed).
- Нагрузочный тест на resolver + cache.

---

## 6. Наблюдаемость и эксплуатация

Ввести и мониторить:

- `rbac_resolver_latency_ms` (p50/p95/p99).
- `rbac_cache_hit_total`, `rbac_cache_miss_total`.
- `rbac_decision_deny_total` с reason labels.
- `rbac_decision_mismatch_total` (только в dual-read период).
- `users_without_roles_total` (consistency gauge).
- `orphan_user_roles_total` (consistency gauge for `user_roles` without `roles`).
- `orphan_role_permissions_total` (consistency gauge for `role_permissions` without `permissions`).
- `consistency_query_failures_total` (операционный счётчик сбоев расчёта consistency-gauges).
- `consistency_query_latency_ms_total` / `consistency_query_latency_samples` (латентность SQL-расчёта consistency-gauges).

Оповещения:

- рост deny/error выше baseline,
- любой `mismatch_total > 0` после cutover,
- рост users_without_roles.

---

## 7. Риски и меры снижения

1. **Риск:** рост латентности из-за join-ов.
   - **Мера:** короткоживущий кэш пермишенов + индексы + батчинг.

2. **Риск:** рассинхрон в период миграции.
   - **Мера:** dual-read, метрика mismatch, staged rollout.

3. **Риск:** частичный backfill.
   - **Мера:** dry-run отчёт + идемпотентные скрипты + post-check job.

4. **Риск:** недокументированное поведение для команд.
   - **Мера:** ADR + обновлённые runbook/архитектурные доки.

---

## 8. Definition of Done (финальный)

- Relation RBAC — единственный runtime source of truth.
- Все публичные user-flow консистентны.
- Legacy role-проверки убраны из decision-path.
- Мониторинг показывает стабильность после cutover.
- Документация в `docs/` отражает фактическое состояние.

---

## 9. Контрольный чеклист «ничего не упустить» (phase-by-phase)

Ниже — операционный чеклист для лидов команды. Каждый пункт должен быть отмечен в трекере задач (Jira/Linear) и привязан к MR/PR.

### 9.1 Фаза 0 — Подготовка

- [x] ADR создан в `DECISIONS/` и согласован platform/team leads (`2026-02-26-rbac-relation-source-of-truth-cutover.md`).
- [ ] Определён владелец migration-program (DRI) и резервный DRI.
- [ ] Определён freeze-период для изменения RBAC API-контрактов.
- [ ] Зафиксированы feature flags:
  - [ ] `rbac_relation_dual_read_enabled`
  - [ ] `rbac_relation_enforcement_enabled`
  - [ ] `rbac_legacy_role_fallback_enabled` (временный)
- [ ] Обновлены `docs/architecture/rbac.md` и `docs/index.md`.

### 9.2 Фаза 1 — Консистентность flow

- [x] Проверены все entrypoints создания пользователя:
  - [x] REST register
  - [x] GraphQL sign_up
  - [x] GraphQL create_user
  - [x] invite accept
  - [x] seed / sync / system bootstrap (seed_user)
- [x] В каждом flow гарантированно формируются `user_roles` (все публичные entrypoints покрыты; `GraphQL update_user` синхронизирует через `replace_user_role`).
- [x] В каждом flow роль и tenant валидируются до записи.
- [x] Reset password в REST и GraphQL имеет одинаковую policy отзыва сессий (`AuthLifecycleService::confirm_password_reset` используется обоими каналами и всегда вызывает `revoke_user_sessions(..., None)`).
- [x] Интеграционные тесты и release-gate по auth parity документированы в `docs/architecture/api.md` (раздел «Auth lifecycle consistency и release-gate»); RBAC-специфичные flow-инварианты отслеживаются в этом плане.

### 9.3 Фаза 2 — Resolver

- [x] Введён единый tenant-aware resolver API.
- [x] Удалено дублирование permission-логики из handlers/resolvers.
- [x] Проверки прав на критичных маршрутах переведены на resolver.
- [x] Включено fail-closed поведение при ошибке резолва.
- [x] Добавлены метрики latency/hit/miss/denied.

### 9.4 Фаза 3 — Auth context и токены

- [x] AuthContext получает permissions только из relation-resolver.
- [x] JWT role-claim не влияет на authorization decision.
- [x] Внедрена инвалидация permission-cache при изменениях ролей.
- [x] Проверено поведение long-lived sessions после изменения прав (добавлен модульный regression-test `role_assignment_operations_invalidate_cached_permissions` в `rustok-rbac::RuntimePermissionResolver`, подтверждающий инвалидацию permission-cache и применение обновлённых прав в рамках активного session-context).

### 9.5 Фаза 4 — Data migration

- [x] Подготовлен idempotent backfill-script (cleanup `target=rbac-backfill` + helper `scripts/rbac_relation_staging.sh`).
- [ ] Выполнен dry-run с отчётом расхождений.
  - [x] Автоматизирована генерация dry-run отчёта (`cleanup target=rbac-backfill report_file=...` + `scripts/rbac_relation_staging.sh` сохраняет `rbac_backfill_dry_run_*.json` и summary в stage-report).
  - [x] Автоматизирована генерация rollback dry-run/apply отчётов (`cleanup target=rbac-backfill-rollback report_file=...` + `scripts/rbac_relation_staging.sh` сохраняет `rbac_backfill_rollback_*.json` и summary в stage-report).
  - [x] Для CI/staging добавлен строгий artifact-gate: `scripts/rbac_relation_staging.sh --require-report-artifacts` (fail-fast, если ожидаемые JSON-артефакты отчётов отсутствуют после каждого включённого этапа).
  - [x] Добавлен smoke-test helper `scripts/test_rbac_relation_staging.sh` (fake `cargo loco task`) для локальной проверки generation/gating/summary без внешних зависимостей.
- [ ] Выполнен backfill на staging.
- [ ] Выполнен post-check целостности.
- [~] Подготовлен rollback-план (CLI rollback target + snapshot-файл), проверка на staging в работе.

### 9.6 Фаза 5 — Cutover

- [ ] Dual-read включён в production.
- [ ] Mismatch-метрика собирается и алертится.
- [ ] Зафиксирован baseline и окно наблюдения.
  - [x] Добавлен автоматизированный helper baseline-съёма (`scripts/rbac_cutover_baseline.sh`) с JSON/markdown артефактами и strict mismatch-gate.
- [ ] Переключение на relation-only выполнено под feature flag.
- [ ] Проведён пост-релизный аудит 401/403/latency.

### 9.7 Фаза 6 — Cleanup

- [ ] Удалены legacy decision-paths.
- [ ] `users.role` переведён в read-only/derived (или удалён по ADR).
- [ ] Удалены временные флаги и fallback-код.
- [ ] Обновлены runbooks/onboarding/операционные инструкции.

---

## 10. Матрица ответственности (RACI, укрупнённо)

| Направление | Responsible | Accountable | Consulted | Informed |
|---|---|---|---|---|
| ADR и архитектурное решение | Platform architect | Head of Engineering | Security lead, Backend lead | Все команды |
| Runtime migration (server) | Backend team | Backend lead | Platform architect | Frontend teams |
| Data backfill и DB integrity | Backend + DBA | Backend lead | SRE | Platform team |
| Observability и алерты | SRE/Platform | SRE lead | Backend lead | On-call rotation |
| Cutover и release control | Release manager | Head of Engineering | SRE, Backend lead | Product/Support |
| Cleanup legacy и docs finalization | Backend + Platform docs owner | Backend lead | QA | Все команды |

> Примечание: конкретные имена и команды назначаются в трекере проекта перед стартом Фазы 1.

---

## 11. Артефакты на выходе каждой фазы

| Фаза | Обязательные артефакты |
|---|---|
| 0 | ADR, список feature flags, baseline docs update |
| 1 | PR-ы по flow-consistency, integration tests report |
| 2 | Resolver API + migration PR-ы, observability dashboard v1 |
| 3 | AuthContext/JWT policy PR, cache invalidation design note |
| 4 | Backfill script + dry-run report + post-check report |
| 5 | Cutover runbook, mismatch trend report, production validation report |
| 6 | Legacy cleanup PR, final architecture docs, deprecation note |

---

## 11.1 MVP execution board (оперативный, только текущий этап)

Используется как короткий операционный статус до завершения relation-only cutover.

- [ ] **MVP-блокер 1 (Фаза 4):** staged rehearsal завершён (dry-run/backfill/rollback) + приложен отчёт по инвариантам.
  - Артефакты: `artifacts/rbac-staging/*`, stage-report markdown, pre/post JSON consistency reports; проверка проходит по runbook-последовательности из раздела 13.1/13.5.
- [x] **MVP-блокер 2 (Фаза 5 prep):** ADR final cutover согласован с явным rollback-gate и stop-the-line условиями.
  - Артефакты: `DECISIONS/2026-03-05-rbac-relation-only-final-cutover-gate.md`.
- [ ] **MVP-блокер 3 (Фаза 5):** production dual-read окно наблюдения завершено, baseline зафиксирован, принято go/no-go решение для relation-only.
  - Артефакты: baseline report/json из `artifacts/rbac-cutover/*`, запись решения (go/no-go) в release-notes/runbook; gate проверяется по разделам 13.2/13.3/13.5.

Правило обновления: после каждого merge меняется только статус соответствующего блокера и ссылка на артефакт; scope этапа не расширяется.

---

## 12. Гейты релиза и критерии остановки (stop-the-line)

### 12.1 Gate перед production cutover

- [ ] Все P0/P1 дефекты по RBAC закрыты.
- [ ] `users_without_roles_total == 0` (или согласованный whitelist).
- [ ] Mismatch в dual-read не растёт и стремится к 0.
- [ ] QA sign-off по критичным permission-сценариям.
- [ ] On-call команда проинструктирована и runbook актуален.

### 12.2 Stop-the-line условия

Немедленный rollback на предыдущий режим при любом из условий:

1. Рост 403/401 выше согласованного SLO-порога после переключения.
2. Непредвиденный массовый deny для системных ролей (admin/superadmin).
3. Ошибки резолва прав с деградацией business-critical endpoints.
4. Выявленная cross-tenant утечка прав или данных.

### 12.3 Post-cutover окно наблюдения

- Рекомендуемое окно: 7–14 дней.
- Ежедневная сверка:
  - `rbac_decision_mismatch_total`
  - denied/error rate
  - latency p95/p99
  - tenant-specific anomalies

---

## 13. Операционный runbook MVP-cutover (фазы 4–5)

Раздел фиксирует минимально необходимый, повторяемый сценарий для staged rehearsal и production cutover.

### 13.1 Staging rehearsal (обязателен перед go-live)

1. Подготовить директорию артефактов (пример: `artifacts/rbac-staging/<date>`).
2. Прогнать полный цикл:
   - dry-run backfill,
   - apply backfill,
   - post-check целостности,
   - rollback dry-run,
   - rollback apply (на тестовых данных/снэпшоте).
3. На каждом этапе требовать наличие JSON-отчёта (`--require-report-artifacts`).
4. Сформировать stage-report (markdown) с итоговыми инвариантами.

Критерий прохождения rehearsal:

- все шаги завершены без fail-fast по артефактам,
- нет роста `orphan_*` и `users_without_roles_total` после apply,
- rollback-цикл подтверждён на staging (в том же rehearsal-окне).

### 13.2 Production dual-read baseline

1. Включить dual-read в production с feature-flag.
2. Выдержать окно наблюдения (минимум 24 часа, целевое 72 часа+).
3. Снять baseline helper-скриптом и сохранить json/markdown артефакты в `artifacts/rbac-cutover/<date>`.
4. Проверить gate:
   - `mismatch_delta == 0` в целевом окне,
   - decision volume не ниже порога (`--min-decision-delta`),
   - нет аномального роста 401/403 и latency p95/p99.

### 13.3 Go / No-Go решение для relation-only

Решение принимается только при выполнении всех условий:

- MVP-блокер 1 закрыт (staged rehearsal complete + отчёты).
- MVP-блокер 2 закрыт (ADR final cutover согласован).
- MVP-блокер 3 закрыт (dual-read baseline complete).
- QA и on-call подтверждают readiness по runbook.

Если хотя бы одно условие не выполнено — фиксируется **No-Go**, переключение relation-only откладывается.

### 13.4 Обязательная структура cutover-отчёта

Каждый релизный цикл cutover должен завершаться единым отчётом (release-notes/runbook append) с 5 блоками:

1. **Scope:** tenant-охват, версия, feature-flag state.
2. **Data health:** значения `users_without_roles_total`, `orphan_user_roles_total`, `orphan_role_permissions_total` до/после.
3. **Decision health:** mismatch trend, deny/error trend, latency p95/p99.
4. **Инциденты:** любые отклонения, ручные интервенции, rollback-шаги (если были).
5. **Решение:** Go/No-Go + ответственные и timestamp.

### 13.5 Минимальный набор команд и артефактов (операционный baseline)

Для снижения вариативности между дежурными сменами используем стандартный каркас команд (параметры окружения подставляются release-менеджером):

1. **Staging rehearsal:** `scripts/rbac_relation_staging.sh --require-report-artifacts`.
2. **Production baseline:** `scripts/rbac_cutover_baseline.sh <...параметры окна наблюдения...>`.
3. **Auth gate перед переключением:** `scripts/auth_release_gate.sh --require-all-gates`.
4. **Final Go/No-Go helper:** `scripts/rbac_cutover_gate.sh --auth-gate-report <auth_report.md>`.

Минимальный набор артефактов, который должен быть приложен к go/no-go решению:

- stage-report markdown из rehearsal-цикла,
- dry-run/apply/rollback JSON-отчёты backfill-циклов,
- baseline json/markdown для dual-read окна,
- финальная запись решения (go/no-go) в release-notes/runbook.

Если хотя бы один обязательный артефакт отсутствует, решение автоматически трактуется как **No-Go**.

### 13.6 Режим rollback: приоритеты и SLA реакции

При срабатывании stop-the-line условий из раздела 12.2:

1. **Приоритет P0:** вернуть предыдущий enforcement-режим (отключить relation-only, восстановить предыдущее состояние флага).
2. **SLA реакции:** начало rollback-операции не позднее 15 минут с момента подтверждения инцидента on-call инженером.
3. **Коммуникация:** инцидент фиксируется в on-call канале и в release-notes с указанием tenant scope и impact.
4. **Post-incident:** в течение 24 часов оформляется краткий RCA и решение о повторном окне cutover.

---

## 14. Post-MVP backlog (вне текущего cutover scope)

Ниже — задачи, которые не блокируют relation-only switch, но должны быть запланированы после стабилизации:

1. Укрепить module boundary: завершить перенос policy/use-case API в `crates/rustok-rbac` и упростить server adapters.
2. Сократить временные feature-flags и удалить fallback-код после окна наблюдения.
3. Уточнить долгосрочную судьбу `users.role` отдельным ADR (удаление vs derived-only).
4. Консолидировать observability dashboard (RBAC migration + steady-state RBAC ops).
5. Обновить onboarding docs для backend/on-call с финальной relation-only моделью.

---

## 15. Переход на Casbin (casbin-rs): когда начинать и что ещё доделать

### 15.0 Текущий кодовый статус Casbin-track

- [x] Подготовлен foundation для staged rollout в `crates/rustok-rbac`: режимы `casbin_shadow` / `casbin_only` и алиасы feature-flags (`RUSTOK_RBAC_CASBIN_SHADOW_ENABLED`, `RUSTOK_RBAC_CASBIN_ENFORCEMENT_ENABLED`) добавлены в `RbacAuthzMode`.
- [x] Добавлен tenant-aware baseline `casbin_model.conf` и экспорт helper `default_casbin_model()` как стартовый артефакт этапа C0.
- [~] Shadow-path для Casbin parity подключён в runtime checks `AuthService::{has_permission,has_any_permission,has_all_permissions}` под режимом `RUSTOK_RBAC_AUTHZ_MODE=casbin_shadow`: сравнение выполняется через `rustok-rbac::evaluate_casbin_shadow` (matcher-compatible in-module evaluator), публикуются метрики `rustok_rbac_engine_decisions_relation_total`, `rustok_rbac_engine_decisions_casbin_total`, `rustok_rbac_engine_mismatch_total`, `rustok_rbac_engine_eval_duration_{ms_total,samples}`, mismatch логируется событием `rbac_engine_mismatch` с обязательными полями (`tenant_id`, `user_id`, `resource`, `action`, `relation_decision`, `casbin_decision`); полноценный `CasbinPermissionResolver` с persistent adapter/wiring остаётся следующим шагом C1/C2.

### 15.1 Точка старта (когда начинаем переход)

Переход на Casbin нужно запускать **после закрытия ядра Фазы 4** и до production relation-only cutover из Фазы 5.

Почему именно так:

1. Casbin не исправляет «грязные» данные сам по себе — сначала нужны валидные `user_roles` / `role_permissions` и нулевые orphan-инварианты.
2. Переключать одновременно и data-source, и policy-engine слишком рискованно; сначала фиксируем data health, затем меняем движок принятия решения.
3. Dual-read окно Фазы 5 можно использовать как controlled rollout для сравнения `legacy relation resolver` vs `casbin enforcer`.

**Решение по старту:**
- Минимальный gate начала Casbin track: `users_without_roles_total == 0` на staging + подтверждённый rehearsal dry-run/apply/rollback.
- Если этот gate не выполнен, Casbin-track откладывается, продолжается доработка Фазы 4.

### 15.2 Что нужно доделать перед интеграцией Casbin (pre-Casbin backlog)

1. Завершить staged rehearsal из раздела 13.1 с обязательными JSON-артефактами и stage-report.
2. Зафиксировать canonical permission naming contract (`resource:action`, wildcard-правила, tenant scope) и исключить неоднозначные алиасы.
3. Закрыть пробелы observability для текущего resolver:
   - стабильные метрики deny-reason,
   - latency p95/p99 по tenant,
   - готовый baseline для сравнения с Casbin.
4. Подготовить policy-fixture набор для регрессии:
   - system roles,
   - tenant-specific custom roles,
   - deny-by-default сценарии,
   - cross-tenant isolation кейсы.
5. Подготовить ADR: «Casbin as policy engine, relation tables as policy data source» с rollback-гейтом и SLO-гейтами.

### 15.3 Целевая модель Casbin в RusToK

- `casbin-rs` используется как **policy evaluation engine**.
- Таблицы `roles`, `permissions`, `user_roles`, `role_permissions` остаются источником данных (policy storage).
- `users.role` не участвует в decision-path (только legacy/derived поле при необходимости).
- Enforcement API в `crates/rustok-rbac` остаётся единым (`has_permission`, `has_any`, `has_all`), а Casbin скрыт за module boundary.

### 15.4 Этапы перехода на Casbin (инкрементально)

#### Этап C0 — Design + ADR (подготовка)

- Описать casbin model (`r/p/g`, wildcard matcher, tenant domain matcher).
- Выбрать adapter-стратегию:
  - DB adapter (прямое чтение relation-таблиц), либо
  - policy snapshot + reload.
- Зафиксировать cache invalidation и policy reload SLA.

**Выход:** утверждённый ADR + модель `model.conf` (или эквивалент в коде).

#### Этап C1 — Shadow integration в `crates/rustok-rbac`

- Добавить `CasbinPermissionResolver` как альтернативную реализацию текущего resolver-контракта.
- Включить shadow-evaluation: runtime decision остаётся за текущим relation resolver, Casbin считает параллельно.
- Добавить mismatch-метрики:
  - `rbac_engine_mismatch_total{source="relation",target="casbin"}`
  - `rbac_engine_eval_latency_ms{engine="casbin"}`.

**Выход:** Casbin работает в shadow без влияния на production decisions.

#### Этап C2 — Staging parity hardening

- Прогнать full regression matrix (раздел 5 + policy-fixtures).
- Добиться `engine_mismatch == 0` на staging окне наблюдения.
- Зафиксировать производительность (p95/p99) не хуже согласованного порога.

**Выход:** staging parity report + perf report.

#### Этап C3 — Production dual-engine rollout

- В production оставить relation engine как active, Casbin как shadow (feature flag).
- Снять baseline 72h+ с decision volume gate.
- Проанализировать все mismatch > 0, устранить причины, повторить окно.

**Выход:** production parity baseline (go/no-go для переключения active engine).

#### Этап C4 — Switch active engine на Casbin

- Переключить active enforcement на Casbin под флагом `rbac_casbin_enforcement_enabled`.
- Relation resolver оставить как fallback только на окно стабилизации.
- Усилить on-call мониторинг 401/403/latency и deny-reason anomalies.

**Выход:** Casbin — active policy engine, инцидентов выше SLO нет.

#### Этап C5 — Cleanup legacy resolver path

- Удалить fallback relation decision-path после окна стабилизации.
- Удалить временные флаги shadow/dual-engine.
- Обновить `docs/architecture/rbac.md`, runbook и onboarding под Casbin steady-state.

**Выход:** один production engine (Casbin), документация синхронизирована.

### 15.5 Рекомендуемая привязка Casbin-этапов к текущим фазам плана

- Текущая Фаза 4 (data migration): закрыть полностью **до старта C0/C1**.
- Текущая Фаза 5 (dual-read/cutover):
  - первая часть = C3 (dual-engine parity),
  - вторая часть = C4 (switch active engine).
- Текущая Фаза 6 (cleanup): включает C5 + удаление оставшихся legacy-role и fallback веток.

Итого: **практический старт Casbin — на стыке Фазы 4 → Фазы 5**, не раньше.

### 15.6 Definition of Done для Casbin-перехода

1. Все permission checks идут через `crates/rustok-rbac`, active engine = Casbin.
2. `engine_mismatch_total` стабильно 0 в согласованном окне после switch.
3. SLO 401/403/latency не ухудшены относительно relation-only baseline.
4. Rollback-сценарий проверен rehearsal-ом и остаётся исполнимым до конца stabilization window.
5. Документация и runbook описывают только актуальный Casbin-based enforcement flow.

---

## 16. Операционный шаблон статуса для Casbin migration

Каждый merge в рамках этапов C0–C5 обновляет минимум:

1. `phase`: текущий этап (`C0..C5`).
2. `engine_mode`: `relation-active/casbin-shadow` или `casbin-active/relation-fallback`.
3. `parity`: mismatch summary + ссылка на артефакт (`artifacts/rbac-cutover/*`).
4. `gate_decision`: `go/no-go/n-a` и краткая причина.

Рекомендуемый формат записи в PR:

`RBAC-CASBIN-UPDATE: phase=<C0..C5>; engine_mode=<...>; parity=<artifact>; gate=<go|no-go|n-a>`

---

## 17. Продолжение реализации: ближайшие deliverables (next PR queue)

Чтобы «продолжить реализацию» без расползания scope, фиксируем очередь следующих PR с чётким DoD.

### 17.1 PR-1 (C0): Casbin ADR + модель

**Что делаем:**
- оформить ADR для Casbin-track (engine role, adapter choice, rollback, SLO-gates),
- зафиксировать `model.conf` (или кодовый эквивалент) для tenant-aware matcher,
- добавить ссылку на ADR в раздел 11.1 (MVP execution board) и в раздел 15.2.

**DoD PR-1:**
- ADR принят,
- matcher покрывает wildcard + tenant domain,
- rollback-путь явно описан и совместим с текущим runbook.

### 17.2 PR-2 (C1): Shadow wiring в `crates/rustok-rbac`

**Что делаем:**
- подключить Casbin-resolver за feature flag без изменения active decision path,
- добавить structured-логи mismatch (с tenant/user/resource/action),
- добавить метрики из раздела 15.4 (C1).

**DoD PR-2:**
- active engine в production остаётся relation,
- shadow path не ломает fail-closed semantics,
- метрики/логи доступны на dashboard и в алертах.

### 17.3 PR-3 (C2): Staging parity report

**Что делаем:**
- прогоняем regression matrix + policy-fixtures,
- собираем parity/perf отчёты в `artifacts/rbac-cutover/<date>/...`,
- фиксируем go/no-go запись по этапу C2.

**DoD PR-3:**
- `engine_mismatch == 0` в окне наблюдения staging,
- p95/p99 укладываются в согласованный порог,
- отчёт приложен и ссылается на конкретные артефакты.

### 17.4 PR-4 (C3/C4): Production dual-engine -> switch

**Что делаем:**
- открываем production dual-engine окно (Casbin shadow),
- при соблюдении gate переводим Casbin в active mode,
- relation оставляем только как rollback fallback на stabilization window.

**DoD PR-4:**
- формально закрыты gate из разделов 12.1 и 15.6,
- on-call подтверждает отсутствие SLO-регрессии,
- запись `RBAC-CASBIN-UPDATE` заполнена полностью.

---

## 18. Casbin-специфичные риски и контрмеры

1. **Risk:** разнобой semantics между SQL relation-check и Casbin matcher.
   - **Countermeasure:** единый contract-test suite на одинаковых fixtures для обоих engines.
2. **Risk:** устаревание policy snapshot при burst-изменениях ролей.
   - **Countermeasure:** event-driven invalidation + верхняя граница TTL + forced reload endpoint для on-call.
3. **Risk:** непредсказуемый рост latency при сложных matcher-правилах.
   - **Countermeasure:** ограничить matcher complexity в ADR, профилировать hot-path до production switch.
4. **Risk:** тихие mismatch без операционного действия.
   - **Countermeasure:** mismatch-alert переводится в page-level инцидент в окне C3/C4.

---

## 19. Детализация ближайшего цикла (execution slice на 2 недели)

Чтобы зафиксировать конкретный «следующий шаг», без расширения scope beyond MVP, вводим короткий execution slice.

### 19.1 Цель цикла

Закрыть подготовку к C1/C2 без переключения active engine:

- завершить PR-1 (Casbin ADR + финальная модель),
- поднять shadow wiring в `crates/rustok-rbac` (PR-2),
- собрать staging parity baseline для go/no-go по C2 (PR-3 prep).

### 19.2 Scope в цикле (что делаем)

1. Оформляем и принимаем Casbin ADR с точным описанием:
   - matcher semantics,
   - rollout/rollback режимов,
   - SLO-гейтов для switch decision.
2. Подключаем `CasbinPermissionResolver` в shadow-mode:
   - relation остаётся active decision path,
   - mismatch и latency пишутся в метрики,
   - fail-closed поведение не меняется.
3. Готовим staging parity прогон:
   - policy-fixtures для системных ролей,
   - negative/tenant-isolation сценарии,
   - экспорт отчётов в `artifacts/rbac-cutover/<date>/`.

### 19.3 Scope вне цикла (что не делаем)

- Не включаем `casbin_only` в production.
- Не удаляем relation fallback.
- Не расширяем матрицу прав beyond уже согласованных системных ролей.

### 19.4 DoD цикла

Цикл считается завершённым только если одновременно выполнено всё:

1. Принят ADR Casbin-track и добавлена cross-ссылка в этот план.
2. В staging есть mismatch/latency артефакты shadow-режима (минимум за 24ч окно).
3. `engine_mismatch_total` документирован в отчёте (ожидаемо: `0`, либо явный список расхождений с RCA).
4. Выпущено go/no-go решение по переходу к C3 (production dual-engine).

---

## 20. Матрица ответственности (RACI-lite для cutover)

Чтобы исключить «ничейные» шаги, фиксируем минимальную роль-матрицу.

| Поток работ | Responsible | Accountable | Consulted | Informed |
|---|---|---|---|---|
| Backfill / consistency checks (Фаза 4) | Platform backend engineer | Platform lead | DBA/on-call | QA, release manager |
| Shadow wiring Casbin (C1) | RBAC module owner | Platform lead | Security reviewer | QA |
| Staging parity report (C2) | QA + RBAC engineer | Release manager | On-call lead | Platform team |
| Production go/no-go (C3/C4) | Release manager | Platform lead | On-call + QA + RBAC owner | All stakeholders |
| Rollback execution (12.2 / 13.6) | On-call engineer | Incident commander | Platform lead | Release manager, QA |

Правило эскалации: если `Responsible` не назначен по конкретному релизному окну, автоматический статус шага — **No-Go**.

---

## 21. Шаблон weekly status update (для section 11.1 + 16)

Чтобы статусы в этом плане обновлялись единообразно, используем weekly-шаблон:

```text
RBAC-WEEKLY-STATUS:
- period: <YYYY-MM-DD..YYYY-MM-DD>
- phase: <4|5|6|C0|C1|C2|C3|C4|C5>
- mvp_blockers: <#1=...; #2=...; #3=...>
- engine_mode: <relation-active/casbin-shadow|casbin-active/relation-fallback>
- data_health: <users_without_roles=...; orphan_user_roles=...; orphan_role_permissions=...>
- decision_health: <mismatch=...; deny_rate=...; latency_p95=...; latency_p99=...>
- gate_decision: <go|no-go|n-a>
- links:
  - artifacts: <path>
  - report: <path>
  - incident_or_rca: <path|n-a>
```

### 21.1 Минимальные требования к weekly-апдейту

1. Числовые поля (`data_health`, `decision_health`) должны быть фактическими, не `TBD`.
2. Любой `no-go` обязан иметь причину + corrective action owner.
3. Любой `go` обязан ссылаться на артефакты из раздела 13.5.

### 21.2 Правило консистентности документа

После публикации weekly-апдейта обязательно синхронизировать:

- section 0 (progress tracker),
- section 11.1 (MVP execution board),
- section 16 (Casbin migration status).

Это предотвращает рассинхрон между «оперативным» и «архитектурным» статусом.

---

## 22. Practical checklist для следующего PR-цикла (C0 → C2)

Ниже фиксируется «исполняемый минимум» на продолжение работ, чтобы каждое изменение можно было проверить по артефактам, а не по формулировкам в описании PR.

### 22.1 Обязательные задачи в порядке выполнения

1. **C0 / ADR-ready:**
   - ADR создан в `DECISIONS/` и содержит разделы: `Context`, `Decision`, `Rollback`, `SLO gates`, `Non-goals`.
   - в разделе 11.1 этого плана добавлена ссылка на ADR и выставлен статус MVP-блокера 2.
2. **C1 / Shadow wiring-ready:**
   - shadow path подключён за feature-flag и не меняет active decision path.
   - добавлены метрики `rbac_engine_decisions_total`, `rbac_engine_mismatch_total`, `rbac_engine_eval_duration_ms`.
   - добавлен structured-log mismatch с обязательными полями: `tenant_id`, `user_id`, `resource`, `action`, `relation_decision`, `casbin_decision`.
3. **C2 / Staging parity-ready:**
   - прогнан staging baseline минимум в 24-часовом окне.
   - собраны markdown+json отчёты и сохранены в `artifacts/rbac-cutover/<date>/`.
   - принято go/no-go решение и отражено в section 16 (`RBAC-CASBIN-UPDATE`).

### 22.2 Минимальный пакет файлов/артефактов на каждый PR

Для PR-ов C0/C1/C2 обязательны ссылки на конкретные артефакты:

- `artifacts/rbac-cutover/<date>/baseline.json`
- `artifacts/rbac-cutover/<date>/baseline.md`
- `artifacts/rbac-cutover/<date>/mismatch-sample.jsonl` (может быть пустым файлом при `mismatch=0`)
- `artifacts/rbac-cutover/<date>/gate-decision.md`

Если хотя бы одна ссылка отсутствует, PR не может считаться закрывающим этап.

### 22.3 Технические критерии приёмки (default thresholds)

До появления отдельного ADR с иными порогами используем значения по умолчанию:

- `engine_mismatch_total == 0` в окне принятия решения,
- `decision_volume_delta >= 0` относительно relation baseline,
- `latency_p95_delta <= +10%`,
- `latency_p99_delta <= +15%`,
- `401/403_rate_delta <= +5%` без подтверждённого функционального изменения policy.

Нарушение любого пункта автоматически переводит решение в **No-Go** до RCA и corrective action.

### 22.4 Формат corrective action (обязателен для каждого No-Go)

Для каждого `No-Go` фиксируется запись в `gate-decision.md`:

1. `root_cause` (кратко, 1–3 пункта),
2. `owner` (конкретная роль/команда),
3. `target_date` (дата повторной проверки),
4. `verification_step` (какой отчёт/метрика подтвердит исправление).

Это требование закрывает «серую зону», когда решение No-Go есть, но плана выхода нет.

---

## 23. План ближайших 3 PR (операционное продолжение без расширения scope)

Чтобы продолжение шло предсказуемо, фиксируем минимальный трек из трёх PR с измеримым результатом каждого шага.

### 23.1 PR-A (C0): ADR + policy contract freeze

**Задача:** закрыть архитектурный долг до внедрения runtime-shadow.

**Минимум в PR-A:**
1. ADR в `DECISIONS/` с финальной формой matcher и перечнем non-goals.
2. Явная карта rollback-переключателей (какой флаг, кто переключает, где фиксируется действие).
3. Decision table для deny-reason категорий (`no-role`, `no-permission`, `cross-tenant`, `resolver-error`).

**Артефакты выхода:**
- `DECISIONS/<date>-rbac-casbin-cutover-adr.md`
- ссылка на ADR в section 11.1 и section 15.2
- обновление статуса `RBAC-CASBIN-UPDATE: phase=C0`

### 23.2 PR-B (C1): runtime shadow + telemetry hardening

**Задача:** подключить Casbin в shadow без изменения production decision path.

**Минимум в PR-B:**
1. Shadow resolver вызывается параллельно relation-resolver.
2. Mismatch логируется только в структурированном формате (без свободного текста как единственного источника).
3. Метрики C1 доступны в dashboard и снабжены базовыми alert conditions.

**Артефакты выхода:**
- `artifacts/rbac-cutover/<date>/shadow-smoke.md`
- `artifacts/rbac-cutover/<date>/metrics-snapshot.json`
- `RBAC-CASBIN-UPDATE: phase=C1; engine_mode=relation-active/casbin-shadow`

### 23.3 PR-C (C2): staging parity + formal gate decision

**Задача:** подтвердить готовность к production dual-engine окну.

**Минимум в PR-C:**
1. parity-окно staging не менее 24 часов.
2. Отчёт включает сравнение объёма решений (`decision volume`) и latency delta.
3. Формальная запись gate (`go`/`no-go`) с owner и timestamp.

**Артефакты выхода:**
- `artifacts/rbac-cutover/<date>/baseline.json`
- `artifacts/rbac-cutover/<date>/baseline.md`
- `artifacts/rbac-cutover/<date>/gate-decision.md`
- `RBAC-CASBIN-UPDATE: phase=C2; gate=<go|no-go>`

### 23.4 Правило последовательности (strict order)

- PR-B не стартует до принятия PR-A.
- PR-C не стартует до подтверждённого telemetry baseline из PR-B.
- Параллельные изменения, затрагивающие policy semantics, в этом окне запрещены (чтобы не ломать parity-интерпретацию).

Нарушение порядка автоматически требует переоткрытия gate и пересчёта baseline-артефактов.

### 23.5 Критерий «план выполняется» (weekly control)

На еженедельном синке план считается исполняемым только если одновременно:

1. У текущего этапа есть owner и целевая дата завершения.
2. У этапа есть хотя бы один свежий артефакт не старше 7 дней.
3. Статус в section 0, section 11.1, section 16 и section 21 не конфликтует между собой.

Если хотя бы один пункт не выполнен — статус плана на неделю: **at risk**, и переключение в следующий этап запрещено.

---

## 24. Ready-to-run checklist для следующего исполнения (операционный минимум на 1 неделю)

Чтобы команда могла продолжить работу без дополнительной декомпозиции, ниже фиксируется «чеклист запуска», привязанный к PR-A/PR-B/PR-C.

### 24.1 День 1 — закрытие PR-A (C0)

1. Подготовить ADR draft и пройти review у platform + security.
2. Зафиксировать финальные названия feature-flags (shadow/enforcement/fallback).
3. Добавить в ADR таблицу rollback-действий (кто, где, за сколько минут).

**Готово, если:**
- ADR merged,
- в section 11.1 MVP-блокер 2 обновлён,
- в section 16 добавлен `RBAC-CASBIN-UPDATE: phase=C0`.

### 24.2 День 2–3 — закрытие PR-B (C1)

1. Подключить shadow path в runtime wiring без изменения active decision.
2. Прокинуть метрики и structured logs в observability stack.
3. Проверить fail-closed семантику на synthetic сценариях `resolver-error`.

**Готово, если:**
- relation остаётся active,
- mismatch/latency видны в dashboard,
- есть артефакт `shadow-smoke.md` с timestamp и owner.

### 24.3 День 4–5 — подготовка PR-C (C2 prep)

1. Запустить staging parity окно минимум на 24 часа.
2. Собрать baseline артефакты (`baseline.json`, `baseline.md`, `gate-decision.md`).
3. Оформить предварительное решение `go/no-go` с указанием причины.

**Готово, если:**
- артефакты присутствуют и валидны,
- расчёт threshold из section 22.3 приложен в отчёте,
- статус section 0/11.1/16/21 синхронизирован.

---

## 25. Definition of Ready для production dual-engine окна (C3)

C3 можно открывать только если одновременно выполнены все пункты:

1. `C0` принят (ADR утверждён, rollback-гейт зафиксирован).
2. `C1` завершён (shadow path стабилен, telemetry без пробелов).
3. `C2` завершён (staging parity отчёт и formal gate decision опубликованы).
4. On-call получил короткий runbook-briefing и подтвердил readiness.
5. Release manager назначил окно переключения и incident commander на период наблюдения.

Если любой пункт не выполнен — решение автоматически `No-Go`.

---

## 26. Шаблон записи для gate-decision.md (единый формат)

```md
# RBAC Gate Decision

- date: <YYYY-MM-DD>
- phase: <C2|C3|C4>
- decision: <go|no-go>
- owner: <role/team>

## Metrics snapshot
- engine_mismatch_total: <value>
- decision_volume_delta: <value>
- latency_p95_delta: <value>
- latency_p99_delta: <value>
- 401_403_rate_delta: <value>

## Evidence
- baseline_json: artifacts/rbac-cutover/<date>/baseline.json
- baseline_md: artifacts/rbac-cutover/<date>/baseline.md
- mismatch_sample: artifacts/rbac-cutover/<date>/mismatch-sample.jsonl

## Notes
- summary: <short>
- rollback_readiness: <ready|not-ready>

## Corrective action (required for no-go)
- root_cause: <...>
- owner: <...>
- target_date: <...>
- verification_step: <...>
```

Этот шаблон обязателен для C2/C3/C4, чтобы исключить несопоставимые форматы решений между релизными окнами.
