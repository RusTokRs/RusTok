# @rustok/notifications-frontend

Next.js storefront package for RusToK customer notification inboxes, unread counter badges, grouped notification timelines, and bulk state management (mark read, archive).

## Features

- **Unread Counter Badge:** Real-time unread counter display (`NotificationUnreadBadge`) for navbar headers.
- **Grouped Timelines:** Cursor-paginated notification groups (`NotificationGroupCard`) with source, priority, and date headers.
- **Detailed Notification Items:** Granular item rows (`NotificationItemRow`) with action anchors and timestamp formatting.
- **Bulk Action State Machine:** Idempotent bulk state actions (`applyNotificationGroupState`) for reading, unreading, and archiving groups with cursor pagination.
- **Inbox Overview:** Complete unified inbox workspace (`NotificationsView`).

