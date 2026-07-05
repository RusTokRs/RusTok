# Документация `apps/server`

Локальная документация для главного backend host-приложения RusToK. Этот файл фиксирует только живой composition/runtime contract; детальные runbook, framework notes и rollout-планы вынесены в профильные документы внутри этой папки и в central docs.

## Назначение

`apps/server` является главным backend composition root. Приложение:

- собирает platform modules, shared foundation crates и host-level capabilities в единый runtime;
- публикует HTTP, GraphQL, Leptos `#[server]`, health, metrics и related control-plane surfaces;
- остаётся thin transport/wiring слоем там, где доменная логика уже вынесена в модульные crates.

## Обязательный platform baseline

Для `apps/server` обязательный baseline состоит из двух слоёв.

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

Логика tenant-toggle относится только к `Optional` modules. `Core` modules не должны трактоваться как отключаемые host-конфигурацией.

## Runtime surface

- `/api/graphql` и `/api/fn/*` являются параллельными transport-слоями; Leptos server functions не заменяют GraphQL API.
- `/api/graphql/schema.graphql` публикует собранную SDL-схему без tenant context для contract tooling; full introspection экспортируется POST-запросом к `/api/graphql`. Оба snapshot входят в reference artifacts вместе с OpenAPI JSON/YAML.
- Embedded UI больше не считается безусловной частью backend binary: `rustok-admin` и `rustok-storefront` линкуются только при compile-time feature-флагах `embed-admin` / `embed-storefront`, а не просто по факту наличия кода в workspace.
- Commerce OpenAPI/REST surface на `/admin/*` теперь включает первый post-order refund contract поверх `payment-collections`; host публикует эти routes, но refund lifecycle остаётся domain-owned в `rustok-payment` и `rustok-commerce`.
- Commerce surface больше не является compile-time baseline для любого server build: `controllers::commerce`, commerce-specific error mapping и commerce fragment в OpenAPI живут только при `mod-commerce`, так что reduced/headless host может собираться без ecommerce transport слоя.
- Content REST/OpenAPI surface для `blog`, `forum` и `pages` тоже больше не считается unconditional частью host binary: соответствующие server controllers и OpenAPI fragments подключаются только при `mod-blog`, `mod-forum` и `mod-pages`, так что module-sliced build не обязан тянуть чужие content transport-зависимости.
- Maintenance binary `migrate_legacy_richtext` принадлежит content storage migration path и собирается только при `mod-content`; headless server profiles без content module не должны линковать этот инструмент.
- `flex` attached field-definition и standalone schemas/entries GraphQL публикуются через `/api/graphql`, а standalone REST остаётся на `/api/v1/flex/schemas*`; это live tenant-scoped surface с отдельными `flex_schemas:*` и `flex_entries:*` permission gates. GraphQL query/mutation roots, runtime handle и DTO принадлежат `flex::graphql`; roots входят в schema через `[provides.graphql]` manifest codegen, а server builder регистрирует только `FlexGraphqlRuntime` поверх concrete `FlexStandaloneSeaOrmService`, `FieldDefRegistry`, DB handle и cache adapter. REST request/response DTO, command mapping и view mapping принадлежат `flex::rest`; attached field-definition row-to-core/view/command mapping, create guardrails, persisted type-name normalization и lifecycle events принадлежат `flex::registry`; server остаётся Loco/Axum/SeaORM adapter.
- Health/observability surface публикуется через `/health*` и `/metrics`.
- Module/runtime wiring опирается на `modules.toml`, `rustok-module.toml` и generated host integration.
- Optional module REST/GraphQL surfaces монтируются только из owner-owned crate entrypoints,
  объявленных в `rustok-module.toml` (`provides.http`, `provides.graphql`) и `modules.toml`.
  OpenAPI fragments для optional modules также живут в owner crates и merge-ятся сервером
  как готовые documents, без перечисления module-owned handlers/DTO в `apps/server`.
  `apps/server/src/controllers/<module>` и `apps/server/src/graphql/<module>` не являются
  valid composition points для optional modules; source guard `module_surface_boundary_guard`
  блокирует возврат server-owned shims.
