# Documentation of the `rustok-forms` module

`rustok-forms` is a platform module for form submission intake, abuse controls, and lead triage in RusToK.

## Purpose

- Provide high-throughput, abuse-protected form intake endpoints (`POST /api/forms/{form_id}/submit` JSON and `POST /api/forms/{form_id}/submit/form` URL-encoded).
- Store submissions in append-only storage with tenant isolation, IP hashing, and honeypot spam detection.
- Emit transactional contract outbox events (`forms.submission.received`) upon intake.
- Support optional transactional email notifications to operators.
- Provide a triage inbox for reviewing submissions through module-owned admin and GraphQL endpoints.

## Scope

- Own the form submission schema (`form_submissions`), intake validation, rate limits, and spam classification.
- Support Page Builder and storefront forms pointing to canonical action URLs.
- Expose typed triage transitions (`new -> read|handled|spam`).
- Delegate email sending to `rustok-email` and event publication to `rustok-outbox`.

## Integration

- Integration with `rustok-page-builder` via standard `actionUrl` contract.
- Integration with `rustok-outbox` for transactional domain events.
- Integration with `rustok-email` for transactional notification dispatches.
- Transport facade exposing REST endpoints in Axum and GraphQL schemas.

## Verification

- `cargo xtask module validate forms`
- Unit and integration tests in `crates/modules/rustok-forms/tests`
- Verification of intake boundaries, abuse limits, and email dispatch hooks.

## Related Documents

- [Implementation plan](./implementation-plan.md)
- [Crate README](../README.md)
- [Central Registry](../../../docs/modules/registry.md)
