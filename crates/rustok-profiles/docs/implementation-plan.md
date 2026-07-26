# Implementation plan for `rustok-profiles`

## Current state

`rustok-profiles` owns the public profile domain over platform users: profile storage/translations, profile tags, handle and visibility policy, `ProfileService`, raw `ProfilesReader`, audience-bound `ProfilePresentationService`, summary batching, GraphQL read/self-service write surfaces, `profile.updated`, backfill helpers, the recipient privacy owner port, Media-backed image presentation policy, and the first module-owned storefront profile surface.

It is not an auth identity, customer, seller, or staff-role aggregate. Raw `ProfileService` / `ProfilesReader` remains available for owner-internal workflows such as mention resolution and backfill. Downstream author/member/customer cards use `ProfilePresentationService`, which evaluates the canonical privacy batch before localized summary/tag loading. Notification recipient policy consumes `ProfilePrivacyReadPort`, whose owner adapter reads only the tenant-scoped base `profiles` row and composes `followers_only` through the bounded Social Graph owner port.

The server GraphQL host replaces the schema fallback `ProfileSummaryLoader` with an audience-bound request loader. The loader delegates to `ProfilePresentationService`, so Blog and Forum `authorProfile` resolution and non-GraphQL presentation consumers share the same owner composition. Public GraphQL profile lookups hide restricted or unavailable profiles as absent. Customer Admin detail/create/update constructs the same service from the authenticated request audience instead of passing raw `ProfileService`; customer permissions do not become profile ownership.

Self-service avatar/banner writes resolve references through the Media owner read port and accept only current-tenant image assets uploaded by the profile owner. Public GraphQL and native storefront image reads call `MediaPublicImageReadPort`, receive the canonical `MediaItem` plus a Media-selected descriptor, reapply the tenant/profile-uploader/image invariant, and expose only the descriptor returned by Media. Direct-public URLs remain unchanged; storage-relative image paths receive a Media-owned immutable capability URL. Profiles never reads object keys or constructs proxy URLs.

`rustok-media-transport` implements remote public descriptor reads and now owns validated extracted-client connection policy. The server defaults to embedded Media but may explicitly select `grpc` through environment configuration. External endpoints and public origins require HTTPS with webpki roots; plaintext requires explicit loopback opt-in; connection timeout is bounded; invalid remote variables are not silently ignored in embedded mode. The selected remote adapter is pre-seeded as `ProfileMediaPublicImageProvider` before runtime/schema materialization, and GraphQL plus native storefront receive the same wrapper. Profiles never imports the gRPC adapter type or endpoint configuration.

`rustok-profiles-storefront` owns the first public profile UX slice. It mounts through `rustok-module.toml`, reads `?handle=` through `leptos-ui-routing`, provides SSR-first native server functions and a parallel GraphQL compatibility transport, renders approved avatar/banner descriptors with deterministic fallbacks, and exposes authenticated follow/unfollow controls through Social Graph owner contracts. Native and GraphQL reads consume a revision-bearing owner follow-state contract, and failed writes recover through one read-only refresh without automatic mutation retry. The host only mounts the package. The compile-time storefront transport accepts only `native` or `graphql`; an unknown configured value fails closed instead of silently selecting another transport.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `not_started`
- Structural shape: `core_transport_ui`
- The module has a module-owned Leptos storefront package with framework-agnostic input/command/recovery and image-presentation policy, explicit fail-closed transport selection, native and GraphQL adapters, locale bundles, and a UI adapter.
- Media has source-level embedded/loopback public-descriptor control-plane parity, and Profiles has source-complete selected-provider host composition plus fail-closed embedded/grpc deployment configuration. FBA remains `not_started` because mutual TLS/client identity, compiled/live provider-consumer behavior, fallback/recovery, public HTTP ingress, Local/S3 capability delivery, and runtime evidence have not been collected.

## Results and next work

1. **Use owner-local direct reads for owner workflows and audience-bound reads for presentation.**
   **Status:** source-complete for the current consumer shape. Localized storage remains on `ProfileService`; `ProfilePresentationService` performs one privacy batch, keeps only `Allow` ids, and then loads localized summaries. Handle lookups read only the tenant-scoped base row before privacy and load localized presentation only after `Allow`. `ProfileSummaryLoader`, Customer Admin, and native storefront reads delegate to this service rather than duplicating privacy composition.
   **Revisit when:** production query telemetry shows sustained summary latency, fan-out, or denormalization requirements that the batched owner read cannot meet.
   **Ownership boundary:** profile rows, translations, tag bindings, locale fallback, batching, and privacy state remain owned by `rustok-profiles`; follow persistence and relation lookup remain owned by `rustok-social-graph`.

