# План устранения косяков в user/auth логике (без миграции RBAC)

- Дата: 2026-02-26 (актуализировано: 2026-03-04)
- Статус: In progress (Phase D: локальный integration gate пройден, staging parity + security sign-off остаются открытыми)
- Область: `apps/server` (REST + GraphQL auth), `apps/server/docs`, `docs/architecture/*`
- Граница плана: этот документ **не дублирует RBAC migration** и не заменяет `rbac-relation-migration-plan.md`. Здесь фиксируем только косяки user/auth потоков, которые нужно закрыть до/параллельно RBAC cutover.

---

## 1. Что именно исправляем

> Ниже перечислены **исторические исходные проблемы**, с которых стартовал remediation-поток.
> Актуальный статус выполнения фиксируется в разделах 5 и 8 (Phases A-C закрыты, Phase D — rollout/gate verification; локальный integration прогон выполнен).

### 1.1 Несогласованная логика создания пользователя

Проблема:
- `GraphQL create_user` создаёт пользователя, но не назначает RBAC-связи через `AuthService::assign_role_permissions`.
- `register`/`sign_up` делают это явно.

Риск:
- Рассинхрон между `users.role` и `user_roles`/`role_permissions`.
- Нестабильное поведение прав в зависимости от entrypoint.

### 1.2 Reset password: REST и GraphQL расходятся

Проблема:
- В GraphQL `reset_password` сессии пользователя отзываются.
- В REST `confirm_reset` пароль меняется, но сессии не инвалидируются.

Риск:
- Скомпрометированная сессия может пережить reset (security issue).

### 1.3 Дублирование бизнес-логики auth между REST и GraphQL

Проблема:
- `register/login/refresh/reset/change-password` реализованы в двух местах и уже имеют расхождения.

Риск:
- Любое следующее изменение увеличивает вероятность расхождений и инцидентов.

---

## 2. Цели и non-goals

## 2.1 Цели

1. Сделать user/auth flows консистентными между REST и GraphQL.
2. Убрать расхождение по policy инвалидирования сессий.
3. Вынести auth lifecycle в единый application service (thin adapters в transport).
4. Закрыть инвариантными интеграционными тестами критичные сценарии.
5. Синхронизировать документацию и ADR после внедрения.

## 2.2 Non-goals

- Полная миграция RBAC source-of-truth (вынесена в отдельный план).
- Перепроектирование всей схемы identity/SSO.

---

## 3. Целевая модель для user/auth

## 3.1 Единый Auth Lifecycle Service

Создаётся слой use-cases, например `AuthLifecycleService`, который является единственным местом бизнес-решений для:

- `create_user(...)`
- `register(...)`
- `login(...)`
- `refresh(...)`
- `request_password_reset(...)`
- `confirm_password_reset(...)`
- `change_password(...)`
- `update_profile(...)`

REST/GraphQL должны вызывать этот сервис, оставаясь thin adapters (парсинг/валидация I/O, маппинг ошибок/ответов).

## 3.2 Единая session invalidation policy

Фиксируем policy (обязательна для всех каналов):

- при `confirm_password_reset` отзываются **все активные сессии пользователя**;
- при `change_password` отзываются все сессии, кроме текущей (или включая текущую — выбрать явно в ADR и применять единообразно);
- policy одинакова для REST и GraphQL.

## 3.3 Единая точка создания пользователя

Любой user-creation flow (register/sign_up/create_user/invite/seed) обязан проходить через общий use-case, где гарантируются:

- tenant-scope валидация;
- email uniqueness;
- password hashing;
- назначение role/статуса;
- формирование связанных auth-данных (в т.ч. вызов назначения RBAC-связей согласно текущему контракту).

---

## 4. План внедрения по фазам

## Фаза A — Hotfix consistency & security (P0)

Шаги:
1. В `create_user` добавить вызов назначения RBAC-связей (или централизовать через общий use-case).
2. В REST `confirm_reset` добавить отзыв сессий пользователя (tenant+user).
3. Выровнять policy между `confirm_reset` REST и `reset_password` GraphQL.
4. Добавить минимальные integration tests на эти 2 инварианта.

Критерии готовности:
- Нет flow создания пользователя без RBAC-связей.
- Нет reset-пути, где пароль меняется без отзыва сессий.

## Фаза B — Application service extraction (P1)

Шаги:
1. Ввести `AuthLifecycleService` + контракты ошибок.
2. Перенести бизнес-логику из REST handlers и GraphQL mutations в сервис.
3. Оставить в транспортах только adapter-логику.
4. Добавить транзакционные границы там, где изменение пользователя и сессий должно быть атомарным.

Критерии готовности:
- REST/GraphQL не содержат дублирующих бизнес-веток auth.
- Ключевые операции проходят через единый сервис.

## Фаза C — Policy hardening (P1)

Шаги:
1. Явно зафиксировать policy для `change_password` и `reset_password` в ADR.
2. Добавить защитные проверки консистентности статуса пользователя при login/sign_in.
3. Проверить и унифицировать аудит/логирование событий auth lifecycle.

