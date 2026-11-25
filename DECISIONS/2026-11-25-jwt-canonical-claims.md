# Canonical JWT claims

- Date: 2026-11-25
- Decision status: Accepted
- Implementation status: In progress
- Owners: Auth; server host composition
- Extends: [Mandatory JWT key identity](2026-11-25-jwt-mandatory-key-identity.md).

## Context

AGENTS.md §13 rejects pre-release compatibility paths for repository-owned
tokens. Previously, missing access grant type defaulted to a direct
principal, missing scopes defaulted to an empty vector, and invites without
an issuance nonce could still decode. Current issuers already produce these
fields.

## Decision

The access claim model requires explicit grant_type and scopes. The
existing principal validation remains authoritative: direct sessions have
no OAuth client or OAuth access row ID and have empty scopes; OAuth access
must bind the client and durable access-token registration. Missing or null
grant type or scopes must fail decoding, not select a default principal.
The nullable OAuth identifiers represent actual domain optionality, not
a historical format.

Invite claims require a unique non-nil UUID jti. Reject missing, null
and nil nonces. A missing custom role slug still denotes a baseline role.
The exact persisted token digest remains the single-use authority; the
nonce is not a replacement for transactional invite consumption.

## Cutover and verification

Reissue obsolete development JWTs and invitation links; do not introduce
old/new claim types, format readers or fallback. No database schema or
destructive operation is introduced. Do not delete non-disposable invite
or session state. Existing canonical issued tokens are unaffected.
Source regressions sign malformed payloads with valid keys to exercise
missing/null claims and nil invite IDs. Scoped compilation, regression
execution and ADR verification remain pending.
