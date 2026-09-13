---
id: doc://docs/modules/translation-readiness-evidence-2026-09-13.md
kind: readiness_handoff
language: en
status: active
---

# Translation retained-evidence handoff — 2026-09-13

This handoff records the repository state after the retained PostgreSQL evidence and Product Attribute slices merged through `main@670e0be29df1d3878ab608382fd2ab0d5aa7ad8f`.

It does **not** promote any Translation surface by itself. A retained test/workflow in the repository is source evidence only until the focused workflow has actually run green on the intended exact head and the resulting evidence has been reviewed/retained. No green run is claimed here for the slices below.

## Newly retained source evidence

| PR | Surface | Retained PostgreSQL test | Retained workflow | Readiness consequence |
| --- | --- | --- | --- | --- |
| #3962 | `commerce/collection_copy` | `apps/server/tests/commerce_collection_translation_target_postgres.rs` | `.github/workflows/commerce-collection-translation-target-postgres.yml` | stays `blocked` pending green exact-head/reviewed post-merge evidence |
| #3963 | `product/product` | `apps/server/tests/product_product_translation_target_postgres.rs` | `.github/workflows/product-product-translation-target-postgres.yml` | stays `blocked` pending green exact-head/reviewed post-merge evidence |
| #3964 | `product/variant` | `apps/server/tests/product_variant_translation_target_postgres.rs` | `.github/workflows/product-variant-translation-target-postgres.yml` | stays `blocked` pending green exact-head/reviewed post-merge evidence |
| #3965 | `product/option` | `apps/server/tests/product_option_translation_target_postgres.rs` | `.github/workflows/product-option-translation-target-postgres.yml` | stays `blocked` pending green exact-head/reviewed post-merge evidence |
| #3966 | `product/image` | `apps/server/tests/product_image_translation_target_postgres.rs` | `.github/workflows/product-image-translation-target-postgres.yml` | stays `blocked` pending green exact-head/reviewed post-merge evidence |
| #3967 | `seo/seo_copy` | `apps/server/tests/seo_translation_target_postgres.rs` | `.github/workflows/seo-translation-target-postgres.yml` | stays `blocked` pending green exact-head/reviewed post-merge evidence |
| #3968 | `flex/schema_copy` | `apps/server/tests/flex_schema_translation_target_postgres.rs` | `.github/workflows/flex-schema-copy-translation-target-postgres.yml` | stays `blocked` pending green exact-head/reviewed post-merge evidence |
| #3969 | `product/attribute` | `apps/server/tests/product_attribute_translation_target_postgres.rs` | `.github/workflows/product-attribute-translation-target-postgres.yml` | newly registered; stays `blocked` pending green exact-head/reviewed post-merge evidence |

All focused workflows above are retained as manual `workflow_dispatch` evidence gates. They were not dispatched as part of these implementation/doc slices.

## Product parity after #3969

`product/attribute` is now a canonical registered Translation target over `ProductCatalogSchemaService`; Attribute label/help/facet/SEO copy and active option labels are no longer an unclassified broad-Product gap.

Broad `product_catalog` remains intentionally separate and blocked. Remaining parity work must classify and onboard the still-uncovered Product-owned presentation surfaces—especially category/schema/group presentation labels and any remaining SEO/Flex-integrated catalog copy—without inventing a synthetic `product/catalog_product` aggregate provider or duplicating ownership already covered by `product/product`, `product/variant`, `product/option`, `product/image`, and `product/attribute`.

## Promotion rule

For every registered surface still marked `blocked`, keep these states separate:

1. **provider registered** — production composition can resolve the target;
2. **retained source present** — repository contains a focused PostgreSQL evidence test/workflow;
3. **retained run green** — exact-head evidence actually passed and is reviewable;
4. **pilot candidate** — readiness registry may be promoted only after the required evidence and remaining owner/lifecycle gates are satisfied.

Do not infer step 3 or 4 from step 2.
