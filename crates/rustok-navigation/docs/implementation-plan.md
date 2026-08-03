# Navigation Implementation Plan

## Scope

Extract the navigation owner from Pages and complete an independent admin/storefront lifecycle.

## Current State

- Domain persistence, exact-locale reads and channel/location bindings are owned here.
- GraphQL, HTTP and storefront slot components are module-owned.
- The registered `navigation/menu` Translation target exposes an atomic locale
  aggregate: the menu name and every nested item title, with resource/source/
  target CAS, shared durable receipt replay, and a content-free owner cursor.
- Navigation has no generic menu-change outbox event. Its translation journal is
  repair evidence only and is committed atomically with the owner locale apply.
- Admin authoring UI remains the next slice.

## Milestones

1. Owner extraction and transport composition.
2. Navigation admin list/editor and active binding controls.
3. Schema normalization that removes unused historical `page_id` storage.
4. Retained PostgreSQL migration, concurrent CAS, and cursor-recovery evidence
   before enabling the Navigation Translation pilot in production.

## Verification

Run focused build, migration, GraphQL, HTTP, native storefront and browser checks.

## Update Rules

Update this plan whenever ownership, schema, transport or slot semantics change.
