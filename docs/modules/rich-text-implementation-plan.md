---
id: doc://docs/modules/rich-text-implementation-plan.md
kind: development_plan
language: en
status: active
---
# Rich Text Implementation Plan

## Current verified state

Rich text is the owner-neutral content representation track. `rt_json_v1` is the platform format; editor integration and content consumers must preserve locale, tenant, validation, and rendering contracts without making Tiptap or a host application the source of truth.

## Next priorities

1. Keep `rt_json_v1` validation, serialization, and rendering behavior aligned across owner modules. Completion: contract fixtures cover accepted and rejected documents.
2. Migrate remaining Markdown-only content paths through owner-owned conversion and preview flows. Completion: each migrated surface has a reversible content migration and rendering verification.
3. Add editor capabilities only through the shared document contract and module-owned UI surfaces. Completion: no editor-specific schema leaks into unrelated modules or hosts.

## Dependencies and verification

- Dependency: Page Builder consumes rendered content but does not own the rich-text document model; module owners retain their content contracts.
- Targeted verification: `npm run verify:reference-artifacts`.