2. **Finish followers-only and downstream presentation policy.**
   **Status:** source-complete for the owner privacy path, public GraphQL lookups, GraphQL author-card consumers, storefront reads, and Customer Admin profile enrichment. Active public profiles allow anonymous, authenticated, and trusted-service audiences. Active authenticated profiles require a human or trusted-service audience. Active private profiles remain owner-only. Active followers-only profiles allow the owner or an audience actor with an active directional Social Graph follow relation. Hidden or blocked state fails closed. Restricted/unavailable profiles remain absent.
   **Social Graph status:** completed for the required owner capability and public GraphQL transport. `SocialRelationKind::Follow`, PostgreSQL/SQLite migration, directional single reads, bounded/deduplicated batch reads, revision-bearing state reads, actor binding, tenant integrity, and targeted source tests are present. Profiles calls `SocialGraphPrivacyReadPort` in chunks of at most 100 and propagates owner failures instead of converting them into allow. `isFollowing`, `followState`, `followUser`, and `unfollowUser` expose tenant-gated human-user transport without relation storage leakage.
   **Batch presentation status:** `ProfileSummaryLoader::for_audience`, Customer Admin, and native storefront profile reads use `ProfilePresentationService`. Human Customer Admin operators retain their real actor id; service principals receive trusted-service audience without an owner id. Customer permissions never grant private-profile ownership or bypass followers-only policy.
   **Media status:** source-complete for embedded self-service writes/public presentation, Media embedded/loopback descriptor transport parity, host provider selection, and server embedded/grpc deployment configuration. `MediaPublicImageReadPort` keeps descriptor selection inside Media. Remote configuration requires an explicit provider selector and endpoint, external HTTPS, bounded connect timeout, and optional validated public-origin rebasing. Deployment-preseeded providers override module/embedded candidates; GraphQL and native storefront receive the same typed wrapper and always revalidate tenant/uploader/MIME on the returned asset. Direct-public URLs remain direct; storage-relative images use Media capability URLs bound to asset id and active-blob SHA-256; opaque, missing, non-image, invalid-owner, and unavailable results produce `null`/fallback without hiding the profile.
   **Remaining:** configure production mutual TLS/client identity and reachable Media HTTP ingress, then collect compiled/runtime evidence for Blog, Forum, Customer Admin, storefront audience states, embedded/remote selection, public-image route/cache/degradation behavior, and Local/S3 delivery.
   **Done when:** GraphQL, presentation services, privacy ports, backfill, and every downstream author/member/customer card expose the same policy with retained evidence and no direct foreign-domain reads.

3. **Publish module-owned profile storefront UI.**
   **Status:** source-complete for the first Leptos slice plus optimistic recovery, accessibility hardening, Media capability presentation, shared host-selected public-image provider composition, server-side embedded/grpc selection, and fail-closed storefront transport configuration. `rustok-profiles-storefront` owns a manifest-mounted `/modules/profiles?handle=<handle>` page, public unavailable state, avatar/banner presentation, authenticated follow/unfollow control, self-profile suppression, package-owned en/ru messages, and explicit native/GraphQL transport selection with a `never falls back` policy. Missing configuration defaults deliberately to Native; supported explicit values are `native` and `graphql`; any other configured value is rejected. Follow-state reads retain optional owner revision; mutations generate unique idempotency keys and retain returned revisions. A failed mutation triggers one read-only state refresh, applies only a matching target state, and never retries the write automatically.
   **Boundary:** `apps/storefront` only includes and mounts the package through generated module UI code. It does not own query strings, profile policy, Media URL classification/capability construction, Media transport type/endpoints, Social Graph persistence, or profile view-model logic.
   **Remaining:** exercise embedded/remote Media providers, public origin routing, invalid/unavailable startup behavior, and both supported storefront transport profiles; collect SSR/hydrate/GraphQL route, auth, i18n, Media direct/proxy/fallback degradation, mutation-conflict/recovery, and accessibility runtime evidence; decide whether a profile directory/search contract is required; add operational telemetry.
   **Done when:** module validation, route/i18n checks, native and GraphQL runtime parity, optimistic conflict recovery, audience states, direct-public/proxy/fallback media states, extracted Media routing, fail-closed configuration, and accessibility have retained evidence.

