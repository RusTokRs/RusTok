# JWT keyring and controlled verification retirement

- Status: Accepted
- Implementation status: In progress (source implementation; compilation and deployment evidence pending)


The missing-ID and raw-root compatibility provisions (where present) are superseded by [Mandatory JWT key identity](2026-11-25-jwt-mandatory-key-identity.md). The remaining security invariants are retained.

## Context and decision

Auth owns the deployment JWT keyring for direct and OAuth access, invite, reset and email verification JWTs. The host supplies a startup configuration snapshot. One active key signs and all newly issued JWTs carry its bounded ASCII `kid`. Verification selects exactly one configured key by ID and configured algorithm; unknown, empty, ambiguous and retired identifiers are rejected. There is no trial verification. Historical keys carry an exclusive Unix-second verification cutoff and never enter the signing path. RSA history stores public PEMs only; HS256 history necessarily retains symmetric verification material. Rotation within one algorithm is supported; algorithm migration and separate purpose keyrings remain pending.

The existing active RSA configuration remains the signing source. HS256 can use independent signing material in the keyring, leaving the host MAC root stable. The default key ID is `default` for deployments without a keyring. The optional environment-backed JSON keyring is secret configuration and is never logged. For HS256, verification-only is a runtime restriction, not a cryptographic restriction on a party holding the symmetric material. Invalid or ambiguous configuration fails startup.

## Rollout and invariants

A deployment can explicitly designate one key ID for pre-key-ID JWTs with an exclusive cutoff. This is a bounded external-token migration only; missing IDs otherwise fail closed. Remove that setting once existing JWTs expire. Do not rotate the root MAC secret as part of JWT rotation. For the initial deployment, keep the existing signing material and default ID, with the explicit pre-key-ID acceptance window configured on every replica before switching signers. Before switching the active key, distribute the future verification key to all verifiers, then switch signers and retain the outgoing key for at least the longest outstanding JWT lifetime. Never reuse a key ID or key material. Keep session and OAuth revocation authority unchanged; no persistence or client contract migration is required.

Runtime configuration is immutable per process; multi-replica rollout is operator-coordinated, not hot reload. Emergency compromise requires removing the affected verification key immediately and handling outstanding credentials separately.

## Verification

Auth unit regressions cover exact key selection, old and new signatures, missing and unknown IDs, cutoff, and invalid configuration. Run scoped Auth and server checks and exercise coordinated replica rollout before claiming deployment readiness.
