# Translation control plane and owner-owned localized data

- Date: 2026-07-26
- Status: Proposed

## Context

RusToK already has server-owned locale selection, tenant locale policy, UI
message catalogs, and an accepted multilingual database shape: language-neutral
base rows with owner-local translation/body records. Content, Pages, Product,
Commerce, Flex, and other modules use this foundation with different DTOs,
revision models, and lifecycle rules.

The platform does not yet have a translation management surface. It cannot
inventory exact-locale gaps across owners, assign/review work, calculate
truthful progress, reuse approved translations, enforce terminology, or
coordinate provider-neutral machine translation.

A central module that reads and writes every `*_translations` table would break
domain ownership, tenant/RBAC policy, owner validation, events, cache/index
updates, and future remote-module boundaries. Treating runtime fallback as an
existing translation would also make progress incorrect. Translating arbitrary
settings JSON or flattening richtext/Page Builder content would corrupt
non-copy data and structured formats.

`rustok-ai` already owns provider routing, model policy, secrets, egress, runs,
approvals, and durable workflows. A translation feature must reuse that
capability without giving the AI runtime ownership of localized data or
publication.

## Decision

### Module role

Create `translation` as an optional, admin-only platform module. It is a
translation control plane, not a locale resolver and not a shared localized
business-data owner.

It owns:

- resource inventory and derived exact-locale progress;
- jobs, proposals, assignments, review, approval, and application receipts;
- tenant-scoped translation memory and terminology glossaries;
- import/export orchestration and operational recovery;
- optional AI proposal orchestration.

Disabling it must not change existing owner reads, storefront content, effective
locale selection, or fallback.

### Owner data boundary

Every domain/settings owner keeps its canonical base, translation, body, and
localized-value storage. Owner modules expose typed translation target providers
through an owner-neutral registry contributed via `ModuleRuntimeExtensions`.

The control plane:

- does not query or mutate owner tables directly;
- has no cross-module foreign keys;
- does not hard-code first-party owner slugs;
- applies approved patches only through an owner service with tenant/actor
  context, provider-declared permission floor, expected source/target revisions,
  deadline, and idempotency key;
- records success only after the owner returns a committed receipt.

Owner application performs normal validation, audit, transactional outbox, and
cache/search/index/SEO projection behavior. Translation workflow events do not
replace owner domain events.

### Locale and progress semantics

Authoring and progress use exact normalized locale records. Runtime fallback is
reported separately and never counts as an existing translation. `und` remains
storage-only unknown provenance and cannot be a translation source, target, or
fallback.

The shared locale foundation must expose distinct runtime/tenant and
stored-provenance locale types over one canonical normalizer. Runtime/tenant
types reject `und`; a stored-provenance type may admit it only for truthful
legacy provenance.

The translation module owns a required-target-locale policy that must be a
subset of the tenant's enabled locales. It does not alter the tenant/runtime
effective-locale chain.

### Provider contract

Use `rustok-translation-targets` as the single runtime-neutral translation
target/provider registry. It owns typed resource/field/value descriptors,
capabilities, registration, and conformance fixtures. Generic context, error,
locale, and permission primitives remain in `rustok-api`.

Providers expose stable owner/resource/field identity, exact locales, opaque
revisions, source hashes, semantic value profiles, constraints, data
classification, permission floors, bounded cursoring, validation, and atomic
apply.

Structured data is translated only through owner segmentation and reassembly.
Raw HTML, secrets, identifiers, URLs, code, arbitrary JSON, and immutable
transaction history are excluded.

### Bounded interchange

Job interchange uses the immutable owner snapshot already captured by the
workflow. Export is bounded by item, field, field-byte, and total-document
limits and carries the exact owner/resource identity, source digest, source and
target revisions, source hashes, constraints, and protected tokens. Only
`public` and `tenant_private` fields whose strategy is not `excluded` may leave
the control plane; personal, sensitive, secret, and immutable transaction data
are never inferred to be exportable.

Import is atomic per job item. It binds schema version, job, item, owner
identity, and source digest before accepting values. Imported values become an
`import` proposal through the canonical proposal service, so owner validation,
deterministic QA, assignment, actor, deadline, idempotency, and later
review/approval/application rules remain unchanged. Interchange never applies
owner data directly.

