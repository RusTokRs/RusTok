# Form submissions backend

- Date: 2026-10-09
- Decision status: Accepted
- Implementation status: Implemented
- Owners: `rustok-forms` (Forms module)
- Extends: [Multilingual content contract](./2026-03-28-multilingual-content-contract.md)
- Supersedes: None
- Superseded by: None

## Context

The pages/page-builder functional audit (2026-10-09) recorded gap G-7: forms render and
submit in the browser (Fly `flyForm` components carry `actionUrl`/`action` and post
natively), but no server receiver exists. `ProviderAction` has no backend, there is no
submissions/leads module, and a "contact" form on a landing page lands nowhere. For lead
generation this is the critical content-operations gap (wave 2 of the audit roadmap).

The platform already provides the supporting pieces: `rustok-email` exposes a
`TransactionalEmailSender` port, `rustok-outbox` carries typed contract events, and
module composition is manifest-driven (`modules.toml` + `rustok-module.toml`, with
generated server/admin registration).

## Decision

Add a first-party **`rustok-forms` module** owning form submission intake and triage:

1. Public intake endpoint `POST /api/forms/{form_id}/submit` accepts a JSON payload for
   one form of the current tenant, applies abuse controls, and stores an append-only
   submission row. A silently-accepted honeypot field and a per-identity rate limit
   protect the endpoint without CAPTCHA.
2. Every accepted submission emits a typed outbox contract event
   (`FormSubmissionEvent::SubmissionReceived`, type `forms.submission.received`) in the
   submission transaction.
3. Email notification is best-effort: when the deployment wires a
   `TransactionalEmailSender` and a recipient, the module sends one templated message
   per non-spam submission; notification failure never blocks or loses the submission.
4. A simple admin inbox lists submissions and moves them through `new -> read ->
   handled` (plus `spam` for honeypot captures), exposed through GraphQL and a Leptos
   admin surface.

Forms address the endpoint by authoring `actionUrl` (`/api/forms/{form_id}/submit`) in
the Fly form definition; no renderer change is required for the native form POST flow.

## Sources of truth and ownership

- `form_submissions` (Forms schema) is the only source of truth for submissions and
  their triage state. Nothing else stores lead payloads.
- Form definitions stay in page documents (Fly `flyForm`); the module stores the
  `form_id` and payload only, so submissions survive document edits.
- `rustok-events` owns the typed `FormSubmissionEvent` contract (schemas + validation),
  like `ForumMentionEvent`.
- `rustok-email` remains the sole delivery authority; Forms only calls its port.

## Invariants

- Submissions are append-only rows; triage updates state fields in place and never
  rewrites or deletes payloads.
- One contract event per stored submission, written in the same transaction.
- Honeypot captures are stored as `spam` and never notify by email.
- Rate limiting is deny-by-response (429), never drop-without-answer for non-honeypot
  traffic; the honeypot path answers 202 and stores `spam` so bots see success.
- Payload size and field counts are bounded before storage.
- Read/handled transitions require `forms:manage` authority; intake is public.

## Non-goals

- Form building UI changes (Fly already authors `flyForm` + `actionUrl`).
- Server-side re-implementation of `ProviderAction` dispatch for arbitrary providers;
  this decision covers the Forms receiver only.
- File uploads in forms (payload is JSON fields only).
- Double-opt-in, mailing-list subscription semantics, or CRM integrations.
- Per-form configurable field schemas and validation rules (payload is schemaless JSON
  within bounds).

## Data, transaction, and concurrency boundary

Table `form_submissions` (migration `m20261009_000003_create_form_submissions`,
append-only, backfill `mode: none`):

- `id uuid` primary key (client-generated).
- `tenant_id uuid` not null; `form_id varchar(64)` not null; `locale varchar(16)` not
  null; `page_id uuid null` (informational link to the landing page).
- `payload jsonb` not null — the submitted fields, honeypot field removed.
- `state text` check (`new|read|handled|spam`).
- `ip_hash char(64)` — salted SHA-256 of the client identity, never the raw address;
  `user_agent text null`.
- `created_at`, `handled_at`, `handled_by uuid null`.

Rate limiting counts recent rows per `(tenant_id, form_id, ip_hash)` with a bounded
query (window and cap are module constants), so no counter table is needed. All writes
happen in one transaction per submission (`insert` + contract event). Triage updates are
optimistic state transitions.

## Context dimensions

- Tenant: every row is tenant-scoped; intake resolves the tenant from the request
  context exactly like public artifact reads.
- Locale: the submission stores the payload locale and notifications use it for
  template selection.
- Actor: intake is anonymous (`created_by` is not recorded); triage records `handled_by`.

## Events and projections

One new typed contract event, `FormSubmissionEvent::SubmissionReceived`
(`forms.submission.received`, schema version 1), published through the transactional
outbox. No read projections are added; the admin inbox reads the table directly.

## Failure semantics

- Abuse controls (honeypot filled, rate limit exceeded) never leak detection details:
  the honeypot answers 202 and stores `spam`; the rate limit answers 429 with a stable
  `FORM_SUBMIT_RATE_LIMITED` code.
- Validation failures answer 400 with stable codes; payload bounds are enforced before
  storage.
- Email notification errors are logged and swallowed (submission and event stand).
- If the submission transaction fails, no event is emitted and the client receives 500;
  retrying is the client's decision and may create a second submission.

## Migration and cutover

One append-only migration with backfill `mode: none`; no existing rows are touched and
no deploy ordering constraint exists. Intake is opt-in per document (only forms whose
`actionUrl` targets the endpoint start landing).

## Alternatives considered

- **Store submissions in the pages module**: rejected — leads are not page state, the
  triage lifecycle and abuse controls do not belong to the page lifecycle, and the
  audit calls for a submissions module.
- **ProviderAction backend with per-provider handlers**: deferred — it solves dispatch
  for editor-defined actions, not the plain form POST that lands today; the receiver
  must exist first.
- **Drop honeypot captures silently**: rejected — storing them as `spam` keeps abuse
  measurable and the response pattern indistinguishable from success for bots.
- **Counter-table rate limiting**: rejected — a bounded recent-rows count is simpler
  and self-cleaning (rows age out of the window).

## Verification

- New sqlite integration tests `tests/form_submissions_sqlite.rs`: intake stores the
  submission and emits the contract event, honeypot captures become `spam` without
  notification, rate limiting answers 429 after the per-window cap, triage transitions
  (`new -> read -> handled`) record `handled_by`, and payload bounds are enforced.
- Contract tests cover the `FormSubmissionEvent` schema (rustok-events).
- Toolchain-less static gates (rustfmt, tree-sitter syntax, module resolution,
  `verify:adrs`, migration backfill self-test, module layout/registry checks) must pass
  in the authoring environment; `cargo test`/`clippy` remain CI authority.

## Consequences

- Landing-page forms gain a real receiver with abuse controls and an inbox; leads stop
  being lost.
- Editors opt in per form by pointing `actionUrl` at the intake endpoint.
- Deployments that want email alerts wire a `TransactionalEmailSender` and a recipient;
  without them the inbox remains the notification surface.
- The typed event contract grows one entry; downstream projections can subscribe to
  `forms.submission.received` later without schema changes.
