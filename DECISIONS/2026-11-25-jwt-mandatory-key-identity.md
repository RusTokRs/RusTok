# Mandatory JWT key identity

- Date: 2026-11-25
- Decision status: Accepted
- Implementation status: In progress
- Owners: Auth; server host composition
- Partially supersedes: [JWT keyring](2026-11-25-jwt-keyring.md), [JWT purpose keyrings](2026-11-25-jwt-purpose-keyrings.md), [durable JWT generation fence](2026-11-25-jwt-generation-fence.md), [rolling admission](2026-11-25-jwt-rolling-admission.md), [emergency revocation](2026-11-25-jwt-emergency-revocation.md), and the pending missing-ID/raw-root provisions of [canonical JWT algorithm metadata](2026-11-25-jwt-canonical-algorithm-metadata.md).

## Decision

Under AGENTS.md §13, pre-release tokens do not justify compatibility bridges.
Every access, OAuth access, password reset, email verification and invite
JWT requires a non-null exact kid and the expected type and algorithm.
There is no missing-ID fallback. Remove the obsolete config field and its
prepared and persisted metadata; reject the field rather than ignoring it.
The host MAC root, including HMAC-equivalent representations of it, must
never enter a JWT ring as active or historical material.

Purpose separation, independent signing keys, exact JOSE algorithm binding,
bounded historical verification, managed staged promotion, lease overlap,
permanent revocation and tombstones remain unchanged. Reissue obsolete
development tokens and links; do not carry their former verification paths
into the canonical implementation.

## Persistence and rollout

The canonical ledger and replica grants have no legacy mapping field.
The witness authenticates these exact payloads. No SQL schema change or
automatic destructive migration is introduced. Follow the coordinated
pre-release retained-state cutover rules in [canonical JWT algorithm metadata](2026-11-25-jwt-canonical-algorithm-metadata.md).
Do not reset non-disposable authority/witness state to regain availability.
Previously retained development credentials must not survive a disposable
state reset.

## Verification

Source regressions reject missing kid before and after same-algorithm and
mixed-algorithm promotion, reject obsolete configuration, forbid raw-root
history, and preserve retirement and tombstone assertions. Compilation,
scoped tests, ADR verification and live multi-replica evidence are pending.
