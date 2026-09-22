---
id: doc://docs/modules/translation-readiness-evidence-2026-09-13.md
kind: readiness_handoff
language: en
status: active
---

# Translation retained-evidence handoff — 2026-09-13

This handoff records the repository state after the retained PostgreSQL evidence and Product catalog slices merged through `main@3af67e63ef05e226863b1fe7d4380ec84f831995` (#3972).

It does **not** promote any Translation surface by itself. A retained test/workflow in the repository is source evidence only until the focused workflow has actually run green on the intended exact head and the resulting evidence has been reviewed/retained. No new green run is claimed here for the slices below.

## Retained source evidence reconciled on 2026-09-13

| PR | Surface | Retained PostgreSQL test | Retained workflow | Readiness consequence |
| --- | --- | --- | --- | --- |
| #3956 | `fulfillment/shipping_option_copy` | `apps/server/tests/fulfillment_shipping_option_translation_target_postgres.rs` | `.github/workflows/fulfillment-shipping-option-translation-target-postgres.yml` | stays `blocked` pending green exact-head/reviewed post-merge evidence |
| #3957 | `inventory/stock_location_copy` | `apps/server/tests/inventory_stock_location_translation_target_postgres.rs` | `.github/workflows/inventory-stock-location-translation-target-postgres.yml` | stays `blocked` pending green exact-head/reviewed post-merge evidence |
| #3958 | `region/region_copy` | `apps/server/tests/region_translation_target_postgres.rs` | `.github/workflows/region-translation-target-postgres.yml` | stays `blocked` pending green exact-head/reviewed post-merge evidence |
| #3959 | `pricing/price_list_copy` | `apps/server/tests/pricing_price_list_translation_target_postgres.rs` | `.github/workflows/pricing-price-list-translation-target-postgres.yml` | stays `blocked` pending green exact-head/reviewed post-merge evidence |
| #3962 | `commerce/collection_copy` | `apps/server/tests/commerce_collection_translation_target_postgres.rs` | `.github/workflows/commerce-collection-translation-target-postgres.yml` | stays `blocked` pending green exact-head/reviewed post-merge evidence |
| #3963 | `product/product` | `apps/server/tests/product_product_translation_target_postgres.rs` | `.github/workflows/product-product-translation-target-postgres.yml` | stays `blocked` pending green exact-head/reviewed post-merge evidence |
| #3964 | `product/variant` | `apps/server/tests/product_variant_translation_target_postgres.rs` | `.github/workflows/product-variant-translation-target-postgres.yml` | stays `blocked` pending green exact-head/reviewed post-merge evidence |
| #3965 | `product/option` | `apps/server/tests/product_option_translation_target_postgres.rs` | `.github/workflows/product-option-translation-target-postgres.yml` | stays `blocked` pending green exact-head/reviewed post-merge evidence |
| #3966 | `product/image` | `apps/server/tests/product_image_translation_target_postgres.rs` | `.github/workflows/product-image-translation-target-postgres.yml` | stays `blocked` pending green exact-head/reviewed post-merge evidence |
| #3967 | `seo/seo_copy` | `apps/server/tests/seo_translation_target_postgres.rs` | `.github/workflows/seo-translation-target-postgres.yml` | stays `blocked` pending green exact-head/reviewed post-merge evidence |
| #3968 | `flex/schema_copy` | `apps/server/tests/flex_schema_translation_target_postgres.rs` | `.github/workflows/flex-schema-copy-translation-target-postgres.yml` | stays `blocked` pending green exact-head/reviewed post-merge evidence |
| #3969 | `product/attribute` | `apps/server/tests/product_attribute_translation_target_postgres.rs` | `.github/workflows/product-attribute-translation-target-postgres.yml` | stays `blocked` pending green exact-head/reviewed post-merge evidence |
| #3971 | `product/attribute_schema` | `apps/server/tests/product_attribute_schema_translation_target_postgres.rs` | `.github/workflows/product-attribute-schema-translation-target-postgres.yml` | newly registered; stays `blocked` pending green exact-head/reviewed post-merge evidence |
| #3972 | `product/category_form` | `apps/server/tests/product_category_form_translation_target_postgres.rs` | `.github/workflows/product-category-form-translation-target-postgres.yml` | newly registered; stays `blocked` pending green exact-head/reviewed post-merge evidence |

All focused workflows above are retained as manual evidence gates. They were not dispatched as part of these implementation/doc slices.

## Product parity after #3972

The separately registered Product Translation targets now cover `product/product`, `product/variant`, `product/option`, `product/image`, `product/attribute`, `product/attribute_schema`, and `product/category_form`.

`product/attribute_schema` owns Attribute Schema name/description and schema-group labels over the canonical `ProductCatalogSchemaService`. `product/category_form` owns only Product-local category form-group labels. Canonical Product Category name, slug, description, hierarchy, and presentation remain Taxonomy-owned and must not be duplicated under Product.

Broad `product_catalog` remains intentionally separate and blocked. Remaining parity work must classify and onboard any still-uncovered Product-owned presentation surfaces, especially SEO/Flex-integrated catalog copy, without inventing a synthetic `product/catalog_product` aggregate provider or duplicating ownership already covered by the narrow targets.

## Promotion rule

For every registered surface still marked `blocked`, keep these states separate:

1. **provider registered** — production composition can resolve the target;
2. **retained source present** — repository contains a focused PostgreSQL evidence test/workflow;
3. **retained run green** — exact-head evidence actually passed and is reviewable;
4. **pilot candidate** — readiness registry may be promoted only after the required evidence and remaining owner/lifecycle gates are satisfied.

Do not infer step 3 or 4 from step 2.