- Shared content canonical query и cross-module conversion mutations также приходят готовыми
  GraphQL roots из `rustok-content` и `rustok-content-orchestration`; host не владеет их resolver/DTO.
- `rustok-content-orchestration` регистрируется host-ом как `SharedContentOrchestrationService`,
  построенный из явных DB и `TransactionalEventBus`; conversion GraphQL resolvers читают этот handle из schema data,
  а не через Loco `AppContext`.
- Auth lifecycle и OAuth GraphQL query/mutation/types принадлежат `rustok-auth`; server реализует только `AuthLifecyclePort`/`OAuthAdminPort` поверх persisted lifecycle/OAuth/email services и регистрирует соответствующие runtimes в shared runtime extensions. `AuthLifecycleService` принимает только `ServerRuntimeContext` и явный `AuthConfig`, без Loco compatibility entrypoints. `ServerAuthLifecycleProvider` получает явные `ServerRuntimeContext`, `AuthConfig` и mailer handle; `CurrentUser`/`OptionalCurrentUser`, `auth_context` middleware и RBAC permission extractors используют узкий `ServerAuthRuntime`; полный Loco `AppContext` остаётся только в bootstrap/REST boundary adapters, которые собирают эти зависимости.
- AI GraphQL/service/direct execution получает `rustok_ai::AiHostRuntime` как schema-owned data: host передаёт явные DB, transactional event bus, module registry, storage и Alloy runtime handles. `rustok-ai` не читает Loco `AppContext`; Leptos admin adapter пока остаётся host-boundary точкой, которая собирает этот runtime из текущего app context.
- MCP GraphQL query/mutation/types принадлежат `rustok-mcp`; server реализует `McpManagementPort` поверх persisted `McpManagementService` и регистрирует `McpManagementRuntime`.
- Content GraphQL dataloaders для `nodes`, `node_translations` и `bodies` живут в
  `rustok-content`; `apps/server` только регистрирует owner-owned loader types в schema builder.
- System GraphQL может публиковать media usage, но читает его через `rustok-media::load_media_usage_snapshot`,
  без прямого импорта module-owned media entities.
- Settings и system GraphQL resolvers получают `ServerRuntimeContext`, DB и schema-owned
  `TransactionalEventBus` через GraphQL data. Они не извлекают и не адаптируют Loco `AppContext`;
  request/connection boundary пока продолжает передавать его только для ещё не перенесённых resolvers.
- App runtime rate-limit bootstrap и shared limiter registration используют `ServerRuntimeContext`;
  Alloy runtime bootstrap также регистрирует `SharedAlloyRuntime` через `ServerRuntimeContext` из явного DB handle,
  а Alloy GraphQL получает этот runtime как schema-owned data без Loco `AppContext`.
  Loco `AppContext` внутри bootstrap остаётся только для текущих boundary adapters вроде mailer.
- User complex fields и build progress subscription используют schema-owned `DatabaseConnection`
  напрямую и также не зависят от Loco host context.
- Host-owned `RootQuery` также не извлекает Loco `AppContext`: DB-only read paths используют
  schema-owned `DatabaseConnection`, а marketplace/cache paths — `ServerRuntimeContext`.
- Весь `apps/server/src/graphql/**`, включая `RootMutation`, RBAC writer и search rate limiter,
  не импортирует Loco. `services/graphql_schema.rs` также принимает только `ServerRuntimeContext`;
  текущий app bootstrap/controller boundary отвечает за однократную адаптацию host context.
- Server event runtime строит обычный и transactional event bus из `ServerRuntimeContext` и больше
  не реэкспортирует `rustok_outbox::loco`; crate-local Loco adapter остаётся до удаления dependency feature.
- GraphQL HTTP и WebSocket handlers извлекают `ServerRuntimeContext`/`ServerAuthRuntime` как Axum
  substate и не передают Loco `AppContext` в request/connection data. `loco_rs::controller::Routes`
  остаётся только временным routing adapter до Phase 2.
