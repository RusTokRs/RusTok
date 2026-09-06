---
id: doc://docs/modules/_index.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Module Documentation Index

Platform plan for runtime registry consolidation, composition, tenant lifecycle,
governance and admin transport: [module control-plane consolidation plan](./module-control-plane-consolidation-plan.md).

Cross-cutting pre-implementation plan for owner-safe business translation,
progress, memory/glossaries, and the `rustok-ai` machine-translation adapter:
[translation module implementation plan](./translation-implementation-plan.md).

Module local documentation lives inside the crates themselves at
`crates/modules/<name>/docs/README.md`. This document provides central navigation only.

## Navigation Rule

- Module documentation is not duplicated in `docs/modules/`.
- Links below lead directly to `crates/modules/<name>/docs/`.
- Platform modules require `README.md`, `docs/README.md`, and
  `docs/implementation-plan.md`.

## Core and Foundation Layer

| Component | Documentation | Implementation Plan |
|---|---|---|
| `rustok-core` | [docs](../../crates/libs/rustok-core/docs/README.md) | [plan](../../crates/libs/rustok-core/docs/implementation-plan.md) |
| `rustok-events` | [docs](../../crates/libs/rustok-events/docs/README.md) | [plan](../../crates/libs/rustok-events/docs/implementation-plan.md) |
| `rustok-events-module` | [docs](../../crates/modules/rustok-events-module/docs/README.md) | [plan](../../crates/modules/rustok-events-module/docs/implementation-plan.md) |
| `rustok-channel` | [docs](../../crates/modules/rustok-channel/docs/README.md) | [plan](../../crates/modules/rustok-channel/docs/implementation-plan.md) |
| `rustok-index` | [docs](../../crates/modules/rustok-index/docs/README.md) | [plan](../../crates/modules/rustok-index/docs/implementation-plan.md) |
| `rustok-search` | [docs](../../crates/modules/rustok-search/docs/README.md) | [plan](../../crates/modules/rustok-search/docs/implementation-plan.md) |
| `rustok-outbox` | [docs](../../crates/modules/rustok-outbox/docs/README.md) | [plan](../../crates/modules/rustok-outbox/docs/implementation-plan.md) |
| `rustok-telemetry` | [docs](../../crates/libs/rustok-telemetry/docs/README.md) | [plan](../../crates/libs/rustok-telemetry/docs/implementation-plan.md) |
| `rustok-tenant` | [docs](../../crates/modules/rustok-tenant/docs/README.md) | [plan](../../crates/modules/rustok-tenant/docs/implementation-plan.md) |
| `rustok-translation-targets` | [docs](../../crates/modules/rustok-translation-targets/docs/README.md) | [central Translation plan](./translation-implementation-plan.md) |
| `rustok-rbac` | [docs](../../crates/modules/rustok-rbac/docs/README.md) | [plan](../../crates/modules/rustok-rbac/docs/implementation-plan.md) |
| `rustok-cache` | [docs](../../crates/modules/rustok-cache/docs/README.md) | [plan](../../crates/modules/rustok-cache/docs/implementation-plan.md) |
| `rustok-auth` | [docs](../../crates/modules/rustok-auth/docs/README.md) | [plan](../../crates/modules/rustok-auth/docs/implementation-plan.md) |
| `rustok-email` | [docs](../../crates/modules/rustok-email/docs/README.md) | [plan](../../crates/modules/rustok-email/docs/implementation-plan.md) |
| `rustok-storage` | [docs](../../crates/utils/rustok-storage/docs/README.md) | [plan](../../crates/utils/rustok-storage/docs/implementation-plan.md) |
| `rustok-api` | [docs](../../crates/libs/rustok-api/docs/README.md) | [plan](../../crates/libs/rustok-api/docs/implementation-plan.md) |
| `rustok-runtime` | [docs](../../crates/libs/rustok-runtime/docs/README.md) | [plan](../../crates/libs/rustok-runtime/docs/implementation-plan.md) |
| `rustok-modules` | [docs](../../crates/modules/rustok-modules/docs/README.md) | [plan](../../crates/modules/rustok-modules/docs/implementation-plan.md) |
| `rustok-verification-transport` | [docs](../../crates/workers/rustok-verification-transport/docs/README.md) | gRPC transport contract is recorded in the crate README. |
| `rustok-media-transport` | [docs](../../crates/modules/rustok-media-transport/docs/README.md) | Loopback-verified gRPC adapter for Media-owned read/write ports. |
| `rustok-verification-worker` | [docs](../../crates/workers/rustok-verification-worker/docs/README.md) | Worker rollout is recorded in the module control-plane plan. |
| `rustok-artifact-node-transport` | [docs](../../crates/workers/rustok-artifact-node-transport/docs/README.md) | Current-only mTLS node-agent transport is recorded in the module control-plane plan. |
| `rustok-artifact-node-controller` | [docs](../../crates/workers/rustok-artifact-node-controller/docs/README.md) | Independent mTLS owner-port composition is recorded in the module control-plane plan. |
| `rustok-artifact-node-reconciler` | [docs](../../crates/workers/rustok-artifact-node-reconciler/docs/README.md) | Independent mTLS topology-authoring composition is recorded in the module control-plane plan. |
| `rustok-artifact-node-agent` | [docs](../../crates/workers/rustok-artifact-node-agent/docs/README.md) | Independent mTLS node materialization and readiness process is recorded in the module control-plane plan. |
| `rustok-module-build-transport` | [docs](../../crates/workers/rustok-module-build-transport/docs/README.md) | Current-only mTLS module and static-distribution build-worker transport is recorded in the module control-plane plan. |
| `rustok-module-build-worker` | [docs](../../crates/workers/rustok-module-build-worker/docs/README.md) | Isolated build-worker rollout is recorded in the module control-plane plan. |
| `rustok-module-sdk` | [docs](../../crates/utils/rustok-module-sdk/docs/README.md) | [plan](../../crates/utils/rustok-module-sdk/docs/implementation-plan.md) |
| `rustok-module-template` | [docs](../../crates/utils/rustok-module-template/docs/README.md) | [plan](../../crates/utils/rustok-module-template/docs/implementation-plan.md) |
| `rustok-build-publication` | [docs](../../crates/utils/rustok-build-publication/docs/README.md) | Shared current-only registry credential and Cosign publication boundary for isolated build workers. |
| `rustok-build-source` | [docs](../../crates/utils/rustok-build-source/docs/README.md) | Shared deterministic source packaging, strict inspection, and immutable CAS materialization. |
| `rustok-static-distribution-worker` | [docs](../../crates/workers/rustok-static-distribution-worker/docs/README.md) | Trusted native-distribution CI worker rollout is recorded in the module control-plane plan. |
| `rustok-module-build-dispatcher` | [docs](../../crates/workers/rustok-module-build-dispatcher/docs/README.md) | Broker-neutral build-delivery contract is recorded in the module control-plane plan. |
| `rustok-worker-transport` | [docs](../../crates/workers/rustok-worker-transport/docs/README.md) | Shared mutually authenticated worker-listener foundation. |
| `rustok-sandbox` | [docs](../../crates/workers/rustok-sandbox/docs/README.md) | [plan](../../crates/workers/rustok-sandbox/docs/implementation-plan.md) |
| `rustok-sandbox-transport` | [docs](../../crates/workers/rustok-sandbox-transport/docs/README.md) | [plan](../../crates/workers/rustok-sandbox-transport/docs/implementation-plan.md) |
| `rustok-sandbox-worker` | [docs](../../crates/workers/rustok-sandbox-worker/docs/README.md) | [plan](../../crates/workers/rustok-sandbox-worker/docs/implementation-plan.md) |
| `rustok-web` | [docs](../../crates/libs/rustok-web/docs/README.md) | [plan](../../crates/libs/rustok-web/docs/implementation-plan.md) |
| `rustok-fba` | [docs](../../crates/libs/rustok-fba/docs/README.md) | [plan](../../crates/libs/rustok-fba/docs/implementation-plan.md) |
| `rustok-cli-core` | [docs](../../crates/utils/rustok-cli-core/docs/README.md) | [plan](../../crates/utils/rustok-cli-core/docs/implementation-plan.md) |
| `rustok-cli-platform` | [docs](../../crates/utils/rustok-cli-platform/docs/README.md) | [plan](../../crates/utils/rustok-cli-platform/docs/implementation-plan.md) |
| `rustok-migrations` | [README](../../crates/utils/rustok-migrations/README.md) | - |
| `rustok-installer` | [docs](../../crates/utils/rustok-installer/docs/README.md) | [plan](../../crates/utils/rustok-installer/docs/implementation-plan.md) |
| `rustok-build` | [docs](../../crates/utils/rustok-build/docs/README.md) | [plan](../../crates/utils/rustok-build/docs/implementation-plan.md) |
| `rustok-cli-registry` | [docs](../../crates/utils/rustok-cli-registry/docs/README.md) | [plan](../../crates/utils/rustok-cli-registry/docs/implementation-plan.md) |
| `rustok-distribution` | [docs](../../crates/modules/rustok-distribution/docs/README.md) | Registry composition and deterministic static-promotion build output are documented locally. |
| `rustok-cli` | [docs](../../crates/utils/rustok-cli/docs/README.md) | [plan](../../crates/utils/rustok-cli/docs/implementation-plan.md) |
| `rustok-graphql` | [docs](../../crates/ui/rustok-graphql/docs/README.md) | [plan](../../crates/ui/rustok-graphql/docs/implementation-plan.md) |
| `rustok-ui-i18n` | [docs](../../crates/ui/rustok-ui-i18n/docs/README.md) | [plan](../../crates/ui/rustok-ui-i18n/docs/implementation-plan.md) |
| `rustok-test-utils` | [docs](../../crates/utils/rustok-test-utils/docs/README.md) | [plan](../../crates/utils/rustok-test-utils/docs/implementation-plan.md) |
| `rustok-iggy` | [docs](../../crates/modules/rustok-iggy/docs/README.md) | [plan](../../crates/modules/rustok-iggy/docs/implementation-plan.md) |
| `rustok-iggy-connector` | [docs](../../crates/modules/rustok-iggy-connector/docs/README.md) | [plan](../../crates/modules/rustok-iggy-connector/docs/implementation-plan.md) |
| `rustok-mcp` | [docs](../../crates/modules/rustok-mcp/docs/README.md) | [plan](../../crates/modules/rustok-mcp/docs/implementation-plan.md) |
| `rustok-ai` | [docs](../../crates/modules/rustok-ai/docs/README.md) | [plan](../../crates/modules/rustok-ai/docs/implementation-plan.md) |
| `rustok-ai-translation` | [docs](../../crates/modules/rustok-ai-translation/docs/README.md) | [plan](../../crates/modules/rustok-ai-translation/docs/implementation-plan.md) |
| `rustok-ai-content` | [docs](../../crates/modules/rustok-ai-content/docs/README.md) | [plan](../../crates/modules/rustok-ai-content/docs/implementation-plan.md) |
| `rustok-ai-product` | [docs](../../crates/modules/rustok-ai-product/docs/README.md) | [plan](../../crates/modules/rustok-ai-product/docs/implementation-plan.md) |
| `rustok-ai-order` | [docs](../../crates/modules/rustok-ai-order/docs/README.md) | [plan](../../crates/modules/rustok-ai-order/docs/implementation-plan.md) |
| `rustok-ai-media` | [docs](../../crates/modules/rustok-ai-media/docs/README.md) | [plan](../../crates/modules/rustok-ai-media/docs/implementation-plan.md) |
| `rustok-ai-alloy` | [docs](../../crates/modules/rustok-ai-alloy/docs/README.md) | [plan](../../crates/modules/rustok-ai-alloy/docs/implementation-plan.md) |
| `alloy` | [docs](../../crates/modules/alloy/docs/README.md) | [plan](../../crates/modules/alloy/docs/implementation-plan.md) |
| `flex` | [docs](../../crates/modules/flex/docs/README.md) | [plan](../../crates/modules/flex/docs/implementation-plan.md) |
| `rustok-commerce-foundation` | [docs](../../crates/modules/rustok-commerce-foundation/docs/README.md) | [plan](../../crates/modules/rustok-commerce-foundation/docs/implementation-plan.md) |
| `rustok-seo-render` | [docs](../../crates/modules/rustok-seo/render/docs/README.md) | [plan](../../crates/modules/rustok-seo/render/docs/implementation-plan.md) |
| `rustok-seo-admin-support` | [docs](../../crates/modules/rustok-seo-admin-support/docs/README.md) | [plan](../../crates/modules/rustok-seo-admin-support/docs/implementation-plan.md) |

