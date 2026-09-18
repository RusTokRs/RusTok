# Architecture Decisions (ADR)

Significant architectural choices are recorded as ADRs. Historical ADR bodies are
preserved as engineering history; this registry is the canonical source for the
**current decision status**, implementation status, and supersession relation.

## Status model

Decision status describes the architectural decision:

- `Proposed` — under review and not yet authoritative;
- `Accepted` — authoritative target architecture;
- `Superseded` — historical decision replaced by a newer ADR;
- `Rejected` — considered but not adopted.

Implementation status is tracked separately:

- `Not started`;
- `In progress`;
- `Implemented`;
- `Not applicable` for decisions without an implementation lifecycle;
- `Not tracked` only for historical ADRs created before this governance contract.

Acceptance does not imply implementation. An implementation plan cannot promote a
proposal or redefine an accepted decision.

## Supersession policy

Do not rewrite an old ADR to make architectural history appear cleaner. When a
decision changes materially:

1. create or accept the replacement ADR;
2. set the old registry row to `Superseded`;
3. link the replacement in the `Relations` column and in the new ADR metadata;
4. update current architecture/component documentation to point at the active decision.

Small editorial fixes that do not change the decision are allowed, but they must
not silently change ownership, invariants, or the accepted architecture.

## How to add an ADR

1. Read `AGENTS.md`, `docs/index.md`, affected owner docs, and active related ADRs.
2. Copy [`template.md`](./template.md).
3. Name the file `YYYY-MM-DD-short-title.md` following the naming contract.
4. Separate decision status from implementation status.
5. Add the ADR to this registry in the same change.
6. Run `npm run verify:adrs`.
7. Update local/central architecture docs when the accepted boundary changes.

Non-ADR plans and research documents do not belong in this registry.

## Index

