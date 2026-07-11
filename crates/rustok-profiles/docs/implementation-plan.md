# Implementation plan for `rustok-profiles`

## Current state

`rustok-profiles` owns the public profile domain over platform users: profile
storage/translations, profile tags, handle and visibility policy,
`ProfileService`, `ProfilesReader`, summary batching, GraphQL read/self-service
write surfaces, `profile.updated`, and backfill helpers.

It is not an auth identity, customer, seller, or staff-role aggregate. Blog and
forum consume `ProfilesReader` for author presentation; taxonomy supplies the
tag dictionary while profile-tag bindings remain module-owned.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- The module has GraphQL and reader contracts but no module-owned UI or FBA
  provider port yet.

## Open results

1. **Decide and implement the required read model.** Determine whether direct
   profile/translations reads remain sufficient for downstream summaries or a
   dedicated projection is needed.
   **Depends on:** measured consumer query patterns and summary latency needs.
   **Done when:** the selected model has tenant/locale semantics, batching
   behavior, and a documented ownership boundary.

2. **Finish profile visibility, media, and handle policy.** Resolve remaining
   public/private visibility, avatar/banner reference, and tenant-scoped handle
   uniqueness decisions without merging customer or seller concerns.
   **Depends on:** public-profile product requirements and media contract.
   **Done when:** GraphQL, `ProfilesReader`, backfill, and downstream author
   cards expose the same policy with targeted tests.

3. **Add UI and operational capabilities only after the domain stabilizes.**
   Introduce a module-owned profile UI, audit trail, observability, and rollout
   runbook only from a defined profile contract.
   **Depends on:** approved UI/operational requirements.
   **Done when:** the new surface has an owner package, public transport
   contract, profile-conflict recovery guidance, and no auth/customer leakage.

4. **Move profile backfill to an owner-local operations adapter.** The current
   legacy server task has a neutral execution chain and reads users through the
   auth-owned `AuthUserBackfillReadPort`, but tenant reads remain platform
   concerns and optional customer enrichment remains customer-owned. Define
   explicit CLI runtime inputs before adding `profiles
   backfill` to a module CLI; the profiles module must not import server models
   or query customer internals directly.
   **Depends on:** the auth-owned `AuthUserBackfillReadPort`, the existing
   `TenantReadPort`, and the customer-owned profile-enrichment projection.
   **Done when:** `rustok-cli profiles backfill` is provided by a
   `rustok-profiles/cli` adapter, preserves dry-run/event semantics, and the
   Loco task is deleted. **Completed:** the provider uses owner-owned auth,
   tenant and customer reads plus `OutboxTransport` for optional event
   publishing; compiled CLI/runtime evidence remains the next verification
   step.

## Verification

- `cargo xtask module validate profiles`
- `cargo xtask module test profiles`
- Targeted handle policy, locale fallback, summary batching, GraphQL
  self-service, backfill, and event tests.

## Change rules

1. Keep public profile policy and storage in this module.
2. Update local docs, `rustok-module.toml`, and blog/forum consumer docs with a
   public-profile contract change.
3. Update `docs/modules/registry.md` and this status block with an FFA/FBA or
   module-owned UI boundary change.
