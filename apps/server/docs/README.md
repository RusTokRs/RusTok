# server docs

В этой папке хранится документация модуля `apps/server`.

## Документы

## Mandatory platform baseline

Для `apps/server` обязательный baseline нужно читать в двух слоях.

Platform `Core` modules:
- `rustok-auth`
- `rustok-cache`
- `rustok-channel`
- `rustok-email`
- `rustok-index`
- `rustok-outbox`
- `rustok-tenant`
- `rustok-rbac`

Shared foundation / support crates:
- `rustok-core`
- `rustok-events`
- `rustok-telemetry`
- `rustok-api`

Агентам: считать этот baseline обязательным. Логика tenant-toggle/disable относится только к `Optional` modules; `Core` modules не должны трактоваться как отключаемые.

Дополнительно для baseline:
- `rustok-channel` уже участвует в request pipeline через channel resolution middleware и общий `ChannelContext` в `rustok-api`;
- channel resolution middleware следует explicit policy order `header -> query -> host -> default`, где `default` теперь означает explicit default channel tenant'а, и прокидывает `resolution_source` в shared request context;
- server держит только thin transport/wiring для channel management: middleware и REST surface `/api/channels/*`, а domain/service/storage остаются в модуле.

- [`library-stack.md`](./library-stack.md) — основные backend-библиотеки сервера и их роль (framework, HTTP, ORM, GraphQL, runtime, observability).
- [`health.md`](./health.md) — health/readiness probes и текущие dependency checks сервера.
- [`event-transport.md`](./event-transport.md) — как работает конфигурация и runtime-пайплайн транспорта событий.
- [`event-flow-contract.md`](../../../docs/architecture/event-flow-contract.md) — канонический контракт полного event-пути (publish → outbox → delivery → consumer/read-model).
- [`loco/README.md`](./loco/README.md) — Loco-specific контекст, workflow для агентов и freshness-политика upstream snapshot.
- [`LOCO_FEATURE_SUPPORT.md`](./LOCO_FEATURE_SUPPORT.md) — decision matrix по Loco-функционалу vs самопису (anti-duplication baseline), включая статус Mailer/Workers/Storage и текущее состояние кэширования.
- [`loco-core-integration-plan.md`](./loco-core-integration-plan.md) — текущий status doc по server/Loco integration, live capabilities и residual scope.
- [`CORE_VERIFICATION_PLAN.md`](./CORE_VERIFICATION_PLAN.md) — периодический чеклист верификации ядра (13 секций, grep-проверки, таблица антипаттернов).
- Framework deviation checklist: для каждого нового отклонения от framework/runtime baseline обязателен checklist из [`docs/standards/forbidden-actions.md`](../../../docs/standards/forbidden-actions.md#64-framework-deviation-checklist-обязателен-для-каждого-нового-отклонения) (benchmark evidence, failure-mode table, rollback strategy, owner sign-off).
- [`upstream-libraries/README.md`](./upstream-libraries/README.md) — локальный snapshot актуальной внешней документации по ключевым crate сервера.
- Cleanup/maintenance: background cleanup task supports `sessions`, `rbac-report`, `rbac-backfill`, `rbac-backfill-rollback` targets (`cargo loco task --name cleanup --args "target=<target>"`); RBAC backfill supports safety controls `dry_run=true`, `limit=<N>`, `continue_on_error=true`, `exclude_user_ids=<uuid,...>`, `exclude_roles=<role,...>` and optional rollback snapshot output `rollback_file=<path>`; rollback target consumes `source=<rollback_file>` and supports `dry_run=true`; rollback now removes only role assignments captured in snapshot (role-targeted revert); app `truncate` hook now performs ordered deletion of server foundation tables (`release`, `build`, `tenant_modules`, `sessions`, `users`, `tenants`).
- Profiles backfill: `cargo loco task --name profiles_backfill --args "tenant_id=<uuid> [dry_run=true] [limit=500] [visibility=authenticated] [emit_events=true]"` создаёт missing profiles для существующих пользователей через `rustok-profiles::ProfileService`, использует customer/user seed-данные для display name/locale и может отдельно публиковать `profile.updated` при rollout downstream consumers.
- RBAC rollout migration helpers удалены из live repo после финального перехода на single-engine `casbin_only`; детали cutover при необходимости фиксируются только в ADR/architecture docs и не являются частью текущего authorization path.
- Rich-text migration job (blog/forum legacy markdown → `rt_json_v1`): `cargo run -p rustok-server --bin migrate_legacy_richtext -- --tenant-id=<uuid> [--dry-run] [--checkpoint-file=...]`; job выполняет конвертацию + server-side sanitize/validation + safe update с retry, выводит метрики `processed/succeeded/failed/skipped`, поддерживает идемпотентный restart через checkpoint и предназначен для tenant-by-tenant rollout (с backup-based rollback на tenant scope).
- Auth/JWT algorithm: `rustok-auth` поддерживает `HS256` (default, симметричный shared secret) и `RS256` (асимметричный, RSA PEM ключи). Алгоритм задаётся через `AuthConfig::algorithm: JwtAlgorithm`; для RS256 используется `AuthConfig::with_rs256(private_pem, public_pem)`. Текущий server bridge (`auth_config_from_ctx`) использует HS256 и читает ключ из `auth.jwt.secret` в loco YAML.
- Cache/Redis URL: Redis URL теперь можно задать в YAML через `settings.rustok.cache.redis_url` — имеет приоритет над env vars `RUSTOK_REDIS_URL` / `REDIS_URL`. `CacheService::from_url(Option<&str>)` — новый фабричный метод.
- Email/transactional: добавлен `TransactionalEmailSender` trait для отправки любых transactional email по template ID (`"{module}/{action}"`). Модули реализуют `EmailTemplateProvider` и регистрируются через `SmtpEmailSender::with_provider()`.
- Channels/WebSocket: добавлен `RustokChannel` trait в `apps/server/src/channels/mod.rs` — единый контракт для WebSocket каналов (path + handle). Не зависит от `loco_rs::controller::channels`; использует собственный auth-handshake.
- Auth/password reset: GraphQL `forgot_password` dispatches reset emails via `tokio::spawn` (non-blocking) — SMTP handshake no longer holds the HTTP request. Delivery errors are logged via `tracing::warn` and do not affect the response. Email transport uses `rustok.email` settings; safe no-send fallback when email delivery is disabled.
- Auth/password reset: REST `POST /api/auth/reset/confirm` теперь отзывает все активные сессии пользователя (через `revoked_at`), что выравнивает policy с GraphQL reset-путём.
- Auth/user lifecycle: GraphQL `create_user` теперь в одной транзакции создаёт пользователя и назначает RBAC связи (`user_roles`/permissions) через `RbacService::assign_role_permissions`.
- Auth/RBAC resolver: `RbacService` использует модульный `rustok-rbac::RuntimePermissionResolver`; в server остаются adapter-реализации `RelationPermissionStore`/`PermissionCache`/`RoleAssignmentStore` (SeaORM + Moka + service wiring), а публичные write-path операции (`assign_role_permissions`/`replace_user_role`) также идут через модульный resolver API.
- Auth/RBAC resolver: `RbacService::get_user_permissions` использует in-memory cache (TTL 60s) с structured cache hit/miss логами и инвалидацией при изменении relation-ролей пользователя.
- Auth/RBAC consistency: REST RBAC extractors (`extractors/rbac.rs`) используют общую wildcard-семантику `resource:manage` через `rustok_rbac::has_effective_permission_in_set`, без локального дублирования permission-логики.
- Auth/RBAC observability: `/metrics` публикует `rustok_rbac_permission_cache_hits`, `rustok_rbac_permission_cache_misses`, `rustok_rbac_permission_checks_allowed`, `rustok_rbac_permission_checks_denied`, `rustok_rbac_claim_role_mismatch_total`, `rustok_rbac_engine_decisions_casbin_total`, `rustok_rbac_engine_eval_duration_ms_total`, `rustok_rbac_engine_eval_duration_samples`, а также consistency-gauges `rustok_rbac_users_without_roles_total`, `rustok_rbac_orphan_user_roles_total`, `rustok_rbac_orphan_role_permissions_total`, `rustok_rbac_consistency_query_failures_total`, `rustok_rbac_consistency_query_latency_ms_total`, `rustok_rbac_consistency_query_latency_samples`.
- Dev onboarding: `seed_development` creates/updates an idempotent demo tenant (`demo`), demo users, and enables core modules for local environments.
- Admin UI embedding: embedded `/admin` assets are compiled only with Cargo feature `embed-admin-assets` (disabled by default for CI/check environments without frontend artifacts). When enabled, build `apps/admin/dist` before compiling `apps/server`; when disabled, `/admin/*` returns `503 Service Unavailable` with an explicit message that embedding is disabled.
- Ecommerce transport: live REST surface для commerce теперь ограничен `/store/*` и `/admin/*`; compatibility-слой `/api/commerce/*` удалён из runtime router и OpenAPI.
- Ecommerce verification: для live-контракта используются обычные contract/smoke tests по `/store/*`, `/admin/*`, GraphQL parity и schema smoke, без отдельного rollout/deprecation слоя.
- Build pipeline: `BuildService::request_build` now publishes `BuildRequested` via configurable `BuildEventPublisher`; `EventBusBuildEventPublisher` maps it to `DomainEvent::BuildRequested`, while default noop publisher logs skipped dispatch when no runtime wiring is provided.
- Build pipeline: `BuildExecutor` now runs the full manifest-derived plan, not only the server cargo build: `apps/admin` is built with `trunk build`, `apps/storefront` as `cargo build -p rustok-storefront`, and the serialized execution plan is reused by release publishing.
- Server composition root: `apps/server/build.rs` now generates optional module registry wiring, GraphQL schema fragments and HTTP route registration from `modules.toml` into `OUT_DIR`; `apps/server` stays a generic host, while module-specific transport remains inside module crates. Server entry points can now be declared explicitly via `rustok-module.toml` (`[crate]`, `[provides.graphql]`, `[provides.http]`) or, for external/non-path crates, as normalized Rust paths in `modules.toml`; old naming conventions remain only as fallback.
- Release pipeline: `settings.rustok.build.deployment.backend` теперь поддерживает `record_only`, `filesystem`, `http` и `container`; filesystem/container backend публикуют реальные `server_artifact_url`, `admin_artifact_url`, `storefront_artifact_url`, а container backend дополнительно собирает runtime image из release bundle, публикует `container_image` и может вызывать generic `rollout_command` hook без provider-specific логики в `apps/server`.
- Event consumer runtime: long-lived server consumers (`server_event_forwarder`, GraphQL build-progress subscription) follow a shared contract from `rustok-core`: `Lagged -> warn + metric`, `Closed -> explicit stop`, reindex decision path documented in `crates/rustok-outbox/docs/README.md`.
- GraphQL subscriptions: server now exposes `/api/graphql/ws` for subscription transport; browser clients pass `token`, `tenantSlug` and `locale` via `connection_init`, and tenant resolution for this route happens inside the GraphQL websocket handshake instead of the regular header-based tenant middleware.
- Search GraphQL hardening: public `storefrontSearch` and `storefrontSearchSuggestions` now run behind a dedicated shared rate-limiter namespace, while admin `updateSearchSettings` and `triggerSearchRebuild` emit best-effort audit events through the outbox without turning the mutation path into a hard dependency on the event transport.
- GraphQL schema hygiene: module-owned GraphQL types must be namespace-safe in the shared schema; if a module exposes a domain-specific enum/object, its GraphQL-visible name must not collide with types from other modules.
- GraphQL/DataLoader: shared schema now registers `rustok-profiles::ProfileSummaryLoader` alongside tenant/content loaders, so module-owned resolvers (`profiles`, `blog`, `forum`) can use request-scoped batching/cache for author/member summaries without server-specific SQL joins.
- Tenant cache invalidation: Redis pubsub listener now uses supervised resubscribe with fixed backoff instead of one-shot startup; operational signals go through `rustok_event_consumer_restarted_total` and incident handling stays in `crates/rustok-outbox/docs/README.md`.
- Health/readiness: `tenant_cache_invalidation` is now exposed in `/health/ready`, and current listener state is exported as `rustok_tenant_invalidation_listener_status`; see [`health.md`](./health.md).
- Auth/session lifecycle: GraphQL `sign_out`, `change_password`, `reset_password` теперь используют soft-revoke через `sessions.revoked_at` (вместо hard delete) и выровнены по поведению с REST (`sign_out` отзывает только текущую сессию, `change_password` — все остальные, `reset_password` — все активные).

- Auth/lifecycle extraction: REST handlers и GraphQL mutations для `register/sign_in`, `login/sign_in`, `refresh`, `change_password`, `reset_password`, `update_profile` теперь маршрутизируют бизнес-логику через общий `AuthLifecycleService` (transport adapters остаются тонкими).

- Auth/observability: `/metrics` публикует auth lifecycle counters `auth_password_reset_sessions_revoked_total`, `auth_change_password_sessions_revoked_total`, `auth_flow_inconsistency_total`, `auth_login_inactive_user_attempt_total`; первые два счётчика отражают количество реально отозванных сессий (rows affected), а счётчики ведутся в `AuthLifecycleService` для rollout-периода remediation-плана.

- Auth/error contracts: `AuthLifecycleService` использует типизированные ошибки (`AuthLifecycleError`), а REST/GraphQL делают единообразный transport-specific mapping без дублирования строковых веток.

- Auth rollout controls: канонические release gates, stop-the-line условия и rollback-процедура ведутся централизованно в `docs/architecture/api.md` (раздел «Auth lifecycle consistency и release-gate»); remediation backlog закрыт, в релизах используется operational handoff через `scripts/auth_release_gate.sh --require-all-gates`.
- Auth rollout controls: helper `scripts/auth_release_gate.sh` автоматизирует сбор локального integration evidence (`cargo test -p rustok-server auth_lifecycle` + `cargo test -p rustok-server auth`), всегда формирует markdown gate-report с полями для parity/security evidence и завершает прогон с non-zero exit code при падении любого локального integration auth-среза.
- RBAC/seed consistency: `seed_user` теперь вызывает `RbacService::assign_role_permissions` после создания пользователя, гарантируя наличие `user_roles` для всех seed-пользователей (dev bootstrap).
- MCP management layer: `apps/server` теперь хранит persisted `mcp_clients`, `mcp_tokens`, `mcp_policies`, `mcp_audit_logs` и публикует management API через REST `/api/mcp/*` и GraphQL `mcp*`; это platform capability, а не tenant module.
- MCP runtime bridge: `DbBackedMcpRuntimeBridge` регистрируется в server bootstrap, резолвит plaintext MCP token в `McpAccessContext` на старте stdio-сессии, обновляет `last_used_at` для client/token, пишет runtime allow/deny audit в `mcp_audit_logs` и может работать persisted draft-store backend для Alloy scaffold MCP tools.
- Alloy scaffold draft control plane: `apps/server` теперь также хранит persisted scaffold drafts для Alloy module generation и публикует их через REST `/api/mcp/scaffold-drafts*` и GraphQL `mcpModuleScaffoldDraft*`.


## Current RBAC contract

- `apps/server` is the only transport/runtime layer that enforces RBAC.
- Runtime modules publish their permission surface through `RusToKModule::permissions()`.
- Runtime module interaction contracts live in `crates/<module>/README.md` under `## Interactions`.
- Server adapters must use `RbacService`, RBAC extractors, or permission-aware `SecurityContext`.
- Manual role-based authorization is not part of the live server contract.