| ADR | Title | Decision status | Implementation status | Relations |
| --- | --- | --- | --- | --- |
| [2026-09-18](./2026-09-18-unified-variant-axis-architecture.md) | Unified variant axis architecture | Accepted | Not started | — |
| [2026-09-06](./2026-09-06-crates-workspace-layout-split.md) | Categorized workspace layout for crates | Accepted | Not tracked | — |
| [2026-08-22](./2026-08-22-taxonomy-category-flex-ownership.md) | Taxonomy owns shared Categories; Flex owns runtime custom fields | Accepted | Not tracked | — |
| [2026-08-22](./2026-08-22-module-command-context-evidence.md) | Typed module command-context evidence | Accepted | Not tracked | — |
| [2026-08-20](./2026-08-20-static-module-lifecycle-revision.md) | Static module lifecycle revision aggregate | Accepted | Not tracked | — |
| [2026-08-15](./2026-08-15-shared-retention-policy.md) | Shared retention policy | Accepted | Not tracked | — |
| [2026-08-14](./2026-08-14-module-node-reconciliation-ledger.md) | Durable module node reconciliation ledger | Accepted | Not tracked | — |
| [2026-08-06](./2026-08-06-oci-registry-transport-boundary.md) | Platform-owned OCI registry transport boundary | Accepted | Not tracked | — |
| [2026-08-06](./2026-08-06-module-release-rollback-safety.md) | Module release rollback safety | Accepted | Not tracked | — |
| [2026-08-03](./2026-08-03-owner-operation-receipts.md) | Shared owner-operation receipt ledger | Accepted | Not tracked | — |
| [2026-07-31](./2026-07-31-forum-search-versioned-invalidation-rollout.md) | Forum Search versioned invalidation rollout | Accepted | Not tracked | — |
| [2026-07-26](./2026-07-26-translation-control-plane-boundary.md) | Translation control plane and owner-owned localized data | Proposed | Not started | — |
| [2026-07-24](./2026-07-24-index-storage-layout.md) | Physical PostgreSQL layout for the Index Engine | Accepted | Not tracked | — |
| [2026-07-23](./2026-07-23-user-registration-event-pii.md) | User-registration event PII boundary | Accepted | Not tracked | — |
| [2026-07-23](./2026-07-23-remote-event-consumer-delivery.md) | Remote event consumer delivery | Accepted | Not tracked | — |
| [2026-07-23](./2026-07-23-index-engine-rewrite.md) | Rewrite rustok-index as a generic cross-module Index Engine | Accepted | Not tracked | — |
| [2026-07-23](./2026-07-23-iggy-bundled-single-node-deployment.md) | Iggy bundled single-node deployment | Accepted | Not tracked | — |
| [2026-07-23](./2026-07-23-global-event-delivery-profiles.md) | Global event delivery profiles | Accepted | Not tracked | — |
| [2026-07-23](./2026-07-23-event-schema-release-discipline.md) | Event schema release discipline | Accepted | Not tracked | — |
| [2026-07-23](./2026-07-23-event-messagepack-wire-format.md) | Event MessagePack wire format | Accepted | Not tracked | — |
| [2026-07-22](./2026-07-22-static-promotion-review-boundary.md) | Static promotion review boundary | Accepted | Not tracked | — |
| [2026-07-22](./2026-07-22-richtext-capability-boundary.md) | Richtext capability boundary and single-document contract | Accepted | Not tracked | — |
| [2026-07-22](./2026-07-22-effective-module-policy-decision.md) | Effective module policy decision | Accepted | Not tracked | — |
| [2026-07-22](./2026-07-22-direct-object-store-runtime-owner-local-lifecycle.md) | Direct object-store runtime and owner-local lifecycle | Accepted | Not tracked | — |
| [2026-07-22](./2026-07-22-channel-binding-policy-boundary.md) | Channel binding policy boundary | Accepted | Not tracked | — |
| [2026-07-22](./2026-07-22-artifact-security-state-boundary.md) | Artifact security state boundary | Accepted | Not tracked | — |
| [2026-07-22](./2026-07-22-artifact-data-snapshot-restore.md) | Durable artifact-data snapshot and guarded restore | Accepted | Not tracked | — |
| [2026-07-21](./2026-07-21-language-agnostic-legacy-locale-provenance.md) | Truthful locale provenance for legacy localized rows | Accepted | Not tracked | — |
| [2026-07-21](./2026-07-21-groups-owner-and-feature-provider-boundary.md) | Groups owner and feature-provider boundary | Accepted | Not tracked | — |
| [2026-07-18](./2026-07-18-storage-physical-owner-media-facade.md) | Storage as the physical file owner and Media as the media facade | Superseded | Not applicable | — |
| [2026-07-18](./2026-07-18-repository-connector-module-github-first.md) | Repository connector module with GitHub as the first provider | Accepted | Not tracked | — |
| [2026-07-18](./2026-07-18-rag-postgres-capability-profiles.md) | Athanor-owned RAG data plane | Accepted | Not tracked | — |
| [2026-07-18](./2026-07-18-artifact-declarative-ddl-boundary.md) | Artifact declarative DDL boundary | Accepted | Not tracked | — |
| [2026-07-17](./2026-07-17-typed-tenant-resolution.md) | Typed tenant resolution boundary | Accepted | Not tracked | — |
| [2026-07-17](./2026-07-17-sealed-typed-event-families.md) | Sealed typed event families | Accepted | Not tracked | — |
| [2026-07-17](./2026-07-17-sandbox-artifact-installation-identity.md) | Exact installation identity for sandboxed module artifacts | Accepted | Not tracked | — |
| [2026-07-16](./2026-07-16-module-build-worker-transport.md) | Module build worker transport | Accepted | Not tracked | — |
| [2026-07-16](./2026-07-16-media-search-extraction-boundaries.md) | Media and Search as whole-module extraction pilots | Proposed | Not started | — |
| [2026-07-16](./2026-07-16-fly-ssr-first-browser-runtime.md) | Fly SSR-first browser runtime | Accepted | Not tracked | — |
| [2026-07-16](./2026-07-16-comments-blog-event-projection.md) | Comments-to-Blog reply count projection | Accepted | Not tracked | — |
| [2026-07-13](./2026-07-13-module-trust-verification-transport.md) | Module trust-verification transport | Accepted | Not tracked | — |
| [2026-07-13](./2026-07-13-module-artifact-rollback-boundary.md) | Module artifact rollback boundary | Accepted | Not tracked | — |
| [2026-07-13](./2026-07-13-fly-page-builder-architecture.md) | Fly page-builder engine and dual Page Builder surfaces | Accepted | Not tracked | — |
| [2026-07-13](./2026-07-13-agent-workflow-platform-contracts.md) | Platform contracts for agent workflow configuration and scheduling | Proposed | Not started | — |
| [2026-07-13](./2026-07-13-agent-principals-and-owner-owned-workflows.md) | Agent principals and owner-owned workflows | Accepted | Not tracked | — |
| [2026-07-12](./2026-07-12-installer-topology-composition-identity.md) | Installer topology composition identity | Accepted | Not tracked | — |
| [2026-07-11](./2026-07-11-product-storage-integrity-and-request-trust.md) | Product storage integrity and request trust | Accepted | Not tracked | — |
| [2026-07-11](./2026-07-11-neutral-sandbox-foundation.md) | Neutral sandbox foundation for Alloy and module artifacts | Accepted | Not tracked | — |
| [2026-07-10](./2026-07-10-mcp-management-owner-boundary.md) | MCP management owner boundary | Accepted | Not tracked | — |
| [2026-07-02](./2026-07-02-axum-runtime-and-ops-cli-boundary.md) | Axum runtime and platform CLI boundary | Accepted | Not tracked | — |
| [2026-07-01](./2026-07-01-product-category-bound-attribute-schemas.md) | Product category-bound attribute schemas | Accepted | Not tracked | — |
| [2026-07-01](./2026-07-01-port-contract-ownership-and-runtime-feature-boundary.md) | Port contract ownership and runtime feature boundary | Accepted | Not tracked | — |
| [2026-05-22](./2026-05-22-module-lifecycle-hook-phases-and-retry-contract.md) | Module lifecycle hook phases and retry contract | Accepted | Not tracked | — |
| [2026-05-18](./2026-05-18-control-plane-lifecycle-and-migration-contracts.md) | Control-plane lifecycle and migration ordering contracts | Accepted | Not tracked | — |
| [2026-04-26](./2026-04-26-hybrid-installer-architecture.md) | Hybrid RusTok installer | Accepted | Not tracked | — |
| [2026-04-24](./2026-04-24-ssr-first-leptos-hosts-with-headless-parity.md) | SSR-first Leptos hosts with headless parity | Accepted | Not tracked | — |
| [2026-04-20](./2026-04-20-module-runtime-extensions-for-capabilities.md) | Module-owned runtime capability registration through `ModuleRuntimeExtensions` | Accepted | Not tracked | — |
| [2026-04-19](./2026-04-19-seo-ui-ownership-by-content-module.md) | SEO UI ownership by content modules | Accepted | Not tracked | — |
| [2026-04-19](./2026-04-19-registry-v2-clean-contract-without-runtime-compat.md) | Registry V2 clean contract without runtime-compat layer | Accepted | Not tracked | — |
| [2026-04-12](./2026-04-12-url-owned-admin-route-selection.md) | URL-owned route selection for module-owned admin UI | Accepted | Not tracked | — |
| [2026-04-05](./2026-04-05-multilingual-db-storage-parallel-localized-records.md) | Multilingual DB storage via parallel localized records | Accepted | Not tracked | — |
| [2026-04-03](./2026-04-03-system-i18n-fluent-migration.md) | Fluent migration path for system i18n bundles | Accepted | Not tracked | — |
| [2026-04-03](./2026-04-03-rustok-ai-capability-module.md) | `rustok-ai` as a separate capability module | Accepted | Not tracked | — |
| [2026-04-03](./2026-04-03-request-trust-and-tenant-hardening.md) | Request trust, strict tenant fallback and forwarded-header policy | Accepted | Not tracked | — |
| [2026-03-29](./2026-03-29-taxonomy-module-scope-aware-terms.md) | `rustok-taxonomy` as a shared scope-aware vocabulary module | Accepted | Not tracked | — |
| [2026-03-29](./2026-03-29-single-alloy-capability-module.md) | Single Alloy capability module | Accepted | Not tracked | — |
| [2026-03-29](./2026-03-29-pages-comments-no-default-integration.md) | `rustok-pages` does not get default integration with `rustok-comments` | Accepted | Not tracked | — |
| [2026-03-29](./2026-03-29-leptos-server-functions-as-internal-data-layer.md) | Leptos `#[server]` functions as the internal data layer | Accepted | Not tracked | Historical amendments recorded in ADR |
| [2026-03-29](./2026-03-29-index-search-boundary.md) | Boundary between `rustok-index` and `rustok-search` | Accepted | Not tracked | — |
| [2026-03-29](./2026-03-29-forum-slug-locale-contract.md) | Forum slug/locale contract after content split | Accepted | Not tracked | — |
| [2026-03-28](./2026-03-28-multilingual-content-contract.md) | Multilingual content contract for `blog` / `pages` / `comments` | Accepted | Not tracked | — |
| [2026-03-28](./2026-03-28-content-orchestration-port-boundary.md) | Port boundary for `rustok-content` orchestration | Accepted | Not tracked | — |
| [2026-03-28](./2026-03-28-content-domain-split-and-comments-module.md) | Content-domain split and `rustok-comments` | Accepted | Not tracked | — |
| [2026-03-27](./2026-03-27-channel-resolution-pipeline-and-typed-policies.md) | Channel resolution pipeline and typed policy trajectory | Accepted | Not tracked | — |
| [2026-03-25](./2026-03-25-rustok-channel-experimental-core.md) | `rustok-channel` as an experimental core platform module | Accepted | Not tracked | — |
| [2026-03-25](./2026-03-25-commerce-module-split-product-pricing-inventory.md) | Split of `rustok-commerce` into `product`, `pricing`, and `inventory` | Accepted | Implemented | — |
| [2026-03-25](./2026-03-25-commerce-family-root-submodules-and-provider-slots.md) | `commerce` root module and submodule provider slots | Accepted | Not tracked | — |
| [2026-03-23](./2026-03-23-rustok-api-thin-shared-host-api-layer.md) | `rustok-api` as a thin and unified shared host/API layer | Accepted | Not tracked | — |
| [2026-03-20](./2026-03-20-persisted-alloy-scaffold-drafts-in-server-control-plane.md) | Persisted Alloy scaffold drafts in server control plane | Accepted | Not tracked | — |
| [2026-03-20](./2026-03-20-mcp-runtime-scaffold-store-binding.md) | MCP runtime scaffold flow via pluggable draft store | Accepted | Not tracked | — |
| [2026-03-20](./2026-03-20-alloy-scaffold-review-apply-boundary.md) | Review/apply boundary for Alloy scaffold flow in `rustok-mcp` | Accepted | Not tracked | — |
| [2026-03-20](./2026-03-20-alloy-is-alloy-not-rustok-alloy.md) | Alloy transport crate naming | Superseded | Not applicable | Superseded by [Single Alloy capability module](./2026-03-29-single-alloy-capability-module.md) |
| [2026-03-19](./2026-03-19-mcp-runtime-binding-through-server-bridge.md) | MCP runtime binding through server-owned bridge | Accepted | Not tracked | — |
| [2026-03-19](./2026-03-19-mcp-persisted-management-layer.md) | Persisted MCP management layer in `apps/server` | Superseded | Not applicable | Superseded by [MCP management owner boundary](./2026-07-10-mcp-management-owner-boundary.md) |
| [2026-03-19](./2026-03-19-mcp-identity-and-tool-policy-foundation.md) | MCP identity and tool policy foundation in `rustok-mcp` | Accepted | Not tracked | — |
| [2026-03-19](./2026-03-19-alloy-module-scaffold-via-mcp.md) | Alloy module scaffold as the first real MCP product slice | Accepted | Not tracked | — |
| [2026-03-17](./2026-03-17-dual-ui-strategy-next-batteries-included.md) | Leptos and Next.js UI strategy | Accepted | Not tracked | — |
| [2026-03-11](./2026-03-11-queue-runtime-source-of-truth-outbox.md) | Queue runtime source of truth: `rustok-outbox` and `event_transport_factory` | Accepted | Not tracked | — |
| [2026-03-07](./2026-03-07-deployment-profiles-and-ui-stack.md) | Deployment profiles and UI stack | Accepted | Not tracked | Partially superseded by [Leptos server functions](./2026-03-29-leptos-server-functions-as-internal-data-layer.md) |
| [2026-03-07](./2026-03-07-admin-module-ui-unification.md) | UI module unification between Next.js and Leptos Admin | Accepted | Implemented | — |
| [2026-03-05](./2026-03-05-rbac-relation-only-final-cutover-gate.md) | Final cutover gate for `casbin_only` RBAC | Accepted | Not tracked | — |
| [2026-02-26](./2026-02-26-rbac-relation-source-of-truth-cutover.md) | RBAC source of truth and staged runtime rollout | Accepted | Not tracked | — |
| [2026-02-26](./2026-02-26-auth-lifecycle-unification-session-invalidation.md) | Auth lifecycle unification and session invalidation policy | Accepted | Not tracked | — |
| [2026-02-25](./2026-02-25-shared-design-system-shadcn-port.md) | Unified design system with shadcn/ui CSS variables | Accepted | Not tracked | — |
| [2026-02-19](./2026-02-19-rustok-events-canonical-contract.md) | Canonical event contract in `rustok-events` | Proposed | Not started | — |
| [2026-02-19](./2026-02-19-module-kind-core-vs-optional.md) | Module split into Core and Optional | Accepted | Implemented | — |
| [2026-02-19](./2026-02-19-core-server-module-bundles-routing.md) | Auto-registration of HTTP routes and `core-server` / `module-bundles` split | Proposed | Not started | — |
