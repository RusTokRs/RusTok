# leptos-ui-routing / CRATE_API

## Purpose
`leptos-ui-routing` provides Leptos-specific reactive URL and query parameter routing helpers, route query sanitization policies, and query writer abstractions for both host applications (`apps/admin`, `apps/storefront`) and module-owned UI surfaces.

## Architecture & Responsibilities
- **Reactive URL & Query State**: Bridges Leptos reactive signals with browser location/query maps and SSR route context representations.
- **Route Query Policy**: Wraps sanitization delegates (`RouteQueryPolicy`) to validate, filter, or transform URL query parameters per route segment and subpath without coupling to individual domain models.
- **Query Mutation Dispatcher**: Exposes `RouteQueryWriter` to perform atomic query updates, push/replace values, clear parameters, or apply FFA `UiRouteQueryIntent` and `UiRouteQueryWrite` objects.
- **Locale Routing Context**: Provides `RouteLocale` and `use_route_locale()` hooks for resolving the active language/locale tag in the reactive UI hierarchy.

## Public Surface
### Primary Structs & Policy Types
- `RouteQueryPolicy`: Context wrapper for route query sanitization callbacks.
  - `RouteQueryPolicy::new(sanitize: F) -> Self`
  - `RouteQueryPolicy::sanitize(&self, route_segment: Option<&str>, subpath: Option<&str>, query: &BTreeMap<String, String>) -> BTreeMap<String, String>`
- `RouteQueryWriter`: Reactive writer for applying query parameter mutations and history replacements.
  - `update(&self, updates: Vec<(String, Option<String>)>, replace: bool)`
  - `push_value(&self, key: impl Into<String>, value: impl Into<String>)`
  - `replace_value(&self, key: impl Into<String>, value: impl Into<String>)`
  - `clear_key(&self, key: impl AsRef<str>)`
  - `replace_query_update(&self, key: impl Into<String>, update: UiRouteQueryUpdate)`
  - `apply_query_write(&self, write: UiRouteQueryWrite)`
  - `apply_query_intent(&self, intent: UiRouteQueryIntent)`
- `RouteLocale`: Reactive wrapper struct `RouteLocale(pub Signal<Option<String>>)`.

### Reactive Signal Hooks & Helper Functions
- `read_route_query_value(route_context: &UiRouteContext, key: &str) -> Option<String>`: Reads a single query value from the route context.
- `use_route_locale() -> Signal<Option<String>>`: Retrieves the current route's reactive locale signal.
- `use_route_query_value(key: &'static str) -> Signal<Option<String>>`: Retrieves a reactive query parameter value supporting both SSR and client-side hydration.
- `use_route_query_writer() -> RouteQueryWriter`: Hook providing write access to current URL query parameters.

## Invariants & Guarantees
- **Generic Routing Plumbing**: No module-specific or domain-specific query keys are defined; parameter policies are injected by host applications.
- **SSR & Hydration Parity**: Route query reads return consistent values across SSR context resolution and browser router state.
- **Zero Domain Logic**: The crate handles only Leptos routing mechanics, leaving validation and business state machines in framework-agnostic core libraries (`rustok-ui-core`).

## Dependencies & Boundaries
- **Direct Dependencies**:
  - `rustok-ui-core`: Framework-agnostic route context and query intent primitives (`UiRouteContext`, `UiRouteQueryIntent`, `UiRouteQueryUpdate`, `UiRouteQueryWrite`).
  - `leptos`: Reactive primitives, component hooks, signal derivation, and hydration.
  - `leptos_router`: Client-side routing, navigation hooks, and location abstractions.
  - `serde_urlencoded`: URL query parameter serialization.
- **No Downstream Domain Dependencies**: Prohibited from importing domain modules (`rustok-commerce`, `rustok-content`, etc.) or host-specific state.