- Users REST handlers также извлекают `ServerRuntimeContext`; Loco response/error helpers пока
  сохраняются как отдельная часть Phase 2, без смешивания state migration и response cutover.
- Metrics handler и весь metrics helper pipeline используют `ServerRuntimeContext`; состояние
  mailer передаётся отдельным `ServerEmailRuntime`, а worker handles читаются через scoped shared API.
- Health readiness/runtime handlers используют те же runtime contracts для DB, settings, cache,
  event transport, rate limits, worker lifecycle и email backend state; Loco response formatter
  остаётся только до общего Phase 2 error/response cutover.
- Channel и standalone Flex REST handlers получают `ServerRuntimeContext`; Flex controller tests
  собирают тот же neutral runtime fixture и не создают Loco `AppContext`.
- Auth REST handlers извлекают узкий `ServerAuthRuntime`; password reset и verification endpoints
  дополнительно получают `ServerEmailRuntime`. Controller не читает Loco config, DB или mailer напрямую.
- Module guard и server channel contract типизированы через `ServerRuntimeContext`; Loco `AppContext`
  не является request/channel contract для server-owned runtime paths.
- OAuth discovery metadata также использует `ServerAuthRuntime` как единственный источник auth config.
- OAuth REST token, authorize/consent, browser-session и revoke handlers извлекают `ServerAuthRuntime`
  или `ServerRuntimeContext`; Loco `AppContext` больше не участвует в OAuth request state.
- Marketplace registry/governance REST handlers извлекают `ServerRuntimeContext`; catalog projection,
  artifact storage и remote executor policy читаются через DB/settings/shared handles neutral runtime.
- Swagger document filtering, installer persistence reads, admin DLQ, MCP management/remote tools
  и build WebSocket извлекают `ServerRuntimeContext`; DB/shared runtime semantics не зависят от Loco state.
