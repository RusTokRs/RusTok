# rustok-ui-dioxus / CRATE_API

## Purpose

`rustok-ui-dioxus` is the canonical Dioxus 0.7 reactive component adapter for `rustok-ui`, providing design system component implementations for web, desktop, and SSR applications.

## Architecture & Responsibilities

- **Fluid Frontend Architecture (FFA) Adapter**: Implements Dioxus component wrappers around framework-agnostic Tailwind class resolvers and tokens from `rustok-ui`.
- **WAI-ARIA Accessibility**: Enforces standard semantic HTML and ARIA accessibility attributes across interactive and composite elements.
- **Full Parity with Leptos UI**: Ensures identical visual styling, variant names, size modifiers, and component APIs with `rustok-ui-leptos`.
- **Cross-Platform Portability**: Builds and runs cleanly across Dioxus web, desktop, and SSR environments.

## Public Surface

### Components
- **Actions & Controls**: `Button`, `Checkbox`, `Switch`, `Input`, `Textarea`, `Select`.
- **Feedback & Status**: `Alert`, `Badge`, `Progress`, `Spinner`, `Skeleton`.
- **Layout & Structure**: `Card` (`CardHeader`, `CardTitle`, `CardDescription`, `CardContent`, `CardFooter`, `CardAction`), `Label`, `Separator`, `Tabs` (`TabsList`, `TabsTrigger`, `TabsContent`).
- **Overlays**: `Dialog` (`DialogHeader`, `DialogTitle`, `DialogDescription`, `DialogFooter`).
- **Data Display**: `Avatar`, `TableOfContents`.

### Re-exported Primitives & Tokens
- Re-exports core design tokens, variant enums, and utility functions from `rustok-ui`:
  `ButtonVariant`, `AlertVariant`, `BadgeVariant`, `CardVariant`, `AvatarSize`, `SkeletonVariant`, `Size`, `SwitchSize`, `Orientation`, `InputType`, `SelectOption`, `TabItem`, `DialogState`, `TabsState`, `TocItem`, `extract_initials`, `slugify_heading`, `merge_classes`.

## Invariants & Guarantees

1. **Zero Styling Divergence**: Components rely directly on `rustok-ui` Tailwind resolvers; no custom CSS classes or styles may diverge from the shared design system.
2. **Accessible Interaction**: Interactive elements include appropriate roles (`button`, `dialog`, `tab`, `tabpanel`), state indicators (`aria-checked`, `aria-selected`, `aria-busy`), and keyboard focus rings.
3. **Reactive Signal Compatibility**: Component event handlers accept standard Dioxus callbacks and closure props.

## Dependencies & Boundaries

- **Upstream Dependencies**:
  - `dioxus`: Reactive UI runtime for Dioxus 0.7.
  - `rustok-ui`: Core design tokens, headless states, and CSS resolvers.
  - `serde`: Serialization for component prop models and options.
- **Downstream Consumers**:
  - `apps/dioxus-admin`.
  - Dioxus storefront and administration surfaces.
- **Boundary Rules**:
  - Contains no business logic, domain entities, or direct network transport calls.
  - Domain state binding remains owned by consuming application views.
