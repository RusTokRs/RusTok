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

`rustok-outbox::idempotency` owns the generic durable receipt primitive and
the `owner_operation_receipts` schema. The canonical production migrator
applies the Outbox-owned schema through an append-only migration; the
standalone Outbox migration provides the same schema for isolated owner and
test runtimes.

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
- A new owner that uses the primitive must declare the Outbox dependency and
  ensure the Outbox migration is applied before accepting writes.
- The shared ledger is not evidence that a downstream consumer performed its
  own side effect. Owner-level transactional and recovery tests remain
  required.
- The former Media-local receipt entity and schema are removed rather than
  preserved as a compatibility path.

## Related contracts

- [`rustok-outbox`](../crates/rustok-outbox/README.md)
- [`rustok-media`](../crates/rustok-media/README.md)
- [`rustok-taxonomy`](../crates/rustok-taxonomy/README.md)
- [Translation control plane and owner-owned localized data](./2026-07-26-translation-control-plane-boundary.md)
