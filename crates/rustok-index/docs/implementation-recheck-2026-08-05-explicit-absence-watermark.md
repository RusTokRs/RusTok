# `rustok-index` implementation recheck — explicit absence watermark

Audited baseline: `main@368c79b78549e97a68120358021552b2552b800c`.
Latest default-branch delta checked through
`main@dd9020b8d259d9466423c47bd92b2c93c6c39225`.

Forty-seven commits are present on `main` after the branch merge base. They cover Commerce
diagnostics, Forum GraphQL/admin work, Pages/Page Builder delivery and routes, event settings, and
supporting server configuration. The latest commit adds bounded Pages historical-route import. No
checked main commit modifies the Index files, Product Index composition, Index GraphQL transports,
Index diagnosis/page composition, or Index guards changed here.

Rechecked predecessor: PR #2983 at
`cea5e0544049c0d9610b85de67f53b9c7e6a02d4`.

## Exact diagnosis and absence proof

The branch retains:

- request-bound `modules:manage` authorization before exact-key validation or dependency access;
- one exact tenant-bound `EntityKey` per caller-known diagnosis;
- Product v1/v2 locale absence proof using positive `products.index_revision` via `product-locale-absence-postgres`;
- source-state and watermark double-read fencing around one read-only repeatable-read materialized
  snapshot, returning `index_drift_source_changed_during_capture` on concurrent changes;
- permanent `index_drift_source_watermark_missing` failure when authoritative absence proof is unavailable;
- a source-ready concurrent Product locale absence PostgreSQL harness.

General exact mismatch diagnosis remains separate from missing-only discovery.

## Missing-only source page

`IndexDriftSourcePageDiagnosisRuntime` scans at most one owner page with a limit in `1..=32`, skips
retained source deletes, and delegates source-present candidates sequentially to
`diagnose_missing_entity_candidate`.

Only source `Upsert` plus materialized `Missing` records a finding. Every other typed state
combination returns `NotCandidate` without recorder access.

## Confidential continuation and keyring

`IndexSourceContinuationCodec` encrypts the complete owner cursor with AES-256-GCM and binds it to
tenant, exact schema, canonical owner/source identity, version, issued-at, and expiry.

The server keyring configuration:

- is bounded to 16 KiB before JSON parsing;
- stores only bounded key IDs and `SecretRef` values;
- admits at most 16 unique references;
- bounds key IDs to 64 bytes and reference keys to 256 bytes;
- requires canonical 43-byte URL-safe unpadded base64 secrets decoding to exactly 32 bytes;
- retains one active sealing key and optional decrypt-only rotation keys.

Asynchronous secret resolution occurs inside the sealed request after authorization and page-limit
validation but before token parsing or source scan. Raw key material is used only by one local codec
and is not inserted into settings, runtime extensions, errors, logs, or debug output.

`diagnose_source_page_sealed` opens the token before constructing `IndexSourceScanRequest`, diagnoses
one page exactly once, and seals the outgoing raw cursor before returning.

## Bounded GraphQL source-page transport

`apps/server/src/graphql/index_drift_source_page_diagnosis.rs` mounts:

- `diagnoseIndexSourcePage(input: IndexDriftSourcePageDiagnosisInput!)`.

The resolver:

- derives tenant and actor only from authenticated context;
- checks effective `modules:manage` before schema, limit, or continuation parsing;
- accepts one bounded module/entity/version identity, one limit string in `1..=32`, and one optional
  opaque token bounded to 16 KiB;
- accepts no tenant, actor, source name, owner module, raw cursor JSON, entity ID, entity list,
  checkpoint, scheduler, lifecycle, or repair input;
- delegates exactly once to `diagnose_source_page_sealed`;
- returns only current-page counters, bounded finding receipts, completion state, and one optional
  opaque continuation token.

The exact-entity resolver remains separate and has not gained source-page authority.

Transport error mapping exposes fixed codes for invalid/expired continuation, unavailable keyring,
source dependency failure, snapshot capture failure, and finding recording failure. Resolver causes,
source names, secret references, token contents, raw cursor JSON, SQL, and database causes are not
exposed.

## Source review findings

Source review confirmed:

- the GraphQL limit is a string, so semantic parsing remains after authorization;
- the continuation is only length-bounded in the resolver and is parsed/authenticated by the sealed
  service boundary;
- the resolver calls the sealed method exactly once and never calls the raw page method;
- GraphQL payload types contain no `IndexSourceCursor` or keyring type;
- the root schema mounts the new mutation as a separate merged object;
- source-page work remains one page only with no loop, persistence, scheduler, or repair authority.

PR #2986 had no conversation comments, submitted reviews, or review threads at the last review.

## Open cursor

The next implementation cursor is a bounded database-neutral contract for stale Index-only entities
and orphan links. It must avoid unbounded table scans and arbitrary in-memory ID collection, keep
continuations server-owned, and remain separate from lifecycle and repair authority.

Retained authorization, secret-resolution, rotation, expiry, PostgreSQL, and GraphQL execution
evidence remains owner-run and pending.

## Validation ownership

Per maintainer instruction, this implementation agent did not run tests, JavaScript verifiers,
formatting, Cargo checks, cryptographic integration, PostgreSQL or GraphQL scenarios, workflows, or
CI.
