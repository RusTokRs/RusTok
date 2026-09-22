---
id: doc://docs/guides/security-audit.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Security Audit Guide

The complete security guide is in [`docs/standards/security.md`](../standards/security.md).

## Quick Summary

RusToK implements protection against OWASP Top 10 2021:

| # | Threat | Protection Mechanism |
|---|--------|---------------------|
| A01 | Broken Access Control | RBAC enforcement (`rustok-rbac`) |
| A02 | Cryptographic Failures | HTTPS, secure headers, Argon2 |
| A03 | Injection | SQL via SeaORM (parameterized), XSS/tenant sanitization |
| A04 | Insecure Design | Secure defaults, defense in depth |
| A05 | Security Misconfiguration | Security headers middleware |
| A06 | Vulnerable Components | `cargo deny` dependency audit |
| A07 | Auth Failures | Rate limiting, JWT, secure sessions |
| A08 | Data Integrity | Request validation framework |
| A09 | Logging Failures | Security audit logging via telemetry |
| A10 | SSRF | URL validation, allowlist enforcement, redirect-chain checks |

## Key Invariants

- Every database query **must** contain a `tenant_id` filter.
- Tenant slug undergoes sanitization (SQL/XSS/Path traversal) — see `rustok-core/src/tenant/sanitize.rs`.
- Events are validated before publication via the `ValidateEvent` trait.

## Full Documentation

→ [`docs/standards/security.md`](../standards/security.md)