### Private workflow collaboration

Translation owns private, append-only workflow notes attached to a job and,
optionally, one job item. They are workflow context for translators and
reviewers, not public content and not a second comment system.

The generic `rustok-comments` capability keeps its own public discussion and
`comments`-resource RBAC contract. Reusing it here would bypass Translation
job/item tenant and participant authorization or force that generic module to
own Translation workflow semantics. Therefore it is not a dependency of this
capability.

Note creation is actor-bound and idempotent; listing is tenant/job scoped and
bounded; resolution is an irreversible explicit-revision transition. Note bodies
remain in Translation workflow storage only: they are excluded from Translation
Memory ingestion and lookup, machine-translation requests, owner apply payloads,
and workflow event bodies. Events publish only note/job/item identifiers and
revisions for audit and projection purposes.

### Settings

Settings are eligible only after their owner declares stable localized leaf
semantics and stores localized values in parallel owner-local locale rows.
Arbitrary strings inside host, platform, tenant-module, provider, or secret JSON
are not inferred to be translatable.

Static setting descriptions, navigation labels, UI messages, and system errors
remain in the separate source-controlled/signed catalog delivery plane.

### AI integration

Create `rustok-ai-translation` as the provider-neutral domain adapter for the
`machine_translation` task.

`rustok-translation` owns `MachineTranslationPort`. `rustok-ai` owns a
high-level `AiStructuredTaskPort` and model/provider routing, credentials,
egress, durable execution/attempt/usage/cost evidence, budgets, retry/fallback,
cancellation, and run traces. `rustok-ai-translation` is a stateless adapter
that depends on both owner contracts and registers through runtime extensions;
it owns no tables, migrations, UI, queues, or canonical content.

`rustok-translation` does not depend on `rustok-ai`, and `rustok-ai` does not
depend on `rustok-translation`. The translation module does not read AI tables,
instantiate AI services, call its own GraphQL endpoint, or resolve providers
and secrets. Billable AI execution uses a write-like port policy with deadline,
idempotency, cancellation, budget reservation, typed retryability, and
content-safe evidence.

The optional cross-module adapter is selected only by the distribution
composition feature `ai-translation`, which requires both owner modules. It
publishes a Translation-owned lazy `MachineTranslationPortFactory` through
`ModuleRuntimeExtensions`; executable hosts transfer that factory generically
and never import the adapter or either owner capability. The factory
materializes the concrete port only from the completed `HostRuntimeContext`, so
database and deployment-owned AI handles are available without a server-owned
capability match.

Successful structured output is handed off through an AI-owned, encrypted,
short-lived result store rather than the generic execution or attempt ledger.
The successful provider attempt, encrypted result row, provider-slot release,
budget settlement, and terminal execution transition commit in one database
transaction. AES-256-GCM additional authenticated data binds tenant, execution,
request, output digest, and key ID; plaintext, raw provider responses, keys, and
ciphertext are prohibited from generic metadata and logs.

The deployment owns a rotation-safe result keyring through secret references.
Rows retain the key ID used for encryption, and retired keys remain resolvable
until the configured result retention expires. Replay after expiry fails closed
without reopening the execution or charging the tenant again. Result cleanup
does not remove durable status, attempt, usage, price, or cost evidence.

The adapter receives bounded exact-locale segments, placeholders, field
semantics, glossary revision, memory suggestions, context, and data
classification. It returns typed, deterministically validated proposals.
Initial AI results always require human review and never write owner data.