- Channel runtime surface остаётся thin transport around `rustok-channel`: `/api/channels/*` уже покрывает bootstrap, channel CRUD-lite, policy-set/rule authoring endpoints и request-level `resolution_trace` diagnostics, а сам resolution pipeline живёт в модуле. Request-level `tenant`, `channel` и `locale` middleware получают `ServerRuntimeContext`, auth context получает `ServerAuthRuntime`; Loco `AppContext` остаётся только в router/controller boundary adapters для текущего host.
- Module-owned event listeners собираются из `ModuleRegistry` в общий `EventDispatcher`; `apps/server` больше не держит отдельные host-owned index/search/workflow listener paths.
- Server migrator является backend composition root для module-owned schema: content-family модули (`blog`, `pages`, `comments`) и search обязаны подключаться здесь через `crates/rustok-*/src/migrations`, иначе внешние Next/Leptos admin surfaces получают рабочий route shell без нужных таблиц.
- `apps/server` может работать как `full` host или как `registry_only`, но `host_mode` не заменяет deployment profile и не меняет build/deploy semantics.
- `settings.rustok.runtime.background_workers` управляет только maintenance workers поверх уже опубликованной HTTP/GraphQL surface. В `development.yaml` для standalone admin debug выключены `workflow_cron_enabled` и `seo_bulk_enabled`, чтобы cron/bulk loops не забивали локальный PostgreSQL pool; production/default runtime оставляет их включёнными.
- `development.yaml` держит `database.max_connections: 30`, потому что тяжёлые admin bootstrap routes вроде AI control plane резолвят несколько GraphQL root fields параллельно. Это локальный debug guardrail для обеих админок, а не новый production contract.
- Для registry/governance surfaces именно сервер остаётся каноническим валидатором lifecycle policy, `reason` / `reason_code` contract и allowed action set; thin clients могут делать preflight, но не определяют policy локально.
- Для control-plane composition install/uninstall/upgrade server использует единый orchestration path: manifest validation, CAS-update `platform_state` и enqueue build выполняются атомарно в одном transaction boundary. `manifest_ref` для build всегда формируется как `platform_state:<revision>`, а `manifest_hash` считается как SHA-256 canonical JSON snapshot.
- Tenant module enable/disable идёт через canonical lifecycle entrypoint `ModuleLifecycleService::toggle_module_with_actor()`; bypass model-level toggle не считается production contract. `module_operations` фиксирует lifecycle status в typed модели `validated/running/committed/failed`, а pre-validation ошибки/no-op переходы не должны создавать лишние journal rows. GraphQL mapper остаётся владельцем lifecycle taxonomy (`BAD_USER_INPUT`, `MODULE_HOOK_FAILED`, `INTERNAL_ERROR`) и journal/recovery metadata; admin/SSR clients не должны remap'ить эти поля.
- Для post-hook failure recovery/compensation используется отдельный runbook `module-lifecycle-retry-compensation-runbook.md`; committed tenant state не откатывается автоматически, а retry/compensation выполняются как отдельные lifecycle операции через canonical entrypoint.
- Registry metadata теперь следует общему multilingual storage contract: publish/release base rows держат language-agnostic state и `default_locale`, а display metadata (`name`, `description`) живут в `registry_*_translations`.
- Registry audit payload больше не держит historical runtime fallback: `registry_governance_events.details` нормализован на typed shape (`stage_key`, nested `owner_transition`, structured principal objects), а controller маппит lifecycle failures от typed `RegistryGovernanceError`, а не от substring matching.
- `GET /v2/catalog/publish/{request_id}` остаётся machine-readable operator status contract: без bearer auth он возвращает status-driven superset `governanceActions`, а при session-backed user bearer режет request-level действия до реально разрешённых для этого principal.
- Registry artifacts больше не читаются и не записываются через прямой filesystem path внутри governance service: persisted state хранит только `artifact_storage_key`, upload/validation идут через `StorageService`, а `GET /v2/catalog/publish/{request_id}/artifact/download` уже работает как storage-backed private download route с presign-or-stream fallback.
- Repo-side surface для текущего `module-system` считается закрытым для цели Admin-driven install/uninstall/upgrade/deploy с progress feedback; дальше остаётся поддерживать targeted verification и docs/audit, а rollout `modules.rustok.dev` остаётся внешней infra-задачей.
- GraphQL control-plane surface публикует read/write contract для lifecycle recovery: `moduleOperationRecoveryPlan` и `failedModuleOperationRecoveryPlans` отдают tenant-scoped retryability/action metadata из `module_operations`, а `retryFailedModuleOperationPostHook` / `compensateFailedModuleOperation` выполняют recovery только через `ModuleLifecycleService` и `modules:manage`, без raw SQL/bypass rollback.
- GraphQL auth surface `me.permissions` отдаёт request-scoped RBAC snapshot для headless/mobile UI gating; это не заменяет server-side permission enforcement на mutations/queries.
- MCP remote bootstrap surface `POST /api/mcp/runtime/bootstrap` выполняет server-owned token-to-runtime-binding handshake для non-stdio transport: принимает Bearer/plaintext MCP token, возвращает tenant/client/token binding и effective access context, обновляет last-used timestamps и пишет audit event `remote_session_bootstrapped` с correlation id. Remote tool transport дополняют `POST /api/mcp/runtime/tools/call` для JSON-вызовов и `POST /api/mcp/runtime/tools/stream` для SSE-вызовов core registry tools (`mcp_health`, `mcp_whoami`, `list_modules`, `query_modules`, `module_exists`, `module_details`) и Alloy scaffold draft tools (`alloy_scaffold_module`, `alloy_review_module_scaffold`, `alloy_apply_module_scaffold`) с тем же persisted token binding, policy enforcement и audit trail. Scaffold tools в remote transport используют server-owned persisted draft store, поэтому stage/review/apply проходят через `mcp_scaffold_drafts`, tenant/client binding и audit surface, а не через process-local память MCP runtime.
- Гибридный product installer вводится через support crate `rustok-installer`:
  CLI `rustok-server install ...` и `/api/install/*` endpoints должны
  делегировать plan/state/receipt/preflight semantics в этот crate. Web wizard
  не должен становиться отдельной реализацией bootstrap logic.
