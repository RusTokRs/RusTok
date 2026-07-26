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

The adapter receives bounded exact-locale segments, placeholders, field
semantics, glossary revision, memory suggestions, context, and data
classification. It returns typed, deterministically validated proposals.
Initial AI results always require human review and never write owner data.

Automatic approval or publication requires a later accepted decision, explicit
tenant policy, measured locale-pair evidence, and deterministic safety/quality
thresholds.

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
