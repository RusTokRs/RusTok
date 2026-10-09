# Page body working copies: draft revisions and body revision journal

- Date: 2026-10-09
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-pages` (Pages module)
- Extends: [Multilingual content contract](./2026-03-28-multilingual-content-contract.md)
- Supersedes: None
- Superseded by: None

## Context

`rustok-pages` stores one localized visual document per page and locale in `page_bodies`
(`format = grapesjs`, the canonical Fly/GrapesJS invariant). Today the single row is both
the published content and the editing target, and the module enforces "published documents
are immutable" (`PAGE_PUBLISHED_DOCUMENT_IMMUTABLE`): editing a live page requires
unpublishing it first, a save overwrites the row in place, and no previous content is
retained. That model blocks the standard editorial cycle (edit a live page without going
offline), loses every previous revision on save, and leaves the scenario-baseline revision
journal as the only durable history anywhere in the subsystem.

The functional audit of 2026-10-09 (`docs/audits/pages-page-builder-functional-audit-2026-10-09.md`,
gaps G-1 and G-2) records both gaps as P0 product defects. This ADR defines the canonical
working-copy and revision model that closes them and keeps the reviewed publish contract
exact.

## Decision

`rustok-pages` adopts a single **working copy** model per localized body with an
append-only **body revision journal**.

### Vocabulary

- **current body** — the `page_bodies` row of a (page, locale). While the page is
  unpublished it is the working copy; while the page is published it is the live content
  and is only rewritten by promotion.
- **draft** — a `page_body_drafts` row: the working copy of a published page. At most one
  draft exists per (page, locale), and only while the page is published.
- **body revision** — one immutable `page_body_revisions` journal row recording an accepted
  body mutation: the resulting content, its source, the working-copy revision token it
  produced, and the acting user.
- **working-copy revision token** — the existing `expected_revision` identity
  (`page:<page_id>:initial`, or the working copy's `updated_at` string). It identifies the
  draft when a draft exists and the current body otherwise. The wire format is unchanged.

### Allowed states

| Page status | Working copy | Live content | `save_document` writes |
|---|---|---|---|
| `draft` / `archived` | `page_bodies` row | none | `page_bodies` (today's behaviour) |
| `published` | `page_body_drafts` row | `page_bodies` row | `page_body_drafts` |

Transitions:

1. **Save** on a published page creates or updates the draft and appends one journal row
   (`draft_save`). It does not touch `page_bodies`, does not bump `pages.updated_at`, and
   emits no domain event: draft edits are pre-publication editor state and change nothing
   that is served or projected. Save on an unpublished page keeps today's semantics
   (`page_bodies` upsert, `pages.updated_at` bump, `NodeUpdated`) and appends `save`.
2. **Create** appends `create` for the initial body.
3. **Promotion** moves a draft into `page_bodies` and deletes the draft in the same
   transaction, preserving the reviewed `updated_at` so the promoted token is exactly the
   reviewed token. It appends `promote` and happens:
   - inside `publish_reviewed` / non-builder publish, for every draft of the page,
     atomically with the artifact staging and binding switch;
   - inside `unpublish` / `archive`, before the status transition, so the page continues
     editing from the work in progress once it leaves the public surface.
4. **Restore** copies a journal row's content into the working copy (draft while published,
   current body otherwise) under `expected_revision` CAS and appends `restore`. Restoring
   is a new mutation; the journal is never rewritten.
5. **Duplicate** (page duplication) copies the source page's translations, channel
   visibility, template/metadata and **current bodies** into a new unpublished page and
   appends `duplicate` per copied body. Pending drafts stay with the source page; no
   publication state or artifact is copied.

### Forbidden states

- A draft row while the page is not published.
- More than one draft per (page, locale); enforced by `UNIQUE (page_id, locale)`.
- A public or read-only read observing a draft. Drafts are exposed only to authorities
  with effective `pages:update` scope, and only through the editor-facing read surface
  (`body.state = "draft"`). Public storefront reads always resolve current bodies.
- A journal row without an accepted body mutation, or a body mutation without its journal
  row: both are written in the same transaction, and a rejected mutation (CAS conflict,
  validation failure) appends nothing.
- Publication of content that was not reviewed: `PublishPageInput.expected_body_revisions`
  now identifies the working copies being published (draft tokens where drafts exist,
  current tokens otherwise). The reviewed-publish CAS, review hash and idempotency receipt
  semantics are unchanged; only the identity of "the exact reviewed set" extends to drafts.

## Sources of truth and ownership

- Canonical owner: `rustok-pages` (`PageService`).
- Authoritative state: `page_bodies` (current/live), `page_body_drafts` (working copies of
  published pages), `page_body_revisions` (append-only history).
- Derived/projections: storefront artifacts and caches (published content only), GraphQL
  and REST DTOs (`PageBodyResponse.state`), admin/Leptos views. None of them is a write
  model.
- Dependency direction: `rustok-page-builder` and the admin/storefront surfaces consume
  Pages through the existing contracts; no other module reads or writes the three tables.

## Invariants

- Exactly one working copy per (page, locale): a draft row iff the page is published and
  has pending edits, otherwise the current body row.
- `page_document_revision` tokens are monotone per working copy (every accepted mutation
  produces a fresh `updated_at`) and are the only save/publish CAS identity.
- The journal is append-only. Retention is deterministic: the newest
  `MAX_PAGE_BODY_REVISIONS_PER_BODY` (50) rows per (page, locale) are kept, older rows are
  pruned inside the appending transaction. History reads are bounded at 200 rows,
  `created_at DESC, id DESC`.
- Promotion preserves content, `format` and the reviewed `updated_at` byte-for-byte, so a
  published token is the token the reviewer acknowledged.
- Tenant scoping is schema-enforced: `page_body_drafts` and `page_body_revisions` carry
  tenant-scoped composite foreign keys to `pages (tenant_id, id)`, which gains the required
  unique key (`uq_pages_tenant_id`); the draft working copy carries
  `UNIQUE (page_id, locale)`, mirroring the existing `idx_page_bodies_page_locale` unique
  index that already bounds current-body identity.
- Only the canonical Fly/GrapesJS document format is accepted by save and restore, so a
  draft can never introduce a body format that the reviewed publish path cannot compile.

## Data, transaction, and concurrency boundary

- `page_body_drafts(id, tenant_id, page_id, locale, content, format, created_at,
  updated_at, created_by)` + `UNIQUE (page_id, locale)` mirroring the existing
  `idx_page_bodies_page_locale` unique index.
- `page_body_revisions(id, tenant_id, page_id, locale, content, format, source,
  body_revision, created_at, created_by)` + history index
  `(tenant_id, page_id, locale, created_at)`.
- `source` vocabulary: `create`, `save`, `draft_save`, `promote`, `restore`, `duplicate`.
- Body revision timestamps are truncated to microseconds at write time: body revision
  tokens are the stored `updated_at` string of a working copy, so the written value, the
  stored round-trip and the token must stay identical on every backend (PostgreSQL
  `timestamptz` stores microseconds while `Utc::now()` carries nanoseconds).
- Composite `FOREIGN KEY (tenant_id, page_id) REFERENCES pages (tenant_id, id)`
  `ON UPDATE CASCADE ON DELETE CASCADE` on both tables, after `pages` gains
  `uq_pages_tenant_id UNIQUE (tenant_id, id)` as the composite key target.
- Migration `m20261009_000001_create_page_body_drafts_and_revisions` is additive
  (`if_not_exists`), registered append-only in the Pages migration plan, with a backfill
  contract `mode: none` (new empty storage; existing `page_bodies` rows are already the
  working copies of their pages and need no transformation).

### Concurrency, idempotency, and destructive operations

- Every accepted body mutation commits its working-copy write, its journal append, the
  retention prune and its (optional) page/event effects in one transaction.
- Save/restore CAS on the working-copy token (`expected_revision`); publish CAS on the
  reviewed working-copy set (`expected_body_revisions`) exactly as today. Lost updates are
  rejected with `PAGE_DOCUMENT_REVISION_CONFLICT`, never last-write-wins.
- Draft rows are locked with the same SQLite/Postgres backend split used by `page_bodies`.

## Context dimensions

- **Tenant**: every draft, revision and promotion read/write is tenant-scoped, and the
  tenant-scoped composite foreign keys make cross-tenant rows impossible at the schema
  layer.
- **Locale**: the working copy, CAS token, journal and retention window are all per
  (page, locale); promotion and duplication iterate locales.
- **Principal/auth**: save and restore require `pages:update` within the actor's scope
  (`enforce_owned_scope`); duplication requires `pages:create`; promotion follows the
  existing `pages:publish` / lifecycle authorization. The acting user is recorded on the
  journal row (`created_by`).
- **Channel**: unchanged; channel visibility gates public serving, not editing, and is
  copied on duplication.
- **Policy**: builder rollout gates (`builder.enabled`, `builder.publish.enabled`) keep
  applying to the same operations as today.
- **Timezone/trace**: timestamps are persisted UTC, matching the existing body rows.
- **Currency**: not applicable.

## Events and projections

- Draft-layer mutations (draft save, restore-into-draft) emit no domain event and do not
  invalidate caches: no served or projected state changed.
- Current-body mutations keep today's event flow (`NodeUpdated`, and `NodeCreated` for
  duplication; publish/unpublish keep `NodePublished` / `NodeUpdated` plus lifecycle
  events). Cache invalidation semantics are unchanged: public content changes only at
  publish/rollback/unpublish.
- `scan_published_pages` keyset cursors stay stable while editors work on drafts, because
  draft saves do not move `pages.updated_at`.

## Failure semantics

- Rejecting a stale save/publish is a typed conflict error; no partial state is written.
- Journal retention prunes only rows beyond the documented window; it is the only deletion
  path inside the journal besides page deletion (cascade).
- Deleting a page cascades to drafts and revisions: they have no independent business
  lifetime outside the page.
- Page duplication never mutates the source page.

## Migration and cutover

- No data transformation: published pages simply start accepting draft writes after
  deploy; unpublished pages are untouched.
- The `PAGE_PUBLISHED_DOCUMENT_IMMUTABLE` error, its `ensure_document_is_mutable` gate and
  every caller-side "published document is immutable" guard are removed in the same change
  (zero-legacy). The new save semantics replace them outright; there is no compatibility
  mode.
- Inline editing of a published page follows the same rule: it loads the working copy
  (draft if present) and commits into the draft. Inline edits of live pages become live at
  the next reviewed publish. This is a behaviour change recorded in the module README.

## Non-goals

- Multi-user merge, collaborative cursors or comment threads (CAS rejection remains the
  concurrency contract).
- Scheduled publication, per-locale partial publication, shareable preview links.
- Body translation through `rustok-translation-targets` (metadata remains the translation
  target; body localization keeps per-locale documents).
- Configurable retention policies or retention jobs; the fixed window is the contract.
- History content preview over the transport (history lists metadata; restore loads
  content server-side).

## Alternatives considered

1. **`page_bodies.state` column (`current|draft`) instead of a separate draft table.**
   Rejected: it weakens the "exactly one current body" uniqueness into query discipline,
   duplicates the existing per-locale body identity, and mixes live and pre-publication
   content in one reader path (a missed filter leaks drafts publicly).
2. **Fully append-only body store with a current pointer.** Rejected for this change: the
   reader blast radius (artifact rebuild provenance, storefront reads, translation,
   inline-edit grants) is a rewrite of every body read for no additional editorial
   capability; the journal plus working copies delivers history and drafts without
   touching read models.
3. **Page-level draft (one draft covering all locales).** Rejected: bodies, save CAS and
   reviewed publish are per-locale; a page-level draft would break per-locale CAS and the
   reviewed-set identity.
4. **Discarding drafts on unpublish.** Rejected as silent data loss; promotion keeps the
   single-working-copy invariant and the editor's work in progress.

## Verification

- `cargo test -p rustok-pages` including the new
  `tests/page_body_draft_and_history_sqlite.rs` (draft save on published pages, CAS
  conflicts, journal rows per source, promotion on publish and unpublish, restore,
  retention prune, duplication, draft invisibility to public reads).
- `cargo fmt --check`, `cargo clippy -p rustok-pages -p rustok-pages-admin -p rustok-pages-storefront -- -D warnings`.
- Migration gates: `verify-migration-plan-compatibility.mjs`,
  `verify-migration-backfill-contracts.mjs` (the appended migration carries a `mode: none`
  contract), `npm run verify:adrs`.
- Toolchain-less static evidence (`scripts/audit/rust_syntax_check.mjs`,
  `rustfmt_check.mjs`, `rust_module_resolution_check.py`) may supplement but never replace
  the cargo checks above.

## Consequences

- The editorial cycle closes: live pages can be edited, reviewed and published without
  going offline, and every accepted body mutation is recoverable inside the retention
  window.
- The reviewed publish contract stays exact but its identity extends to drafts; callers
  that build `expected_body_revisions` must read working-copy tokens (the transport keeps
  exposing them as the body `updated_at` of the working copy).
- Published pages become mutable under editors again, so every public read path must stay
  on current bodies; this is enforced by construction (drafts live in their own table) and
  by the read-surface rule, not by call-site discipline.
- Draft saves stop touching `pages.updated_at`, so the admin page list's `updated_at` no
  longer reflects unpublished draft edits; the draft's own timestamp is visible in the
  page detail (`body.state`, `body.updated_at`).
- Storage grows one journal row per accepted save; the 50-per-(page, locale) window bounds
  it, and history reads are capped at 200 rows.
- Follow-up work inside this change set: schema + entities, `PageService` working-copy
  save/restore/promote/duplicate, GraphQL and REST surfaces, admin (Leptos) wiring for
  history/restore/duplicate and published-page editing, module documentation updates,
  and the verification listed above.
