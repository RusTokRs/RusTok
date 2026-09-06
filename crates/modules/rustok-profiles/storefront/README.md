# `rustok-profiles-storefront`

Module-owned storefront surface for public profiles and follow/unfollow controls.

## Contract

- route: `/modules/profiles?handle=<profile-handle>`;
- public profile data is loaded through the audience-bound Profiles owner presentation contract;
- anonymous visitors can view only profiles allowed by the canonical visibility matrix;
- authenticated human users can read and mutate directional follow state through Social Graph owner ports;
- follow-state reads include the current optional optimistic revision without exposing relation ids;
- self profiles never render a follow action;
- missing, restricted, hidden, blocked, and cross-tenant profiles share one unavailable state;
- native and GraphQL transports are selected explicitly and never fall back to each other;
- missing `RUSTOK_UI_TRANSPORT_PROFILE` deliberately selects Native, while explicit values are limited to `native` and `graphql`; unknown configured values fail closed instead of silently changing transport;
- optimistic revisions are retained after successful writes and revision-bearing reads;
- after a failed write, the UI performs one read-only state refresh and never retries the mutation automatically;
- avatar/banner presentation calls `MediaPublicImageReadPort`, then Profiles revalidates tenant, uploader, and image MIME before rendering;
- server composition selects one `ProfileMediaPublicImageProvider` and publishes the same wrapper through `ModuleRuntimeExtensions` to GraphQL and native server-function `HostRuntimeContext`;
- an explicitly configured remote provider wins over an existing extension provider, which wins over the embedded Media owner-service fallback;
- invalid remote configuration or an unavailable configured gRPC service fails server startup rather than silently falling back;
- the package never imports `GrpcMediaProvider`, tonic, Media endpoints, object keys, or public route construction;
- absolute direct-public Media descriptors remain unchanged;
- an extracted transport may rebase a root-relative Media capability descriptor against a validated public Media origin before Profiles receives it;
- storage-relative images receive a Media-owned immutable capability URL containing the asset id and active-blob SHA-256; Profiles never constructs that URL;
- opaque, missing, invalid-owner, non-image, and unavailable descriptors degrade to the package fallback without hiding the profile;
- banner imagery is decorative, avatar fallbacks retain an accessible name, follow controls expose pressed/busy state, and async status is announced through live regions;
- the storefront never reads Media or Social Graph storage directly.

## Extraction boundary

`ProfileMediaPublicImageProvider` is transport-neutral. Embedded deployments use `MediaPublicImageService`. The server may select an extracted public-image provider through `RUSTOK_PROFILE_MEDIA_PROVIDER=grpc`, a validated HTTPS gRPC endpoint, bounded connection timeout, optional TLS domain, and optional Media public origin. Plaintext transport requires explicit loopback opt-in.

The returned descriptor may point to a Media-owned HTTP capability route, but image bytes and cache behavior never cross the Profiles or gRPC control plane. Source composition carries one selected wrapper to both presentation paths. Production mutual TLS/client identity, reachable Media HTTP ingress, Local/S3 capability delivery, and retained runtime evidence remain operational gates.

## Verification

```bash
cargo check -p rustok-profiles-storefront --all-targets
cargo test -p rustok-profiles-storefront
cargo test -p rustok-media --test public_image_proxy -- --nocapture
cargo test -p rustok-media-transport --test port_conformance -- --nocapture
cargo test -p rustok-social-graph --test follow_state_sqlite -- --nocapture
node scripts/verify/verify-media-public-image-proxy.mjs
node scripts/verify/verify-profiles-media-provider-composition.mjs
node scripts/verify/verify-profiles-storefront-boundary.mjs
```

These commands are maintainer-run and were not executed while publishing this slice.
