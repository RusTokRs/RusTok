# Shared owner-operation receipt ledger

- Date: 2026-08-03
- Status: Accepted

## Context

Write-like owner ports need durable idempotency across retries, process loss,
and recovery operators. Media previously carried a private receipt table even
though Taxonomy now requires the same fencing, immutable request binding, and
terminal replay semantics for Translation-target apply.

A generic facility must not become a generic domain write service: it cannot
know an owner's resource identity, validation rules, permissions, lifecycle,
or business side effects.

## Decision

Outbox owns the generic durable receipt primitive and the
`owner_operation_receipts` schema. `rustok-outbox-storage` provides the local
SQL implementation and migration helpers. The canonical production migrator
applies the same schema through the existing append-only migration. Outbox
re-exports the storage API for the shared runtime and existing callers.

Receipts are uniquely namespaced by `(tenant_id, owner_slug,
idempotency_key)`. Admission binds that identity to one operation and a
canonical request digest. A receipt is `processing`, `completed`, or `failed`;
only a fenced lease can complete or fail it, and an expired processing lease
can be reclaimed.

An owner calls `admit` before its work, performs domain validation and its
business mutation through its own service, then writes the owner result and
`complete` in the same owner transaction. After a rollback, the owner may
persist a typed terminal failure with `fail`. The receipt layer stores no
domain-specific payload shape beyond the owner's serialized result/error and
does not publish domain events.

Media uses the namespace `owner_slug = media`; Taxonomy uses
`owner_slug = taxonomy`. Both retain responsibility for authorization,
revision/CAS checks, owner change evidence, and atomic business writes.

## Consequences

- Owners share one tested implementation for immutable request binding,
  replay, stale-lease recovery, and fencing without sharing domain logic.
- An owner that uses the primitive depends on `rustok-outbox-storage` and
  applies the shared migration on the same database before accepting writes.
  The dependency carries no Outbox module lifecycle or relay.
- The shared ledger is not evidence that a downstream consumer performed its
  own side effect. Owner-level transactional and recovery tests remain
  required.
- The former Media-local receipt entity and schema are removed rather than
  preserved as a compatibility path.

## Related contracts

- [`rustok-outbox`](../crates/modules/rustok-outbox/README.md)
- [`rustok-media`](../crates/modules/rustok-media/README.md)
- [`rustok-taxonomy`](../crates/modules/rustok-taxonomy/README.md)
- [Translation control plane and owner-owned localized data](./2026-07-26-translation-control-plane-boundary.md)

## Exact owner-operation receipt transitions (source only)

Fresh admission verifies the inserted receipt before and after restamping its
lease following possible unique-index contention. Renewal and terminal success
or failure compare the full persisted model to the expected transition before
commit, including request identity, token, timestamps and result/error payload.
Terminal readback mismatch explicitly rolls back the nested receipt savepoint;
callers must still roll back the entire business transaction on receipt error.
PostgreSQL acquires the exact receipt row with FOR UPDATE NOWAIT; SQLite reserves
the writer with a zero-row write. Neither lock path fires receipt row UPDATE
triggers merely to acquire ownership. Contention is retryable. Nonterminal rows
with terminal payloads or completion stamps fail closed. SQLite regression
source models rewritten admission, renewal, success and failure; compilation
and runtime execution remain pending. This is not yet a DB-level immutable
receipt history or durable receipt retention policy.
