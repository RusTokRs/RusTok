---
id: doc://docs/guides/testing-oauth2-rfc.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# OAuth2 RFC Compliance Tests

Набор unit-тестов, проверяющих соответствие нашей OAuth2 реализации стандартам RFC.

## Покрытие по RFC

| RFC | Название | Файл тестов | Кол-во тестов |
|---|---|---|---|
| RFC 6749 | OAuth 2.0 Authorization Framework | `services/oauth_app.rs` | 15 |
| RFC 7636 | PKCE (Proof Key for Code Exchange) | `services/oauth_app.rs` | 7 |
| RFC 7519 | JSON Web Token (JWT) | `auth.rs` | 5 |
| RFC 7009 | Token Revocation | `services/oauth_app.rs` | 2 |
| RFC 8414 | Authorization Server Metadata | `services/oauth_app.rs` | 3 |
| — | OAuth2 scope enforcement | `context/auth.rs` | 7 |
| — | Credential security | `auth.rs` | 6 |

**Итого: 45 тестов**

## Запуск

```bash
# Все OAuth2 тесты (не требуют БД)
cargo test -p rustok-server rfc
cargo test -p rustok-server oauth2
cargo test -p rustok-server scope
cargo test -p rustok-server require_scope
cargo test -p rustok-server token_hash
cargo test -p rustok-server password_hash
cargo test -p rustok-server pkce

# Все unit-тесты сервера
cargo test -p rustok-server --lib
```

## Что проверяется

### RFC 6749 — OAuth 2.0 Framework

**Scope validation (§3.3)**
- Точное совпадение scopes
- Case-sensitive сравнение (`Catalog:Read` ≠ `catalog:read`)
- Wildcard `resource:*` matching (e.g. `storefront:*` → `storefront:read`)
- Superadmin wildcard `*:*`
- Пустой scope → reject
- Space-delimited parsing scopes из запроса

**Token Response (§5.1)**
- `token_type` всегда `"Bearer"`
- `access_token` обязательное поле
- `expires_in` присутствует
- `client_credentials` → без `refresh_token` (§4.4.3)
- `authorization_code` → с `refresh_token`

**Error Response (§5.2)**
- Все error коды соответствуют спецификации
- `invalid_client`, `invalid_grant`, `unsupported_grant_type`, `invalid_request`, `invalid_scope`

**Grant Types**
- Поддержаны: `authorization_code`, `client_credentials`, `refresh_token`
- Не поддержаны: `implicit`, `password` (по security best practices)

**TTL**
- `client_credentials` → 1 час
- `authorization_code` → 15 минут
- `refresh_token` → 30 дней
- Authorization code → 10 минут (§4.1.2 recommendation)

### RFC 7636 — PKCE

- Официальный тест-вектор из Appendix B
- S256 transform: `BASE64URL(SHA256(ASCII(code_verifier)))`
- Отклонение неверного verifier
- Отклонение неверного challenge
- Verifier длина 43–128 символов (§4.1)
- Constant-time сравнение (`subtle::ConstantTimeEq`)
- Пустой verifier отклоняется

### RFC 7519 — JWT

- Обязательные claims: `sub`, `iss`, `aud`, `exp`, `iat`
- Валидация `exp` — просроченный токен отклоняется
- Валидация `iss` — неверный issuer отклоняется
- Валидация `aud` — неверная audience отклоняется
- Валидация подписи — неверный secret отклоняется

### RFC 7009 — Token Revocation

- Revocation endpoint всегда возвращает 200 OK (§2.2)
- `token_type_hint` принимает только `access_token` или `refresh_token`

### RFC 8414 — Authorization Server Metadata

- Обязательные поля: `issuer`, `token_endpoint`, `response_types_supported`
- Metadata соответствует реальной реализации
- Well-known paths: `/.well-known/oauth-authorization-server`, `/.well-known/openid-configuration`

### OAuth2 расширения JWT Claims

- Direct login: `client_id = None`, `scopes = []`, `grant_type = "direct"`
- OAuth2 token: `client_id = Some(uuid)`, `scopes = [...]`, `grant_type = "client_credentials"`
- Backward compatibility: старые токены без OAuth2 полей декодируются с defaults

### Scope Enforcement (`AuthContext.require_scope`)

- Direct grant (без `client_id`) — scopes не проверяются
- OAuth2 token — exact match
- OAuth2 token — wildcard `resource:*`
- OAuth2 token — superadmin `*:*`
- Пустые scopes → reject
- Ошибка содержит требуемый и выданный scopes

### Credential Security

- Refresh token: 256-bit entropy (64 hex chars)
- Refresh tokens уникальны
- Token hash: SHA-256 → 64 hex chars
- Password hash: Argon2id
- Password verify roundtrip
- Unique salt per password hash

## Используемые библиотеки

Все крипто-операции используют battle-tested библиотеки:

| Операция | Crate | Статус |
|---|---|---|
| JWT | `jsonwebtoken` 10.x | Стандарт де-факто, >50M downloads |
| Password hashing | `argon2` 0.5 (RustCrypto) | OWASP рекомендация |
| SHA-256 | `sha2` 0.10 (RustCrypto) | Аудирован |
| Constant-time cmp | `subtle` (RustCrypto) | Аудирован |
| Random generation | `rand` 0.10 + OsRng | CSPRNG |
| Base64 URL-safe | `base64` 0.22 | Стандарт |

## Что НЕ покрыто unit-тестами

Следующие аспекты требуют интеграционных тестов с БД:

- [ ] Token revocation persistence (запись в `oauth_tokens.revoked_at`)
- [ ] Authorization code single-use (`used_at` update)
- [ ] Refresh token rotation (revoke old + issue new)
- [ ] Tenant isolation (cross-tenant access denied)
- [ ] `sync_app_connections` upsert logic
- [ ] Consent scope coverage check

Для этих тестов используйте подход из [`testing-integration.md`](./testing-integration.md).
