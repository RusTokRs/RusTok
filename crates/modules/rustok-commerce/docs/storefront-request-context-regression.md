# Commerce storefront RequestContext regression recheck

Status: `source_corrected_unvalidated`.

## Source-of-truth relation

The ecommerce execution source of truth remains
`crates/rustok-commerce/docs/implementation-plan.md`. Its immediate execution item 10
remains open: this correction removes one false-complete regression but does not finish
raw public error cleanup across all ecommerce owners and non-`PortError` envelopes.

## Recheck finding

PR #2528 replaced raw Commerce storefront context-extraction errors with static public
envelopes, but its diagnostics referenced `RequestContext.correlation_id`. The current
`rustok_api::RequestContext` contract exposes tenant, optional authenticated user,
channel, channel resolution source, and locale; it does not expose a correlation id.
The stale field reference therefore made the source slice internally inconsistent and
could prevent the Commerce storefront package from compiling.

## Correction

- Keep the static request-context and tenant-context public envelopes introduced by
  PR #2528.
- Keep existing cart validation, cart owner, payment owner, DTO, endpoint, locale, and
  transport-selection behavior.
- Record only fields that exist on the current request contract:
  `tenant_id`, `user_id`, `channel_id`, `channel_slug`, and `locale`.
- Forbid `request_context.correlation_id` in the focused storefront transport guard.
- Retain source evidence as unvalidated until the maintainer runs the verifier and
  Commerce storefront compilation.

## Remaining item 10 work

This correction does not close the master ecommerce public-error item. Customer admin,
Pricing admin, remaining owner adapters, and remaining non-`PortError` REST, GraphQL,
native, and operator envelopes still require separate audits and source slices.

## Suggested maintainer verification

```text
node scripts/verify/verify-commerce-storefront-transport-error-safety.mjs
node scripts/verify/verify-commerce-storefront-transport-handoff.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-commerce-storefront --all-features
```

No command above was run by the implementation agent.
