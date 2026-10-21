# `rustok-messenger-storefront`

Storefront and workplace collaboration UI package for the `rustok-messenger` module, implemented according to the Fluid Frontend Architecture (FFA).

## Capabilities

1. **Dedicated Full-Page Workplace Messenger:**
   - Responsive 3-column desktop and tablet workstation layout (Slack/Telegram/Discord style).
   - Sidebar with folder filters (`All`, `Channels`, `Direct`, `EntityThreads`), live search, and unread badges.
   - Central chat pane with virtualized message history, delivery ticks, typing indicators, and markdown composer.
   - Collapsible context drawer with channel members, shared media/files, and sub-threads.

2. **Contextual Floating Chat Widget:**
   - Dockable bottom-right overlay for e-commerce storefront pages (PDP, order tracking, seller inquiries).
   - Automatically binds to Universal Entity Reference (UER: `entity_type` + `entity_id`).

3. **Strict FFA Decomposition:**
   - `core/`: Zero UI runtime imports. Pure Rust domain view-models, input validation, message queues, optimistic state machines, and date grouping verified with unit tests.
   - `transport/`: Dual-path facade wrapping REST, WebSockets (`/api/messenger/*`), and GraphQL.
   - `ui/`: Leptos rendering components.
   - `locales/`: Project Fluent `.ftl` catalogs (`en.ftl`, `ru.ftl`).
