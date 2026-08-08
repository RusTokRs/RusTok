# rustok-blog implementation plan — slice 97 continuation

Status: `canonical_outbox_relay_postgres_evidence_source_ready_maintainer_execution_pending`.

## Goal

Close the next source cursor from slice 96 without creating another Blog relay.
The canonical Blog Comments delegation-schedule audit event is already admitted
transactionally into `rustok-outbox`; delivery, retry and DLQ ownership therefore
remain entirely in `rustok-outbox`.

This slice retains one outbox-owned, maintainer-run PostgreSQL harness proving the
exact sealed Blog Comments audit envelope through the existing canonical relay.
No production Blog, Events or Outbox behavior changes.

## Ownership boundary

The source chain remains:

```text
Blog audited schedule owner
-> Blog canonical writer
-> TransactionalEventBus::publish_contract_once_direct_in_tx_with_envelope_id
-> durable sys_events pending row
-> rustok-outbox OutboxRelay
-> configured EventTransport
```

The Blog audit `request_id` remains the canonical envelope UUID and correlation
UUID. `rustok-outbox` remains the only relay/retry/DLQ owner. The evidence target
is a test-only `EventTransport`; it does not add a second relay, dispatcher,
worker, queue or application transport.

## Retained PostgreSQL scenarios

### Retry -> owner reconstruction -> delivery acknowledgement

The harness:

1. creates an isolated PostgreSQL schema and applies the real Outbox migrations;
2. writes the registered `blog.comments_delegation_schedule.replacement_succeeded`
   contract through the same write-once primitive used by the Blog canonical writer;
3. requires the durable row to be `pending` before relay;
4. runs relay worker `blog-audit-relay-before-restart` against a target that fails
   exactly once;
5. requires the same row to remain pending, increment retry count, clear claim
   ownership, retain a retry schedule and keep `dispatched_at = NULL`;
6. constructs a new relay with worker identity
   `blog-audit-relay-after-restart` over the same PostgreSQL state;
7. requires the restarted relay to deliver the same sealed envelope exactly once;
8. requires event id, correlation id, tenant, actor, request id, event type and
   schema version to remain exact;
9. only after target success may the durable row become `dispatched`, set
   `dispatched_at`, and clear retry error/schedule/claim state.

This is relay-owner reconstruction evidence, not an assertion that a complete
server process was restarted.

### Retry -> owner reconstruction -> DLQ

A separate exact Blog audit envelope uses `max_attempts = 2` and a target that
continues to fail.

The first relay attempt must leave the row pending at retry count one. A newly
constructed relay with a distinct worker identity performs the second attempt.
Attempt-budget exhaustion must retain the same envelope row with:

```text
status = failed
retry_count = 2
next_attempt_at = NULL
dispatched_at = NULL
claim = cleared
last_error = present
```

No successful target delivery may be recorded for that envelope. This proves the
existing Outbox `Failed`/DLQ transition for the Blog sealed contract; it does not
add Blog-owned dead-letter state or automatic replay.

## Machine evidence

Source packet:

`crates/rustok-outbox/contracts/evidence/blog-comments-audit-relay-postgres-source.json`

Harness:

`crates/rustok-outbox/tests/blog_comments_audit_relay_postgres.rs`

Guard:

`scripts/verify/verify-blog-comments-audit-outbox-relay-postgres-source.mjs`

Status remains source-only and unvalidated until the maintainer executes the
PostgreSQL scenarios.

## Preserved boundaries

This slice does not change:

- Blog schedule replacement, source audit, source retry/dead-letter or audited
  recovery semantics;
- the Blog canonical event payload/schema/digests;
- canonical writer identity or write-once behavior;
- Outbox relay claim, backoff, retry budget, DLQ or acknowledgement code;
- `sys_events` schema or migrations;
- server relay worker topology;
- any HTTP, GraphQL, CLI, MCP or admin transport;
- FFA/FBA status.

The outbox implementation plan still has a broader durability gap between
transport acceptance and durable completion of local fan-out consumers. This
slice does not claim to solve that platform-wide consumer-receipt problem; it
proves the narrower canonical relay contract requested by Blog slice 96.

## Suggested maintainer execution

```bash
export RUSTOK_OUTBOX_BLOG_AUDIT_TEST_DATABASE_URL=postgres://...

cargo test -p rustok-outbox \
  --test blog_comments_audit_relay_postgres \
  -- --nocapture --test-threads=1

node scripts/verify/verify-blog-comments-audit-outbox-relay-postgres-source.mjs
```

No tests, Cargo commands, Node verifiers, formatting, PostgreSQL scenarios,
workflows, CI or runtime validation were executed by the implementation agent.

## Next cursor

After retained maintainer execution of slices 95–97, define bounded lifecycle and
retention for terminal Blog source rows and the immutable recovery-audit ledger.
Retention must preserve exact request/recovery identities required for ambiguous
commit reconciliation and must not weaken the append-only audit ownership or
create another relay/recovery path.
