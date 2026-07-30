# Customer admin native error safety

Status: `customer_admin_native_error_safety_source_unvalidated`

## Scope

This source slice covers the mounted `rustok-customer-admin` native server-function endpoints for bootstrap, list, detail, create, and update.

Before this slice, framework extraction and typed `CustomerError` values were converted directly with `ServerFnError::new`. Database, profile, validation, duplicate-email, duplicate-user-link, and not-found details could therefore cross the native transport boundary as raw error text.

## Public boundary

The adapter now returns static public messages for:

- authentication and tenant-context extraction failures;
- customer request validation;
- customer not-found outcomes;
- duplicate email and duplicate user-link conflicts;
- profile presentation failures;
- customer storage failures.

Permission and UUID validation remain transport-owned and unchanged.

## Private diagnostics

The original typed error remains server-side and is logged with bounded context when available:

- owner and consumer;
- owner operation;
- per-call correlation id;
- tenant and actor ids;
- optional customer id;
- request tenant/user, channel, and locale;
- stable public code;
- native transport boundary.

No customer email, name, phone, search text, profile payload, or request body is logged.

`RequestContext` extraction is diagnostic-only. Its absence is logged but does not change the existing customer operation admission path.

## Preserved behavior

The endpoint set, request/response DTOs, permissions, pagination, profile audience selection, locale fallback, and customer/profile bridge remain unchanged.

## Verification boundary

The focused source guard is:

```text
node scripts/verify/verify-customer-admin-native-error-safety.mjs
```

The retained evidence file is:

```text
crates/rustok-customer/contracts/evidence/admin-native-error-safety-source.json
```

Tests, Cargo commands, formatting, verifiers, workflows, CI, native runtime behavior, and the profile audience matrix remain unvalidated until executed and retained separately. No FFA or FBA status is promoted by this source inspection.
