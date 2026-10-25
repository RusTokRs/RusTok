# Logic Audit R2 — rustok-modules (2026-10-23)

Second complete re-review of the Core control-plane crate `rustok-modules`
(`crates/modules/rustok-modules`, slug `modules`; 232 files, ~123.7k lines at
review start), executed per the repository governance contract (`AGENTS.md`).
Every subsystem was re-inspected against the six Universal Logic Dimensions
(preconditions and bounds; branch completeness and anti-swallowing;
state-transition integrity; side-effect completeness including cache
invalidation; calculation/algorithmic correctness; order and concurrency /
TOCTOU), following the Phase A–H remediation recorded in
`docs/logic-audit-remediation-2026-10-21.md`. Every real defect found was
remediated in-crate in the same change set (AGENTS.md §6); suspicious code
proven sound is recorded as adjudicated so it is not "re-fixed" later.

Base: `dd7ac39` (tip of the remediation + internal-quality repair series).
Change set after the initial pass and continued review: 15 fix commits plus 4 documentation commits; 51 files, +1,478/−571.

Evidence status: the authoring sandbox has no Rust toolchain; no local
`cargo` evidence is claimed. All changes were verified by full-source
censuses, call-site sweeps, and line-level diff self-review. The maintainer
runs the test suite, Clippy, and rustfmt on this branch.

## 1. Defects found and fixed

### R2-1. Trust decisions accepted under the wrong policy revision — `4979f87`
OCI admission and external-prebuilt ingress accepted a trust decision via
`admitted()` alone and defaulted absent policy revisions with
`unwrap_or(0)`, so a release verified under trust/capability policy revision
A could be admitted or re-used while the platform required revision B.
Admitted OCI rows store no trust basis (migration
`m20261022_000057`), so already-admitted reuse must re-verify, not trust the
row. Fix: canonical owner `TrustVerificationDecision::admitted_for`
(exact `trust_policy_revision` + `capability_policy_revision` equality AND
`admitted()`); both ingress commands now require non-zero `u64` revisions
(was `Option<u64>`); verification runs on the fresh-admission path and on
already-admitted reuse in `installation.rs`, `oci_admission.rs`,
`external_prebuilt_ingress.rs`, and `publication_evidence.rs`. Regression
tests cover revision mismatch on both paths.

### R2-2. Fabricated queue-drain receipts; cause-swallowing execution audit — `7cc53f6`
`queue_drain.rs` mapped COUNT/decode failures to a receipt claiming an empty
queue, so a broken store read was indistinguishable from a drained tenant.
Fix: count failures propagate as errors; counts convert with checked
`u64::try_from`. `execution_audit.rs::persist` swallowed database causes
behind a log line; it now returns `SandboxResult<()>`, preserves the cause,
binds datetimes through the canonical backend converters (new
`optional_datetime_value` owner), sets platform scope for tenant-less rows,
and reads timestamps back RFC 3339-normalized.

### R2-3. SQLite timestamp text had two representations — `2010116`
sqlx encodes `DateTime<Utc>` binds as RFC 3339 (`…T…+00:00`), while SQL-side
producers used SQLite `datetime('now')` space-form (`… …`). Raw lexical
comparisons between the two forms are non-chronological on the same day;
concretely, governance claim-expiry comparisons kept expired claims alive
for up to a day. Fix: exactly one canonical SQLite timestamp text,
`backend.now_sql()` = `strftime('%Y-%m-%dT%H:%M:%f+00:00','now')`, matching
the bind encoding; `now_rfc3339_sql` deleted; every expiry/backoff/lease/
settings/installation/secrets/recovery writer unified; `operation_store`
templates gained a `{now}` token in `render_parameters`. Readers still parse
the legacy space form via `parse_sqlite_datetime`. PostgreSQL unaffected.
Intentional residuals (parse-safe): `DEFAULT CURRENT_TIMESTAMP` DDL defaults
can still emit space-form if a writer omits the column; applied migrations
and governance test seeds keep `datetime('now')` (applied migrations are
never edited); pre-existing dev-database space-form rows may order against
RFC 3339 rows (pre-release, accepted).

### R2-4. Digest validation had five owners — `8ead4ab`
Four private `sha256:` validators duplicated two semantics (lowercase-
canonical vs any-case) with drift risk, and snapshot readiness accepted a
bare `sha256:` prefix as a valid manifest digest. Fix: private copies
deleted; all callers use `trust::is_sha256_digest` (lowercase canonical) or
`installation::valid_digest` (any-case), strictness preserved per caller;
snapshot readiness now requires a full digest.

### R2-5. Transition finalize ran unscoped; schedule slots claimable early — `f2637ee`
`transition_service::finalize` was the last production transaction opened
without setting tenant/platform RLS scope (re-census: 181 production begins;
the 8 remaining scope-less begins are test code). It now scopes from
`command.context.tenant_id` immediately after `begin`, mirroring the
watchdog. `schedule_delivery::enqueue` bound `available_at = now`, so the
public enqueue API allowed claiming a future-dated slot immediately (the
sole production caller enqueues only due slots, so no live early execution
existed). It now binds `available_at = scheduled_for`.