- Текущий начальный CLI surface уже доступен как offline pre-apply слой:
  `rustok-server install preflight ...` валидирует install plan и возвращает
  JSON report, а `rustok-server install plan ...` печатает redacted plan snapshot
  без подключения к БД и без запуска миграций.
- `rustok-server install apply ...` выполняет текущий CLI bootstrap end-to-end:
  preflight, при `--create-database` может создать PostgreSQL database/role
  через `--pg-admin-url`, проверяет target DB через `SELECT 1`, запускает server
  `Migrator::up`, создаёт `install_sessions`, ставит session lock, выполняет
  tenant/module seed, создаёт или синхронизирует superadmin, проверяет результат,
  пишет `Preflight` / `Config` / `Database` / `Migrate` / `Seed` / `Admin` /
  `Verify` / `Finalize` receipts и переводит session в `completed`.
  `apply` резолвит локальные secret refs `env:<VAR>`, `file:<path>`,
  `mounted-file:<path>`, `dotenv:<path>#<VAR>` и `dotenv:<VAR>`; external
  backends вроде `vault:*`, `kubernetes:*` и cloud secret managers пока
  принимаются только как contract-level refs для `plan`/`preflight` и fail-fast
  на `apply` до подключения внешнего resolver-а.
- HTTP adapter для Leptos wizard доступен как thin surface поверх того же
  pipeline: `GET /api/install/status`, `POST /api/install/plan`,
  `POST /api/install/preflight`, `POST /api/install/apply`,
  `GET /api/install/jobs/{job_id}` и
  `GET /api/install/sessions/{session_id}/receipts`. HTTP `apply` стартует
  background job и возвращает `202 Accepted` с `job_id`; wizard должен poll-ить
  job status и читать persisted receipts для progress UI. Mutating HTTP install
  requests поддерживают setup-token guard через
  `RUSTOK_INSTALL_SETUP_TOKEN` и header `x-rustok-setup-token` или
  `Authorization: Bearer <token>`; production HTTP apply без setup token
  отклоняется. `/api/install/*` намеренно обходит tenant resolution middleware,
  потому что первый install запускается до создания tenant context. CLI остаётся
  canonical automation path.
- Tenant middleware resolution contract зафиксирован integration tests в
  `apps/server/tests/tenant_resolver_invariants_test.rs`: active tenant
  разрешается через `header`, `host` и `subdomain`, disabled tenant стабильно
  отвечает `403`, отсутствующий tenant — `404`.
- Provisioning/deprovisioning path обязан инициировать cache invalidation
  (`invalidate_tenant_cache_by_uuid/slug/host`) после create/update/deactivate/
  domain-change операций: положительный cache живёт до `TENANT_CACHE_TTL=300s`,
  negative cache miss — до `TENANT_NEGATIVE_CACHE_TTL=60s`, поэтому без
  invalidation stale resolver state допустим только в рамках этих TTL.
  Regression matrix дополнительно фиксирует lifecycle сценарии stale positive
  cache после deactivate/update, negative cache после create-like flow, host
  cache после domain-change и UUID invalidation.

## Границы ответственности

`apps/server` отвечает за:

- transport adapters, middleware, request/runtime context и host wiring;
- общий GraphQL schema surface и Leptos server-function entrypoints;
- композицию owner-owned AI GraphQL roots из `rustok-ai` и узкий RBAC persistence adapter
  `AiGraphqlRoleSlugProvider`; AI resolver/DTO surface в `apps/server` не размещается;
- композицию `rustok-media::MediaQuery`, включая owner-owned `mediaUsage`; media resolver/DTO
  не размещаются в server `SystemQuery`;
- композицию dashboard order statistics через `rustok-order::load_order_stats_snapshot` при
  включённом `mod-order`; SQL и DTO для order analytics принадлежат `rustok-order`, а не
  `apps/server::RootQuery`;
- композицию dashboard post statistics через `rustok-content::load_post_stats_snapshot` при
  включённом `mod-content`; SQL и DTO для content analytics принадлежат `rustok-content`,
  а не `apps/server::RootQuery`;
