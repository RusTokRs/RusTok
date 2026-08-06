# `rustok-index` implementation recheck — explicit absence watermark

Audited baseline: `main@368c79b78549e97a68120358021552b2552b800c`.
Latest default-branch delta checked through
`main@9822131a59619805dc34b67fdcecf44f9bbcd766`.

Forty-four commits are present on `main` after this branch merge base. They touch Commerce
diagnostics, Forum GraphQL/admin and route ownership, Pages/Page Builder route and delivery work,
event-delivery settings, and general server configuration. They do not modify `crates/rustok-index`,
Product Index composition, the server Index GraphQL files, Index diagnosis/page composition, or
Index verifier files changed by this branch. `apps/server/Cargo.toml` changed on `main`; this branch
does not modify the server manifest in the continuation slice.

Rechecked predecessor: PR #2983 at
`cea5e0544049c0d9610b85de67f53b9c7e6a02d4`.

## Rechecked guarded diagnosis

The guarded exact-entity diagnosis remains internally consistent at source level:

- authorization is request-bound and precedes digest request validation and dependency access;
- one typed `EntityKey` must match the authorized tenant;
- composition reuses frozen source/schema registries;
- exact GraphQL accepts no caller-selected tenant or actor;
- missing-only discovery remains separate from general caller-known exact mismatch diagnosis;
- no repair, lifecycle, scheduler, raw registry, or database handle is exposed.

PR #2986 had no conversation comments, submitted reviews, or review threads at this recheck. Tests
and retained execution evidence remain owner-owned and pending.

## Explicit absence contract

The branch retains the database-neutral optional absence registry without weakening ordinary targeted
load:

- `IndexSourceAbsenceWatermark` carries one exact typed key and one positive source version;
- provider names, schema sets, and schema-identity ownership are bounded and deterministic;
- materialization requires the frozen canonical replay source registry;
- every provider owner must equal the replay source owner for every exact schema;
- registry lookup performs one exact call and rejects cross-scope evidence;
- existing `IndexSource::scan` and `IndexSource::load` remain source-compatible.

The selected Product bridge registers `ProductLocaleAbsenceProvider` as
`product-locale-absence-postgres` for Product schema versions 1 and 2. It returns positive
`products.index_revision` only when the live Product exists, the exact translation locale is absent,
and no exact Product tombstone owns that locale.

The reader compares typed `Missing` plus the positive absence version around its materialized
PostgreSQL snapshot. A changed version, newly appearing source mutation, or lost proof after the
first positive observation returns retryable `index_drift_source_changed_during_capture`. The
absence version is domain-tagged into the opaque boundary only for source `Missing`; existing
Upsert/Delete boundary derivation remains unchanged.

Missing registration, provider `None`, key mismatch, zero version, and malformed evidence remain
fail-closed. An empty targeted load alone still returns `index_drift_source_watermark_missing`.

## Source-ready PostgreSQL continuation

`crates/rustok-distribution/tests/product_locale_absence_postgres.rs` retains the real-migration
Product locale absence scenario without replacing either production adapter. It composes production
sources and the production snapshot reader, verifies stable exact locale absence, and performs a
deterministic concurrent translation insertion while the exact materialized read is blocked.

The harness is `source_ready_owner_execution_pending`. Its presence is not retained execution
evidence; the repository owner must run and admit its PostgreSQL output.

## Bounded GraphQL transport continuation

`apps/server/src/graphql/index_drift_diagnosis.rs` mounts one exact caller-known mutation:

- tenant and actor come from authenticated request context;
- effective `modules:manage` is checked before parsing untrusted identity strings;
- every string is bounded before domain parsing;
- one tenant-bound `EntityKey` is built;
- the resolver delegates once to guarded `diagnose_entity`;
- output contains only bounded digest and finding-receipt metadata.

The transport performs no SeaORM query and owns no source scan, source continuation, scheduler,
finding lifecycle, or repair capability.

## Missing-only outcome continuation

`IndexDriftDigestProducer` preserves general `produce(request)` behavior and adds a separate
missing-only path:

- every captured source/materialized state is exact-scope checked and validated before
  classification;
- only source `Upsert` plus materialized `Missing` computes and records a mismatch;
- source `Missing`/`Delete` and materialized `Upsert`/`Delete` return `NotCandidate` without recorder
  access;