### R2-6. HTTP capability broker: fail-open DNS, rebinding window, unbounded body — `4f2257f`
`validate_target_address` skipped the SSRF check entirely when DNS
resolution failed (`if let Ok(addrs)`), and the address it checked was not
the address reqwest connected to (second resolution → DNS-rebinding
TOCTOU). Response bodies were buffered in full before the size cap applied
(chunked responses have no Content-Length), enabling memory exhaustion.
Guest-supplied `Host`/framing headers passed through unchanged. Fix:
resolve exactly once, fail closed on resolution error or empty answer,
reject if any resolved address is disallowed, and pin the request to the
validated address with `ClientBuilder::resolve`; body read streams under
the cap; hop-by-hop/`Host`/`Content-Length` headers are rejected; scheme
restricted to http/https at the broker. `is_disallowed_ip` now also covers
multicast, CGNAT 100.64/10, 0.0.0.0/8, 192.0.0.0/24, 198.18/15, 240/4,
documentation ranges, IPv6 multicast/documentation, and IPv4-mapped IPv6
literals (checked under IPv4 rules). Grant constraints
(`HttpCapabilityConstraints`) are enforced by the sandbox runtime before
broker invocation; the broker remains the SSRF mechanism boundary.

### R2-7. GC tombstone state machine had three transition gaps — `5f61f06`
In `gc.rs` (design-accepted in-memory coordinator, §5.2 of the remediation
record): (a) `collect_with_token` burned the execution token before the
adapter purge ran, so a purge failure stranded the tombstone in
`FinalRecheckPassed` forever — the token is now restored on purge failure;
(b) a target in `FinalRecheckPassed` could be tombstoned a second time —
that status now blocks re-tombstoning; (c) a `Collected` tombstone could be
"revoked", rewriting executed-purge history, and revocation left live
execution tokens behind — revoking `Collected` is now a typed error
(`GcError::InvalidTombstoneTransition`), revoking `Revoked` is idempotent,
and revocation invalidates the tombstone's outstanding tokens.

### R2-8. Tenant-module writes reported success without affecting a row — `a177641`
`operation_store::{record, upsert state, persist_settings}` selected the
`tenant_modules` row and then updated it by id through a helper that
discarded `rows_affected`; a concurrently deleted row produced a success
record for a write that never happened. The helper is now `execute_one`:
all five call sites (three INSERTs, two by-id UPDATEs) require exactly one
affected row.

### R2-9. Operator status projection mislabeled serving releases — `db93b31`
`operator.rs::get_status` projected only the newest installation and matched
its admission status against a vocabulary containing a non-existent
`"failed"` arm while every other status fell into a silent `Observing`
catch-all. With a rolled-back candidate newest, the actively serving
predecessor was presented as not serving; a retired newest candidate hid a
serving predecessor behind `Cancelled`. Fix: derivation is serving-aware
(newest `active` row is the current identity; work-generation/retired and
retention-hold count read from the serving basis) and exact over the real
admission vocabulary (`admitted`, `installed`, `inactive`, `active`,
`rolled_back`): pending candidates present as `Running`, a rolled-back
newest with a serving predecessor presents as `Recovered`, a rollback with
no serving predecessor and any unknown status present as
`RecoveryRequired` with a naming diagnostic. Retired candidates no longer
appear as pending candidates. Single-installation presentations pinned by
existing tests are unchanged.

### R2-10. Conflict fences were documented as enforcement — `bc5acc9`
`ConflictFenceSet` values are derived, ordered, and recorded on transition
checkpoints/receipts; they are descriptive evidence, not a mutual-exclusion
mechanism. `has_conflict_with` is test-only and `start_transition` is an
unwired constructor. Real durable exclusion remains the per-installation
admission-revision CAS and the tenant-lifecycle same-transaction active
transition check keyed by tenant and slug. Documentation now says this
honestly; no partial cross-transition fence lock was introduced.

### R2-11. Resolver caret bound and infallible trait boundary — `92367b6`
The custom PubGrub range conversion mapped bare `^0` to `<0.1.0`, while
semver admits all `0.x` versions (`<1.0.0`); candidates could pass
`VersionReq::matches` but be rejected by the solver, producing false
conflicts. Fixed the bound and added cases for `^0`, `^0.0`, and `^0.0.3`.
A parse-valid requirement whose upper bound overflows `u64` also reached an
`expect` inside PubGrub's infallible `get_dependencies` trait method. Snapshot
validation now proves every dependency range is representable and reports a
typed error before solving; the trait adapter returns `Unavailable` rather
than panicking. Provider request de-duplication now uses a `(slug, req)` tuple
instead of a delimiter-concatenated string, eliminating key collisions.

## 2. Continued adversarial review after the initial R2 pass

The follow-up deep read covered `mcp.rs`, `secrets.rs`, OCI transport and
publication, the structured/binary data brokers, resumable uploads, namespace
purge, and the snapshot/restore/collection paths. Findings below were fixed
in the same crate; the user runs tests/checks because this sandbox has no
Rust toolchain.