- host-level user dashboard statistics и recent user activity через
  `services::dashboard_user_activity`; `RootQuery` только мапит service DTO в GraphQL DTO и не
  содержит SQL/read-model логику для этих dashboard виджетов;
- manifest-driven композицию owner-owned `flex::graphql::FlexQuery` / `flex::graphql::FlexMutation` и
  регистрацию concrete persistence adapter в `FlexGraphqlRuntime`; standalone Flex
  и attached field-definition resolver/DTO/error/RBAC/event mapping в `apps/server` не размещается;
- Loco/Axum REST handler для standalone Flex, который использует owner-owned `flex::rest`
  request/response DTO, request-to-command mapping и view mapping; server не владеет Flex REST contract types;
- SeaORM adapter для standalone Flex, который хранит persisted schema rows,
  но fields_config parsing/schema build/serialization, localized key derivation, normalize/defaults/strip/validate, shared/localized split, read resolution и PATCH merge выполняет через owner-owned helpers из `flex::standalone`;
- field-definition registry bootstrap, который регистрирует donor persistence adapters для `user`/`order`/`product`/`topic`,
  но row-to-core `FieldDefinition` mapping, `FieldDefinitionView` shape mapping, command-to-adapter-input conversions и lifecycle policy/event construction делегирует owner-owned `flex::registry`;
- bootstrap общего module-owned event runtime через `ModuleRegistry` и `EventDispatcher`;
- health/runtime guardrails, build/release orchestration и operator control-plane endpoints;
- installer HTTP/CLI adapters поверх `rustok-installer`, install locks и
  persistence installer session receipts;
- RBAC enforcement, auth/session integration и host-level observability.

`apps/server` не должен:

- дублировать module-owned domain services, storage и permission logic;
- подменять модульные interaction contracts собственными ad hoc соглашениями;
- превращать cron, relay worker или maintenance task в псевдо-`event_listener` мимо модульного runtime contract;
- ломать dual-path contract между GraphQL и `#[server]`, если добавляется новый internal path.

## Health и runtime guardrails

- [health.md](./health.md) является каноническим документом для readiness, runtime guardrails, `registry_only` smoke и rollout evidence.
- `apps/server` обязан явно различать `DeploymentProfile` и `settings.rustok.runtime.host_mode`.
- Для reduced hosts health/runtime surface должен описывать фактически поднятый runtime, а не full monolith по умолчанию.

## Verification

Минимальный локальный verification path для изменений в `apps/server`:

- точечные `cargo check` и `cargo test` по затронутым crates и transport slices;
- для изменений build/profile wiring отдельно проверять хотя бы один reduced build без embedded UI и один module-sliced профиль вроде `mod-commerce`-only или no-commerce content host, чтобы server binary не тащил лишние surface-зависимости;
- `cargo xtask module validate <slug>` для модулей, чей host wiring или manifest contract изменился;
- targeted contract checks для GraphQL, REST, server functions и health/runtime surface;
- отдельная проверка health/runtime paths, если затронуты deployment profile, `host_mode` или remote executor/runtime guardrails.
- export API contracts через `node scripts/verify/export-reference-artifacts.mjs artifacts/reference`; Bash-обёртка `scripts/verify/export-reference-artifacts.sh` предназначена для CI и Unix-сред.

## Связанные документы

- [Health и runtime guardrails](./health.md)
- [Стек библиотек](./library-stack.md)
- [План ухода от Loco RS к чистому Axum и своим CLI](../../../docs/architecture/loco-exit-plan.md)
- [Контракт транспорта событий](./event-transport.md)
- [План верификации ядра](./CORE_VERIFICATION_PLAN.md)
- [Loco integration](./loco-core-integration-plan.md) — historical/deprecated context; active roadmap is the Loco exit plan above
- [Контракт event flow](../../../docs/architecture/event-flow-contract.md)
- [Контракты manifest-слоя](../../../docs/modules/manifest.md)
- [Runbook retry/compensation lifecycle hook failures](./module-lifecycle-retry-compensation-runbook.md)
- [Карта документации](../../../docs/index.md)
