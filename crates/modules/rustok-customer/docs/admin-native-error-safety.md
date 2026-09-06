# Customer admin native error safety

Status: `customer_admin_native_error_safety_source_unvalidated`

## Scope

This source slice covers the mounted `rustok-customer-admin` native server-function endpoints for:

- bootstrap;
- customer list;
- customer detail;
- customer creation;
- customer update.

The public envelope was already static and bounded. This continuation hardens the private diagnostic envelope without changing endpoint, DTO, permission, pagination, profile, locale, or owner-call behavior.

## Confirmed residual gap

Framework extraction failures in `customer_context_error` and `optional_request_context` were logged with `error = ?error`.

`customer_owner_error` also logged the complete `CustomerError` Debug payload. Depending on the variant, that payload could contain:

- request validation text;
- a duplicate customer email value;
- customer or user UUIDs duplicated from the typed error;
- profile error details;
- database error details.

The public result did not expose those values, but the complete framework or owner error payload is not logged after this slice.

## Public boundary

The adapter continues to return the same static public messages for:

- authentication and tenant-context extraction failures;
- customer request validation;
- customer not-found outcomes;
- duplicate email and duplicate user-link conflicts;
- profile presentation failures;
- customer storage failures.

Permission and UUID validation remain transport-owned and unchanged.

## Bounded diagnostics

Framework extraction failures retain only the static Rust error type together with:

- owner and operation;
- context kind;
- correlation ID;
- stable code;
- native transport boundary.

The error value is accepted without a `Debug` bound and is never formatted.

Customer owner failures retain typed customer error classification only:

- `validation`;
- `customer_not_found`;
- `customer_by_user_not_found`;
- `duplicate_email`;
- `duplicate_user_link`;
- `profile`;
- `database`.

The existing severity policy remains unchanged: profile and database failures use error-level tracing; validation, not-found, and conflict outcomes use warning-level tracing.

Owner context remains available where it was already recorded:

- owner and consumer;
- owner operation;
- correlation ID;
- tenant and actor IDs;
- optional customer ID;
- request tenant/user, channel, and locale;
- stable public code;
- native transport boundary.

No validation text, duplicate email value, profile error payload, database error payload, customer email, name, phone, search text, profile payload, or request body is written to structured tracing.

`RequestContext` extraction remains diagnostic-only. Its absence does not change the customer operation admission path.

## Preserved behavior

This slice does not change:

- the five mounted endpoint paths;
- `CustomerAdminBootstrap`, `CustomerList`, `CustomerDetail`, or `CustomerDraft`;
- permissions or UUID validation;
- page/per-page normalization;
- search normalization;
- profile audience selection;
- locale fallback;
- customer/profile owner calls or response mapping;
- public messages or error type;
- FFA or FBA status.

## Static evidence

The focused source guard is:

```text
node scripts/verify/verify-customer-admin-native-error-safety.mjs
```

The retained evidence file is:

```text
crates/rustok-customer/contracts/evidence/admin-native-error-safety-source.json
```

The verifier fails closed if complete framework or `CustomerError` payload logging returns, if the type/variant-only classification disappears, or if endpoint and owner behavior markers drift.

## Validation boundary

Tests, Cargo commands, formatting, Node verifiers, workflows, CI, native runtime behavior, and the profile audience matrix were intentionally not executed. Source inspection does not promote FFA or FBA status.

Suggested maintainer execution:

```bash
node scripts/verify/verify-customer-admin-native-error-safety.mjs
node scripts/verify/verify-customer-admin-client-transport-error-safety.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-customer-admin
cargo check -p rustok-customer-admin --features hydrate
cargo check -p rustok-customer-admin --features ssr
```

## Remaining work

The broader ecommerce cleanup remains open for Customer surfaces outside this mounted adapter, Inventory, Tax, Promotion, other owner adapters, and remaining non-`PortError` public envelopes. Runtime and mounted evidence also remain open.
