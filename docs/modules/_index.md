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

Module local documentation lives inside the crates themselves at
`crates/<name>/docs/README.md`. This document provides central navigation only.

## Navigation Rule

- Module documentation is not duplicated in `docs/modules/`.
- Links below lead directly to `crates/<name>/docs/`.
- Platform modules require `README.md`, `docs/README.md`, and
  `docs/implementation-plan.md`.

## Core and Foundation Layer

| Component | Documentation | Implementation Plan |
|---|---|---|
| `rustok-core` | [docs](../../crates/rustok-core/docs/README.md) | [plan](../../crates/rustok-core/docs/implementation-plan.md) |
| `rustok-events` | [docs](../../crates/rustok-events/docs/README.md) | [plan](../../crates/rustok-events/docs/implementation-plan.md) |
| `rustok-channel` | [docs](../../crates/rustok-channel/docs/README.md) | [plan](../../crates/rustok-channel/docs/implementation-plan.md) |
| `rustok-index` | [docs](../../crates/rustok-index/docs/README.md) | [plan](../../crates/rustok-index/docs/implementation-plan.md) |
| `rustok-search` | [docs](../../crates/rustok-search/docs/README.md) | [plan](../../crates/rustok-search/docs/implementation-plan.md) |
| `rustok-outbox` | [docs](../../crates/rustok-outbox/docs/README.md) | [plan](../../crates/rustok-outbox/docs/implementation-plan.md) |
| `rustok-telemetry` | [docs](../../crates/rustok-telemetry/docs/README.md) | [plan](../../crates/rustok-telemetry/docs/implementation-plan.md) |
| `rustok-tenant` | [docs](../../crates/rustok-tenant/docs/README.md) | [plan](../../crates/rustok-tenant/docs/implementation-plan.md) |
| `rustok-rbac` | [docs](../../crates/rustok-rbac/docs/README.md) | [plan](../../crates/rustok-rbac/docs/implementation-plan.md) |
| `rustok-cache` | [docs](../../crates/rustok-cache/docs/README.md) | [plan](../../crates/rustok-cache/docs/implementation-plan.md) |
| `rustok-auth` | [docs](../../crates/rustok-auth/docs/README.md) | [plan](../../crates/rustok-auth/docs/implementation-plan.md) |
| `rustok-email` | [docs](../../crates/rustok-email/docs/README.md) | [plan](../../crates/rustok-email/docs/implementation-plan.md) |
| `rustok-storage` | [docs](../../crates/rustok-storage/docs/README.md) | [plan](../../crates/rustok-storage/docs/implementation-plan.md) |
| `rustok-api` | [docs](../../crates/rustok-api/docs/README.md) | [plan](../../crates/rustok-api/docs/implementation-plan.md) |
| `rustok-runtime` | [docs](../../crates/rustok-runtime/docs/README.md) | [plan](../../crates/rustok-runtime/docs/implementation-plan.md) |
| `rustok-modules` | [docs](../../crates/rustok-modules/docs/README.md) | [plan](../../crates/rustok-modules/docs/implementation-plan.md) |
| `rustok-verification-transport` | [docs](../../crates/rustok-verification-transport/docs/README.md) | gRPC transport contract is recorded in the crate README. |
| `rustok-media-transport` | [docs](../../crates/rustok-media-transport/docs/README.md) | Loopback-verified gRPC adapter for Media-owned read/write ports. |
| `rustok-verification-worker` | [docs](../../crates/rustok-verification-worker/docs/README.md) | Worker rollout is recorded in the module control-plane plan. |
| `rustok-module-build-transport` | [docs](../../crates/rustok-module-build-transport/docs/README.md) | Current-only mTLS module and static-distribution build-worker transport is recorded in the module control-plane plan. |
| `rustok-module-build-worker` | [docs](../../crates/rustok-module-build-worker/docs/README.md) | Isolated build-worker rollout is recorded in the module control-plane plan. |
| `rustok-build-publication` | [docs](../../crates/rustok-build-publication/docs/README.md) | Shared current-only registry credential and Cosign publication boundary for isolated build workers. |
| `rustok-build-source` | [docs](../../crates/rustok-build-source/docs/README.md) | Shared strict immutable CAS source-archive materialization for build workers. |
| `rustok-static-distribution-worker` | [docs](../../crates/rustok-static-distribution-worker/docs/README.md) | Trusted native-distribution CI worker rollout is recorded in the module control-plane plan. |
| `rustok-module-build-dispatcher` | [docs](../../crates/rustok-module-build-dispatcher/docs/README.md) | Broker-neutral build-delivery contract is recorded in the module control-plane plan. |
| `rustok-worker-transport` | [docs](../../crates/rustok-worker-transport/docs/README.md) | Shared mutually authenticated worker-listener foundation. |
| `rustok-sandbox` | [docs](../../crates/rustok-sandbox/docs/README.md) | [plan](../../crates/rustok-sandbox/docs/implementation-plan.md) |
| `rustok-web` | [docs](../../crates/rustok-web/docs/README.md) | [plan](../../crates/rustok-web/docs/implementation-plan.md) |
| `rustok-fba` | [docs](../../crates/rustok-fba/docs/README.md) | [plan](../../crates/rustok-fba/docs/implementation-plan.md) |
| `rustok-cli-core` | [docs](../../crates/rustok-cli-core/docs/README.md) | [plan](../../crates/rustok-cli-core/docs/implementation-plan.md) |
| `rustok-cli-platform` | [docs](../../crates/rustok-cli-platform/docs/README.md) | [plan](../../crates/rustok-cli-platform/docs/implementation-plan.md) |
| `rustok-migrations` | [README](../../crates/rustok-migrations/README.md) | - |
| `rustok-installer` | [docs](../../crates/rustok-installer/docs/README.md) | [plan](../../crates/rustok-installer/docs/implementation-plan.md) |
| `rustok-build` | [docs](../../crates/rustok-build/docs/README.md) | [plan](../../crates/rustok-build/docs/implementation-plan.md) |
| `rustok-cli-registry` | [docs](../../crates/rustok-cli-registry/docs/README.md) | [plan](../../crates/rustok-cli-registry/docs/implementation-plan.md) |
| `rustok-distribution` | [docs](../../crates/rustok-distribution/docs/README.md) | Registry composition and deterministic static-promotion build output are documented locally. |
| `rustok-cli` | [docs](../../crates/rustok-cli/docs/README.md) | [plan](../../crates/rustok-cli/docs/implementation-plan.md) |
| `rustok-graphql` | [docs](../../crates/rustok-graphql/docs/README.md) | [plan](../../crates/rustok-graphql/docs/implementation-plan.md) |
| `rustok-ui-i18n` | [docs](../../crates/rustok-ui-i18n/docs/README.md) | [plan](../../crates/rustok-ui-i18n/docs/implementation-plan.md) |
| `rustok-test-utils` | [docs](../../crates/rustok-test-utils/docs/README.md) | [plan](../../crates/rustok-test-utils/docs/implementation-plan.md) |
| `rustok-iggy` | [docs](../../crates/rustok-iggy/docs/README.md) | [plan](../../crates/rustok-iggy/docs/implementation-plan.md) |
| `rustok-iggy-connector` | [docs](../../crates/rustok-iggy-connector/docs/README.md) | [plan](../../crates/rustok-iggy-connector/docs/implementation-plan.md) |
| `rustok-mcp` | [docs](../../crates/rustok-mcp/docs/README.md) | [plan](../../crates/rustok-mcp/docs/implementation-plan.md) |
| `rustok-ai` | [docs](../../crates/rustok-ai/docs/README.md) | [plan](../../crates/rustok-ai/docs/implementation-plan.md) |
| `rustok-ai-content` | [docs](../../crates/rustok-ai-content/docs/README.md) | [plan](../../crates/rustok-ai-content/docs/implementation-plan.md) |
| `rustok-ai-product` | [docs](../../crates/rustok-ai-product/docs/README.md) | [plan](../../crates/rustok-ai-product/docs/implementation-plan.md) |
| `rustok-ai-order` | [docs](../../crates/rustok-ai-order/docs/README.md) | [plan](../../crates/rustok-ai-order/docs/implementation-plan.md) |
| `rustok-ai-media` | [docs](../../crates/rustok-ai-media/docs/README.md) | [plan](../../crates/rustok-ai-media/docs/implementation-plan.md) |
| `rustok-ai-alloy` | [docs](../../crates/rustok-ai-alloy/docs/README.md) | [plan](../../crates/rustok-ai-alloy/docs/implementation-plan.md) |
| `alloy` | [docs](../../crates/alloy/docs/README.md) | [plan](../../crates/alloy/docs/implementation-plan.md) |
| `flex` | [docs](../../crates/flex/docs/README.md) | [plan](../../crates/flex/docs/implementation-plan.md) |
| `rustok-commerce-foundation` | [docs](../../crates/rustok-commerce-foundation/docs/README.md) | [plan](../../crates/rustok-commerce-foundation/docs/implementation-plan.md) |
| `rustok-seo-render` | [docs](../../crates/rustok-seo/render/docs/README.md) | [plan](../../crates/rustok-seo/render/docs/implementation-plan.md) |
| `rustok-seo-admin-support` | [docs](../../crates/rustok-seo-admin-support/docs/README.md) | [plan](../../crates/rustok-seo-admin-support/docs/implementation-plan.md) |

