# Implementation plan for `rustok-profiles`

## Current state

`rustok-profiles` owns the public profile domain over platform users: profile
storage/translations, profile tags, handle and visibility policy,
`ProfileService`, `ProfilesReader`, summary batching, GraphQL read/self-service
write surfaces, `profile.updated`, backfill helpers, and the recipient privacy
owner port.

It is not an auth identity, customer, seller, or staff-role aggregate. Blog and
forum consume `ProfilesReader` for author presentation; taxonomy supplies the
tag dictionary while profile-tag bindings remain module-owned. Notification
recipient policy consumes `ProfilePrivacyReadPort`, whose owner adapter reads
only the tenant-scoped base `profiles` row and does not depend on translations,
tags, media joins, or downstream models.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- The module has GraphQL and reader contracts but no module-owned UI or FBA
  provider port yet.

## Results and next work

1. **Use owner-local direct reads for the current read model.**
   **Status:** completed for the current consumer shape. Localized author/member
   summaries remain on the tenant-scoped batched `ProfilesReader` path; a
   dedicated projection table is not justified while blog/forum consume bounded
   summary batches and no measured latency requirement exceeds that contract.
   Privacy decisions use a separate minimal base-row adapter so presentation
   integrity cannot make access-policy reads unavailable.
   **Revisit when:** production query telemetry shows sustained summary latency,
   fan-out, or denormalization requirements that the batched owner read cannot
   meet.
   **Ownership boundary:** profile rows, translations, tag bindings, locale
   fallback, batching, and privacy state remain owned by `rustok-profiles`.

2. **Finish profile visibility, media, and handle policy.** Resolve remaining
   public/private visibility, authenticated/followers semantics, avatar/banner
   reference validation, and tenant-scoped handle uniqueness decisions without
   merging customer or seller concerns. The notification privacy adapter now
   reads status/visibility independently from localized copy; downstream public
   profile and author-card enforcement still needs one documented policy.
   **Depends on:** public-profile product requirements, social-graph follower
   capability, and the media owner contract.
   **Done when:** GraphQL, `ProfilesReader`, privacy ports, backfill, and
   downstream author cards expose the same policy with targeted tests.

3. **Add UI and operational capabilities only after the domain stabilizes.**
   Introduce a module-owned profile UI, audit trail, observability, and rollout
   runbook only from a defined profile contract.
   **Depends on:** approved UI/operational requirements.
   **Done when:** the new surface has an owner package, public transport
   contract, profile-conflict recovery guidance, and no auth/customer leakage.

4. **Move profile backfill to an owner-local operations adapter.**
   **Status:** source-complete; compiled/runtime verification pending. The module
   CLI provider uses owner-owned auth, tenant, and customer reads plus
   `OutboxTransport` for optional event publishing, preserves dry-run/event
   semantics, and does not import server models or query customer internals
   directly. The legacy task has been removed.
   **Next verification:** compile and exercise `rustok-cli profiles backfill`
   against the supported runtime inputs.

## Verification

- `cargo xtask module validate profiles`
- `cargo xtask module test profiles`
- Targeted handle policy, locale fallback, summary batching, GraphQL
  self-service, backfill, event, and privacy base-row/tenant-isolation tests.

## Change rules

1. Keep public profile policy and storage in this module.
2. Keep privacy/access-policy reads independent from localized presentation
   integrity and foreign domain tables.
3. Update local docs, `rustok-module.toml`, and blog/forum consumer docs with a
   public-profile contract change.
4. Update `docs/modules/registry.md` and this status block with an FFA/FBA or
   module-owned UI boundary change.
