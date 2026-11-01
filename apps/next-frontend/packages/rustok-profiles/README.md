# @rustok/profiles-frontend

Storefront user profile package for the `rustok-profiles` module (`crates/modules/rustok-profiles`) in the Next.js storefront (`apps/next-frontend`).

## Responsibilities

- Renders public storefront user profiles (`/profiles/[handle]`) with profile avatar, banner, bio, interests/tags, and visibility.
- Manages user follow / unfollow state with optimistic concurrency control (`expectedRevision`) and idempotency keys.
- Automatic recovery on concurrency revision conflict.
- Provides fallback profile initials calculation (`getProfileInitials`) supporting multi-word names, Cyrillic, and handle fallback.
- Disables follow controls for unauthenticated viewers and viewer's own profile (`isSelf`).

## Architecture & Integration

- Fluid Frontend Architecture (FFA) client components (`ProfilesView`, `ProfilePanel`, `ProfileAvatar`, `ProfileBanner`).
- Typed GraphQL documents: `PROFILE_QUERY`, `FOLLOW_STATE_QUERY`, `FOLLOW_MUTATION`, `UNFOLLOW_MUTATION`.

## Verification

Run scoped tests:
```bash
npm --prefix apps/next-frontend run test:profiles
```

Run host type check:
```bash
npm run typecheck:next-frontend
```

