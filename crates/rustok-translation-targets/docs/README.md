# Translation target provider contract

An owner provider is the only supported boundary between Translation workflow
and canonical localized domain data. Providers must expose exact source and
target locale state, stable field identities, source hashes, opaque resource and
locale revisions, permission floors, and explicit field classifications.

Runtime fallback is presentation context only. It is returned separately and
never makes a target locale exact or complete. Storage-only `und` provenance
cannot enter this contract because every locale field uses
`rustok_api::TenantLocale`.

Apply calls carry expected resource, source, and target revisions.
`PortContext` supplies tenant, actor, deadline, correlation, and idempotency
identity. An owner must validate and write through its normal service,
transaction, audit, and outbox path. Cross-module SQL is forbidden.

Provider registration is keyed by `(owner_slug, resource_kind)` and duplicate
keys fail startup. Owners declare only implemented capabilities; consumers must
not emulate a missing capability.

