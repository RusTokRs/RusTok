# `@rustok/profiles-frontend` API Specification

## 1. Overview & Ownership

`@rustok/profiles-frontend` is the Next.js storefront package for the `rustok-profiles` module (`crates/modules/rustok-profiles`). It delivers public user profile cards, banner and avatar media rendering, bio descriptions, interest tags, and interactive follow/unfollow capabilities with revision-based optimistic concurrency control.

- **Package Name**: `@rustok/profiles-frontend`
- **Location**: `apps/next-frontend/packages/rustok-profiles`
- **Owner**: `rustok-profiles` domain module
- **Runtime Environment**: Next.js App Router (React 19, Client Components)

---

## 2. Public API Surface & Exports

### Types

- `ProfilesStorefrontImage`: Profile avatar or banner image descriptor (`url`, `alt`, `width`, `height`, `mimeType`).
- `ProfilesStorefrontProfile`: Full public profile view model (`userId`, `handle`, `displayName`, `bio`, `tags`, `avatarImage`, `bannerImage`, `preferredLocale`, `visibility`).
- `ProfilesStorefrontFollowState`: Follow relationship status (`userId`, `following`, `revision`).
- `ProfilesStorefrontPage`: Aggregated profile page data including profile, follow state, and viewer flags (`viewerAuthenticated`, `isSelf`).
- `SetProfilesStorefrontFollowCommand`: Mutation command with `userId`, target `following` boolean, `expectedRevision`, and `idempotencyKey`.

### Operations & Queries

- `PROFILE_QUERY`: GraphQL document retrieving public profile by handle.
- `FOLLOW_STATE_QUERY`: GraphQL document retrieving active viewer follow state for a user.
- `FOLLOW_MUTATION` / `UNFOLLOW_MUTATION`: GraphQL documents executing follow/unfollow state transitions.
- `fetchProfileByHandle(graphql, handle, locale?, token?, tenantSlug?): Promise<ProfilesStorefrontProfile | null>`:
  - Fetches public profile by handle; propagates GraphQL errors.
- `fetchFollowState(graphql, userId, token?, tenantSlug?): Promise<ProfilesStorefrontFollowState | null>`:
  - Fetches follow state for target user; propagates GraphQL errors.
- `setProfilesStorefrontFollow(graphql, command, token?, tenantSlug?): Promise<ProfilesStorefrontFollowState>`:
  - Submits follow/unfollow mutation with idempotency key and revision tracking.
- `loadProfilesStorefrontPage(graphql, handle, locale?, token?, tenantSlug?, viewerUserId?): Promise<ProfilesStorefrontPage>`:
  - Orchestrates fetching profile and follow state, computing `isSelf` and authentication flags.

### Pure Domain Helpers & Components

- `getProfileInitials(profile)`:
  - Calculates up to two uppercase initial letters from `displayName`, falling back to first letter of `handle` or `'?'`.
- `ProfileAvatar`:
  - Renders user avatar image or initials badge with high-contrast accessibility labels.
- `ProfileBanner`:
  - Renders cover image or responsive background gradient fallback.
- `ProfilePanel`:
  - Profile card displaying header, bio, follow button, visibility, and tags with concurrency recovery.
- `ProfilesView`:
  - Discovery page view coordinating handle search form, router navigation, loading skeletons, and profile presentation.

---

## 3. Architecture & Follow State Lifecycle

1. **Viewer Authentication & Ownership Separation**:
   - `viewerUserId === profile.userId` sets `isSelf = true`, replacing the follow button with a neutral "This is your profile" notice.
   - Unauthenticated visitors see "Sign in to follow this profile".
2. **Revision-Based Concurrency & Recovery**:
   - When following or unfollowing, the client sends `expectedRevision`.
   - If another session updated the follow status concurrently, the mutation rejects. `ProfilePanel` automatically fetches the latest follow state (`fetchFollowState`), resets state to the remote revision, and prompts the user to retry.
3. **Idempotent Mutations**:
   - Every mutation payload includes an `idempotencyKey` formatted as `follow-<userId>-<timestamp>`.

---

## 4. Universal Logic Dimensions

### 1. Preconditions & Bounds
- `getProfileInitials`: Handles empty names, multi-space delimiters, Cyrillic / Unicode scripts, single-letter handles, and empty strings.
- Search form in `ProfilesView` strips leading `@` and trims whitespace, ignoring empty submissions.

### 2. Branch Completeness & Anti-Swallowing
- `fetchProfileByHandle`, `fetchFollowState`, and `setProfilesStorefrontFollow` inspect `result.errors` and throw descriptive errors instead of silently swallowing execution failures.

### 3. State Transition Integrity
- Follow button tracks `busy` and `aria-busy` states to disable repeated clicks during pending network roundtrips.
- Unauthenticated viewers and self-profiles cannot transition follow state.

### 4. Side-Effect Completeness
- `useEffect` in `ProfilesView` checks `cancelled` before committing asynchronous state updates.
- Concurrency conflicts refresh active revision state automatically.

### 5. Calculation & Algorithmic Correctness
- `split(/\s+/)` correctly splits consecutive spaces in names without producing empty initial characters.

### 6. Order of Operations & Concurrency
- `loadProfilesStorefrontPage` first validates profile existence; only fetches follow state when the viewer is authenticated and `!isSelf`.

---

## 5. Verification & Testing

- **Contract Unit Tests**: `apps/next-frontend/packages/rustok-profiles/src/profiles-frontend.test.mjs`.
- **Execution Command**:
  ```bash
  npm --prefix apps/next-frontend run test:profiles
  ```
- **Type Checking**:
  ```bash
  npm run typecheck:next-frontend
  ```

