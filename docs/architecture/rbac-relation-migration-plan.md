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
- [ ] **Фаза 6 — Cleanup legacy-модели:** не начато.

### Что осталось приоритетно на ближайший шаг

1. Провести staged dry-run/backfill/rollback в staging и приложить отчёт по инвариантам (MVP-блокер №1, Фаза 4).
2. Подготовить и согласовать ADR по final cutover (`relation-only`) с чётким rollback-гейтом (MVP-блокер №2) и ссылкой на актуальные auth parity gate-артефакты из user/auth плана.
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
- [ ] **MVP-блокер 2 (Фаза 5 prep):** ADR final cutover согласован с явным rollback-gate и stop-the-line условиями.
  - Артефакты: ADR в `DECISIONS/` + ссылка в этом плане.
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