Критерии готовности:
- Security policy одинакова во всех входах.
- Нет расхождения по active/inactive поведению между каналами.

## Фаза D — Test coverage & rollout controls (P1/P2)

Шаги:
1. Добавить инвариантные e2e/integration тесты (см. раздел 6).
2. Добавить release gate перед выкатом.
3. Подготовить rollback-инструкцию для auth-lifecycle изменений.

Критерии готовности:
- Тесты покрывают критичные сценарии и проходят стабильно.
- Есть формальные go/no-go критерии релиза.

---

## 5. Backlog задач (исполняемый)

## 5.1 Кодовые задачи

- [x] `create_user`: гарантировать назначение связей после создания пользователя.
- [x] `confirm_reset` (REST): добавить отзыв active sessions.
- [x] Вынести общий `AuthLifecycleService` и перевести:
  - [x] register/sign_up
  - [x] login/sign_in
  - [x] refresh
  - [x] reset flows
  - [x] change_password
- [x] Унифицировать маппинг ошибок (REST status codes и GraphQL errors).
- [x] Выравнять session invalidation semantics между REST и GraphQL для `sign_out`/`change_password`/`reset_password` (soft-revoke через `revoked_at`).

## 5.2 Документационные задачи

- [x] ADR: "Auth lifecycle unification + session invalidation policy".
- [x] Обновить `docs/architecture/api.md` (REST/GraphQL adapters + service layer).
- [x] Обновить `docs/architecture/rbac.md` ссылкой на разделение ответственности между этим планом и RBAC migration plan.
- [x] Обновить `apps/server/docs/README.md` кратким changelog по auth behavior.

---

## 6. Инвариантные тесты (обязательный минимум)

1. `create_user` (GraphQL) создаёт пользователя и требуемые связанные auth-данные.
2. `confirm_reset` (REST) отзывает сессии пользователя.
3. `reset_password` GraphQL и REST `confirm_reset` дают одинаковый результат по сессиям.
4. Проверка, что поведение users-permissions одинаково для пользователей из разных entrypoints.
5. Негативные сценарии:
   - invalid reset token,
   - inactive user login/sign_in,
   - повторный вызов на уже отозванной сессии.

---

## 7. Наблюдаемость и эксплуатация

Метрики/сигналы:

- `auth_password_reset_sessions_revoked_total`
- `auth_change_password_sessions_revoked_total`
- `auth_flow_inconsistency_total` (временная диагностическая метрика в период миграции)
- `auth_login_inactive_user_attempt_total`

- Примечание: `auth_password_reset_sessions_revoked_total` и `auth_change_password_sessions_revoked_total` считаются по количеству фактически отозванных сессий (`rows_affected`), а не по числу вызовов API.

Оповещения:

- рост ошибок reset/confirm выше baseline;
- аномалии в login success/error ratio после cutover;
- рост инцидентов с «валидный токен после reset».

---

## 8. Release gates и stop-the-line

Gate перед выкладкой:

- [ ] Integration tests из раздела 6 проходят.
- [ ] REST/GraphQL parity проверена на staging.
- [ ] Security review по reset/change-password закрыт.

Артефакты gate-проверки (обновлять при каждом pre-release прогоне):

- Integration report: `apps/server` (`cargo test -p rustok-server auth_lifecycle` + auth integration suite).
- Staging parity report: сравнение REST/GraphQL результатов для `create_user`, `confirm_reset`/`reset_password`, `change_password`.
- Security review checklist: подтверждение инварианта «password reset => revoke all active sessions» и отсутствие bypass по inactive user.

Шаблон фиксации результатов gate (заполняется перед релизом):

| Гейт | Артефакт | Статус | Подтверждение | Ответственный | Дата |
| --- | --- | --- | --- | --- | --- |
| Integration | `cargo test -p rustok-server auth_lifecycle` + auth integration suite | In progress | Локально пройдено: `cargo test -p rustok-server auth_lifecycle` (2026-03-04); CI/staging report pending | Platform foundation | 2026-03-04 |
| REST/GraphQL parity | staging report (`create_user`, `confirm_reset`/`reset_password`, `change_password`) | Pending | Ссылка на parity report | Platform foundation | YYYY-MM-DD |
| Security review | checklist по reset/change-password + inactive-user bypass | Pending | Ссылка на checklist/sign-off | Platform foundation + security reviewer | YYYY-MM-DD |

Порядок заполнения gate-артефактов:

1. Перед релизом обновить таблицу фактическими статусами и ссылками на evidence.
2. Для каждого gate зафиксировать owner и дату завершения проверки.
3. Если хотя бы один gate не закрыт, релиз блокируется до устранения причины.

Текущий прогресс по gate:

