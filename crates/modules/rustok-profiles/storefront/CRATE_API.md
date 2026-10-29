# rustok-profiles-storefront / CRATE_API

## Public surface

### Entry Point & Components
- `ProfilesView`: mounted Leptos storefront component rendering public profiles and follow controls.
- `core`: module-level business validation, handle normalization, and transport profile resolution.

### Models & Commands
- `ProfilesStorefrontPage`: composite view model for public profile page presentation.
- `ProfilesStorefrontProfile`: storefront representation of a user profile.
- `ProfilesStorefrontFollowState`: follower relationship state and optimistic revision.
- `SetProfilesStorefrontFollowCommand`: command DTO to follow/unfollow a user profile.
- `ProfilesStorefrontTransportProfile`: transport selector (`Native`, `Graphql`).

## Platform Capability Bindings
- **Storefront Route Integration**: Registers `ProfilesView` for `/modules/profiles?handle=<profile-handle>`.
- **Fluid Frontend Architecture**: Decouples framework-agnostic validation in `core` from Leptos rendering.
- **Audience-Bound Presentation**: Resolves profile presentation respecting tenant boundary and visibility matrix.

## Domain Collaborations
- **`rustok-profiles`**: Core profiles module providing profile query and persistence services.
- **`rustok-social-graph`**: Social graph module managing directional follow/unfollow relations.
- **`rustok-media`**: Public media image resolution for profile avatars and banners.
- **`rustok-ui-core`**: Core UI layout and accessibility primitives.
- **`rustok-ui-i18n`**: Localization and Fluent translation loading.

## Minimum Contract Set

### Input DTOs/Commands
- `SetProfilesStorefrontFollowCommand`: command DTO containing target user id, desired follow state, expected revision, and idempotency key.
- `normalize_profile_handle`: handle sanitizer validating `@` prefix and whitespace.
- `selected_transport_profile`: resolver parsing `RUSTOK_UI_TRANSPORT_PROFILE`.

### Domain Invariants
- **Non-Swallowed Errors**: Network or mutation failures must display diagnostic feedback and avoid silent swallowing.
- **Self-Follow Prohibition**: Self profiles never render or allow follow actions.
- **Privacy & Isolation**: Unavailable, hidden, blocked, or cross-tenant profiles share an identical unavailable state to prevent user enumeration.
- **Optimistic Concurrency**: Follow mutations track optimistic revisions and trigger a read-only refresh on failure rather than retrying blindly.

### Events / Outbox Side Effects
- Emits directional follow/unfollow relationship mutations to `rustok-social-graph`.
- Outbox events dispatch profile viewing metrics when configured.

### Errors / Failure Codes
- `ProfilesStorefrontInputError`: validation errors (`MissingHandle`, `InvalidUserId`, `InvalidRevision`).
- `ProfilesStorefrontActionError`: failure handling follow mutations and presentation queries.
