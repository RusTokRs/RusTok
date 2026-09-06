# rustok-blog implementation plan — slice 81 continuation

This document continues
`crates/rustok-blog/docs/implementation-plan-slice-80.md`. Slices 1–80 retain
the typed Comments remote boundary, bounded framing and listener lifecycle,
bearer-authenticated reads, signed user-write delegation, overlapping keyrings,
time-aware activation and retirement, strict process-local replacement, one
replay gate across key lifecycle transitions, an explicitly authorized
programmatic trigger, and one bounded process-local audit ring.

## 2026-08-01 continuation audit

Slice 80 prevents unaudited callers from reaching raw schedule mutation, but the
accepted generation exists only in memory. After restart, an operator could
supply an older schedule and the process would have no retained fact against
which to reject it. A same-generation schedule with different secret material
would also be indistinguishable from the previously accepted schedule.

Slice 81 adds a separate persisted trigger profile. It binds every accepted
generation to a canonical secret-inclusive SHA-256 schedule digest and requires
a host-owned durable compare-and-store boundary before the in-memory snapshot is
published.

This slice deliberately does not provide a database, filesystem, broker, cloud,
or sidecar implementation of that boundary. A concrete backend must satisfy the
strict atomicity contract below before restart rollback prevention can be
claimed for a deployment.

Tests, source verifiers, formatting, Cargo commands, TCP execution, workflows,
and CI remain intentionally unexecuted by request.

## Slice 81 — durable generation and canonical schedule digest contract

### Separate persisted trigger profile

`SharedCommentsTcpDelegationPersistedScheduleTrigger` is separate from the
process-local `SharedCommentsTcpDelegationScheduleTrigger`.

It can be constructed only from:

1. a canonical programmatic schedule document; or
2. one fixed version-2 schedule file path.

Construction also requires:

- the runtime maximum delegation TTL;
- the slice-80 mandatory authorizer;
- a `SharedCommentsTcpDelegationSchedulePersistenceStore`;
- an explicit startup mode;
- an audit capacity within the existing `1..=1024` bound.

The trigger guard rejects:

- persisted and process-local triggers together;
- either trigger together with a standalone schedule handle.

The trigger-owned read-only schedule handle is inserted before the unchanged
slice-79 runtime-policy guard and composition path. Static, ordinary reload,
environment-only schedule, and process-local trigger profiles remain available
unchanged when the persisted trigger is absent.

### Canonical programmatic document

`CommentsTcpDelegationSchedulePersistenceDocument` contains:

- generation;
- propagation budget;
- optional legacy-unkeyed key ID;
- 1..=8 scheduled key records.

Each key record contains:

- key ID;
- secret;
- activation timestamp;
- optional retirement timestamp.

The document and key `Debug` implementations redact key IDs and secrets.
Programmatic callers cannot provide a precomputed digest. The server builds the
typed Comments schedule and computes the digest from the same canonical source
document.

File-backed persisted composition reads the existing bounded version-2 schedule
schema from its fixed private path and converts it into the same canonical
document. The existing regular-file and 1..=65536-byte bounds remain in force.
Paths and operating-system details do not enter public errors or status.

### Secret-inclusive canonical digest

`CommentsTcpDelegationScheduleDigest` is a 32-byte SHA-256 value. The digest
input is a domain-separated, length-prefixed binary encoding of the complete
effective schedule configuration:

- propagation budget;
- runtime maximum TTL;
- Comments clock skew;
- optional legacy key ID;
- scheduled key count;
- keys sorted by activation timestamp and then key ID;
- every key ID;
- every exact validated secret byte sequence;
- activation timestamp;
- retirement presence and timestamp.

Generation and source category are not part of the schedule digest because they
are separate fields in the persistence record.

The domain separator is:

```text
rustok-comments-tcp-delegation-schedule-state-v1\0
```