4. **Move profile backfill to an owner-local operations adapter.**
   **Status:** source-complete; compiled/runtime verification pending. The module CLI provider uses owner-owned auth, tenant, and customer reads plus `OutboxTransport` for optional event publishing, preserves dry-run/event semantics, and does not import server models or query customer internals directly.
   **Next verification:** compile and exercise `rustok-cli profiles backfill` against supported runtime inputs.

5. **Add audit and operational capabilities from defined owner contracts.**
   Introduce profile audit trail, observability, rollout guidance, Social Graph command receipts/outbox/reconciliation, and moderation repair commands without moving owner state into UI or host applications.
   **Depends on:** approved operational requirements and runtime evidence from the storefront/follower slice.
   **Done when:** operations have typed owner ports, stable error/recovery guidance, retained evidence, and no auth/customer leakage.

## Recheck checkpoint — 2026-07-26

- Reconciled the canonical plan with draft PR #2152 and current `main`.
- Rechecked privacy-before-presentation ordering, bounded followers-only reads, owner-scoped follow commands, Media-owned public descriptors, shared provider composition, optimistic revision recovery, and the no-write-retry rule at source level.
- Closed the storefront transport-policy gap: unknown configured transport values no longer silently select Native, and the source verifier now locks this fail-closed behavior.
- Compilation, tests, formatters, verifier execution, workflows, and runtime evidence remain maintainer-run and were not executed for this checkpoint.

## Verification

- `cargo xtask module validate profiles`
- `cargo xtask module test profiles`
- `cargo check -p rustok-profiles-storefront --all-targets`
- `cargo test -p rustok-profiles-storefront`
- `cargo test -p rustok-media --test public_image_proxy -- --nocapture`
- `cargo test -p rustok-media-transport --test port_conformance -- --nocapture`
- `cargo test -p rustok-social-graph --test follow_sqlite -- --nocapture`
- `cargo test -p rustok-social-graph --test follow_state_sqlite -- --nocapture`
- `npm run verify:customer:admin-boundary`
- `node scripts/verify/verify-media-public-image-proxy.mjs`
- `node scripts/verify/verify-profiles-media-provider-composition.mjs`
- `node scripts/verify/verify-profiles-storefront-boundary.mjs`
- storefront route, module-package, en/ru i18n, Media provider/delivery/degradation, and accessibility verification

## Change rules

1. Keep public profile policy and storage in this module.
2. Keep privacy/access-policy reads independent from localized presentation integrity and foreign-domain tables.
3. Public GraphQL and storefront reads must use the owner-defined visibility matrix and must not distinguish restricted, unavailable, and missing profiles.
4. Downstream presentation consumers must use `ProfilePresentationService` or an equivalent owner-bound batch; raw `ProfileService` / `ProfilesReader` is reserved for explicitly owner-internal workflows.
5. `followers_only` must resolve through the Social Graph owner port with directional actor-to-profile-owner semantics, bounded batches, and fail-closed errors.
6. GraphQL hosts must bind `ProfileSummaryLoader` to the current request audience; the schema-level constructor remains anonymous and fail-closed.
7. Profile media references and descriptors must resolve through Media owner ports. Profiles may expose only Media-selected public descriptors, must revalidate tenant/uploader/MIME, and must never construct storage or capability URLs.
8. Remote Media selection must be injected through `ProfileMediaPublicImageProvider`; Profiles must not know gRPC endpoints, Media object storage, public route construction, or deployment ingress.
9. Module-owned UI must live under `crates/rustok-profiles/storefront`, read URL state through shared routing, use package-owned i18n, keep transports explicit with no automatic fallback, and reject unknown configured transport profiles.
10. Storefront follow controls must bind authenticated tenant/user through owner ports, suppress self-follow, use idempotency and optimistic revision semantics, recover stale state through read-only refresh, never retry writes automatically, and avoid exposing internal identifiers or transport details.
11. Update Profiles and affected owner docs with every presentation-boundary change.
