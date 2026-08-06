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

Resource validation occurs after current Fly preparation and static publish policy validation, and before runtime materialization or immutable artifact creation.

## Evidence and integrity

New sanitization outputs use:

```text
page_builder_static_publish_sanitization_v3
```

The v3 sanitization hash binds:

- static HTML/CSS/URL/attribute policy format and hash;
- resource-limit format and hash;
- observed project bytes, page count, component count, maximum depth, asset count and style-rule count;
- the sanitized current Fly project.

Integrity verification recomputes both static policy evidence and resource evidence from the retained sanitized project.

## Legacy compatibility

Existing immutable evidence using:

```text
page_builder_static_publish_sanitization_v2
```

remains verifiable with the exact prior hash formula. Legacy v2 packets must not contain v3 resource evidence and do not receive retroactive rejection under the new budgets.

This preserves already published immutable artifacts while requiring every newly sanitized reviewed publication to carry v3 resource evidence.

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
cargo test -p rustok-page-builder static_publish_resource_limits -- --nocapture
cargo test -p rustok-page-builder publish_sanitization -- --nocapture
cargo check -p rustok-page-builder
```

Real-project, publish/materialization, workflow and tenant evidence remain pending.
