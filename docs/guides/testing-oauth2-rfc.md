---
id: doc://docs/guides/testing-oauth2-rfc.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# OAuth2 RFC Compliance Tests

A set of unit tests verifying our OAuth2 implementation compliance with RFC standards.

## RFC Coverage

| RFC | Title | Test File | Test Count |
|-----|-------|-----------|------------|
| RFC 6749 | OAuth 2.0 Authorization Framework | `services/oauth_app.rs` | 15 |
| RFC 7636 | PKCE (Proof Key for Code Exchange) | `services/oauth_app.rs` | 7 |
| RFC 7519 | JSON Web Token (JWT) | `auth.rs` | 5 |
| RFC 7009 | Token Revocation | `services/oauth_app.rs` | 2 |
| RFC 8414 | Authorization Server Metadata | `services/oauth_app.rs` | 3 |
| — | OAuth2 scope enforcement | `context/auth.rs` | 7 |
| — | Credential security | `auth.rs` | 6 |

**Total: 45 tests**

## Running Tests

```bash
# All OAuth2 tests (no DB required)
cargo test -p rustok-server rfc
cargo test -p rustok-server oauth2
cargo test -p rustok-server scope
cargo test -p rustok-server require_scope
cargo test -p rustok-server token_hash
cargo test -p rustok-server password_hash
cargo test -p rustok-server pkce

# All server unit tests
cargo test -p rustok-server --lib
```

## What Is Verified

### RFC 6749 — OAuth 2.0 Framework

**Scope validation (§3.3)**
- Exact scope matching
- Case-sensitive comparison (`Catalog:Read` ≠ `catalog:read`)
- Wildcard `resource:*` matching (e.g. `storefront:*` → `storefront:read`)
- Superadmin wildcard `*:*`
- Empty scope → reject
- Space-delimited scope parsing from request

**Token Response (§5.1)**
- `token_type` is always `"Bearer"`
- `access_token` is a required field
- `expires_in` is present
- `client_credentials` → no `refresh_token` (§4.4.3)
- `authorization_code` → includes `refresh_token`

**Error Response (§5.2)**
- All error codes comply with the specification
- `invalid_client`, `invalid_grant`, `unsupported_grant_type`, `invalid_request`, `invalid_scope`

**Grant Types**
- Supported: `authorization_code`, `client_credentials`, `refresh_token`
- Not supported: `implicit`, `password` (per security best practices)

**TTL**
- `client_credentials` → 1 hour
- `authorization_code` → 15 minutes
- `refresh_token` → 30 days
- Authorization code → 10 minutes (§4.1.2 recommendation)

### RFC 7636 — PKCE

- Official test vector from Appendix B
- S256 transform: `BASE64URL(SHA256(ASCII(code_verifier)))`
- Invalid verifier rejection
- Invalid challenge rejection
- Verifier length 43–128 characters (§4.1)
- Constant-time comparison (`subtle::ConstantTimeEq`)
- Empty verifier rejected

### RFC 7519 — JWT

- Required claims: `sub`, `iss`, `aud`, `exp`, `iat`
- `exp` validation — expired token rejected
- `iss` validation — invalid issuer rejected
- `aud` validation — invalid audience rejected
- Signature validation — invalid secret rejected

### RFC 7009 — Token Revocation

- Revocation endpoint always returns 200 OK (§2.2)
- `token_type_hint` accepts only `access_token` or `refresh_token`

### RFC 8414 — Authorization Server Metadata

- Required fields: `issuer`, `token_endpoint`, `response_types_supported`
- Metadata matches the actual implementation
- Well-known paths: `/.well-known/oauth-authorization-server`, `/.well-known/openid-configuration`

### OAuth2 JWT Claims Extensions

- Direct login: `client_id = None`, `scopes = []`, `grant_type = "direct"`
- OAuth2 token: `client_id = Some(uuid)`, `scopes = [...]`, `grant_type = "client_credentials"`
- Backward compatibility: old tokens without OAuth2 fields are decoded with defaults

### Scope Enforcement (`AuthContext.require_scope`)

- Direct grant (without `client_id`) — scopes are not checked
- OAuth2 token — exact match
- OAuth2 token — wildcard `resource:*`
- OAuth2 token — superadmin `*:*`
- Empty scopes → reject
- Error contains the required and granted scopes

### Credential Security

- Refresh token: 256-bit entropy (64 hex chars)
- Refresh tokens are unique
- Token hash: SHA-256 → 64 hex chars
- Password hash: Argon2id
- Password verify roundtrip
- Unique salt per password hash

## Libraries Used

All crypto operations use battle-tested libraries:

| Operation | Crate | Status |
|-----------|-------|--------|
| JWT | `jsonwebtoken` 10.x | De facto standard, >50M downloads |
| Password hashing | `argon2` 0.5 (RustCrypto) | OWASP recommendation |
| SHA-256 | `sha2` 0.10 (RustCrypto) | Audited |
| Constant-time cmp | `subtle` (RustCrypto) | Audited |
| Random generation | `rand` 0.10 + OsRng | CSPRNG |
| Base64 URL-safe | `base64` 0.22 | Standard |

## What Is NOT Covered by Unit Tests

The following aspects require integration tests with a DB:

- [ ] Token revocation persistence (write to `oauth_tokens.revoked_at`)
- [ ] Authorization code single-use (`used_at` update)
- [ ] Refresh token rotation (revoke old + issue new)
- [ ] Tenant isolation (cross-tenant access denied)
- [ ] `sync_app_connections` upsert logic
- [ ] Consent scope coverage check

For these tests, use the approach from [`testing-integration.md`](./testing-integration.md).
