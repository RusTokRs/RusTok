# FORUM-20AN notification group-state GraphQL commands

Status: source-ready / unvalidated

## Scope

`FORUM-20AN` closes the GraphQL group-state command residual left by
`FORUM-20AL`. It exposes bounded exact-group `MARK_READ`, `MARK_UNREAD`, and
`ARCHIVE` through the same authenticated storefront owner port used by the
native Leptos server-function path.

## Admission and owner scope

The mutation `notificationInboxApplyGroupState` requires an authenticated human
user, matching auth/tenant context, and the enabled Notifications capability
before validating the caller idempotency key or invoking the owner port. Tenant
and recipient are derived from request context and cannot be supplied as GraphQL
arguments.

The mutation accepts one opaque group key, one typed action, optional bounded
cursor/limit inputs, and one non-empty control-free idempotency key capped at 128
bytes. The generated `PortContext` carries the canonical user actor, permission
claims, effective locale, storefront channel, correlation identity, a five-second
write deadline, and the caller idempotency key.

## Owner and transport behavior

The resolver delegates only to
`NotificationInboxStorefrontPort::apply_group_state`, which preserves the
existing owner action-specific selection, exact tenant/recipient/group scope,
state timestamp invariants, terminal archive, and bounded continuation contract.
The GraphQL response exposes only scanned/changed counts and continuation
metadata.

SSR and hydrate builds select the native command path. CSR and headless builds
select the GraphQL mutation. No path falls back to the other. The existing UI
call site remains unchanged and still performs an authoritative resource refresh
after every successful command.

## Boundary

This slice adds no selected-ID GraphQL commands, shadow storage, direct storefront
database access, automatic auth-reactive bootstrap refresh, scheduled
reconciliation/redaction, channel delivery, or PostgreSQL runtime evidence.

Suggested maintainer validation commands are recorded in the machine contract.
Tests, Cargo commands, formatting, verifiers, workflows, and CI validation were
not run by the implementation agent.
