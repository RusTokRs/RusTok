---
id: doc://docs/guides/input-validation.md
kind: guide
language: en
status: verified
---

# Input Validation

Validation belongs at the owning boundary. Transport decoding establishes the
shape of an input; the owner service then enforces tenant, locale, permission,
state, concurrency, and domain invariants before persistence or outbox writes.

## Rules

1. Use typed DTOs instead of accepting generic JSON at domain boundaries.
2. Reject unknown fields on security-sensitive structured documents.
3. Normalize locale, slug, and other canonical values once through shared
   helpers before uniqueness checks.
4. Select capabilities and profiles server-side. A client must not select a
   richtext schema, Page Builder body format, permission scope, or lifecycle
   policy.
5. Validate before starting side effects. A failed validation must not persist
   owner state, audit state, or outbox events.
6. Keep the same semantic validation in native server functions, GraphQL, REST,
   CLI, and worker adapters by delegating to one owner service.
7. Return typed domain errors. Do not turn validation failures into an internal
   error or silently coerce an unsupported representation.

## Richtext

Richtext owners accept `rustok_api::RichTextDocument`. Locale stays in the
owner request or translation row, and the owner selects one fixed
`RichTextProfile`.

```rust
use rustok_api::RichTextDocument;
use rustok_content::richtext::{RichTextProfile, validate_and_normalize};

fn validate_article(document: RichTextDocument) -> Result<RichTextDocument, String> {
    validate_and_normalize(document, RichTextProfile::Article)
        .map_err(|error| error.to_string())
}
```

Do not accept a parallel string body, `content_json`, Markdown mode, version
envelope, or caller-selected richtext format. Blog, Forum, and Comments own
their persistence and apply the shared validator with their fixed profiles.

## Page Builder documents

Pages is not a richtext owner. `PageBodyInput` carries localized Page Builder
project data, and Pages selects the sole internal format. Validation is owned by
`rustok-page-builder::validate_page_builder_document`; clients do not submit a
format discriminator.

## Locale and slug values

Use `rustok_content::normalize_locale_code` for owner content locales and the
owner's canonical slug helper for slugs. Normalize before duplicate checks so
equivalent input cannot create multiple rows.

```rust
use rustok_content::normalize_locale_code;

fn require_locale(value: &str) -> Result<String, String> {
    normalize_locale_code(value).ok_or_else(|| "Invalid locale".to_string())
}
```

Locale fallback is a read concern. Writes target one explicit normalized
locale and must not silently write into a fallback locale.

## Validation order

For owner writes, use this order:

1. decode the transport DTO;
2. establish authenticated tenant and security context;
3. normalize identifiers, locale, and bounded text fields;
4. validate the typed document or domain payload;
5. enforce permissions, state transition, and expected revision;
6. persist owner state and outbox records in one transaction;
7. return the canonical owner response.

Adapters may add transport-specific size limits, but they must not replace or
fork owner validation.

## Tests

Every public write surface should cover:

- valid input through the real owner service;
- unknown or malformed structured fields;
- size, depth, and count limits;
- invalid locale and duplicate normalized locale;
- forbidden tenant or permission context;
- stale expected revision;
- no persisted state or outbox event after failure;
- transport parity for GraphQL and native/REST surfaces where both exist.

Shared richtext fixtures live under
`crates/rustok-content/fixtures/richtext/`. Pages tests use canonical Page
Builder documents and must not restore format aliases or alternate body modes.

## Related documents

- [Richtext implementation plan](../modules/rich-text-implementation-plan.md)
- [Module backend implementation](../backend/module-backend-implementation.md)
- [Error handling](./error-handling.md)
- [Security](../standards/security.md)