- stale fields, stale links, and source-version-only differences are not recorded through this path;
- `IndexDriftMissingEntityCandidateOutcome` exposes no raw key, record, state, or boundary.

`IndexDriftSourcePageDiagnosisRuntime` scans at most one owner page with limit `1..=32`, skips
retained source deletes, sequentially invokes missing-only exact diagnosis, and returns only aggregate
counts, missing finding receipts, and one internal server-held cursor through its historical raw
method. It owns no loop, checkpoint, scheduler, public transport, lifecycle, or repair capability.

## Confidential source continuation

`IndexSourceContinuationCodec` is source complete as a database-neutral transport prerequisite. It
is intentionally distinct from the query `CursorCodec`, whose checksum is not keyed encryption.

The source continuation contract:

- constructs canonical tenant/schema/owner/source scope only from the frozen
  `SharedIndexSourceRegistry`;
- encrypts the complete raw `IndexSourceCursor` with AES-256-GCM and a fresh 96-bit OS nonce;
- authenticates a domain, version, and bounded key id as additional data;
- keeps tenant, exact `SchemaRef`, canonical owner/source identity, issued-at, expiry, and raw cursor
  only inside ciphertext;
- allows one active sealing key and bounded retained decrypt-only rotation keys;
- limits lifetime to 1 second through 15 minutes and future clock skew to 30 seconds;
- independently bounds encoded input, decoded envelope, and decrypted claims;
- rejects tampering, unsupported versions, tenant/schema/source mismatch, invalid lifetime, future
  issuance, expiry, and unavailable or retired keys before returning raw cursor state;
- redacts key bytes and token content from `Debug`.

The codec itself reads no environment, configuration, database, or secret resolver.

## Server continuation keyring and sealed page boundary

The server now composes a deployment-owned keyring from
`RUSTOK_INDEX_SOURCE_CONTINUATION_KEYRING_JSON` when a frozen source registry exists.

The configuration contains only:

- one active key ID;
- one bounded lifetime;
- at most 16 key-ID-to-`SecretRef` mappings.

This slice admits only deployment-owned `env` and `mounted_file` aliases. References are validated
against exact resolver policy. Duplicate references, missing active key, invalid key IDs, unsupported
resolver aliases, and out-of-range lifetime fail synchronous composition with one generic server
message.

Secret resolution is asynchronous. Therefore actual values are resolved inside
`diagnose_source_page_sealed` after authorization and page-limit validation but before token parsing
or source scan. Every secret must be URL-safe unpadded base64 and decode to exactly 32 bytes. Missing,
forbidden, malformed, or wrong-length material returns a bounded continuation-keyring error without
resolver cause, reference key, or secret value.

The decoded map constructs one short-lived `IndexSourceContinuationCodec`. The sealed method then:

1. derives canonical tenant/schema/owner/source scope from the frozen registry;
2. opens the incoming token before `IndexSourceScanRequest` construction;
3. executes exactly one existing missing-only page path;
4. seals any outgoing raw cursor;
5. returns `IndexDriftSourcePageDiagnosisSealedOutcome` containing only bounded counters, receipts,
   completion state, and one optional opaque token.

Raw key bytes, the local codec, and the decoded map are dropped after the request. The keyring is
passed privately into the page runtime and is not inserted as its own `ModuleRuntimeExtensions`
capability. Its `Debug` output excludes secret references and key values.

The sealed method is not attached to GraphQL, HTTP, CLI, MCP, or native admin by this slice. The
existing exact-entity GraphQL mutation remains unchanged.

## Open cursor

The next implementation step is one bounded source-page transport over
`diagnose_source_page_sealed` only. It must derive authority from authenticated context, authorize
before parsing schema/limit/token input, accept no tenant, actor, source identity, raw cursor, or
entity ID list, delegate once, and return only bounded aggregate counters, receipts, completion state,
and an opaque token.

Cursor persistence, multi-page lifecycle, stale Index-only discovery, orphan-link diagnosis,
automatic finding resolution, resolve/ignore commands, and repair remain open.

## Validation ownership

Suggested commands are retained in the dated implementation plan and the continuation, absence,
harness, GraphQL, digest-producer, and source-page documents. Per maintainer instruction, this
implementation agent did not run tests, JavaScript verifiers, formatting, Cargo checks,
cryptographic integration, PostgreSQL or GraphQL scenarios, workflows, or CI.