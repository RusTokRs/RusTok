---
id: doc://docs/guides/security-audit.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Security Audit Guide

Полное руководство по безопасности находится в [`docs/standards/security.md`](../standards/security.md).

## Краткое резюме

RusToK реализует защиту от OWASP Top 10 2021:

| # | Угроза | Механизм защиты |
|---|--------|----------------|
| A01 | Broken Access Control | RBAC enforcement (`rustok-rbac`) |
| A02 | Cryptographic Failures | HTTPS, secure headers, Argon2 |
| A03 | Injection | SQL через SeaORM (parameterized), XSS/tenant sanitization |
| A04 | Insecure Design | Secure defaults, defense in depth |
| A05 | Security Misconfiguration | Security headers middleware |
| A06 | Vulnerable Components | `cargo deny` dependency audit |
| A07 | Auth Failures | Rate limiting, JWT, secure sessions |
| A08 | Data Integrity | Request validation framework |
| A09 | Logging Failures | Security audit logging через telemetry |
| A10 | SSRF | URL validation, allowlist enforcement, redirect-chain checks |

## Ключевые инварианты

- Каждый запрос к БД **обязан** содержать фильтр по `tenant_id`.
- Tenant slug проходит sanitization (SQL/XSS/Path traversal) — см. `rustok-core/src/tenant/sanitize.rs`.
- Events валидируются перед публикацией через trait `ValidateEvent`.

## Полная документация

→ [`docs/standards/security.md`](../standards/security.md)
