# @rustok/notifications-frontend / CRATE_API

## 1. Public Interfaces & Architecture

`@rustok/notifications-frontend` (`apps/next-frontend/packages/rustok-notifications`) provides the customer-facing notification inbox, timeline groupings, and unread badge widgets for RusToK. It operates in full parity with `crates/modules/rustok-notifications/storefront` (Leptos).

### Public Surface (`src/index.tsx`)
- **Components:** `NotificationGroupCard`, `NotificationItemRow`, `NotificationUnreadBadge`, `NotificationsView`.
- **API Callers:**
  - `fetchUnreadCount(graphql, token, tenantSlug): Promise<number>`
  - `fetchGroupSummaries(graphql, cursor, limit, token, tenantSlug): Promise<{ groups, nextCursor, hasMore }>`
  - `fetchGroupItems(graphql, groupKey, state, cursor, limit, token, tenantSlug): Promise<NotificationGroupItemsPage>`
  - `applyNotificationGroupState(graphql, command, token, tenantSlug): Promise<NotificationGroupStateResult>`
  - `loadInboxSnapshot(graphql, token, tenantSlug): Promise<NotificationStorefrontInboxSnapshot>`
- **GraphQL Documents:** `UNREAD_COUNT_QUERY`, `GROUP_SUMMARIES_QUERY`, `GROUP_ITEMS_QUERY`, `GROUP_STATE_MUTATION`.
- **Types:** `NotificationItemState`, `NotificationPriority`, `NotificationGroupStateAction`, `NotificationTemplateDatum`, `NotificationStorefrontItem`, `NotificationStorefrontGroupSummary`, `NotificationStorefrontInboxSnapshot`, `NotificationGroupItemsPage`, `NotificationGroupStateResult`.

---

## 2. Key Contracts, Routes & Layouts

### Data Contracts
- `NotificationStorefrontItem`: Unique ID, source module, event type, template key, actor ID, priority (`LOW`, `NORMAL`, `HIGH`, `URGENT`), state (`UNREAD`, `SEEN`, `READ`, `ARCHIVED`), key-value `templateData` pairs, and ISO 8601 timestamps.
- `NotificationStorefrontGroupSummary`: `groupKey`, `itemCount`, `unreadCount`, and preview of `latestItem`.
- `NotificationStorefrontInboxSnapshot`: Aggregated inbox state with total `unreadCount`, list of group summaries, and cursor pagination bounds.

---

## 3. Integration, Transport & Security Boundary

### Transport Boundary
- Dispatches typed GraphQL operations over shared `storefrontGraphql`.
- Propagates Bearer authorization tokens and `X-Tenant-Slug` headers.
- Re-throws server and transport errors on state mutations (`applyNotificationGroupState`) while providing resilient defaults (`catch(() => 0)`) for non-critical snapshot fetches.
- Requires caller-supplied `idempotencyKey` on `applyNotificationGroupState`.

---

## 4. Verification & Testing Evidence

- **Unit Tests**: `notifications-frontend.test.mjs` verifies GraphQL operation schemas, snapshot aggregation fallbacks, and command mapping.
- **Next.js Host Typecheck**: `npm run typecheck:next-frontend` completes with 0 errors.
- **Next.js Host Linter**: `npm run lint` passes with 0 warnings.

---

## 5. Anti-Patterns & Operational Invariants

1. **Idempotent Group State Mutations**: Bulk operations must always supply an idempotency key to prevent double execution on retry.
2. **Fail-Safe Counter Fallbacks**: If unread counter queries fail on network interruption, fallback gracefully to `0` rather than crashing header bars.
3. **Cursor Paging Safety**: Always observe `hasMore` and `nextCursor` when iterating over notification groups and items.

