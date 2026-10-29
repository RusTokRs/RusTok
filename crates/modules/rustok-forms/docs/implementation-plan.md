# Implementation plan for `rustok-forms`

## Current state

The `rustok-forms` module implements form intake endpoints, spam filtering (honeypot, payload limits, IP rate limits), outbox contract event emission (`forms.submission.received`), transactional email notifications, and an administrative submission triage inbox.

## Scope

- Maintain intake reliability and abuse mitigation for all public form submissions.
- Support Page Builder form components and custom storefront integrations.
- Enforce tenant isolation and role-based permissions (`forms:read`, `forms:update`, `forms:manage`).

## Milestones

1. **Intake and Abuse Controls**: JSON and HTML form submission routes with rate limiting and honeypot validation.
2. **Outbox and Notification Wiring**: Atomic persistence with transactional outbox and email delivery.
3. **Admin and Triage Integration**: Administrative inbox operations and GraphQL endpoints for lead management.

## Verification

- `cargo xtask module validate forms`
- Run local unit tests: `cargo test -p rustok-forms`

## Update Rules

- Keep this implementation plan synchronized with architectural decisions and owner contracts.
- Document additions to form fields, intake protocols, or rate-limiting strategies before changing contracts.
