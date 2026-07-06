# Unification of auth lifecycle and session invalidation policy

- Date: 2026-02-26
- Status: Accepted

## Context

In `apps/server`, parallel implementations of auth use-cases historically existed in REST and GraphQL. This led to divergences:

- different side-effects when creating a user in separate entrypoints;
- different reset/change password semantics regarding sessions;
- duplication of business branches and error-mapping across transports.

At the same time, the RBAC source-of-truth migration to the relation model is already described separately and should not be mixed with the auth lifecycle consistency task.

## Decision

1. Establish `AuthLifecycleService` as the single application service for auth use-cases (`register`, `login/sign_in`, `refresh`, `request/confirm reset`, `change_password`, `update_profile`, `create_user`).
2. Keep REST handlers and GraphQL mutations as thin adapter layers (I/O parsing, transport mapping).
3. Apply a unified session invalidation policy across all channels:
   - `confirm_password_reset`/`reset_password`: soft-revoke all active sessions of the user via `sessions.revoked_at`;
   - `change_password`: soft-revoke all other active sessions of the user (except the current one);
   - `sign_out`: soft-revoke only the current session.
4. Explicitly separate document responsibilities:
   - auth lifecycle consistency, transport parity and release-gate process — in `docs/architecture/api.md` (section "Auth lifecycle consistency and release-gate");
   - RBAC relation migration and source-of-truth cutover — in `docs/architecture/rbac-relation-migration-plan.md`.

## Consequences

- Reduces the likelihood of drift between REST and GraphQL regarding auth behavior.
- Critical security scenarios (reset/change password) become predictable and verifiable through invariant tests.
- Documentation and rollout gates can check consistency independently of the RBAC cutover.
- The next step is to maintain parity with tests and not return business branches to the transport layer.
