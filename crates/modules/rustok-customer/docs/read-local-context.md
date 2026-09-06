# Customer read local diagnostic safety

Status: **source-ready / unvalidated**

## Scope

The canonical root `InProcessCustomerReadPort` retains actionable diagnostics for all four `CustomerReadPort` operations while delegating the original context and request to the unchanged `CustomerService` implementation:

- `read_customer_projection`;
- `read_customer_projection_by_user`;
- `list_customer_projections`;
- `list_profile_enrichment`.

The module-path factory under `rustok_customer::ports` remains an explicit compatibility path. Root construction continues through `in_process_customer_read_port` and the canonical wrapper.

## Bounded context shape

Covered events retain owner operation, local operation, stable boundary, and correlation-ID character length. Raw correlation IDs are not recorded.

The remaining delegated context is represented only through bounded facts:

- tenant and actor-ID character lengths;
- a closed actor-kind label;
- claim and role counts;
- optional channel, causation, traceparent, and idempotency presence/length;
- locale length and deadline.

Raw tenant, actor, channel, locale, causation, traceparent, and idempotency values are not recorded by the wrapper.

## Bounded request shape

Operation-specific diagnostics retain only:

- customer or user UUID presence and non-nil status;
- page and page-size presence plus non-zero status;
- search presence and character length;
- enrichment-list emptiness and duplicate-user-ID presence.

Raw customer/user UUIDs, exact pagination values, exact profile-enrichment counts, search text, email, customer names, profile names, preferred locale values, rows, and result payloads are not recorded.

## Stable local classification

The wrapper preserves the exact stable `operation + code + message` classification for invalid tenant context, invalid page and page size, storage unavailability, customer and customer-by-user not found, owner validation, and profile projection unavailability.

Unknown envelopes pass through without an additional local event. Unavailable, timeout, and invariant kinds remain error severity; ordinary validation, not-found, and conflict outcomes remain warning severity.

## Bounded error shape

Covered events retain stable code, retryability, message presence/length, and a closed seven-value error-kind label. The complete delegated `PortError`, its message text, and debug-formatted kind are not copied into diagnostics.

The same delegated `PortError` is returned unchanged.

## Preserved behavior

This source slice does not change:

- the `CustomerReadPort` trait or request/response DTOs;
- root or compatibility factory names;
- owner read-policy admission and deadline semantics;
- tenant parsing, list validation, tenant isolation, or profile enrichment;
- `CustomerService` queries and DTO construction;
- exact public codes, messages, kinds, or retryability;
- Commerce not-found handling or GraphQL fallback behavior;
- Customer FFA/FBA status.

## Evidence and remaining gaps

- `scripts/verify/verify-customer-read-local-context.mjs`;
- `scripts/verify/verify-customer-read-policy-context.mjs`;
- `crates/rustok-customer/contracts/evidence/customer-read-diagnostic-safety-source.json`;
- `crates/rustok-customer/contracts/evidence/customer-read-diagnostic-safety-source-review.json`.

Direct compatibility-path callers, customer write adapters, consumer-side transport mappers, profile transports, and runtime/remote evidence remain separate work. The broader ecommerce correlation-safe mapper cleanup remains open.

No test, Node verifier, Cargo command, formatter, workflow, CI, or mounted runtime target was executed for this source slice.
