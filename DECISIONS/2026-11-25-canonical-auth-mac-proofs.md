# Canonical Auth MAC proofs

- Date: 2026-11-25
- Decision status: Accepted
- Implementation status: In progress
- Owners: Auth; server host composition
- Extends: [OAuth browser authorization CSRF boundary](2026-11-25-oauth-browser-authorization-csrf.md).

## Decision

AGENTS.md §13 requires one canonical pre-release proof format. Remove
the reset fingerprint version prefix and version suffixes from the
ephemeral host MAC purpose domains. Preserve distinct closed HKDF-SHA256
labels, a dedicated host-MAC salt, HMAC-SHA256, and constant-time tag
comparison. There is no old-format or old-domain fallback.

The password-reset credential fingerprint is exactly 64 lowercase hex
characters representing the full tag over the credential state.
The decoder rejects malformed fingerprints before credential comparison.
The OAuth consent proof is a canonical positive decimal Unix expiry,
a dot, and the same fixed-size tag encoding. Reject noncanonical expiry,
malformed tags, nil tenant/user/session identities, expired or excessively
future proofs. Keep the existing length-framed binding of authorization
parameters, normalized effective scope set, PKCE challenge and direct
session, and preserve the existing transactional authorization boundary.

The optional OAuth refresh retry proof uses the same canonical secret
header encoding as direct refresh, with a distinct closed
`oauth-refresh-retry` purpose and tenant/app/family/predecessor binding.
See [OAuth access-token revocation fence](2026-11-01-oauth-access-token-revocation-fence.md).
No proof or plaintext successor is persisted.

## Cutover and verification

Stop all old hosts and deploy together. Restart outstanding consent
forms and issue fresh password-reset links. No persisted session/refresh
digest, JWT signing derivation, generation fence or witness MAC domain is
changed; do not re-enroll or delete retained authority state. Root rotation
is a separate task. No new dependency or database migration is introduced.
Source regressions exercise malformed signed reset claims, former prefixes,
canonical MAC tags, alternate expiry encodings and nil identifiers.
Scoped Auth and server compilation, tests and ADR checks remain pending.