Variable-length text is encoded as a big-endian 32-bit length followed by exact
bytes. Integer policy and lifecycle fields use big-endian encoding. Optional
values have an explicit presence byte.

The digest type exposes bytes and hexadecimal representation but custom `Debug`
does not print the value. The persisted record contains no key ID, secret,
schedule JSON, credential, nonce, token, or path.

### Persistence record

`CommentsTcpDelegationSchedulePersistenceRecord` contains only:

- persistence schema version 1;
- source category (`HostProvided` or `File`);
- accepted generation;
- canonical schedule digest.

A record binds the generation and exact secret-bearing schedule without storing
the secret material itself. Source category is immutable across replacement.

### Durable store contract

`CommentsTcpDelegationSchedulePersistenceStore` is host-owned and synchronous.
It provides:

- `verify_current(expected)`;
- `compare_and_store(expected, candidate)`.

The contract requires a conforming implementation to provide linearizable
process-visible operations.

`compare_and_store` must:

1. compare the complete expected record, including digest;
2. reject a missing, different, or concurrently advanced record as `Conflict`;
3. durably commit the complete candidate before returning success;
4. return `Unavailable` for an unavailable persistence boundary;
5. guarantee that **any error leaves durable state exactly unchanged**.

The final requirement is mandatory. A backend that can return an indeterminate
result after partially writing state does not satisfy this interface and must
not be used.

No free-form backend error text enters trigger errors or audit records.

### Explicit startup modes

Startup never silently creates or replaces persistence state.

`BootstrapEmpty` performs:

```text
compare_and_store(None, initial_record)
```

It succeeds only when the durable store is empty.

`ResumeExact` performs:

```text
verify_current(initial_record)
```

It succeeds only when source category, generation, and digest exactly match the
initial canonical schedule.

Consequently:

- an older generation fails startup;
- a newer but unexpected generation fails startup;
- the same generation with different key IDs, secrets, timestamps, legacy
  selection, propagation budget, TTL, or skew fails startup;
- a different source category fails startup;
- an unavailable store fails startup.

There is no resume-or-bootstrap fallback. Deleting persistence state does not
silently authorize a new baseline.

A schedule changed while the process was offline cannot be promoted
automatically because the prior full schedule is not retained in the persistence
record for overlap validation. The process must resume the exact accepted
schedule and use an authorized online replacement, or follow a separately
reviewed recovery procedure.

### Persist-before-publish replacement

Each persisted replacement retains the slice-80 operation mutex, principal
admission, host authorizer, audit mutex, and checked audit sequence.

After authorization:

1. the host document or fixed schedule file is parsed and converted into one
   fully validated typed schedule plus canonical digest;
2. the trigger locks its current persistence record and verifies that its source
   and generation match the active schedule selection;
3. the schedule handle acquires the slice-79 write lock;
4. source, increasing generation, active key, retained keys, secret immutability,
   activation, retirement, propagation, TTL, skew, legacy policy, and current
   signing-key availability are revalidated under that lock;
5. while the schedule write lock remains held, the durable store performs
   `compare_and_store(current_record, candidate_record)`;
6. only after durable success, the candidate snapshot is assigned in one
   infallible in-memory assignment;
7. the trigger updates its process-local current persistence record;
8. the final bounded audit outcome is appended before return.

A store conflict or unavailability leaves the old in-memory snapshot active.
A schedule validation error never calls the store.

After durable success there is no fallible operation between store return and
the complete schedule assignment. If the process terminates in that narrow
boundary, restart with the old schedule fails `ResumeExact`; restart must supply
the newly persisted generation and digest.

### Persisted trigger audit

The persisted trigger owns a separate bounded process-local audit ring using the
same 1..=1024 capacity and checked sequence policy as slice 80.

Closed outcomes are:

