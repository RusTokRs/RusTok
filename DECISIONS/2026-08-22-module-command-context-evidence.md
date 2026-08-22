# Typed module command-context evidence

- Date: 2026-08-22
- Status: Accepted

## Context

The module control plane declared `ModuleCommandContext`, but mutable artifact
lifecycle and destructive-data commands still accepted separate actor and
idempotency fields. Their durable operation receipts omitted trace and
correlation identities, while outbox events generated a new correlation. A
retry could therefore be idempotent in storage without preserving a single
observable command identity.

The platform authentication, event envelope, and durable audit contracts all
use UUID principal identities. String-form command identities added parsing and
format ambiguity without supporting a distinct repository-owned principal
model.

## Decision

`ModuleCommandContext` is the one typed evidence contract for mutable module
owner commands:

- actor, correlation, and idempotency identities are non-nil UUIDs;
- tenant scope is either absent for platform commands or a non-nil UUID;
- trace identity is a non-empty string bounded by the durable event metadata
  limit;
- an artifact lifecycle command must carry a context whose tenant identity
  exactly matches its installation scope.

Artifact activation, deactivation, tenant enablement, uninstall, rollback,
and migration checkpoint commands use this context directly. The matching
durable operation receipts persist actor, trace, correlation, and idempotency
facts, and their idempotency replay rejects a different command context.
Owner-created lifecycle outbox envelopes preserve the same tenant, actor,
trace, and correlation identity.

Tenant-scoped `dynamic_artifact_data_purge` uses the same context. Its
destructive namespace receipt persists the full command evidence, requires its
tenant to match the data namespace, rejects conflicting idempotency reuse, and
writes an outbox envelope with the same observable identity.

The tenant-scoped artifact settings-recovery lifecycle uses the same context
for recovery-point creation, purge, restore, retention update, KMS rewrap,
collection, and continuity bind. Each operation receipt retains the complete
evidence and rejects a conflicting idempotency replay. A collection job stores
its original context before entering `collecting`; a crash resume reloads that
stored context and emits its terminal outbox event with the original identity,
not that of the worker invocation that resumed it.

Artifact-data snapshot create, restore, retention, and collection commands
also use this contract. The staging snapshot persists the create context until
its final event, and the collection receipt persists the same context across a
crash resume.

Tenant-scoped artifact secret binding uses the same contract. Its binding
operation receipt retains the complete context, rejects an idempotency replay
with changed evidence, and writes `module.artifact.secret_bound` with the
identical actor, tenant, trace, and correlation identity. Sandbox handle and
secret-use requests remain execution-scoped rather than management commands,
so they retain their sandbox identity contract and cannot supply a resolver.

Global artifact security transitions (quarantine, clear quarantine, and
revocation) also use this contract, with an absent tenant identity. Their
operation receipt persists actor, trace, correlation, and idempotency facts;
replay verifies the complete platform-scoped context, and the security-state
outbox event preserves its actor, trace, and correlation identity.

Static promotion request and approval commands are likewise platform-scoped.
Their independent operation receipts preserve the complete context and their
promotion events retain the same actor, trace, and correlation identity.

Static distribution bootstrap import, admission, and revocation are also
platform-scoped. Their shared release idempotency ledger retains the complete
context and rejects a replay whose actor, trace, correlation, or idempotency
facts differ. Admission and revocation use that original context for their
outbox envelopes; bootstrap import has no separate domain event.

GraphQL constructs this canonical context from authenticated tenant and actor
state, the client idempotency UUID, and the active telemetry trace. When a
deployment has no active trace, it records a deterministic local GraphQL root
derived from the idempotency UUID; it does not create a second command DTO or
silently omit trace evidence.

## Consequences

- Repository-owned callers must supply typed command evidence rather than
  separately shaped actor or idempotency fields.
- Unreleased lifecycle receipt migrations are amended in place so no
  corrective migration or compatibility reader remains.
- Other mutable owner families remain responsible for their own atomic cutover
  before the platform can claim that every command uses this context.
