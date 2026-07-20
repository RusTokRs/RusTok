---
id: doc://docs/index.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# RusTok: Documentation Map

This file is the canonical entry point for the repository documentation.
Start here when following the rules in [AGENTS.md](../AGENTS.md).

Documentation in `docs/` describes the platform as a whole.
Local documents for applications and crates live in `apps/*/docs/`,
`crates/*/docs/` and `README.md` next to the code.

## How to Use the Map

1. Open the platform overview and the relevant architecture section.
2. For modules, use `docs/modules/*` and `docs/modules/registry.md`.
3. For UI, use `docs/UI/*` and local app docs.
4. For verification and quality gates, use `docs/verification/*`
   and `docs/guides/*`.
5. For architecture decisions, use `DECISIONS/*`.

## Required Starting Documents

- [Platform Overview](./architecture/overview.md)
- [Architecture Principles](./architecture/principles.md)
- [API and Surface Contracts](./architecture/api.md)
- [Routing](./architecture/routing.md)
- [Module Architecture](./architecture/modules.md)
- [Module and Owner Map](./modules/registry.md)

## Module System

- [Module Platform Overview](./modules/overview.md)
- [How to Write a Module in RusToK](./modules/module-authoring.md)
- [`rustok-module.toml` Contract](./modules/manifest.md)
- [Module and Application Registry](./modules/registry.md)
- [FFA/FBA Readiness Board and Boundary Evidence (inside module registry)](./modules/registry.md#ffafba-readiness-board-module-owned-ui)
- [Module Platform Crate Registry](./modules/crates-registry.md)
- [`rustok-modules` control-plane documentation](../crates/rustok-modules/docs/README.md)
- [`rustok-verification-transport` gRPC documentation](../crates/rustok-verification-transport/docs/README.md)
- [`rustok-verification-worker` documentation](../crates/rustok-verification-worker/docs/README.md)
- [`rustok-module-build-transport` gRPC documentation](../crates/rustok-module-build-transport/docs/README.md)
- [`rustok-module-build-worker` documentation](../crates/rustok-module-build-worker/docs/README.md)
- [`rustok-registry-validation-worker` documentation](../crates/rustok-registry-validation-worker/README.md)
- [`rustok-module-build-dispatcher` documentation](../crates/rustok-module-build-dispatcher/docs/README.md)
- [`rustok-worker-transport` mTLS foundation](../crates/rustok-worker-transport/docs/README.md)
- [`rustok-sandbox` execution foundation documentation](../crates/rustok-sandbox/docs/README.md)
- [`rustok-page-builder` Runtime Contract](../crates/rustok-page-builder/docs/README.md)
- [Machine-readable FBA Registry Page Builder](../crates/rustok-page-builder/contracts/page-builder-fba-registry.json)
- [Machine-readable Page Builder Wave Evidence Template](../crates/rustok-page-builder/contracts/page-builder-wave-evidence-template.json)
- [Machine-readable Page Builder Control-plane Dry-run Contract](../crates/rustok-page-builder/contracts/page-builder-control-plane-dry-run.json)
- [Machine-readable Page Builder Flutter Wave Hand-off Contract](../crates/rustok-page-builder/contracts/page-builder-flutter-wave-handoff.json)
- [Synthetic Pages Wave 0 Dry-run Evidence Packet](../crates/rustok-page-builder/contracts/evidence/pages-wave0-dry-run-evidence.json)
- [Module Documentation Index](./modules/_index.md)
- [Auth Module and Owner-owned Auth/OAuth GraphQL Boundary Documentation](../crates/rustok-auth/docs/README.md)
- [MCP and Owner-owned Management GraphQL Documentation](../crates/rustok-mcp/docs/README.md)
- [AI Capability and Owner-owned GraphQL Documentation](../crates/rustok-ai/docs/README.md) - includes provider-neutral RAG ingestion and the Athanor-owned data-plane boundary.
- [Content and Owner-owned Dashboard Analytics Documentation](../crates/rustok-content/docs/README.md)
- [Cart Ownership and Guest Capability Adapter Documentation](../crates/rustok-cart/docs/README.md)
- [Media and Owner-owned GraphQL Documentation](../crates/rustok-media/docs/README.md)
- [Order and Owner-owned Dashboard Analytics Documentation](../crates/rustok-order/docs/README.md)
- [Flex and Manifest-composed Owner-owned GraphQL Documentation](../crates/flex/docs/README.md)
- [Implementation Plans Registry](./modules/implementation-plans-registry.md)
- [Module Control-plane Consolidation Plan](./modules/module-control-plane-consolidation-plan.md)
- [Module Documentation Template](./templates/module_contract.md)
- [Module UI Packages Index](./modules/UI_PACKAGES_INDEX.md)
- [UI Packages Quick Start](./modules/UI_PACKAGES_QUICKSTART.md)
- **Module Backend Guides** (read these when working on module services, ports, GraphQL/REST handlers, Leptos server-function adapters, FBA metadata or CLI adapters):
  - [Architecture Guide](./backend/module-backend-architecture.md) - backend ownership, runtime boundaries, foundation crates and FBA/CLI split
  - [Implementation Guide](./backend/module-backend-implementation.md) - crate layout, runtime helpers, transport adapters, ports and forbidden patterns
  - [Verification Guide](./backend/module-backend-verification.md) - fast guardrails, targeted Rust checks and FBA evidence
- [Rich Text Implementation Plan](./modules/rich-text-implementation-plan.md)
- [Page Builder Implementation Plan](./modules/page-builder-implementation-plan.md)
- AI support adapter plans for `rustok-ai-media` and `rustok-ai-alloy` are available through the [module documentation index](./modules/_index.md).

## UI and Client Surfaces

- [UI Overview](./UI/README.md)
- [GraphQL and Leptos Server Functions](./UI/graphql-architecture.md) — including native/GraphQL parity for owner-owned storefront payment collection/refund reads
- [Storefront and Checkout Slots Contract](./UI/storefront.md)
- [Flutter Mobile Storefront Host](../rustok_mobile/apps/rustok_frontend_mobile/README.md)
- [Flutter Mobile Package Catalog/Cart](../rustok_mobile/packages/rustok_catalog_mobile/README.md)
- [Admin ↔ Server Quick Start](./UI/admin-server-connection-quickstart.md)
- [SEO Runtime/Control-plane Contracts (`rustok-seo`)](../crates/rustok-seo/docs/README.md)
- [SEO Operations Runbook](../crates/rustok-seo/docs/operations-runbook.md)
- [Rust UI Component Catalog](./UI/rust-ui-component-catalog.md)
- [Rich Text Track](./modules/rich-text-implementation-plan.md)
- [Page Builder Track](./modules/page-builder-implementation-plan.md)
- [i18n Architecture](./architecture/i18n.md)
- **Module UI Package Guides** (read the relevant one when working on `crates/rustok-*/admin` or `crates/rustok-*/storefront`):
  - [Architecture Guide](./UI/module-package-architecture.md) — FFA, `core/transport/ui` split, dual-path model, Dioxus-readiness
  - [Implementation Guide](./UI/module-package-implementation.md) — file structure, internal crates, i18n, URL-selection, manifest wiring, forbidden patterns
  - [Verification Guide](./UI/module-package-verification.md) — all verification commands, what each checks, common errors

## Architecture and Foundation

- [ADR: Athanor-owned RAG data plane](../DECISIONS/2026-07-18-rag-postgres-capability-profiles.md)
- [ADR: Repository connector module with GitHub as the first provider](../DECISIONS/2026-07-18-repository-connector-module-github-first.md)
- [ADR: Storage physical ownership and Media facade](../DECISIONS/2026-07-18-storage-physical-owner-media-facade.md)

- [`rustok-installer` contract and implementation plan](../crates/rustok-installer/docs/README.md) — installer ownership, browser-safe contract surface, native seed-runtime boundary, monolith/distributed topology contract and CLI/HTTP adapter boundaries

- [Platform Diagram](./architecture/diagram.md)
- [Backend Module Guides](./backend/README.md) - target backend module architecture, implementation and verification for `rustok-runtime`, `rustok-web`, `rustok-fba` and `rustok-cli-core`
- [Database](./architecture/database.md) — live DB/i18n storage contract: `base + translations + optional bodies`, `VARCHAR(32)` locale storage, `tenant_locales` policy layer, `flex` standalone schema translations, shared attached localized Flex values, live donor paths for `user`, `product`, `order`, and `topic`
- [Hybrid Installer ADR](../DECISIONS/2026-04-26-hybrid-installer-architecture.md) — installer-core/CLI/web wizard layering, PostgreSQL production policy, explicit separation of build composition, schema composition and tenant enablement
- [Axum Runtime and Operations CLI Boundary](../DECISIONS/2026-07-02-axum-runtime-and-ops-cli-boundary.md)
- [ADR: Installer Topology Composition Identity](../DECISIONS/2026-07-12-installer-topology-composition-identity.md) — trusted distribution identity and versioned installer topology ownership
- [ADR: Axum Runtime and Platform CLI Boundary](../DECISIONS/2026-07-02-axum-runtime-and-ops-cli-boundary.md) — pure Axum server binary without maintenance CLI code, separate `rustok-cli`, module-local `cli/` adapters and generated registries for distribution-aware builds
- [ADR: Lifecycle Hook Phases/Retry Contract](../DECISIONS/2026-05-22-module-lifecycle-hook-phases-and-retry-contract.md) — `validated/running/committed/failed`, explicit `pre/post` hooks and retryable post-hook failures without partial rollback
- [ADR: Neutral Sandbox Foundation](../DECISIONS/2026-07-11-neutral-sandbox-foundation.md) — one sandbox contract for Alloy-authored Rhai, WebAssembly module artifacts and future sidecars
- [ADR: Exact Sandbox Artifact Installation Identity](../DECISIONS/2026-07-17-sandbox-artifact-installation-identity.md) — exact owner-selected identity for dynamic artifact capability scope resolution
- [ADR: Product Storage Integrity and Request Trust](../DECISIONS/2026-07-11-product-storage-integrity-and-request-trust.md) — PostgreSQL product storage, tenant-composite integrity, canonical primary category and request-bound product writes
- [ADR: Shared API Contract Ownership](../DECISIONS/2026-07-01-port-contract-ownership-and-runtime-feature-boundary.md) — `Port*`, permission and locale contracts in `rustok-api`, one-way graph `rustok-core -> rustok-api` and owner-owned outbox adapter
- [ADR: Media and Search Extraction Boundaries](../DECISIONS/2026-07-16-media-search-extraction-boundaries.md) — whole-module remote pilots with search connectors kept inside `rustok-search`
- [Channels](./architecture/channels.md)
- [DataLoader](./architecture/dataloader.md)
- [Event Flow Contract](./architecture/event-flow-contract.md)
- [Matryoshka / Composition Model](./architecture/matryoshka.md)
- [Performance Baseline](./architecture/performance-baseline.md)

## Examples and Smoke Scenarios

- [Executable Examples Catalog](./examples/README.md)

## Guides and Standards

- [Quick Start](./guides/quickstart.md)
- [Testing](./guides/testing.md)
- [Observability Quick Start](./guides/observability-quickstart.md)
- [Runtime Guardrails](./guides/runtime-guardrails.md)
- [Alloy Runtime Hardening Contract](../crates/alloy/contracts/alloy-runtime-contract.json)
- [ADR: Control-plane Lifecycle and Migration Ordering Contracts](../DECISIONS/2026-05-18-control-plane-lifecycle-and-migration-contracts.md)
- [Input Validation](./guides/input-validation.md)
- [Error Handling](./guides/error-handling.md)
- [Security Audit](./guides/security-audit.md)
- [Logging](./standards/logging.md)
- [Errors](./standards/errors.md)
- [Security](./standards/security.md)
- [Coding Standards](./standards/coding.md)
- [RT JSON v1 Standard](./standards/rt-json-v1.md)

## Platform Verification

- [Workspace CLI Tool `xtask`](../xtask/README.md)
- [Athanor Operations Documentation](./operations/README.md)
- [Main Verification README](./verification/README.md)
- [Cross-platform OpenAPI/GraphQL Reference Artifacts Export](../scripts/verify/export-reference-artifacts.mjs)
- [OpenAPI/GraphQL Reference Artifacts Verification](../scripts/verify/verify-reference-artifacts.mjs)
- [Flex Multilingual Contract Verification](../scripts/verify/verify-flex-multilingual-contract.mjs)
- [Flex Standalone Contract Guardrails Verification](../scripts/verify/verify-flex-standalone-contract.mjs)
- [Consolidated Cyclic Verification Plan](./verification/PLATFORM_VERIFICATION_PLAN.md) - resumable Core-modules-first pre-release sweep with a resettable cycle cursor and local implementation-plan handoffs; `rustok-core` is tracked separately as a foundation crate.
- [Foundation Layer Verification](./verification/platform-foundation-verification-plan.md)
- [API Surfaces Verification](./verification/platform-api-surfaces-verification-plan.md)
- [Frontend Surfaces Verification](./verification/platform-frontend-surfaces-verification-plan.md)
- [Core Integrity Verification](./verification/platform-core-integrity-verification-plan.md)
- [Quality and Operations Verification](./verification/platform-quality-operations-verification-plan.md)

## AI, Research and Templates

- [AI Context](./AI_CONTEXT.md)
- [AI Session Template](./ai/SESSION_TEMPLATE.md)
- [Known Pitfalls](./ai/KNOWN_PITFALLS.md)
- [MCP Reference Index](./references/mcp/README.md)
- [RusTok vs Medusa Architecture Comparison](./research/medusa-vs-rustok-architecture.md)
- [Fluid Frontend Architecture for RusTok](./research/fluid-frontend-architecture.md)
- [Fluid Backend Architecture for RusTok](./research/fluid-backend-architecture.md)
- [Flutter Application Architecture for RusTok](./research/flutter.md)
- [FFA for Flutter: Platform Mobile Architecture Article](./research/flutter-ffa-architecture-article.md)
- [Unified Fluid Backend Architecture Implementation Plan](./research/fluid-backend-architecture-unified-plan.md)
- [FFA UI Refactoring Plan and Dioxus Preparation](./research/dioxus-ffa-ui-migration-plan.md)
- [FFA/Dioxus Pilot Connectivity Map (Phase A)](./research/dioxus-ffa-pilot-connectivity-map.md)
- [FFA UI Migration Parity Checklist](./verification/ffa-ui-parity-checklist.md)

## Application Documentation

- [Server Documentation](../apps/server/docs/README.md)
- [Server Runbook: Retry/Compensation Lifecycle Hook Failures](../apps/server/docs/module-lifecycle-retry-compensation-runbook.md)
- [Admin Documentation](../apps/admin/docs/README.md)
- [Storefront Documentation](../apps/storefront/docs/README.md)
- [Next Admin Documentation](../apps/next-admin/docs/README.md)
- [Next Frontend Documentation](../apps/next-frontend/docs/README.md)
- [Flutter Admin Mobile Documentation](../rustok_mobile/apps/rustok_admin_mobile/README.md)
- [Flutter Frontend Mobile Documentation](../rustok_mobile/apps/rustok_frontend_mobile/README.md)

## Crate Documentation

- For platform modules: `crates/rustok-*` per the
  [module and application registry](./modules/registry.md).
- For foundation/shared libraries see `crates/rustok-*`
  and the corresponding `README.md`.
- For infrastructure/capability crates see `crates/*`
  and `docs/modules/crates-registry.md`.
- For UI libraries use `crates/leptos-*`, `crates/leptos-ui`,
  `crates/rustok-ui-*`, `crates/rustok-graphql`
  and `crates/rustok-graphql-leptos`.
- Every crate must have an up-to-date `README.md`,
  and `docs/` if needed.

## Keeping Documentation Up to Date

- All documentation is written in English.
- `README.ru.md` is the only file allowed in Russian (localized translation of the main README).
- One file — one language.
- Do not create a new document if a suitable one already exists:
  extend the current one.
- When changing architecture, API, tenancy, routing, observability
  or the module system, update both local component docs
  and central documents in `docs/`.
- Any new schema undergoes an i18n audit;
  localized display fields live in `*_translations`.
- Read-side locale matching uses shared normalization
  (`requested -> tenant default -> first available`).
- Module-owned admin UI stores selection state in URL
  with typed `snake_case` query keys.

## Architecture Decisions

- [ADR Index](../DECISIONS/README.md)

- [Security: RUSTSEC-2026-0045 Remediation Note](./security/aws-lc-rustsec-2026-0045.md)
- [Security: RUSTSEC-2026-0098 / 0099 / 0104 Remediation Note](./security/rustls-webpki-rustsec-2026-0099-0104.md)
- [Security: RUSTSEC-2023-0071 Remediation Note](./security/rsa-rustsec-2023-0071.md)
