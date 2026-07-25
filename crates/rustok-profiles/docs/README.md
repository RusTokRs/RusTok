# `rustok-profiles` Documentation

`rustok-profiles` is the domain module for the unified public user profile in RusToK. It defines the profile boundary over platform `users` without mixing auth identity, customer, and future seller/merchant surfaces.

## Purpose

- publish the canonical profile runtime contract for public profile and author/member summaries;
- keep storage, service, transport, privacy, and module-owned storefront boundaries inside the Profiles module family;
- provide downstream modules with one author/member presentation source without a direct dependency on `users`;
- provide privacy/access-policy reads from the tenant-scoped base profile row without coupling decisions to localized presentation data;
- resolve `followers_only` through bounded directional Social Graph owner reads and fail closed on owner errors;
- validate self-service avatar/banner references through the Media owner port before persisting profile links;
- expose Media-selected direct or immutable capability image descriptors through a public-safe Profiles DTO without constructing URLs locally;
- select embedded or extracted Media presentation through a typed runtime provider rather than transport-specific consumer code;
- publish a profile-by-handle storefront page and authenticated follow/unfollow control without moving domain ownership into the host application.

## Scope

- profile aggregate: `profiles`, `profile_translations`, `profile_tags`;
- `ProfileService`, `ProfilesReader`, `ProfileSummary`, and related DTO/enum contracts;
- `ProfilePrivacyReadPort` and the owner-local `ProfilePrivacyService` base-row adapter;
- bounded follower-policy composition through `SocialGraphPrivacyReadPort`;
- profile media owner/tenant/image validation over Media owner results;
- `ProfileMediaPublicImageProvider`, a transport-neutral `Arc<dyn MediaPublicImageReadPort>` wrapper supplied through schema/runtime extensions;
- `ProfileImagePresentation`, which accepts Media-selected absolute or root-relative image descriptors and rejects storage-relative/opaque/non-image descriptors that have not been made public by Media;
- public handle, display name, bio, avatar/banner references, locale, and visibility policy;
- GraphQL read/write surfaces for public profile lookup and self-service edit paths;
- `rustok-profiles-storefront` with `core`, explicit native/GraphQL `transport`, locale bundles, and Leptos UI;
- event contract `profile.updated` and backfill path for existing users.

## Integration

- `users` remains the identity/security boundary and does not become the public profile source;
- `rustok-customer` remains a separate commerce-domain profile with optional linkage to `user_id`;
- `rustok-blog` and `rustok-forum` use request-scoped audience-aware profile summary batches for author presentation;
- notification recipient policy consumes the privacy owner port without reading profile translations, tags, or foreign tables;
- `rustok-social-graph` owns follow persistence, actor binding, tenant scope, batch caps, directional reads, and follow mutations;
- `rustok-media` owns media lookup, public-addressability classification, capability URL construction, active-blob validation, and public byte delivery;
- GraphQL selects `ProfileMediaPublicImageProvider` from schema data or `ModuleRuntimeExtensions`; native storefront selects it from `HostRuntimeContext`; both use an embedded Media owner-service fallback when no provider is supplied;
- Profiles never imports `GrpcMediaProvider`, tonic, Media endpoint configuration, object storage, or public route strings;
- `rustok-taxonomy` provides a shared dictionary for `profile_tags`, while ownership of bindings remains with Profiles;
- `apps/storefront` mounts the manifest-declared `ProfilesView`; it does not own profile policy, Media provider selection, or follow state;
- native storefront server functions and GraphQL adapters use the same Profiles, Media, and Social Graph owner contracts and never fall back between their own page-data transports.

## Extraction boundary

Media control-plane descriptor selection may be embedded or remote because both implement `MediaPublicImageReadPort`. The returned descriptor can point to Media-owned HTTP delivery; binary image bytes and cache behavior never enter Profiles or generic gRPC DTOs. Deployment composition must register the selected provider and make its public descriptor URL reachable. Until compiled/live embedded and extracted-provider evidence exists, Profiles FBA remains `not_started`.

## Verification

- `cargo xtask module validate profiles`
- `cargo xtask module test profiles`
- `cargo check -p rustok-profiles-storefront --all-targets`
- `cargo test -p rustok-profiles-storefront`
- `cargo test -p rustok-media --test public_image_proxy -- --nocapture`
- `cargo test -p rustok-media-transport --test port_conformance -- --nocapture`
- `cargo test -p rustok-social-graph --test follow_sqlite -- --nocapture`
- `node scripts/verify/verify-media-public-image-proxy.mjs`
- targeted tests for handle policy, locale fallback, summary batching, GraphQL self-service/public visibility, profile backfill, privacy audience/tenant isolation, follower overlay policy, media owner/tenant/MIME validation, public descriptor presentation, provider selection, storefront input preparation, transport contract strings, and profile presentation helpers;
- storefront route/i18n/module-package verification after maintainer compilation.

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Storefront package](../storefront/README.md)
- [Platform documentation map](../../../docs/index.md)