## Domain Modules

| Component | Documentation | Implementation Plan |
|---|---|---|
| `rustok-content` | [docs](../../crates/modules/rustok-content/docs/README.md) | [plan](../../crates/modules/rustok-content/docs/implementation-plan.md) |
| `rustok-cart` | [docs](../../crates/modules/rustok-cart/docs/README.md) | [plan](../../crates/modules/rustok-cart/docs/implementation-plan.md) |
| `rustok-customer` | [docs](../../crates/modules/rustok-customer/docs/README.md) | [plan](../../crates/modules/rustok-customer/docs/implementation-plan.md) |
| `rustok-product` | [docs](../../crates/modules/rustok-product/docs/README.md) | [plan](../../crates/modules/rustok-product/docs/implementation-plan.md) |
| `rustok-profiles` | [docs](../../crates/modules/rustok-profiles/docs/README.md) | [plan](../../crates/modules/rustok-profiles/docs/implementation-plan.md) |
| `rustok-groups` | [docs](../../crates/modules/rustok-groups/docs/README.md) | [plan](../../crates/modules/rustok-groups/docs/implementation-plan.md) |
| `rustok-reactions` | [docs](../../crates/modules/rustok-reactions/docs/README.md) | [plan](../../crates/modules/rustok-reactions/docs/implementation-plan.md) |
| `rustok-social-graph` | [docs](../../crates/modules/rustok-social-graph/docs/README.md) | [plan](../../crates/modules/rustok-social-graph/docs/implementation-plan.md) |
| `rustok-region` | [docs](../../crates/modules/rustok-region/docs/README.md) | [plan](../../crates/modules/rustok-region/docs/implementation-plan.md) |
| `rustok-pricing` | [docs](../../crates/modules/rustok-pricing/docs/README.md) | [plan](../../crates/modules/rustok-pricing/docs/implementation-plan.md) |
| `rustok-tax` | [docs](../../crates/modules/rustok-tax/docs/README.md) | [plan](../../crates/modules/rustok-tax/docs/implementation-plan.md) |
| `rustok-inventory` | [docs](../../crates/modules/rustok-inventory/docs/README.md) | [plan](../../crates/modules/rustok-inventory/docs/implementation-plan.md) |
| `rustok-order` | [docs](../../crates/modules/rustok-order/docs/README.md) | [plan](../../crates/modules/rustok-order/docs/implementation-plan.md) |
| `rustok-payment` | [docs](../../crates/modules/rustok-payment/docs/README.md) | [plan](../../crates/modules/rustok-payment/docs/implementation-plan.md) |
| `rustok-fulfillment` | [docs](../../crates/modules/rustok-fulfillment/docs/README.md) | [plan](../../crates/modules/rustok-fulfillment/docs/implementation-plan.md) |
| `rustok-commerce` | [docs](../../crates/modules/rustok-commerce/docs/README.md) | [plan](../../crates/modules/rustok-commerce/docs/implementation-plan.md) |
| `rustok-marketplace` | [docs](../../crates/modules/rustok-marketplace/docs/README.md) | [plan](../../crates/modules/rustok-marketplace/docs/implementation-plan.md) |
| `rustok-marketplace-seller` | [docs](../../crates/modules/rustok-marketplace-seller/docs/README.md) | [plan](../../crates/modules/rustok-marketplace-seller/docs/implementation-plan.md) |
| `rustok-marketplace-listing` | [docs](../../crates/modules/rustok-marketplace-listing/docs/README.md) | [plan](../../crates/modules/rustok-marketplace-listing/docs/implementation-plan.md) |
| `rustok-marketplace-allocation` | [docs](../../crates/modules/rustok-marketplace-allocation/docs/README.md) | [plan](../../crates/modules/rustok-marketplace-allocation/docs/implementation-plan.md) |
| `rustok-marketplace-commission` | [docs](../../crates/modules/rustok-marketplace-commission/docs/README.md) | [plan](../../crates/modules/rustok-marketplace-commission/docs/implementation-plan.md) |
| `rustok-marketplace-ledger` | [docs](../../crates/modules/rustok-marketplace-ledger/docs/README.md) | [plan](../../crates/modules/rustok-marketplace-ledger/docs/implementation-plan.md) |
| `rustok-marketplace-payout` | [docs](../../crates/modules/rustok-marketplace-payout/docs/README.md) | [plan](../../crates/modules/rustok-marketplace-payout/docs/implementation-plan.md) |
| `rustok-moderation` | [docs](../../crates/modules/rustok-moderation/docs/README.md) | [plan](../../crates/modules/rustok-moderation/docs/implementation-plan.md) |
| `rustok-blog` | [docs](../../crates/modules/rustok-blog/docs/README.md) | [plan](../../crates/modules/rustok-blog/docs/implementation-plan.md) |
| `rustok-comments` | [docs](../../crates/modules/rustok-comments/docs/README.md) | [plan](../../crates/modules/rustok-comments/docs/implementation-plan.md) |
| `rustok-forum` | [docs](../../crates/modules/rustok-forum/docs/README.md) | [plan](../../crates/modules/rustok-forum/docs/implementation-plan.md) |
| `rustok-notifications` | [docs](../../crates/modules/rustok-notifications/docs/README.md) | [plan](../../crates/modules/rustok-notifications/docs/implementation-plan.md) |
| `rustok-pages` | [docs](../../crates/modules/rustok-pages/docs/README.md) | [plan](../../crates/modules/rustok-pages/docs/implementation-plan.md) |
| `rustok-navigation` | [docs](../../crates/modules/rustok-navigation/docs/README.md) | [plan](../../crates/modules/rustok-navigation/docs/implementation-plan.md) |
| `rustok-page-builder` | [docs](../../crates/modules/rustok-page-builder/docs/README.md) | [plan](../../crates/modules/rustok-page-builder/docs/implementation-plan.md) |
| `rustok-seo` | [docs](../../crates/modules/rustok-seo/docs/README.md) | [plan](../../crates/modules/rustok-seo/docs/implementation-plan.md) |
| `rustok-taxonomy` | [docs](../../crates/modules/rustok-taxonomy/docs/README.md) | [plan](../../crates/modules/rustok-taxonomy/docs/implementation-plan.md) |
| `rustok-media` | [docs](../../crates/modules/rustok-media/docs/README.md) | [plan](../../crates/modules/rustok-media/docs/implementation-plan.md) |
| `rustok-workflow` | [docs](../../crates/modules/rustok-workflow/docs/README.md) | [plan](../../crates/modules/rustok-workflow/docs/implementation-plan.md) |
| `rustok-translation` | [docs](../../crates/modules/rustok-translation/docs/README.md) | [plan](../../crates/modules/rustok-translation/docs/implementation-plan.md) |
| `rustok-translation-admin` | [admin UI package](../../crates/modules/rustok-translation/admin/README.md) | [owner plan](../../crates/modules/rustok-translation/docs/implementation-plan.md) |

