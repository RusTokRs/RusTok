# Navigation Implementation Plan

## Scope

Extract the navigation owner from Pages and complete an independent admin/storefront lifecycle.

## Current State

- Domain persistence, exact-locale reads and channel/location bindings are owned here.
- GraphQL, HTTP and storefront slot components are module-owned.
- Admin authoring UI remains the next slice.

## Milestones

1. Owner extraction and transport composition.
2. Navigation admin list/editor and active binding controls.
3. Schema normalization that removes unused historical `page_id` storage.
4. Cache invalidation and operational evidence.

## Verification

Run focused build, migration, GraphQL, HTTP, native storefront and browser checks.

## Update Rules

Update this plan whenever ownership, schema, transport or slot semantics change.
