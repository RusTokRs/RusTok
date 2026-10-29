# rustok-iggy-connector-admin / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA UI components for Iggy connector management and runtime mode selection.
- Integrates with `rustok-ui-core`, `rustok-ui-transport`, and `rustok-ui-i18n`.

## Domain Collaborations
- Collaborates with `rustok-iggy-connector` for bundled server supervision and external endpoint settings.
- Collaborates with `apps/admin` as a registered admin module package.

## Public surface
- `pub mod i18n`: Project Fluent localization helpers.
- `pub use ui::IggyConnectorAdmin`: Root admin view component for Iggy connector configuration.

## Minimum Contract Set

### Input DTOs/Commands
- Connector configuration form inputs (bundled vs external, ports, endpoints, credentials).
- Connector status snapshots.

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework types.
- Validates external endpoint URLs and port numbers.
- Bundled server controls respect platform OS compatibility guards.

### Events / Outbox Side Effects
- Connector setting mutations trigger re-evaluation of event streaming transports via `rustok-iggy-connector`.

### Errors / Failure Codes
- Network, validation, and connection errors are surfaced through typed UI notices.

