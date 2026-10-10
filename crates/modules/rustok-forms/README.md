# rustok-forms

Form submission intake, abuse controls and lead triage for the RusTok content
surfaces. Pages built with the page builder point their form `actionUrl` at this
module; the module stores submissions, publishes contract events and optionally
sends a transactional email notification.

## What it does

- **Intake** — `POST /api/forms/{form_id}/submit` (JSON, one flat object of
  scalar fields) and `POST /api/forms/{form_id}/submit/form` (HTML form
  encoding). Both answer `202 { "id", "accepted": true }`.
- **Abuse controls** — honeypot field `website` (any non-empty value is stored
  as `spam` and answers exactly like success), per-identity rate limit
  (`FORM_SUBMIT_RATE_LIMIT = 5` per `FORM_SUBMIT_RATE_WINDOW_MINUTES = 10`
  minutes, keyed by `(tenant, form_id, ip_hash)`, answer `429
  FORM_SUBMIT_RATE_LIMITED`) and payload bounds (`MAX_FORM_PAYLOAD_BYTES =
  16384`, `MAX_FORM_PAYLOAD_FIELDS = 50`, `MAX_FORM_FIELD_NAME_BYTES = 64`,
  `MAX_FORM_FIELD_VALUE_BYTES = 4096`).
- **Storage** — append-only `form_submissions` table (`m20261009_000003`):
  `id`, `tenant_id`, `form_id`, `locale`, `page_id?`, `payload` (JSONB),
  `state` (`new|read|handled|spam`, CHECK-constrained), `ip_hash` (SHA-256,
  salted with `rustok-forms-ip-hash-v1:`), `user_agent`, `created_at`,
  `handled_at?`, `handled_by?`. Indexes: `(tenant, form, created)`,
  `(tenant, state, created)`, `(tenant, form, ip_hash, created)`.
- **Contract event** — `forms.submission.received` v1 is written to the unified
  contract events outbox inside the same transaction as the submission
  (spam captures do not emit it).
- **Email notification** — best-effort `TransactionalEmailSender::send_transactional`
  after commit. Enabled by `FORMS_NOTIFY_TO` (recipient) with optional
  `FORMS_NOTIFY_TEMPLATE` (template id, default `form_submission`).
- **Triage inbox** — admin routes and GraphQL for the module state machine:
  `new -> read|handled|spam`, `read -> handled|spam`, `spam|handled -> read`.
  `handled` records `handled_at`/`handled_by`. Permissions: `forms:read`,
  `forms:list` for listings, `forms:update`/`forms:manage` for triage.

## Endpoints

| Route | Auth | Purpose |
| --- | --- | --- |
| `POST /api/forms/{form_id}/submit` | public | JSON intake |
| `POST /api/forms/{form_id}/submit/form` | public | HTML form intake |
| `GET /api/admin/forms/submissions` | `forms:read` | list + filters (`form_id`, `state`, `page`, `per_page`) |
| `PATCH /api/admin/forms/submissions/{id}` | `forms:manage` | state transition |

GraphQL (tenant-scoped): `formSubmissions(filter)`, `updateFormSubmissionState(id, input)`.

## Configuration

| Variable | Meaning |
| --- | --- |
| `FORMS_NOTIFY_TO` | notification recipient; unset = inbox only |
| `FORMS_NOTIFY_TEMPLATE` | template id (default `form_submission`) |

## Known Limitations

- Rate-limit identity comes from `x-forwarded-for`/`x-real-ip` request headers
  (first hop) and falls back to the literal `unknown` when both are absent. The
  platform request context does not expose the socket peer address, so direct
  connections without proxy headers share one rate-limit bucket and hashed
  identity.
- Email notification is fire-and-forget after commit: a failed send is logged
  and never retried. Guaranteed delivery consumers must subscribe to
  `forms.submission.received` through the contract events outbox.
- `form_id` is an opaque slug taken from the author document; the module does
  not validate it against page documents and accepts any 1–64 character ascii
  slug, so typos in `actionUrl` create separate buckets instead of errors.
- The migration supports Postgres and SQLite; other backends are rejected
  explicitly.