## Domain Modules

| Component | Documentation | Implementation Plan |
|---|---|---|
| `rustok-content` | [docs](../../crates/rustok-content/docs/README.md) | [plan](../../crates/rustok-content/docs/implementation-plan.md) |
| `rustok-cart` | [docs](../../crates/rustok-cart/docs/README.md) | [plan](../../crates/rustok-cart/docs/implementation-plan.md) |
| `rustok-customer` | [docs](../../crates/rustok-customer/docs/README.md) | [plan](../../crates/rustok-customer/docs/implementation-plan.md) |
| `rustok-product` | [docs](../../crates/rustok-product/docs/README.md) | [plan](../../crates/rustok-product/docs/implementation-plan.md) |
| `rustok-profiles` | [docs](../../crates/rustok-profiles/docs/README.md) | [plan](../../crates/rustok-profiles/docs/implementation-plan.md) |
| `rustok-groups` | [docs](../../crates/rustok-groups/docs/README.md) | [plan](../../crates/rustok-groups/docs/implementation-plan.md) |
| `rustok-region` | [docs](../../crates/rustok-region/docs/README.md) | [plan](../../crates/rustok-region/docs/implementation-plan.md) |
| `rustok-pricing` | [docs](../../crates/rustok-pricing/docs/README.md) | [plan](../../crates/rustok-pricing/docs/implementation-plan.md) |
| `rustok-tax` | [docs](../../crates/rustok-tax/docs/README.md) | [plan](../../crates/rustok-tax/docs/implementation-plan.md) |
| `rustok-inventory` | [docs](../../crates/rustok-inventory/docs/README.md) | [plan](../../crates/rustok-inventory/docs/implementation-plan.md) |
| `rustok-order` | [docs](../../crates/rustok-order/docs/README.md) | [plan](../../crates/rustok-order/docs/implementation-plan.md) |
| `rustok-payment` | [docs](../../crates/rustok-payment/docs/README.md) | [plan](../../crates/rustok-payment/docs/implementation-plan.md) |
| `rustok-fulfillment` | [docs](../../crates/rustok-fulfillment/docs/README.md) | [plan](../../crates/rustok-fulfillment/docs/implementation-plan.md) |
| `rustok-commerce` | [docs](../../crates/rustok-commerce/docs/README.md) | [plan](../../crates/rustok-commerce/docs/implementation-plan.md) |
| `rustok-blog` | [docs](../../crates/rustok-blog/docs/README.md) | [plan](../../crates/rustok-blog/docs/implementation-plan.md) |
| `rustok-comments` | [docs](../../crates/rustok-comments/docs/README.md) | [plan](../../crates/rustok-comments/docs/implementation-plan.md) |
| `rustok-forum` | [docs](../../crates/rustok-forum/docs/README.md) | [plan](../../crates/rustok-forum/docs/implementation-plan.md) |
| `rustok-notifications` | [docs](../../crates/rustok-notifications/docs/README.md) | [plan](../../crates/rustok-notifications/docs/implementation-plan.md) |
| `rustok-pages` | [docs](../../crates/rustok-pages/docs/README.md) | [plan](../../crates/rustok-pages/docs/implementation-plan.md) |
| `rustok-navigation` | [docs](../../crates/rustok-navigation/docs/README.md) | [plan](../../crates/rustok-navigation/docs/implementation-plan.md) |
| `rustok-page-builder` | [docs](../../crates/rustok-page-builder/docs/README.md) | [plan](../../crates/rustok-page-builder/docs/implementation-plan.md) |
| `rustok-seo` | [docs](../../crates/rustok-seo/docs/README.md) | [plan](../../crates/rustok-seo/docs/implementation-plan.md) |
| `rustok-taxonomy` | [docs](../../crates/rustok-taxonomy/docs/README.md) | [plan](../../crates/rustok-taxonomy/docs/implementation-plan.md) |
| `rustok-media` | [docs](../../crates/rustok-media/docs/README.md) | [plan](../../crates/rustok-media/docs/implementation-plan.md) |
| `rustok-workflow` | [docs](../../crates/rustok-workflow/docs/README.md) | [plan](../../crates/rustok-workflow/docs/implementation-plan.md) |

