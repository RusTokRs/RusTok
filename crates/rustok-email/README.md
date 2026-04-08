# rustok-email

## Purpose

`rustok-email` owns SMTP transport, email rendering, and email delivery contracts for RusToK.

## Responsibilities

- Provide `EmailModule` metadata for the runtime registry.
- Expose SMTP configuration and delivery abstractions.
- Render typed email payloads used by auth and notification flows.

## Interactions

- Depends on `rustok-core` for module contracts.
- Used by `apps/server` auth lifecycle and operational notification paths.
- Does not publish a dedicated RBAC surface.
- Any admin-facing actions that trigger email delivery are authorized in `apps/server`
  through permissions owned by the calling module, not by `rustok-email`.

## Entry points

- `EmailModule`
- `EmailService`
- `EmailConfig`
- `PasswordResetEmail`
- `PasswordResetEmailSender`

See also [docs/README.md](docs/README.md).
