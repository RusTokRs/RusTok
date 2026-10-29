# leptos-ui-routing / CRATE_API

## Public surface

### Primary Structs & Policy Types
- `RouteQueryPolicy`: context wrapper for route query sanitization callbacks.
- `RouteQueryWriter`: reactive writer for applying query parameter mutations and replacements.
- `RouteLocale`: reactive wrapper around optional locale signals.

### Reactive Signal Hooks & Helper Functions
- `read_route_query_value(route_context: &UiRouteContext, key: &str) -> Option<String>`: reads a single query value from the route context.
- `use_route_locale() -> Signal<Option<String>>`: retrieves the current route's reactive locale signal.
- `use_route_query_value(key: &'static str) -> Signal<Option<String>>`: retrieves a reactive query parameter value supporting both SSR and client-side hydration.
- `use_route_query_writer() -> RouteQueryWriter`: hook providing write access to current URL query parameters.

## Invariants

- **Generic Routing Plumbing**: No module-specific or domain-specific query keys are defined; parameter policies are injected by host applications.
- **SSR & Hydration Parity**: Route query reads return consistent values across SSR context resolution and browser router state.
- **Zero Domain Logic**: The crate handles only Leptos routing mechanics, leaving validation and business state machines in framework-agnostic core libraries.
