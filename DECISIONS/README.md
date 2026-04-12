# Architecture Decisions (ADR)

All significant architectural choices should be recorded as ADRs.

## How to add an ADR

1. Copy [`template.md`](./template.md).
2. Name the new file `YYYY-MM-DD-short-title.md`.
3. Keep it concise and link to relevant specs or code.

## Index

| ADR | Title | Status |
|-----|-------|--------|
| [2026-04-12](./2026-04-12-url-owned-admin-route-selection.md) | URL-owned route selection for module-owned admin UI | Accepted |
| [2026-04-05](./2026-04-05-multilingual-db-storage-parallel-localized-records.md) | Multilingual DB storage via parallel localized records | Accepted |
| [2026-03-29](./2026-03-29-taxonomy-module-scope-aware-terms.md) | `rustok-taxonomy` as a shared scope-aware vocabulary module | Accepted |
| [2026-04-03](./2026-04-03-request-trust-and-tenant-hardening.md) | Request trust, strict tenant fallback and forwarded-header policy | Accepted |
| [2026-04-03](./2026-04-03-system-i18n-fluent-migration.md) | Fluent migration path for system i18n bundles | Accepted |
| [2026-03-29](./2026-03-29-forum-slug-locale-contract.md) | Forum slug/locale contract after content split | Accepted |
| [2026-03-29](./2026-03-29-index-search-boundary.md) | Граница между `rustok-index` и `rustok-search` | Accepted |
| [2026-03-29](./2026-03-29-pages-comments-no-default-integration.md) | `rustok-pages` не получает default-интеграцию с `rustok-comments` | Accepted |
| [2026-03-28](./2026-03-28-content-orchestration-port-boundary.md) | Портовая граница для `rustok-content` orchestration | Accepted |
| [2026-03-28](./2026-03-28-multilingual-content-contract.md) | Multilingual content contract for `blog` / `pages` / `comments` | Accepted |
| [2026-03-28](./2026-03-28-content-domain-split-and-comments-module.md) | Разведение `content`-storage, введение `rustok-comments` и новая роль `rustok-content` | Accepted |
| [2026-03-27](./2026-03-27-channel-resolution-pipeline-and-typed-policies.md) | Channel resolution pipeline и typed policy trajectory | Accepted |
| [2026-03-25](./2026-03-25-rustok-channel-experimental-core.md) | `rustok-channel` как experimental core-модуль платформы | Accepted |
| [2026-02-26](./2026-02-26-auth-lifecycle-unification-session-invalidation.md) | Унификация auth lifecycle и policy инвалидирования сессий | Accepted |
| [2026-02-26](./2026-02-26-rbac-relation-source-of-truth-cutover.md) | RBAC source of truth и staged runtime rollout | Accepted |
| [2026-02-19](./2026-02-19-module-kind-core-vs-optional.md) | Разделение модулей на Core и Optional | Accepted & Implemented |
| [2026-03-23](./2026-03-23-rustok-api-thin-shared-host-api-layer.md) | `rustok-api` как тонкий и единый shared host/API layer | Accepted |
| [2026-03-20](./2026-03-20-mcp-runtime-scaffold-store-binding.md) | Live MCP scaffold flow через pluggable persisted draft store | Accepted |
| [2026-03-20](./2026-03-20-persisted-alloy-scaffold-drafts-in-server-control-plane.md) | Persisted Alloy scaffold drafts в server control plane | Accepted |
| [2026-03-20](./2026-03-20-alloy-is-alloy-not-rustok-alloy.md) | Alloy называется `alloy`, а не `rustok-alloy` | Accepted |
| [2026-03-20](./2026-03-20-alloy-scaffold-review-apply-boundary.md) | Review/apply boundary для Alloy scaffold flow в `rustok-mcp` | Accepted |
| [2026-03-19](./2026-03-19-alloy-module-scaffold-via-mcp.md) | Alloy module scaffold как первый реальный MCP product slice | Accepted |
| [2026-03-19](./2026-03-19-mcp-runtime-binding-through-server-bridge.md) | MCP runtime binding через server-owned bridge | Accepted |
| [2026-03-19](./2026-03-19-mcp-persisted-management-layer.md) | Persisted MCP management layer в `apps/server` | Accepted |
| [2026-03-19](./2026-03-19-mcp-identity-and-tool-policy-foundation.md) | MCP identity и tool policy foundation в `rustok-mcp` | Accepted |
| [2026-03-11](./2026-03-11-queue-runtime-source-of-truth-outbox.md) | Queue runtime source of truth: rustok-outbox + event_transport_factory | Accepted |
| [2026-03-11](./2026-03-11-loco-mailer-storage-as-server-infra.md) | Loco Mailer и Storage как server-infra слой (без отдельного модуля) | Accepted |
| [2026-03-07](./2026-03-07-admin-module-ui-unification.md) | Унификация UI модулей между Next.js и Leptos Admin | Accepted & Implemented |
| [2026-03-07](./2026-03-07-deployment-profiles-and-ui-stack.md) | Deployment Profiles: composable layers (monolith / hybrid / headless) | Proposed (v2) |
| [2026-03-07](../docs/concepts/plan-oauth2-app-connections.md) | OAuth2 App Connections: подключение внешних приложений к API | Draft |
