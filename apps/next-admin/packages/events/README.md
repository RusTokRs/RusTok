# @rustok/events-admin

Next.js admin UI package for platform event stream monitoring, outbox inspection, and profile management.

## Responsibilities

- Exposes the event infrastructure dashboard and delivery profile configuration workspace for `apps/next-admin`.
- Communicates directly with the RusToK GraphQL API via typed queries (`eventsStatus`, `eventDeliveryConfiguration`) and mutations (`updateEventDeliveryConfiguration`).
- Provides diagnostic metrics for pending outbox events, dead-letter queue (DLQ) backlog, and available stream transports.
- Enforces Iggy streaming broker configuration prerequisites before allowing activation of `outbox_iggy` transport profile.
- Registers navigation items under Infrastructure with admin role access guards.
