# rustok-ui / CRATE_API

## Public surface

### Modules
- `classes`: deterministic Tailwind CSS class resolvers for all UI components.
- `contracts`: headless state contracts and data models (`SelectOption`, `TabItem`, `DialogState`, `TabsState`, `TocItem`).
- `tokens`: shared design system tokens (focus rings, disabled styles, transition classes, `radius`, `shadow`).
- `types`: component variant and sizing enums (`ButtonVariant`, `AlertVariant`, `BadgeVariant`, `CardVariant`, `AvatarSize`, `SkeletonVariant`, `Size`, `SwitchSize`, `Orientation`, `InputType`).

### Primary Types & Models
- `ButtonVariant`: button styling variants (`Default`, `Destructive`, `Outline`, `Secondary`, `Ghost`, `Link`).
- `Size`: standard control sizing (`Sm`, `Md`, `Lg`, `Icon`).
- `AlertVariant`: alert styling variants (`Default`, `Destructive`, `Success`, `Warning`, `Info`).
- `BadgeVariant`: badge variants (`Default`, `Secondary`, `Destructive`, `Outline`, `Success`, `Warning`, `Info`).
- `CardVariant`: card styling variants (`Default`, `Muted`, `Outline`).
- `AvatarSize`: avatar sizing (`Sm`, `Md`, `Lg`, `Xl`).
- `SkeletonVariant`: skeleton placeholder variants (`Text`, `Circular`, `Rectangular`).
- `SelectOption`: select dropdown option model (`value`, `label`, `disabled`).
- `DialogState`: headless modal state management (`open`, `close`, `toggle`).
- `TabsState`: headless tab selection state tracking (`select`, `is_active`).
- `TocItem`: table of contents item structure.

### Helper & Resolver Functions
- `merge_classes(parts: &[&str]) -> String`: joins and cleans whitespace-separated CSS classes.
- `button_classes(...)`, `input_classes(...)`, `select_classes(...)`, etc.: deterministic Tailwind resolvers for components.
- `extract_initials(name: &str) -> String`: extracts one or two character initials for avatars.
- `normalize_progress_value(...)`, `progress_value_percentage(...)`: clamped progress calculation.
- `extract_headings_from_html(html: &str) -> Vec<TocItem>`: parses HTML headings for table of contents generation.

## Invariants

- **Deterministic Styling**: Pure functions with zero environment or runtime state dependencies; identical variant/size arguments yield the exact same space-separated CSS class string.
- **Pure FFA Core**: Zero DOM or web framework imports; compatible with SSR, wasm32, desktop, and embedded runtimes.
- **Headless Separation**: Interactive state (e.g. `disabled`, `aria-checked`, `aria-invalid`) is handled by adapters and HTML attributes, keeping core class signatures clean and invariant across state changes.
