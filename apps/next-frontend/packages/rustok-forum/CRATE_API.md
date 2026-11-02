# @rustok/forum-frontend / CRATE_API

## 1. Public Interfaces & Architecture

`@rustok/forum-frontend` (`apps/next-frontend/packages/rustok-forum`) is the Next.js storefront package for the RusToK community forum module. It provides full discussion viewing, topic discovery, thread navigation, rich text replies with inline quotes, and draft persistence in full parity with `crates/modules/rustok-forum/storefront` (Leptos).

### Public Surface (`src/index.tsx`)
- **API Callers:**
  - `fetchStorefrontCategories`: Complete category tree resolution across all pagination pages.
  - `fetchStorefrontTopics`: Keyset-paginated topic feed with tag and category filtering.
  - `fetchStorefrontTopic`: Topic detail retrieval with current revision validation.
  - `fetchStorefrontReplies`: Thread reply pagination with forward/backward cursor navigation.
  - `fetchForumMemberCards`: Member identity cards and metadata hydration.
  - `markForumTopicRead`: Authenticated read receipt dispatch.
  - `createForumReply`: Multi-quote reply submission with OCC version checks.
  - `createForumTopic`: Topic creation with initial rich text document.
  - `resolveStorefrontTopicRoute`: Canonical route builder (`/forum/t/[...slug]`).
- **Components:** `ForumSection`, `Composer`, `QuoteTooltip`, `TimelineScroller`, `CategoryRail`, `TopicFeed`, `ThreadPanel`, `CategoryOverview`, `TopicStatusLabel`, `TopicDetailView`, `AuthorBadge`, `MemberCardProvider`.
- **Context:** `ComposerProvider`, `useComposer`.
- **Slot System:** Registers `forum-discussions` into `home:afterHero` with `order: 30` and `moduleSlug: "forum"`.

---

## 2. Key Contracts, Routes & Layouts

### Data Contracts
- `ForumTopicListItem`: ID, title, canonical slug, author, reply count, view count, pinned/locked/status flags, timestamps, and read state.
- `ForumTopicDetail`: Full topic document with `RichTextView`, category metadata, author profile, and read state.
- `ForumReplyDetail`: Thread reply node with author card, quote references, and rich text content.
- `ForumMemberCard`: User profile snapshot, join date, post counts, and badges.

### Thread Navigation & Deep Linking
- Route `/[locale]/forum/t/[...slug]` is the canonical forum topic entry point.
- URL parameter `?reply=<uuid>` triggers targeted timeline jump, auto-paging forward through cursors up to 20 pages, and scrolling into view.

---

## 3. Integration, Transport & Security Boundary

### Transport Boundary
- Dispatches typed GraphQL operations over shared `storefrontGraphql`.
- Respects backend `topics_per_page` and `replies_per_page` settings by omitting client-side override parameters.
- Error handling: Preserves GraphQL error details and renders contextual retry banners instead of masking errors as empty topic feeds.
- Read receipts: `markForumTopicRead` is strictly authenticated and never executed for anonymous viewers or missing topics.

### Draft Storage Boundary
- Local drafts are isolated by composite keys combining route, tenant slug, user ID, mode, and target reply parent to prevent cross-viewer or cross-tenant leaks.

---

## 4. Verification & Testing Evidence

- **Unit Test Suites (120 tests)**:
  - `category-pages.test.mjs`: Keyset connection paging without fake total fields.
  - `forum-selection.test.mjs` & `forum-unread.test.mjs`: Topic selection and unread feed filters.
  - `category-overview-hierarchy.test.mjs`: Grandchild hierarchy and cycle breaking.
  - `composer-preview.test.mjs`: Controlled discussion frame without duplicate quotes.
  - `topic-recovery.test.mjs`: Concurrent write serialization and lock release.
  - `quote-tooltip.test.mjs`: Quote targeting and unmount cancellation guards.
  - `thread-navigation.test.mjs`: Deep link jump, position clamping, and DOM commits.
  - `draft-storage.test.mjs`: Access-token subject binding and composite storage keys.
- **Storefront Boundary Gate**: `npm run verify:forum:storefront-boundary` passes cleanly.
- **Next.js Host Typecheck**: `npm run typecheck:next-frontend` completes with 0 errors.

---

## 5. Anti-Patterns & Operational Invariants

1. **No Client Page Size Overrides**: Never supply hardcoded `perPage` on storefront topic/reply queries; let backend tenant settings dictate pagination sizes.
2. **Never Mask Denials as Empty State**: Authorization refusals or field errors must render localized error banners with retry buttons, never empty lists.
3. **No Cross-Viewer Draft Bleed**: Draft storage keys must strictly include authenticated viewer identity and tenant scope.
4. **Canonical Topic Route**: Topics must always be addressed at `/forum/t/[...slug]`, matching search indexing, SEO targets, and redirect maps.

