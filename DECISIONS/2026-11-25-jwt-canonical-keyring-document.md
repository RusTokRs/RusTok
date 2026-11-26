# Canonical JWT keyring document

- Date: 2026-11-25
- Decision status: Accepted
- Implementation status: In progress
- Owners: Auth; server host composition
- Extends: [Process-local JWT keyring publication](2026-11-25-jwt-hot-rotation.md), [durable JWT generation fence](2026-11-25-jwt-generation-fence.md) and [canonical JWT claims](2026-11-25-jwt-canonical-claims.md).

## Decision

The secret-manager-mounted keyring document is a repository-owned pre-release
contract. AGENTS.md §13 requires a single canonical shape, so remove the
schema_version field and reject it as an unknown field. All four purpose
entries remain required. Do not add a previous-format reader.

Generation is a monotonic security-state identifier, not a schema version.
The canonical file accepts generations from 1 through i64::MAX, consistent
with the durable PostgreSQL fence. Runtime initialization and publication
also reject values above that maximum, including for unmanaged embedders.
Generation zero remains reserved for static cluster initialization, not a
file or a process-local rotation candidate. Keep identity tombstones,
lease and witness fencing, exact key identities and atomic publication.

## Cutover and verification

Remove the obsolete schema tag from deployment documents before starting
the current binary. Keep the actual monotonic generation and existing
key material; do not reset the durable authority or witness to change a
configuration shape. The earlier persisted metadata cutover remains
subject to its separate retained-state requirements.

Source regressions exercise the canonical shape, rejection of the old
tag, zero and overflow generations, direct runtime constructors and
publication. Update all deployment fixtures to the canonical shape.
No database schema changes or dependencies are introduced. Scoped Auth
and host compilation, tests and ADR verification remain pending.
