# Reactions neutral contract

`rustok-reactions-api` is a support crate, not a persistence owner or installable
application. It allows an optional Reactions owner to work with revisioned
subjects without depending on their domain crates.

## Subject contract

A subject is exactly `(tenant_id, source, kind, subject_id, subject_revision)`.
The source module owns existence, current revision, visibility, lifecycle and
which reaction catalog is allowed for the current actor and operation.

Providers are unique by source slug. Duplicate registration, empty/duplicate or
over-bound kind sets, factory/provider source mismatch and malformed subject
identity fail closed.

## Reaction contract

Catalogs are bounded and explicit. Selection is either one key or a bounded
number of keys. Actor state and aggregates may contain only keys in the
currently authorized catalog. Writes carry a stable command UUID and actor UUID
for future owner idempotency.

## Deliberate omissions

This foundation does not define persistence, SQL migrations, event schemas,
transport DTOs, UI presentation, emoji/media rendering, ranking, reputation,
achievements, notifications or module-specific votes.

## Verification

```bash
cargo test -p rustok-reactions-api
node scripts/verify/verify-reactions-foundation.mjs
```

Source status remains unvalidated until the maintainer runs these commands.
