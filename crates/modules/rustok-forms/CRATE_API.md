# rustok-forms-module / CRATE_API

## Platform Capability Bindings
- `FormsModule` implements `rustok_core::module::RusToKModule` and `MigrationSource`.
- Declared as `ModuleKind::Optional`.
- Self-check returns `HealthStatus::Healthy`.
- Slug: `forms`.
- Dependencies: `outbox`, `email`.
- Permissions: `Resource::Forms` with `Action::Read`, `Action::List`, `Action::Update`, `Action::Manage`.

## Domain Collaborations
- Form submission intake, abuse controls, and lead triage inbox for content surfaces.
- Receives submissions from pages created with page-builder via public endpoints `POST /api/forms/{form_id}/submit` and `POST /api/forms/{form_id}/submit/form`.
- Emits unified contract event `forms.submission.received` v1 to transactional outbox.
- Optional transactional email notification via `rustok-email` (`TransactionalEmailSender`).
- Triage inbox managed via REST and GraphQL (`formSubmissions`, `updateFormSubmissionState`).

## Minimum Contract Set

### Input DTOs/Commands
- `SubmitFormInput`: flat JSON map or urlencoded fields for public form submission.
- `FormSubmissionsFilter`: filter by `form_id`, `state` (`new`, `read`, `handled`, `spam`), page, per_page.
- `UpdateFormSubmissionStateInput`: transition submission state with optional operator identity.
- `FormSubmissionDto`: public view of stored submission metadata and payload.

### Domain Invariants
- Abuse controls: honeypot field `website` intercepts bots (stored as `spam`, responds identically to success).
- Per-identity rate limiting: `FORM_SUBMIT_RATE_LIMIT = 5` submissions per `FORM_SUBMIT_RATE_WINDOW_MINUTES = 10` minutes keyed by `(tenant_id, form_id, ip_hash)` returning `429`.
- Payload bounds: maximum payload 16 KB, at most 50 fields, field names <= 64 bytes, field values <= 4096 bytes.
- IP hashing: client IP hashed with SHA-256 and salt `rustok-forms-ip-hash-v1:` for privacy.
- State transitions: `new -> read | handled | spam`, `read -> handled | spam`, `spam | handled -> read`. `handled` records `handled_at` and `handled_by`.
- Spam submissions do not emit `forms.submission.received` events.

### Events / Outbox Side Effects
- Emits `forms.submission.received` v1 atomically within the submission transaction for non-spam entries.
- Best-effort transactional email notification dispatched post-commit when configured (`FORMS_NOTIFY_TO`).

### Errors / Failure Codes
- `FormsError::NotFound`: submission not found for tenant.
- `FormsError::Validation`: field bounds, payload size, or format violation.
- `FormsError::RateLimited`: IP rate limit exceeded (`429`).
- `FormsError::InvalidStateTransition`: disallowed state change.
- `FormsError::Storage`: underlying database failure.
