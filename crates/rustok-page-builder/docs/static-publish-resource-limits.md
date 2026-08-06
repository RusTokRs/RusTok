# Page Builder Reviewed Static Publish Resource Limits

Date: 2026-08-06  
Status: `source-ready / maintainer-validation-pending`

## Purpose

The reviewed Page Builder publication path already applies structural Fly validation and a fail-closed HTML, CSS, URL, attribute, metadata and public-resource policy before runtime materialization.

This slice adds provider-owned global resource budgets to the same pre-materialization sanitization seam. It does not add a second sanitizer, parser, renderer or persistence path.

## Current limits

```text
serialized prepared project: 16 MiB
pages: 128
component nodes: 50,000
component depth: 128
assets: 4,096
style rules: 20,000
```

Existing per-value policy limits remain separate and continue to cover content, attribute names and values, URLs, CSS properties and values, and media queries.

## Authority

The resource policy is implemented in:

```text
crates/rustok-page-builder/src/static_publish_resource_limits.rs
```

It measures the compiler-prepared current Fly project. Component count and depth traverse only the current `pages[].component` authority. Obsolete frame trees are not consulted or synchronized.

The authoritative call remains:

```text
sanitize_static_landing_project
```

Resource validation occurs after current Fly preparation and static publish policy validation, and before sanitized-project hashing, runtime materialization or immutable artifact creation.

The same resource validation is repeated when the transient sanitization envelope verifies its integrity immediately before materialization.

## Sanitization identity

The existing contract remains unchanged:

```text
page_builder_static_publish_sanitization_v2
```

Its SHA-256 payload remains exactly:

```text
format
policy_format
policy_hash
sanitized_project
```

No new persisted DTO or database field is introduced. Pages continues to retain the resulting `sanitized_hash` inside the existing locale-ordered sanitized-set identity.

Global budgets are an additional fail-closed admission condition on the reviewed sanitization operation. They do not rewrite historical immutable artifacts or alter the sanitization hash schema.

## Resource policy evidence

The provider-owned resource policy has its own deterministic SHA-256 identity and bounded observation DTO covering:

- prepared-project bytes;
- page count;
- component count;
- maximum component depth;
- asset count;
- style-rule count.

This DTO is used by the validator and focused source/tests. It is not a new public transport or persistence schema.

## Typed rejection

Each exceeded budget produces a bounded diagnostic with a stable code and path:

```text
landing_project_bytes_exceeded
landing_page_count_exceeded
landing_component_count_exceeded
landing_component_depth_exceeded
landing_asset_count_exceeded
landing_style_rule_count_exceeded
```

No raw runtime context, credentials, tenant secrets or public artifact payloads are added to the evidence contract.

## Preserved boundaries

This source slice does not change:

- Pages metadata or document persistence;
- publish transaction ownership or idempotency;
- sanitization format or hash payload;
- runtime scenario selection;
- public route, cache or artifact schemas;
- anonymous storefront rendering;
- inline authoring grants or save transport;
- database migrations, GraphQL, REST or event schemas;
- deployment, workflow or rollout behavior.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-page-builder/scripts/verify/verify-page-builder-static-publish-resource-limits.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-publish-runtime-review.mjs
cargo test -p rustok-page-builder static_publish_resource_limits -- --nocapture
cargo test -p rustok-page-builder publish_sanitization -- --nocapture
cargo check -p rustok-page-builder
```

Real-project, publish/materialization, workflow and tenant evidence remain pending.
