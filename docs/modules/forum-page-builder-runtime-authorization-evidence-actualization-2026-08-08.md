# Forum / Page Builder runtime authorization evidence actualization — 2026-08-08

Status: `source-ready / maintainer-runtime-execution-pending / browser-execution-pending / wave-pending`

## Rechecked cursor

PR #3264 retained the browser-evidence harness for the real Pages admin contribution flow. Browser execution was deliberately not performed. The next FORUM-32 source cursor was direct runtime authorization evidence for the Forum-owned preview/property boundary.

This slice makes that runtime evidence source-ready without claiming that it has executed.

## Shared transport authorization

Forum preview and property native transports now use one production authorization helper before owner work begins.

The shared gate requires:

1. `AuthContext.tenant_id == TenantContext.id`;
2. effective `forum_topics:read` through the platform `has_any_effective_permission` helper;
3. an exact enabled `forum` tenant-module row through `is_tenant_module_enabled`.

The effective permission rule is the platform rule, not a Forum-local implication table. Therefore `forum_topics:manage` satisfies `forum_topics:read` through the ordinary same-resource `manage -> read` behavior, while an empty or unrelated permission set fails closed.

Both owner-property schema/validation server functions and the owner-preview server function cross this same gate. Property transport no longer carries a duplicate effective-permission check.

## Tenant module evidence

`rustok-api::is_tenant_module_enabled` remains the production database query used by GraphQL/native transport adapters.

A retained SQLite test source now exercises its exact scope:

- the current tenant's enabled `forum` row is admitted;
- a disabled module row is rejected;
- a `forum` row owned by another tenant does not satisfy the request;
- a missing module row does not satisfy the request.

The Forum transport maps disabled and lookup-error states to explicit fail-closed server-function errors.

## Owner moderator preview evidence

`ForumWidgetPreviewService` keeps reply preview policy inside Forum.

`approved_only=true` returns only the `Approved` status set and does not require moderation authority.

`approved_only=false` requires effective `forum_replies:moderate`. A same-resource `Manage` permission satisfies that effective scope through the existing security model.

The moderator status set is explicitly:

```text
Pending
Approved
Rejected
Hidden
Flagged
```

`Deleted` is intentionally absent, so soft-delete/tombstone rows cannot become moderator Page Builder preview data.

## Owner visibility evidence

The retained SQLite integration source calls the public production `ForumTopicVisibilityService::filter_visible_topic_ids` path rather than copying visibility decisions into a test helper.

Its fixture contains:

- a public category;
- an authenticated category;
- a child inheriting that authenticated visibility floor;
- a closed current-tenant topic;
- a foreign-tenant topic.

The anonymous storefront scope sees only the current-tenant open topic in the public category. The authenticated scope may also see the authenticated category and its descendant, while closed and foreign-tenant topics remain excluded.

This is the same category floor consumed by the Forum widget topic-list preview before the bounded owner query executes.

## Retained execution contract

The runtime contract is:

```text
crates/rustok-forum/contracts/evidence/forum-page-builder-runtime-authorization-execution-contract.json
```

The runner is:

```text
scripts/evidence/forum-page-builder-runtime-authorization-evidence.mjs
```

A successful maintainer execution may write only:

```text
format: forum_page_builder_runtime_authorization_execution_v1
status: runtime_authorization_execution_passed_wave_pending
```

The runner executes four bounded `cargo test` commands declared by the contract:

1. Forum admin transport authorization source tests;
2. exact tenant-module runtime lookup evidence;
3. Forum owner moderator preview policy tests;
4. Forum owner topic/category visibility SQLite evidence.

The runner uses `spawnSync` without a shell, removes any prior success packet before execution, hashes required source files at the exact checkout `HEAD`, and writes a success packet only after every command exits successfully.

Raw command output is not retained. The packet stores only command identity/argv, exit status, stdout/stderr byte lengths and SHA-256 hashes, exact source commit and required-source SHA-256 hashes.

## What this packet does not prove

Even after a future successful local/runtime harness execution, the packet does not independently prove deployed server-function transport attestation. It does not send authenticated HTTP requests to a deployed host and does not retain cookies, Authorization headers, tenant ids or actor ids.

That distinction is intentional: the source-level runtime evidence proves the production authorization/owner policy seams under execution, while environment/deployment transport attestation remains a separate reviewed evidence boundary.

The browser packet from #3264 also remains execution-pending. Provider SLO health remains `unobserved`; no source in this slice converts missing health observation into a healthy claim.

## Source-only guard

The source guard is:

```text
node scripts/verify/verify-forum-page-builder-runtime-authorization-evidence.mjs
```

It checks the command matrix, privacy/retention boundary, shared transport authorization, generic effective-permission source, exact module-state test, moderator `Deleted` exclusion and owner visibility SQLite source.

## Maintainer execution cursor

When reviewed execution inputs/environment are available, maintainers can run:

```text
node scripts/evidence/forum-page-builder-runtime-authorization-evidence.mjs
```

The browser harness retained by #3264 remains a separate command and packet. Acceptance should correlate the exact source revision used for both retained packets before any Forum Page Builder Wave claim.

## Promotion boundary

FORUM-32 remains `in_progress`.

Source is now ready for:

- contribution metadata/Fly identity;
- Forum owner preview;
- Forum owner-backed properties;
- browser evidence harness;
- direct runtime authorization/visibility evidence harness.

Still open:

1. execute and retain the Forum browser packet;
2. execute and retain this runtime authorization packet;
3. retain any required deployed transport provenance/attestation separately;
4. satisfy the existing Pages reference-consumer gate;
5. only then evaluate observed Forum Page Builder Wave evidence.

No runtime, Cargo, browser, database or verifier execution is claimed by this source slice. No tests, Cargo commands, Node verifiers, formatters, builds, workflows, CI, database fixtures or browser/runtime evidence were executed while preparing it.