- Кодовые и документационные задачи Phases A-C завершены (см. раздел 5).
- Rollout controls и rollback-инструкция добавлены в `apps/server/docs/README.md`.
- Метрики rollout-периода из раздела 7 (`auth_password_reset_sessions_revoked_total`, `auth_change_password_sessions_revoked_total`, `auth_flow_inconsistency_total`, `auth_login_inactive_user_attempt_total`) публикуются через `/metrics`.
- Локальный integration прогон выполнен: `cargo test -p rustok-server auth_lifecycle` проходит (19/19), что подтверждает текущий минимум по unit/integration инвариантам в окружении разработки.
- Unit coverage для инварианта из раздела 6 (повторный reset на уже отозванных сессиях) расширено: повторный вызов не добавляет новых revoked session.
- Добавлены unit-checks для transport error contracts `UserInactive` и `InvalidResetToken` (единый mapping в unauthorized), чтобы удерживать parity REST/GraphQL по негативным auth-сценариям.
- Добавлены service-level тесты для негативных auth-сценариев: `login` для inactive user инкрементирует `auth_login_inactive_user_attempt_total`, `confirm_password_reset` отвергает невалидный token payload как `InvalidResetToken`.
- Добавлен service-level тест на стабильный error contract для duplicate email: повторный `create_user` в рамках tenant возвращает `EmailAlreadyExists` (без transport-зависимой вариативности).
- Добавлен service-level тест на tenant-scoped uniqueness contract: одинаковый email может существовать в разных tenant, что фиксирует expected behavior для multi-tenant entrypoints и предотвращает ложные cross-tenant конфликты при `create_user`.
- Добавлен service-level parity-тест users-permissions для одной роли (`Manager`) между двумя путями создания пользователя (через `AuthLifecycleService::create_user` и legacy insert + `AuthService::replace_user_role`): итоговый набор разрешений совпадает.
- Добавлен service-level тест tenant/user scoping для revoke сессий: `revoke_user_sessions` отзывает только сессии целевого `tenant+user` и не затрагивает соседний tenant.
- Расширены transport-level проверки GraphQL mapping для негативных auth-кейсов: `UserInactive` и `InvalidResetToken` стабильно маппятся в согласованные сообщения ошибок.
- `GraphQL create_user` переведён на общий `AuthLifecycleService::create_user` (единая транзакция создания пользователя + назначения RBAC-связей, без отдельной бизнес-ветки в transport-слое).
- `REST invite/accept` переведён на общий `AuthLifecycleService::create_user`, чтобы убрать последний transport-level bypass для user-creation flow и удерживать единый контракт назначения RBAC-связей.
- Для `create_user` добавлена защита от race-condition по `tenant+email`: конкурентный insert теперь нормализуется в `EmailAlreadyExists`, чтобы REST/GraphQL сохраняли стабильный transport-контракт на дубликат email.
- Phase D остаётся открытой до фиксации результатов integration/staging/security проверок.

Открытый operational tail для продолжения реализации:

1. Поднять/обновить auth integration suite до уровня release-gate evidence (не только выборка `auth_lifecycle`, но и полный прогон auth-сценариев из раздела 6).
2. Зафиксировать staging parity report по REST/GraphQL для `create_user`, `confirm_reset`/`reset_password`, `change_password`.
3. Закрыть security checklist/sign-off и проставить финальные статусы в таблице gate.

Stop-the-line условия:

1. Найден сценарий, где reset не отзывает сессии.
2. Найдено расхождение behavior между REST и GraphQL для одного и того же use-case.
3. Рост 401/403/5xx по auth-эндпоинтам выше согласованного порога после релиза.

Rollback-процедура (операционная):

1. Немедленно приостановить rollout auth-изменений и вернуть предыдущий release-artifact.
2. Принудительно отозвать скомпрометированные/сомнительные сессии через soft-revoke `sessions.revoked_at`.
3. Включить усиленный мониторинг метрик из раздела 7 и сверить parity REST/GraphQL на staging.
4. После стабилизации открыть postmortem и обновить этот план с конкретной причиной отката.

---

## 9. Зависимости и связь с RBAC-планом

- Этот план должен быть реализован **до** финального RBAC cutover или синхронно с его ранними фазами.
- Ссылка на RBAC-план: `docs/architecture/rbac-relation-migration-plan.md`.
- После выполнения этого плана проще и безопаснее выполнять relation-RBAC migration (меньше дублирования и расхождений).

Синхронизация статуса с RBAC-планом:

- Для user/auth parity, session invalidation policy и rollout gate-артефактов source-of-truth остаётся этот документ.
- В RBAC-плане (`rbac-relation-migration-plan.md`) эти пункты учитываются как зависимость readiness, а не как отдельный параллельный backlog.
- Для операций назначения/замены role-relations используется выделенный модуль `rustok-rbac` (`crates/rustok-rbac`); auth lifecycle слой должен опираться на его контракты, а не дублировать RBAC-логику локально.

---

## 10. Definition of Done

- User/auth бизнес-логика централизована в application service.
- REST и GraphQL не расходятся по ключевым auth сценариям.
- Session invalidation policy едина и проверена тестами.
- Документация и ADR отражают фактическое поведение системы.