### R2-12. Secret binding could rotate after resolution but before use — `be0026da`, `340141fc`
`use_secret` read the exact bound revision, committed, then awaited an
external resolver and passed the resulting `SecretString` to its consumer
without rechecking the binding. A concurrent unbind/rotation during that
await could consume a value after its authorization basis had changed. It
now takes a tenant-scoped write lock on the exact binding revision before
resolution and holds it through the fixed trusted consumer, preventing a
rotation between the final revision check and use. Stored non-positive
revisions are reported as corrupted/conflicting state, not disguised as a
missing handle. Plain handle serialization failure is an internal host
error, not a guest denial.

### R2-13. Object-only capability skipped shared namespace resolution — `0789b030`
`platform.data.objects` derived the installation's namespace ID directly,
while `platform.data` first resolved the tenant's serving owner reference
(and initializes the first active namespace). Object-only grants could not
write until an unrelated structured-data call initialized that namespace,
and later installations could receive a stale namespace ID. The object
resolver now calls the same serving-scope resolver before quota and broker
construction.

### R2-14. Upload chunk sequence allowed unbounded row amplification — `0789b030`
A one-byte chunk was valid, and the byte quota therefore still allowed tens
of millions of chunk rows; an arbitrary large sequence could also poison a
session that completion requires to be contiguous from zero. Added one
canonical 1,024-chunk/session bound at request validation and in the
unreleased upload-chunk migration. Completion reads at most 1,025 rows and
fails closed if stored metadata exceeds the bound. A boundary test covers
sequence 1,023 versus 1,024.

### R2-15. Namespace purge left staged upload bytes unreachable — `0b0151f3`
The irreversible namespace purge removed structured records and completed
object metadata but did not retire open/completing upload sessions or their
external chunk objects. Those bytes were inaccessible after purge and only
queued for GC after ordinary session expiry. Purge now queues every staged
chunk key as a retention-aware GC candidate, then deletes chunks and sessions
inside the same tenant-scoped purge transaction. The existing SQLite purge
test now seeds a live session/chunk and asserts session/chunk removal plus
one durable GC candidate.

### R2-16. Data broker errors conflated guest constraints with grant denial — `0789b030`
Value/quota limits, revision conflicts, idempotency conflicts, malformed
keys/pages/objects, and schema violations had been mapped to generic
`CapabilityDenied`, making normal guest correction indistinguishable from a
missing grant. They now map to `LimitExceeded` or
`CapabilityConstraintDenied`; purged/held namespace state is a constraint,
not a missing grant; storage/integrity/index-contract failures map to host
capability failure; actual policy/scope denial stays denied. Zero expected
revisions and nil idempotency keys are classified separately.
Rollback errors in chunk writes are no longer silently discarded: a typed
`RollbackFailed` retains both the primary operation error and rollback cause.

## 3. Adjudicated clean paths and explicit residuals

- `capability_events.rs` and `mcp.rs`: input fields are allowlisted, execution
  and tenant/subject scope are checked, event constraints are enforced by the
  sandbox before broker invocation, and MCP routes only logical aliases to a
  host-owned invoker; MCP output is bounded.
- `oci.rs` and `oci_transport.rs`: digest-pinned identity, TLS/no-proxy/no-
  redirect policy, bearer challenge origin handling, transfer bounds,
  streamed payload hashing, and publication evidence validation were read
  through; no new defect was found in this continuation.
- Snapshot restore/collection use durable copy intents, exact metadata and
  byte receipts, namespace/snapshot locks, active restore holds, and
  resumable `collecting` state. No new state-transition defect was found in
  the reviewed paths. Snapshot holds and external retention policy remain
  independent policy inputs as designed.
- Existing review findings remain: conflict fences are descriptive rather
  than enforcement; GC coordinator in `gc.rs` is design-accepted/in-memory;
  Phase F FORCE RLS remains blocked on role separation and PostgreSQL
  evidence; DDL timestamp defaults and already-applied historical migrations
  retain the documented parse-safe space-form residual; raw snapshots can
  differ from effective policy; execution-audit RLS breadth and hand-written
  DDL test fixtures remain in the Phase F inventory.

## 4. Verification boundary

No `cargo`, `rustc`, or `rustfmt` is installed in this sandbox. I did not
claim tests, type-checking, Clippy, or rustfmt evidence. The author verifies
this branch. Manual verification was limited to source/call-site sweeps,
SQL bind/placeholder review, migration ordering, `git diff --check`, and
line-level review. The added regression tests are present but unexecuted.

## 5. 2026-10-25 mainline integration reconciliation

The refreshed `main` moved `ModulesModule` into `src/module.rs`. Its migration
ordering now uses the module-owned migration identifiers from this branch, and
`src/lib.rs` re-exports that single implementation. The crate API contract added
on `main` did not match this branch's current public surface; it was reconciled
to the actual public modules, root re-exports, owner boundaries, and typed errors,
and is included in crate rustdoc. The root README now summarizes production
gaps already tracked in the owner documentation and implementation plan.

No tests, compiler checks, formatters, or verification scripts were run for this
integration. The maintainer's verification remains outstanding.
