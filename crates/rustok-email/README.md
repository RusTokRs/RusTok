# rustok-email

## Purpose

`rustok-email` owns SMTP transport, email rendering, and email delivery contracts for RusToK.

## Responsibilities

- Provide `EmailModule` metadata for the runtime registry.
- Expose SMTP configuration and delivery abstractions.
- Keep targeted delivery-port contract tests for shared write-policy mapping, validation errors, and disabled-provider noop fallback.
- Render typed email payloads used by auth and notification flows.

## Interactions

- Depends on `rustok-core` for module contracts and `rustok-api` for shared port context/error/write-policy primitives.
- Used by `apps/server` auth lifecycle and operational notification paths.
- Module-level health intentionally reports `Degraded` because effective SMTP transport validation requires host runtime context; `apps/server` owns the concrete `email_backend` readiness check.
- Does not publish a dedicated RBAC surface.
- Any admin-facing actions that trigger email delivery are authorized in `apps/server`
  through permissions owned by the calling module, not by `rustok-email`.

## Entry points

- `EmailModule`
- `EmailService`
- `EmailConfig`
- `PasswordResetEmail`
- `PasswordResetEmailSender`
- `EmailDeliveryPort`

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