## Module UI Packages

### Optional/Admin Surfaces

- `rustok-groups`: [README](../../crates/modules/rustok-groups/admin/README.md)
- `rustok-product`: [README](../../crates/modules/rustok-product/admin/README.md)
- `rustok-pages`: [README](../../crates/modules/rustok-pages/admin/README.md)
- `rustok-blog`: [README](../../crates/modules/rustok-blog/admin/README.md)
- `rustok-forum`: [README](../../crates/modules/rustok-forum/admin/README.md)
- `rustok-notifications`: [README](../../crates/modules/rustok-notifications/admin/README.md)
- `rustok-commerce`: [README](../../crates/modules/rustok-commerce/admin/README.md)
- `rustok-translation`: [README](../../crates/modules/rustok-translation/admin/README.md);
  the manifest-mounted Leptos package and matching Next package share one
  module-owned operation and URL-state contract.
- `rustok-marketplace-seller`: module-owned package in
  [`admin/`](../../crates/modules/rustok-marketplace-seller/admin/)
- `rustok-marketplace-listing`: module-owned package in
  [`admin/`](../../crates/modules/rustok-marketplace-listing/admin/)
- Additional owner packages are listed in [UI Packages Index](./UI_PACKAGES_INDEX.md).

### Optional/Storefront Surfaces

- `rustok-groups`: [README](../../crates/modules/rustok-groups/storefront/README.md)
- `rustok-blog`: [README](../../crates/modules/rustok-blog/storefront/README.md)
- Comments storefront authoring support: [README](../../crates/modules/rustok-comments-storefront-support/README.md)
- `rustok-forum`: [README](../../crates/modules/rustok-forum/storefront/README.md)
- `rustok-notifications`: [README](../../crates/modules/rustok-notifications/storefront/README.md)
- `rustok-pages`: [README](../../crates/modules/rustok-pages/storefront/README.md)
- Additional owner packages are listed in [UI Packages Index](./UI_PACKAGES_INDEX.md).

## Related Documents

- [Module Platform Overview](./overview.md)
- [Module and Application Registry](./registry.md)
- [Implementation Plans Registry](./implementation-plans-registry.md)
- [Module UI Packages Index](./UI_PACKAGES_INDEX.md)
- [`rustok-module.toml` Contract](./manifest.md)
