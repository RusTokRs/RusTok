# Documentation `rustok-email`

`rustok-email` is the core email delivery module of the platform. It holds SMTP transport,
typed email rendering and delivery helpers for auth and operational notification flows.

## Purpose

- publish the canonical runtime entry type `EmailModule`;
- keep SMTP transport and email rendering outside the host layer;
- provide the platform with a unified delivery contract for typed email payloads.

## Scope

- SMTP configuration and sender wiring at the module level;
- typed rendering contract for password reset and adjacent email flows;
- delivery abstractions and email-related error model on the shared `rustok_api::PortContext`/`PortError` + `PortCallPolicy::write()` baseline;
- targeted contract tests for policy mapping, typed validation and disabled-provider noop fallback are located in `src/ports.rs`.
- no own RBAC vocabulary or UI surface.

## Integration

- depends on `rustok-core` and shared libraries;
- used by `apps/server` for auth lifecycle and operational notification path;
- module-level `health()` returns `Degraded` because effective SMTP transport can only be verified with host runtime context; the specific check is in `apps/server` as an `email_backend` readiness check;
- does not publish its own UI and remains `ui_classification = "capability_only"`;
- any admin-facing actions that trigger email sending are authorized in the calling module, not in `rustok-email`.

## Verification

- `cargo xtask module validate email`
- `cargo xtask module test email`
- `cargo test -p rustok-email ports::tests` for targeted delivery-port contract tests;
- targeted host tests for auth/email delivery flows when changing runtime wiring

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Manifest layer contract](../../../docs/modules/manifest.md)
