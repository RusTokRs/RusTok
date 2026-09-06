# Commerce tenant-locale owner cutover

Status: `source_ready_unvalidated`

## Finding

`StoreContextService` previously queried `tenants.default_locale` and
`tenant_locales` directly and normalized locale tags with a package-local
lowercasing rule. That made commerce a second authority for tenant locale
policy, bypassed the tenant owner's revisioned policy projection, and diverged
from canonical locale semantics such as `pt_br -> pt-BR`.

Severity: `P1` cross-module ownership and multilingual consistency defect.

## Source correction

- `StoreContextService` keeps its public database-backed constructor, but the
  constructor now composes one `TenantService` behind `TenantReadPort` and
  `TenantLocalePolicyPort` trait objects.
- Tenant existence and active-state admission use `TenantReadPort` with an
  explicit deadline and `include_inactive: false`.
- Default and enabled locales come from `TenantLocalePolicyPort`; commerce no
  longer reads tenant owner tables.
- Requested locale normalization uses `rustok_api::TenantLocale` instead of a
  package-local normalizer.
- A mismatch between the tenant read projection and locale-policy default fails
  closed with `tenant.locale_policy_default_mismatch`.
- Cargo, `rustok-module.toml`, and `CommerceModule::dependencies()` all declare
  the tenant owner dependency.

## Retained evidence

- `scripts/verify/verify-commerce-tenant-locale-boundary.mjs` rejects direct
  tenant SQL, the removed local loaders/normalizer, or dependency declaration
  drift.
- `context_service_test::resolve_context_uses_owner_canonical_locale_tags`
  inserts `pt-BR`, requests `pt_br`, and requires the owner-canonical `pt-BR`
  result.
- The shared commerce SQLite fixture now contains the current tenant locale
  `policy_revision` and `updated_at` columns required by the owner entity.
- `.github/workflows/tenant-hardening.yml` runs focused formatting, the source
  guard, `cargo check -p rustok-commerce --test context_service_test`, the
  context test, and `cargo xtask module validate commerce` on one SHA.

## Pending execution

The source correction does not promote Commerce FBA/FFA status. Completion
requires the final-SHA focused workflow to compile and run the context test and
module validation. Broader commerce owner-boundary, migration, replay, and
remote-profile gates remain governed by
`crates/rustok-commerce/docs/implementation-plan.md`.
