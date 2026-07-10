# rustok-seo-render implementation plan

## Current state

`rustok-seo-render` is the pure Rust-host last-mile renderer for canonical
`rustok-seo::SeoPageContext`. It serializes canonical URLs, robots, alternates,
Open Graph, Twitter, verification, pagination, generic tags, and JSON-LD in
deterministic order. `apps/storefront` consumes it instead of implementing a
second Rust renderer. The crate owns no SEO resolution, locale policy, storage,
or Next.js metadata mapping.

## Boundary

- Canonical SEO resolution and field precedence remain in `rustok-seo`.
- Rust hosts consume `render_head_html`; Next remains a separate TypeScript
  adapter over the same `SeoPageContext` contract.
- Parity compares semantic output, with explicit normalization only for
  documented nondeterministic fixture values.

## Next results

1. **Lock cross-host semantic fixtures.** Compare Rust rendering and Next
   metadata output over the shared runtime-parity fixture set, including the
   documented allowlist for host-specific long-tail differences. Done when a
   canonical, robots, hreflang, social, verification, pagination, or JSON-LD
   drift fails one cross-host contract check.
2. **Exercise the storefront SSR path.** Run `SeoPageContext` through
   `storefront/seo-page-context` and head rendering under tenant/module/locale
   and fallback scenarios. Done when an SSR integration test proves the host
   uses this renderer and cannot fall back to a local serializer.
3. **Harden renderer safety regressions.** Add focused cases for escaping,
   malformed structured-data payloads, deterministic ordering, and multi-block
   JSON-LD metadata preservation. Done when unsafe or non-deterministic output
   is rejected by renderer self-tests.

## Verification

- `cargo test -p rustok-seo-render --lib`
- `cargo check -p rustok-storefront --config profile.dev.debug=0`
- Next storefront lint/typecheck and shared SEO runtime-parity fixture checks.

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [SEO module plan](../../docs/implementation-plan.md)
- [Next runtime parity fixtures](../../../../apps/next-frontend/contracts/seo/runtime-parity-fixtures.json)
