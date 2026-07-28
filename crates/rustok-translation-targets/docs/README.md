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

Owners re-authorize the current actor on every apply call, including replay.
The idempotency request binding is tenant + operation kind + exact
`TranslationPatchRequest`; it must not include the actor identity. This permits
an explicitly authorized control-plane recovery operator to reconcile an
unknown outcome under the original mutation identity without impersonating the
original actor or creating a second write key. The owner records the actor that
actually commits a first execution; a replay returns the already committed
receipt after current-actor authorization.

Provider registration is keyed by `(owner_slug, resource_kind)` and duplicate
keys fail startup. Owners declare only implemented capabilities; consumers must
not emulate a missing capability.

`AggregateProgress` returns bounded, content-free facts for one exact
source/target locale pair: required and optional unit totals, exact target
counts, required-field resource completeness, and the owner change cursor that
frames the observation. Providers must reject impossible facts where an exact
count exceeds its denominator. Fallback values and storage-only `und` values
never increment exact coverage. Because cursors are opaque, consumers may test
equality but must not invent a numeric cursor distance.

The executable reference provider in
`tests/reference_provider_conformance.rs` demonstrates the minimum owner
behavior without becoming a production fallback. It proves exact-locale
discovery, validation before mutation, opaque source/resource/target CAS,
idempotent replay, conflict rejection, and preservation of the previously
accepted value when a replay key is reused with a different payload.
