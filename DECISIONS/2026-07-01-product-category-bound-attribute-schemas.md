# Product category-bound attribute schemas

## Status

Accepted

## Context

RusToK needs a native ecommerce product model where operators choose a product
category and immediately get the correct product form. A mandatory
Magento-style attribute set flow would make simple catalog work heavier than
needed, but a pure category-only model would make schema reuse and stable
non-navigation product types harder.

## Decision

`rustok-product` uses `product_attributes` as the single attribute dictionary
and resolves product forms from `products.primary_category_id`.

Categories may:

- inherit the effective schema from a parent category;
- use an optional reusable `product_attribute_schema`;
- clone another category's effective schema as a snapshot;
- define a custom local attribute set through category bindings.

Additional product categories are navigation/merchandising assignments and do
not affect the product form. Detached values are preserved when the primary
category changes, but only effective attributes participate in validation,
storefront output, search, facets, and sorting.

## Consequences

- Attribute codes are tenant-unique and immutable after creation.
- Requiredness belongs to schema/category bindings, not global attributes.
- Local category overrides never mutate the global attribute definition.
- Index/search consume denormalized projections, not write-side tables directly.
- Localized labels and text-like values use dedicated translation tables.
