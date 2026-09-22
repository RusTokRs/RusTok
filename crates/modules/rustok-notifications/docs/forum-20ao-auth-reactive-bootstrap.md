# FORUM-20AO auth-reactive grouped inbox bootstrap

Status: source-ready / unvalidated

## Scope

`FORUM-20AO` closes the automatic auth-change bootstrap residual left by
`FORUM-20AN`. `NotificationsView` now keys its owner-backed bootstrap resource
by both the existing manual refresh nonce and the current storefront transport
context.

The context resolver reads the reactive `AuthContext` session signals for the
bearer token and tenant slug. Sign-in, sign-out, token refresh, or tenant change
therefore changes the resource source and starts a new bootstrap without a
caller invoking the mutation refresh callback.

## Exact context and stale-state boundary

One resolved transport context is passed into the bootstrap future and reused
for both the exact unread-count read and the first bounded group-summary page.
The future does not re-resolve credentials between those reads. Manual
post-command refresh remains available and continues to use the same resource.

Auth-scope changes clear mutation feedback before the replacement bootstrap is
rendered, so a message produced under one session is not carried into another
session or tenant. Existing interaction reads and writes continue to resolve
the current context at click time.

## Degraded behavior

Unauthenticated, mismatched-tenant, disabled-module, or unavailable transport
states are returned through the existing explicit error surface. The change
adds no timer, polling loop, local storage, shadow inbox, owner database query,
transport fallback, migration, or dependency.

Suggested maintainer validation commands are recorded in the machine contract.
Tests, Cargo commands, formatting, verifiers, and repository CI validation were
not run by the implementation agent.
