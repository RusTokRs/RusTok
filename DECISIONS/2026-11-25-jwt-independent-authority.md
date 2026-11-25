# Independent JWT authority

- Date: 2026-11-25
- Decision status: Accepted
- Implementation status: In progress
- Owners: Auth; server host composition; Auth operator CLI
- Extends: [JWT clock discontinuity protection](2026-11-25-jwt-clock-discontinuity.md), [JWT emergency revocation](2026-11-25-jwt-emergency-revocation.md)
- Supersedes: None
- Superseded by: None

## Context

Restoring the application database can resurrect a validly authenticated
older ledger, including forgotten revocations and identity tombstones.
A MAC, revision or checkpoint stored in that same backup cannot prevent it.
Two separately written ledgers introduce a crash-consistency problem.

## Decision

Make a separately administered PostgreSQL database the sole production JWT
authority. It holds the complete existing Auth ledger, grants, drain and
lease horizons in one CAS boundary. The old application row remains an
offline import source, never a production fallback or live mirror. This
prevents application database restoration from rolling back JWT key authority;
it does not detect all application data rollback.

Pin deployment identity using a non-nil UUID (`RUSTOK_JWT_AUTHORITY_ID`)
and a purpose-separated root MAC on that UUID. Supply the URI through
secret management as `RUSTOK_JWT_AUTHORITY_DATABASE_URL`. Both settings
are mandatory together. Every bound authority read checks the marker in
the same SQL statement as the ledger; missing/wrong identity, unavailable
DB or inconsistent ledger refuses authority. Production refuses absent
settings. Development-only hosts may use the application DB if both
settings are absent; that mode lacks rollback protection. The revocation
CLI always requires the independent authority, never local fallback.
Emergency revocation remains possible during a clock incident; no grant
expiry is inferred. No request-path DB queries or new network API.

## Offline cutover

Apply existing Auth migrations to the trusted source. Stop all old hosts,
CLI writers and unmanaged writers. Wait at least the conservative lease
window on an independent clock and for any drain deadline. Back up the
verified source. Provision a new empty PostgreSQL database in a separate
restore/failure domain, with a new externally pinned UUID. Set both authority
variables and the stable root through `RUSTOK_JWT_REVOCATION_ROOT`. Point
the ordinary CLI runtime at the trusted source and execute
`rustok-cli auth jwt-authority-init --confirm`.

The command authenticates the source envelope and validates the registry,
refuses active leases/drains, and copies the exact generation, payload,
tag and all horizon/registry fields. Existing Auth fence migrations plus
the identity marker are created transactionally in the new authority.
Existing tables are refused; no overwrite, reset or automatic import.
The source is not changed. No distributed transaction is needed because
all source writers must be offline. After a lost command response, inspect
the pinned authority; do not create another or reset it. Use restricted
runtime credentials and verified TLS after provisioning. All hosts and
incident operators must use the same pinned target. Physical/administrative
independence is an operator requirement, not proven by different URI strings.

## Recovery and limits

Restore application data without restoring the independent authority.
Keep the latest keyring generation and stable root. A stale keyring fails
admission; a revoked key remains denied, including after restart. If the
independent store is lost, refuse JWT traffic until current authoritative
state is recovered. Never initialize a replacement from the old application
row or restore an older independent snapshot. Protect the separate authority
with independent backup/replication/access controls. Rollback of that store
or compromise of deployment configuration/root is not detected by the pin.
Static generation zero still has no enrolled key history; enroll a file-backed
generation for key revocation. Session, refresh, OAuth and RBAC application
data rollback is outside this JWT-key contract. No application migration
history change.

## Verification

Source regressions cover independent-store persistence across application
restore, stale/revoked key refusal, missing/wrong identity, wrong root,
reimport refusal, existing DB refusal and active grant refusal.
Compile and run scoped Auth/CLI/server tests plus PostgreSQL transactional
DDL, independent-pool cutover, failover, outage and restore exercises before
production. Real external database provisioning is an operator action.
