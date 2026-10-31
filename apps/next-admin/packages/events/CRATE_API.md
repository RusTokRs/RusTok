# apps/next-admin/packages/events / CRATE_API

## Purpose

`@rustok/events-admin` (`apps/next-admin/packages/events`) is the Next.js administration UI package for the `rustok-events` platform subsystem. It provides operational visibility into event delivery queues (transactional outbox and DLQ) and dynamic transport profile selection between standard outbox and Iggy streaming broker.

## Architecture & Responsibilities

- **Module Registration:** Registers the events module with the host admin registry via `registerAdminModule` and supplies structured navigation items (`eventsNavItems`) under the Infrastructure section with `admin` role access guards.
- **Operational Workspace:** Exposes `EventsPage` and `EventsForm` for monitoring configured profile, pending outbox count, dead-letter count, and selecting delivery profiles (`outbox`, `outbox_iggy`).
- **Prerequisite Validation:** Evaluates system readiness (`evaluateProfileSelection` in `events-model.ts`) when selecting `outbox_iggy`, presenting an alert modal directing administrators to configure the Iggy connector if unconfigured.
- **Restart Signaling:** Detects profile drift (`isRestartRequired`) and informs operators when a server restart is required to apply changes (`formatSaveOutcomeKey`).
- **GraphQL Integration:** Directly dispatches typed queries (`EVENTS_STATUS_QUERY`, `EVENT_DELIVERY_CONFIGURATION_QUERY`) and mutations (`UPDATE_EVENT_DELIVERY_CONFIGURATION_MUTATION`).

## Public Surface

- `eventsNavItems`: Navigation configuration for the Next.js admin sidebar under Infrastructure (`access: { role: 'admin' }`).
- `EventsPage`: Server-rendered async container fetching status and configuration snapshots.
- `EventsForm`: Interactive client component managing profile selection, modal confirmations, and status presentation.
- `isRestartRequired`, `evaluateProfileSelection`, `formatSaveOutcomeKey`: Pure business logic functions for evaluating profile transitions and restart requirements.
- `EVENTS_STATUS_QUERY`, `EVENT_DELIVERY_CONFIGURATION_QUERY`, `UPDATE_EVENT_DELIVERY_CONFIGURATION_MUTATION`: Exported GraphQL documents.
- `getEventsStatus(opts)`: Fetches outbox queue depths and transport availability.
- `getEventDeliveryConfiguration(opts)`: Fetches active and desired event delivery profiles.
- `saveEventDeliveryProfile(profile, opts)`: Dispatches desired profile update and returns restart requirements.
- `EventsStatus`, `EventDeliveryConfiguration`, `EventDeliveryUpdate`, `GqlOpts`: TypeScript domain types.

## Invariants & Guarantees

1. **GraphQL Contract Hygiene:** All queries and mutations match backend `rustok-events` schema definitions with exact field selection.
2. **Iggy Connector Prerequisite:** `outbox_iggy` cannot be selected if `iggyConfigured` is false; an interactive modal intercepts the selection and guides the operator.
3. **Admin Guard:** Navigation and operational controls strictly require the `admin` role (`access: { role: 'admin' }`).
4. **Zero Domain Logic in UI:** State transition logic and prerequisite checks live in framework-agnostic `events-model.ts` covered by unit tests.
5. **Parity with Leptos Admin:** Status indicators and profile switching maintain functional parity with `crates/modules/rustok-events/admin`.

## Dependencies & Boundaries

- **Host Interface:** Depends on React 19, `@rustok/next-fluent`, `@/modules/registry`, and `@/shared/ui/shadcn` design system components from `apps/next-admin`.
- **Backend API:** Communicates exclusively with `apps/server` via GraphQL endpoint (`/api/graphql`).
- **No Direct Queue Access:** Never accesses PostgreSQL outbox tables or Iggy streaming brokers directly.
