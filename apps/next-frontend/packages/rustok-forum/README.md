# @rustok/forum-frontend

Next.js storefront package for RusToK community forum discussions, topic feeds, thread navigation, rich text composer with quote tooltips, and category overviews.

## Features

- **Storefront Slot Registration:** Registers `forum-discussions` into `home:afterHero` slot (`order: 30`) via `@/modules/registry`.
- **Hierarchical Categories:** Recursive category hierarchy with section total aggregation, category rail, and overview cards (`CategoryOverview`, `CategoryRail`).
- **Topic Feeds & Read States:** Feeds (`TopicFeed`) supporting unread filtering, status badge labels (`TopicStatusLabel`), and automated read receipts (`markForumTopicRead`).
- **Interactive Thread Panel:** Virtualized timeline scrolling (`TimelineScroller`), bidirectional cursor paging, and jump-to-reply deep linking (`?reply=<uuid>`).
- **Rich Text Composer & Quotes:** Multi-target rich text authoring (`Composer`) with quote tooltips (`QuoteTooltip`), draft persistence, and optimistic concurrency control.
- **Member Identity:** Hover member profiles and author badges (`AuthorBadge`, `MemberCardProvider`).

