# ADR: Alloy scaffold review/apply boundary in `rustok-mcp`

## Status

Accepted

## Context

`alloy_scaffold_module` introduced the first real `AI -> MCP -> Alloy -> Platform` capability, but
it still allowed direct file writes. That meant there was no explicit boundary between generation,
review, and apply.

## Decision

We split the scaffold flow into three MCP tools:

- `alloy_scaffold_module` stages a draft scaffold and returns a draft id;
- `alloy_review_module_scaffold` returns the staged draft for inspection;
- `alloy_apply_module_scaffold` writes the reviewed scaffold into the workspace only with `confirm=true`.

This boundary currently lives in the in-memory `rustok-mcp` runtime through `AlloyScaffoldState`.

## Consequences

Positive:

- generated code no longer goes straight from request to disk write;
- orchestrators and humans now have an explicit review step;
- the flow is a better foundation for a future persisted control plane.

Constraint:

- draft state is still in-memory and process-local;
- this is not yet a persisted server-side review system.

## Next steps

1. Persist staged drafts in the server control plane.
2. Add admin UI for Alloy draft review and apply.
3. Extend the flow toward richer codegen and publication pipelines.