Translation owns the machine-proposal command and a content-free durable
handoff journal. The command derives an exact batch from the persisted job
snapshot, immutable glossary binding, and bounded Translation Memory lookup,
then calls `MachineTranslationPort`. It persists translated values only by
calling the canonical proposal workflow, which repeats owner validation and
deterministic QA. The journal stores request and context digests plus
execution/attempt/usage/cost evidence and proposal identity, never source,
memory, or translated values. Deterministic child idempotency keys let restart
replay both the AI result handoff and proposal save without rebilling while
the request projection remains intact; projection drift fails closed as an
idempotency conflict. Registered operations pin normalized memory entry
identities, order, and match scores without duplicating segment content. Replay
can read a tombstoned pinned entry, purge is blocked while the pin exists, and
completion or explicit actor-bound cancellation releases the pins atomically.
Cancellation is accepted only before proposal save enters `saving`; AI-runtime
status and cancellation resolve by stable owner/idempotency identity, including
a durable content-free cancellation intent before execution registration.
Translation cancellation receipts retain propagation evidence and exact replay
retries incomplete propagation. Audited stuck-save recovery is
Manage/Update-authorized, revision-guarded, actor/idempotency-bound, and stores
no content. It retrieves only an already completed result through the stable
execution key, revalidates the reconstructed request digest, and resumes the
canonical proposal save without another billable execution.

Automatic approval or publication requires a later accepted decision, explicit
tenant policy, measured locale-pair evidence, and deterministic safety/quality
thresholds.

### Translation Memory retention

Translation owns retention execution for its memory content. Owner providers
remain the lifecycle authority: inventory synchronization records only the
matching resource identity, opaque owner revision, and deletion observation
time when the owner reports `Deleted`. `Unavailable` is not deletion evidence.
The lifecycle evidence is committed in the same transaction as the inventory
checkpoint and contains no source or translated text.

The module publishes a `translation_memory_retention` adapter through the
generic module-work registry. The memory entry itself is the durable work
source; its revision is the optimistic claim boundary, and the existing
actor-bound mutation receipt is completion evidence. This keeps Translation
tables and retention rules out of the host scheduler and makes duplicate
multi-replica claims harmless.

Automatic execution tombstones expired `retain_until` entries and
`owner_lifecycle` entries with owner-deletion evidence. It never tombstones or
purges `legal_hold`. Purge waits at least 24 hours after tombstone, rechecks the
entry revision and policy, excludes entries pinned by a registered machine
translation operation, and preserves only the content-free purge receipt.

### UI and transport

The module publishes a module-owned Leptos admin package and a Next admin
package over the same service contract. Leptos uses native `#[server]` functions
in SSR/hydrate with GraphQL retained in parallel. REST is limited to bounded
streaming exchange/integration surfaces.

The UI locale comes from the host. Source and target content locales are
explicit workbench state and do not introduce another request-locale fallback
chain.

## Consequences

- Translation workflow can span first- and third-party modules without taking
  ownership of their data.
- Owner validation, permissions, revisions, events, and remote-ready boundaries
  remain intact.
- Progress distinguishes exact, fallback, draft, approved, applied, stale,
  blocked, and excluded work.
- Manual translation remains usable without AI, and AI policy remains
  deployment/tenant controlled.
- Supporting a new resource requires an owner provider and evidence, not a
  central schema or host edit.
- Settings and structured formats need explicit preparation before onboarding;
  the module cannot honestly claim universal coverage until every candidate is
  ready or explicitly excluded.
- Translation workflow storage contains durable working copies and memory, so
  tenant isolation, retention, deletion propagation, backup/restore, and
  content-free observability become production requirements.

## Rejected alternatives

- **One shared business `translations` table:** rejected because it erases
  owner invariants and creates cross-module coupling.
- **Direct SQL adapters per owner inside `rustok-translation`:** rejected because
  they bypass owner services and cannot support remote modules safely.
- **Runtime fallback as translation progress:** rejected because rendered
  availability is not exact-locale completeness.
- **Translate every string/JSON leaf:** rejected because many strings are
  identifiers, configuration, secrets, URLs, templates, or immutable facts.
- **AI runtime writes owner rows:** rejected because generation, review, and
  domain persistence have different authority and consistency boundaries.
- **Product-specific translation prompts as the generic system:** rejected
  because copy generation and translation memory/workflow are separate
  capabilities.
- **Tenant runtime edits compiled UI bundles:** rejected for the initial module;
  static catalogs require a versioned artifact/release path.

## Implementation reference

The preparation gates, target provider contract, storage/workflow model, AI
adapter, rollout waves, and verification matrix live in the
[Translation Module Implementation Plan](../docs/modules/translation-implementation-plan.md).
