# rustok-outbox-admin / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA UI components for the outbox administrative dashboard.
- Integrates with `rustok-ui-core` and `rustok-ui-i18n` for styling, layout, and localization.

## Domain Collaborations
- Collaborates with `rustok-outbox` to display event relay states and counters.
- Collaborates with `apps/admin` as a registered admin module extension.

## Public surface
- `pub mod core`: Pure Rust view-models, counter definitions, and shell text structs (`OutboxAdminBootstrap`, `OutboxCounterSnapshot`, `OutboxAdminShellText`, `OutboxInfoCardViewModel`, `outbox_info_cards`).
- `pub mod ui`: Leptos admin page component (`OutboxAdmin`).
- `pub use ui::leptos::OutboxAdmin`: Top-level re-export for admin routing.

## Minimum Contract Set

### Input DTOs/Commands
- `OutboxAdminBootstrap`: Carries tenant slug, health string, counter snapshots, and relay notes.
- `OutboxCounterSnapshot`: Metric key, localized label, and numeric value.
- `OutboxAdminShellText`: Localized labels for cards and page layout.

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework dependencies.
- View-model transformation in `outbox_info_cards` preserves card ordering (health, tenant, followed by counters).
- Safe fallback to global tenant label when tenant slug is absent.

### Events / Outbox Side Effects
- Read-only UI package; does not dispatch events or write database state directly.

### Errors / Failure Codes
- Load failure states are captured gracefully in UI state signals with `load_error_prefix`.

