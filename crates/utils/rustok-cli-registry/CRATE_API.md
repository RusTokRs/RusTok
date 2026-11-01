# rustok-cli-registry — Crate API

Selected-distribution command registry for the RusToK platform CLI.

## Public surface

- `SelectedDistributionRegistry` — aggregate of the command providers selected for the active distribution.
- `SelectedDistributionRegistry::empty()` / `from_providers(providers)` — constructors.
- `SelectedDistributionRegistry::providers()` / `commands()` / `is_empty()` — read access.
- `selected_distribution_registry(runtime)` — builds the registry from the runtime composition.

## Invariants

- The registry aggregates providers only; it never executes commands or owns provider lifecycle.
- Command ordering and deduplication follow the selected distribution's provider order.
- The registry remains crate-agnostic: it depends on `rustok-cli-core` contracts, not on concrete providers.
