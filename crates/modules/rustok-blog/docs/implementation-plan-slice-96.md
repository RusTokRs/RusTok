# rustok-blog implementation plan — slice 96 continuation

Status: `restart_ambiguity_postgres_evidence_source_ready_maintainer_execution_pending`.

## Goal

Retain maintainer-run PostgreSQL evidence that the Blog Comments schedule-audit
source handoff and audited recovery owners survive owner/process reconstruction
and reconcile a committed state when the caller treats the commit acknowledgement
as unavailable.

This slice does not inject a database-driver failure into production code. The
harness performs a successful owner commit, discards the original owner instance,
opens a new PostgreSQL connection in the same isolated schema, constructs a new
owner, and invokes the exact private reconciliation path through a `cfg(test)`
wrapper. This models the durable state visible after an ambiguous acknowledgement
without changing production transaction semantics.

## Retained scenarios

### Active claim acknowledgement

1. Apply the real Outbox migration and Blog audit migrations `000007`–`000011`.
2. Seed one pending source row.
3. Claim it through `claim_next_retry_ready(8)`.
4. Drop the original handoff owner.
5. Reconstruct the owner on a new PostgreSQL connection.
6. Reconcile the exact claim token and require the same request, token, and
   attempt count.
7. Reject an unrelated token with the closed `Unavailable` outcome.

### Expired claim restart recovery

1. Persist a first retry-aware claim.
2. Move only its expiry into the past to represent a stopped worker whose lease
   elapsed.
3. Reconstruct the owner on a new connection.
4. Require a second claim for the same source request at attempt two with a new
   token.
5. Require the old token to fail reconciliation and the new token to reconcile.

### Publication acknowledgement

1. Claim a pending source row through the production handoff owner.
2. Publish it with the production
   `RustokOutboxCommentsTcpDelegationScheduleAuditCanonicalWriter`.
3. Require one atomic source terminal pair and one exact canonical `sys_events`
   envelope.
4. Drop the original owner and reconstruct it on a new connection.
5. Reconcile by exact request ID and require the stored envelope identity.
6. Require the canonical row to remain `pending`, unclaimed, and at retry count
   zero. No relay task is started.

### Requeue acknowledgement

1. Seed one exact `attempt_budget_exhausted` source dead letter.
2. Requeue it through the production recovery store.
3. Drop the original store and reconstruct it on a new connection.
4. Reconcile by exact audit ID plus tenant, request, actor, reason, prior attempt,
   and recovery epoch facts.
5. Require a modified reason to fail with `InvalidStoredState`.
6. Require one reset source row and exactly one immutable recovery audit fact.

## Test-only seams

The handoff and recovery modules expose only `pub(crate)` methods under
`#[cfg(test)]` that delegate directly to the existing private reconciliation
functions. They do not add a production method, configuration value, task,
transport, database table, or alternate SQL implementation.

## Preserved boundaries

- Production claim, publication, recovery, retry, worker, bootstrap, lifecycle,
  shutdown, listener, and authorization code are unchanged.
- `rustok-outbox` remains the sole canonical event admission and relay owner.
- This slice proves canonical admission only; relay delivery, relay retry/DLQ,
  restart of the relay process, and transport acknowledgement remain separate.
- No HTTP, GraphQL, CLI, MCP, admin endpoint, automatic requeue, bulk recovery,
  second worker, second relay, or replacement source row is added.
- Source-row and recovery-audit retention remain open.

## Suggested maintainer execution

```bash
export RUSTOK_BLOG_COMMENTS_AUDIT_TEST_DATABASE_URL=postgres://...

cargo test -p rustok-server \
  --no-default-features \
  --features mod-blog \
  comments_provider_runtime::keyring_schedule_audit_operator::retained_restart_ambiguity_evidence \
  -- --ignored --nocapture --test-threads=1

node scripts/verify/verify-blog-comments-audit-restart-ambiguity-evidence.mjs
```

Cargo checks, Rust tests, PostgreSQL scenarios, verifier execution, formatting,
Clippy, workflows, runtime, and production validation were not executed by the
implementation agent.

## Next cursor

Retain canonical relay restart, delivery acknowledgement, retry, and DLQ evidence
as a separate `rustok-outbox`-owned slice. Then define source-row and immutable
recovery-audit retention without weakening exact identities or append-only audit
ownership.
