# Shared React form controls

`@rustok/ui-controls` owns portable presentational controls for the Next hosts. It does not
resolve auth, tenant, locale, routes, validation, SKU availability or prices.

- `Input` is the existing Next Admin native Input extracted unchanged into a shared package.
  Admin re-exports it from its existing FSD path; Product storefront consumes it directly.
- `NativeSelect` retains native keyboard/label/disabled-option semantics, including an explicit
  selectable empty value. It is the React counterpart of the native Leptos Select, not a second
  implementation of Admin's different Radix popup contract.
- `Label` accepts native label props, including `htmlFor` and implicit label composition.

These files intentionally have no `@/` host aliases. Add this package to the host's
`transpilePackages` and Tailwind sources. Tokens remain the shared shadcn-compatible theme;
Rust adapter class contracts remain in `crates/ui/rustok-ui/src/classes.rs`.

Mounted keyboard, focus, controlled-value and screen-reader verification is still required;
source sharing and typechecking alone do not certify functional parity.