## Module UI Packages

### Optional/Admin Surfaces

- `rustok-groups`: [README](../../crates/rustok-groups/admin/README.md)
- `rustok-product`: [README](../../crates/rustok-product/admin/README.md)
- `rustok-pages`: [README](../../crates/rustok-pages/admin/README.md)
- `rustok-blog`: [README](../../crates/rustok-blog/admin/README.md)
- `rustok-forum`: [README](../../crates/rustok-forum/admin/README.md)
- `rustok-notifications`: [README](../../crates/rustok-notifications/admin/README.md)
- `rustok-commerce`: [README](../../crates/rustok-commerce/admin/README.md)
- Additional owner packages are listed in [UI Packages Index](./UI_PACKAGES_INDEX.md).

### Optional/Storefront Surfaces

- `rustok-groups`: [README](../../crates/rustok-groups/storefront/README.md)
- `rustok-blog`: [README](../../crates/rustok-blog/storefront/README.md)
- `rustok-forum`: [README](../../crates/rustok-forum/storefront/README.md)
- `rustok-notifications`: [README](../../crates/rustok-notifications/storefront/README.md)
- `rustok-pages`: [README](../../crates/rustok-pages/storefront/README.md)
- Additional owner packages are listed in [UI Packages Index](./UI_PACKAGES_INDEX.md).

## Related Documents

- [Module Platform Overview](./overview.md)
- [Module and Application Registry](./registry.md)
- [Implementation Plans Registry](./implementation-plans-registry.md)
- [Module UI Packages Index](./UI_PACKAGES_INDEX.md)
- [`rustok-module.toml` Contract](./manifest.md)
