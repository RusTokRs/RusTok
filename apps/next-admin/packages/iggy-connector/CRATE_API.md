# apps/next-admin/packages/iggy-connector / CRATE_API

## Purpose

`@rustok/iggy-connector-admin` (`apps/next-admin/packages/iggy-connector`) is the Next.js administration UI package for configuring and monitoring the high-throughput Iggy streaming message broker connector in the Next.js admin dashboard.

## Architecture & Responsibilities

- **Module Registration:** Registers the Iggy connector with the host admin registry via `registerAdminModule` and supplies structured navigation items (`iggyConnectorNavItems`) under the Infrastructure group with `admin` role access guards.
- **Connection Mode Management:** Exposes `IggyConnectorPage` and `IggyConnectorForm` allowing operators to switch between bundled embedded instances and external distributed Iggy broker deployments.
- **Connection Configuration:** Provides inputs for external broker cluster addresses, authentication credentials (via environment or deployment secrets resolvers), and optional TLS parameters (enabled flag and domain name).
- **Restart Signaling:** Informs administrators via `formatSaveOutcome` when changes to the connector mode require a platform server restart to take effect.
- **GraphQL Integration:** Directly dispatches typed queries (`CONFIGURATION_QUERY`) and mutations (`UPDATE_MUTATION`) against the platform Iggy connector schema.

## Public Surface

- `iggyConnectorNavItems`: Navigation configuration for the Next.js admin sidebar under Infrastructure (`access: { role: 'admin' }`).
- `IggyConnectorPage`: Server-rendered async container fetching connector configuration.
- `IggyConnectorForm`: Interactive client form managing mode selection, address parsing, and credential resolvers.
- `CONNECTOR_MODES`, `isKnownMode`, `parseAddresses`, `formatSaveOutcome`: Framework-agnostic validation and formatting utilities in `iggy-connector-model.ts`.
- `CONFIGURATION_QUERY`, `UPDATE_MUTATION`: Exported GraphQL documents.
- `getIggyConnectorConfiguration(opts)`: Fetches current connector status and active/desired modes.
- `saveIggyConnectorConfiguration(input, opts)`: Updates connector configuration and returns restart signaling.
- `IggyConnectorConfiguration`, `IggyConnectorInput`, `GqlOpts`: TypeScript domain types.

## Invariants & Guarantees

1. **GraphQL Contract Hygiene:** All queries and mutations match backend `rustok-iggy-connector` schema definitions with exact field selection.
2. **Bundled Availability Guard:** Bundled mode button is conditionally disabled if `bundledAvailable` is false in the runtime environment.
3. **Admin Guard:** Navigation and operational controls strictly require the `admin` role (`access: { role: 'admin' }`).
4. **Zero Domain Logic in UI:** Address parsing and mode validation live in pure TypeScript model functions covered by unit tests.
5. **Parity with Leptos Admin:** Mode toggles and connection parameter configurations maintain functional parity with `crates/modules/rustok-iggy-connector/admin`.

## Dependencies & Boundaries

- **Host Interface:** Depends on React 19, `@/modules/registry`, and `@/shared/ui/shadcn` design system components from `apps/next-admin`.
- **Backend API:** Communicates exclusively with `apps/server` via GraphQL endpoint (`/api/graphql`).
- **No Direct Socket Access:** Never communicates over raw TCP/QUIC Iggy wire protocols directly; all interactions route through the RusToK GraphQL gateway.
