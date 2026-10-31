# rustok-ui / CRATE_API

## Purpose
`rustok-ui` is RusToK's framework-agnostic design system core under Fluid Frontend Architecture (FFA). It defines component visual variants, sizing scales, headless interactive contracts, design tokens, and deterministic Tailwind CSS class resolvers used across Leptos (`rustok-ui-leptos`), Dioxus (`rustok-ui-dioxus`), and server-side renderers.

## Architecture & Responsibilities
- **Deterministic Tailwind Class Resolvers (`classes.rs`)**: Pure functions taking component variant, size, and optional caller classes to produce normalized, deterministic space-separated class lists without DOM access or runtime theming.
- **Visual Vocabulary (`types.rs`)**: Component enums defining visual styling contracts independent of any web framework (`ButtonVariant`, `AlertVariant`, `BadgeVariant`, `CardVariant`, `Size`, `AvatarSize`, `SkeletonVariant`, `SwitchSize`, `Orientation`, `InputType`).
- **Headless State Contracts (`contracts.rs`)**: Framework-free interactive state containers (`DialogState`, `TabsState`, `SelectOption`, `TabItem`, `TocItem`) and helpers (`extract_initials`, `slugify_heading`, `extract_headings_from_html`, `progress_value_percentage`).
- **Design Tokens (`tokens.rs`)**: Centralized design system constants (`FOCUS_RING_CLASSES`, `INPUT_FOCUS_RING_CLASSES`, `DISABLED_CONTROL_CLASSES`, `DISABLED_INPUT_CLASSES`, `TRANSITION_COLORS_CLASSES`, `radius`, `shadow`).

## Public Surface
### Primary Types & Models
- `ButtonVariant`: Button styling variants (`Default`, `Destructive`, `Outline`, `Secondary`, `Ghost`, `Link`).
- `Size`: Standard control sizing (`Xs`, `Sm`, `Md`, `Lg`, `Xl`, `Icon`).
- `AlertVariant`: Alert banner variants (`Default`, `Destructive`, `Success`, `Warning`, `Info`).
- `BadgeVariant`: Badge variants (`Default`, `Secondary`, `Destructive`, `Outline`, `Success`, `Warning`, `Info`).
- `CardVariant`: Card styling variants (`Default`, `Muted`, `Outline`).
- `AvatarSize`: Avatar sizing (`Sm`, `Md`, `Lg`, `Xl`).
- `SkeletonVariant`: Skeleton placeholder variants (`Text`, `Circular`, `Rectangular`).
- `SelectOption`: Select dropdown option model (`value`, `label`, `disabled`).
- `DialogState`: Headless modal state management (`open`, `close`, `toggle`).
- `TabsState`: Headless tab selection state tracking (`select`, `is_active`).
- `TocItem`: Table of contents heading representation (`id`, `text`, `level`).

### Class Resolvers
- `merge_classes(parts: &[&str]) -> String`: Joins and cleans whitespace-separated CSS classes.
- `button_classes(...)`, `badge_classes(...)`, `alert_classes(...)`, `input_classes(...)`, `select_classes(...)`, `checkbox_classes(...)`, `switch_classes(...)`, `card_classes(...)`, `dialog_content_classes(...)`, `tabs_trigger_classes(...)`, `progress_classes(...)`, `skeleton_classes(...)`, `spinner_classes(...)`, `textarea_classes(...)`, `label_classes(...)`, `separator_classes(...)`.

### State & Parsing Helpers
- `extract_initials(name: &str) -> String`: Extracts one or two character initials for avatars.
- `slugify_heading(text: &str, index: usize) -> String`: Deterministically generates an HTML anchor id from heading text.
- `extract_headings_from_html(html: &str) -> Vec<TocItem>`: Parses heading elements for table of contents navigation.
- `normalize_progress_value(...)`, `progress_value_percentage(...)`: Clamped progress calculation.

## Invariants & Guarantees
- **Deterministic Styling**: Pure functions with zero environment or runtime state dependencies; identical variant/size arguments yield the exact same space-separated CSS class string.
- **Pure FFA Core**: Zero DOM or web framework imports; compatible with SSR, wasm32, desktop, and embedded runtimes.
- **Headless Separation**: Interactive state (e.g. `disabled`, `aria-checked`, `aria-invalid`) is handled by adapters and HTML attributes, keeping core class signatures clean and invariant across state changes.
- **Unicode & Initial Handling**: `extract_initials` handles Unicode, accents, non-ASCII scripts, and compound whitespace gracefully.

## Dependencies & Boundaries
- **Direct Dependencies**:
  - `serde`: Serialization traits for state models and options.
- **Zero Framework Coupling**: Prohibited from importing Leptos, Dioxus, or Next.js types; acts as the universal visual primitive provider for all UI adapters.
