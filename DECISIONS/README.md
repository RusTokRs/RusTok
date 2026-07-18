# Architecture Decisions (ADR)

Significant architectural choices are recorded as ADRs. Accepted and
superseded decisions remain in this index as engineering history; the status
column identifies the current canonical decision.

## How to add an ADR

1. Copy [`template.md`](./template.md).
2. Name the new file `YYYY-MM-DD-short-title.md`.
3. Keep it concise and link to relevant specifications or code.

## Index

| ADR | Title | Status |
| --- | --- | --- |
| [2026-07-18](./2026-07-18-rag-postgres-capability-profiles.md) | Athanor-owned RAG data plane | Accepted |
| [2026-07-18](./2026-07-18-repository-connector-module-github-first.md) | Repository connector module with GitHub as the first provider | Accepted |
| [2026-07-18](./2026-07-18-storage-physical-owner-media-facade.md) | Storage as the physical file owner and Media as the media facade | Accepted |
| [2026-07-17](./2026-07-17-sandbox-artifact-installation-identity.md) | Exact installation identity for sandboxed module artifacts | Accepted |
| [2026-07-16](./2026-07-16-module-build-worker-transport.md) | Module build worker transport | Accepted |
| [2026-07-16](./2026-07-16-media-search-extraction-boundaries.md) | Media and Search as whole-module extraction pilots | Proposed |
| [2026-07-16](./2026-07-16-comments-blog-event-projection.md) | Comments-to-Blog reply count projection | Accepted |
| [2026-07-13](./2026-07-13-agent-workflow-platform-contracts.md) | Platform contracts for agent workflow configuration and scheduling | Proposed |
| [2026-07-13](./2026-07-13-module-artifact-rollback-boundary.md) | Module artifact rollback boundary | Accepted |
| [2026-07-13](./2026-07-13-module-trust-verification-transport.md) | Module trust-verification transport | Accepted |
| [2026-07-13](./2026-07-13-agent-principals-and-owner-owned-workflows.md) | Agent principals and owner-owned workflows | Accepted |
| [2026-07-11](./2026-07-11-product-storage-integrity-and-request-trust.md) | Product storage integrity and request trust | Accepted |
| [2026-07-11](./2026-07-11-neutral-sandbox-foundation.md) | Neutral sandbox foundation for Alloy and module artifacts | Accepted |
| [2026-07-10](./2026-07-10-mcp-management-owner-boundary.md) | MCP management owner boundary | Accepted |
| [2026-07-02](./2026-07-02-axum-runtime-and-ops-cli-boundary.md) | Axum runtime and platform CLI boundary | Accepted |
| [2026-07-01](./2026-07-01-product-category-bound-attribute-schemas.md) | Product category-bound attribute schemas | Accepted |
| [2026-07-01](./2026-07-01-port-contract-ownership-and-runtime-feature-boundary.md) | Port contract ownership and runtime feature boundary | Accepted |
| [2026-05-22](./2026-05-22-module-lifecycle-hook-phases-and-retry-contract.md) | Module lifecycle hook phases and retry contract | Accepted |
| [2026-05-18](./2026-05-18-control-plane-lifecycle-and-migration-contracts.md) | Control-plane lifecycle and migration ordering contracts | Accepted |
| [2026-04-26](./2026-04-26-hybrid-installer-architecture.md) | Hybrid RusTok installer | Accepted |
| [2026-04-24](./2026-04-24-ssr-first-leptos-hosts-with-headless-parity.md) | SSR-first Leptos hosts with headless parity | Accepted |
| [2026-04-20](./2026-04-20-module-runtime-extensions-for-capabilities.md) | Module-owned runtime capability registration through `ModuleRuntimeExtensions` | Accepted |
| [2026-04-19](./2026-04-19-seo-ui-ownership-by-content-module.md) | SEO UI ownership by content modules | Accepted |
| [2026-04-19](./2026-04-19-registry-v2-clean-contract-without-runtime-compat.md) | Registry V2 clean contract without runtime-compat layer | Accepted |
| [2026-04-12](./2026-04-12-url-owned-admin-route-selection.md) | URL-owned route selection for module-owned admin UI | Accepted |
| [2026-04-05](./2026-04-05-multilingual-db-storage-parallel-localized-records.md) | Multilingual DB storage via parallel localized records | Accepted |
| [2026-04-03](./2026-04-03-system-i18n-fluent-migration.md) | Fluent migration path for system i18n bundles | Accepted |
| [2026-04-03](./2026-04-03-rustok-ai-capability-module.md) | `rustok-ai` as a separate capability module | Accepted |
| [2026-04-03](./2026-04-03-request-trust-and-tenant-hardening.md) | Request trust, strict tenant fallback and forwarded-header policy | Accepted |
| [2026-03-29](./2026-03-29-taxonomy-module-scope-aware-terms.md) | `rustok-taxonomy` as a shared scope-aware vocabulary module | Accepted |
| [2026-03-29](./2026-03-29-single-alloy-capability-module.md) | Single Alloy capability module | Accepted |
| [2026-03-29](./2026-03-29-pages-comments-no-default-integration.md) | `rustok-pages` does not get default integration with `rustok-comments` | Accepted |
| [2026-03-29](./2026-03-29-leptos-server-functions-as-internal-data-layer.md) | Leptos `#[server]` functions as the internal data layer | Accepted, amended |
| [2026-03-29](./2026-03-29-index-search-boundary.md) | Boundary between `rustok-index` and `rustok-search` | Accepted |
| [2026-03-29](./2026-03-29-forum-slug-locale-contract.md) | Forum slug/locale contract after content split | Accepted |
| [2026-03-28](./2026-03-28-multilingual-content-contract.md) | Multilingual content contract for `blog` / `pages` / `comments` | Accepted |
| [2026-03-28](./2026-03-28-content-orchestration-port-boundary.md) | Port boundary for `rustok-content` orchestration | Accepted |
| [2026-03-28](./2026-03-28-content-domain-split-and-comments-module.md) | Content-domain split and `rustok-comments` | Accepted |
| [2026-03-27](./2026-03-27-channel-resolution-pipeline-and-typed-policies.md) | Channel resolution pipeline and typed policy trajectory | Accepted |
| [2026-03-25](./2026-03-25-rustok-channel-experimental-core.md) | `rustok-channel` as an experimental core platform module | Accepted |
| [2026-03-25](./2026-03-25-commerce-module-split-product-pricing-inventory.md) | Split of `rustok-commerce` into `product`, `pricing`, and `inventory` | Accepted & Implemented |
| [2026-03-25](./2026-03-25-commerce-family-root-submodules-and-provider-slots.md) | `commerce` root module and submodule provider slots | Accepted |
| [2026-03-23](./2026-03-23-rustok-api-thin-shared-host-api-layer.md) | `rustok-api` as a thin and unified shared host/API layer | Accepted |
| [2026-03-20](./2026-03-20-persisted-alloy-scaffold-drafts-in-server-control-plane.md) | Persisted Alloy scaffold drafts in server control plane | Accepted |
| [2026-03-20](./2026-03-20-mcp-runtime-scaffold-store-binding.md) | MCP runtime scaffold flow via pluggable draft store | Accepted |
| [2026-03-20](./2026-03-20-alloy-scaffold-review-apply-boundary.md) | Review/apply boundary for Alloy scaffold flow in `rustok-mcp` | Accepted |
| [2026-03-20](./2026-03-20-alloy-is-alloy-not-rustok-alloy.md) | Alloy transport crate naming | Superseded by [Single Alloy capability module](./2026-03-29-single-alloy-capability-module.md) |
| [2026-03-19](./2026-03-19-mcp-runtime-binding-through-server-bridge.md) | MCP runtime binding through server-owned bridge | Accepted |
| [2026-03-19](./2026-03-19-mcp-persisted-management-layer.md) | Persisted MCP management layer in `apps/server` | Superseded by [MCP management owner boundary](./2026-07-10-mcp-management-owner-boundary.md) |
| [2026-03-19](./2026-03-19-mcp-identity-and-tool-policy-foundation.md) | MCP identity and tool policy foundation in `rustok-mcp` | Accepted |
| [2026-03-19](./2026-03-19-alloy-module-scaffold-via-mcp.md) | Alloy module scaffold as the first real MCP product slice | Accepted |
| [2026-03-17](./2026-03-17-dual-ui-strategy-next-batteries-included.md) | Leptos and Next.js UI strategy | Accepted |
| [2026-03-11](./2026-03-11-queue-runtime-source-of-truth-outbox.md) | Queue runtime source of truth: `rustok-outbox` and `event_transport_factory` | Accepted |
| [2026-03-07](./2026-03-07-deployment-profiles-and-ui-stack.md) | Deployment profiles and UI stack | Partially superseded by [Leptos server functions](./2026-03-29-leptos-server-functions-as-internal-data-layer.md) |
| [2026-03-07](./2026-03-07-admin-module-ui-unification.md) | UI module unification between Next.js and Leptos Admin | Accepted & Implemented |
| [2026-03-05](./2026-03-05-rbac-relation-only-final-cutover-gate.md) | Final cutover gate for `casbin_only` RBAC | Accepted |
| [2026-02-26](./2026-02-26-rbac-relation-source-of-truth-cutover.md) | RBAC source of truth and staged runtime rollout | Accepted |
| [2026-02-26](./2026-02-26-auth-lifecycle-unification-session-invalidation.md) | Auth lifecycle unification and session invalidation policy | Accepted |
| [2026-02-25](./2026-02-25-shared-design-system-shadcn-port.md) | Unified design system with shadcn/ui CSS variables | Accepted |
| [2026-02-19](./2026-02-19-rustok-events-canonical-contract.md) | Canonical event contract in `rustok-events` | Proposed |
| [2026-02-19](./2026-02-19-module-kind-core-vs-optional.md) | Module split into Core and Optional | Accepted & Implemented |
| [2026-02-19](./2026-02-19-core-server-module-bundles-routing.md) | Auto-registration of HTTP routes and `core-server` / `module-bundles` split | Proposed |
| [2026-03-07](../docs/concepts/plan-oauth2-app-connections.md) | OAuth2 App Connections: connecting external applications to the API | Draft |