- `PreflightRejected`;
- `PrincipalIneligible`;
- `AuthorizationDenied`;
- `AuthorizationUnavailable`;
- `CandidateRejected`;
- `PersistenceStateMismatch`;
- `PersistenceConflict`;
- `PersistenceUnavailable`;
- `ReplacementRejected`;
- `ReplacementSucceeded`.

Records contain only sequence/time, request and actor UUIDs, typed principal
kind, operation, closed outcome, source category, and previous/candidate/current
generation metadata.

Digest values, paths, key IDs, secrets, documents, credentials, nonces, tokens,
raw schedule errors, backend errors, and arbitrary authorizer metadata are not
stored in the audit ring.

The process-local audit remains non-durable and is not transactionally bound to
the persistence store.

### Preserved behavior

This slice does not change:

- schedule file schema version 2;
- Comments lifecycle validation;
- activation, verification lead, retirement, overlap, TTL, and skew formulas;
- monotonic process-local schedule clock floor;
- keyed HMAC domain, key-ID binding, or delegation wire version;
- request, tenant, actor, operation, digest, correlation, idempotency, TTL, and
  clock verification;
- slice-78 replay continuity;
- service bearer reads and system moderation;
- external authority, channel, and provider precedence;
- loopback-only endpoint, bind, and peer policy;
- framing, deadlines, concurrency, listener lifecycle, and shutdown;
- manifest, feature, dependency, and lock-file state.

### Explicit non-claims

Slice 81 does not implement or claim:

- a concrete database, filesystem, broker, cloud, KMS, HSM, or sidecar
  persistence backend;
- runtime verification that a host store satisfies the atomic durability
  contract;
- durable audit or transactional outbox;
- an HTTP, GraphQL, native RPC, MCP, CLI, signal, watcher, or polling trigger;
- automatic offline schedule advancement;
- automatic recovery from lost, corrupt, or externally rolled-back durable
  state;
- multi-process or multi-replica compare-and-store coordination;
- synchronized clocks or distributed atomic activation;
- shared, durable, multi-replica, or restart-safe replay protection;
- secret zeroization, locked memory, or file permission/ownership attestation;
- TLS/mTLS or non-loopback publication;
- successful compilation, tests, source-verifier execution, formatting, TCP
  execution, database execution, browser execution, workflow execution, CI, or
  production validation.

Status: `source_verified_no_compile`.
Compile policy: `not_run_by_request`.
Runtime status: `not_run`.

## Next implementation results

1. Bind one concrete database or strongly atomic local persistence adapter to
   the durable-store contract, including crash and conflict evidence.
2. Persist the authorized audit/outbox record in the same transaction before
   claiming crash-safe audit completeness.
3. Define an explicit operator recovery ceremony for lost or corrupt durable
   state without silently resetting the generation baseline.
4. Add one concrete host transport only after direct-user/service admission,
   CSRF/replay policy, rate limiting, and response redaction are selected.
5. Replace process-local replay admission with a bounded shared store before
   claiming multi-replica or restart-safe replay prevention.

## Suggested verification — intentionally not run

- `node scripts/verify/verify-blog-comments-tcp-delegation-schedule-persistence.mjs`
- `node scripts/verify/verify-blog-comments-tcp-delegation-schedule-trigger.mjs`
- `node scripts/verify/verify-blog-comments-tcp-delegation-key-schedule.mjs`
- `cargo test -p rustok-server --features mod-blog comments_provider_runtime`
- `cargo check -p rustok-server --features mod-blog --locked`

## Ownership retained

- Comments owns lifecycle validation, effective keyring selection, signing,
  verification, request binding, and process-local replay admission.
- The server host owns canonical source acquisition, digest construction,
  persistence composition, mutation authorization, process-local audit, runtime
  TTL selection, provider composition, listener lifecycle, concurrency, and
  shutdown.
- A concrete persistence implementation owns linearizable compare-and-store,
  durability, integrity, backup, recovery, and operational evidence.
- Blog remains transport-neutral and owns authenticated rendering and degraded
  presentation only.
