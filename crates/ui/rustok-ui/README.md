# rustok-ui

Framework-agnostic design system primitives, variants, contracts, and styling resolvers for the RusToK platform.

## Overview

`rustok-ui` is a pure FFA (Framework-Free Architecture) foundational crate with zero DOM, zero web framework, and zero runtime dependencies beyond standard serialization. It serves as the single source of truth for:

- Component variants, sizes, and states (`ButtonVariant`, `AlertVariant`, `BadgeVariant`, `CardVariant`, `AvatarSize`, `SkeletonVariant`, `Size`, `SwitchSize`, `Orientation`, `InputType`).
- Deterministic CSS class generation for Tailwind CSS / shadcn styling.
- Headless state models and contracts (`SelectOption`, `TabItem`, `DialogState`, `TabsState`, `extract_initials`).
- Design system tokens (focus rings, disabled states, elevation, border radiuses, transitions) in `tokens`.

All class resolvers are pure functions: the same input always produces the same
space-separated Tailwind class string, with no environment, DOM, or theme lookup
involved. Caller-supplied `class` values are appended for layout adjustments;
they cannot override an already-resolved utility, because Tailwind resolves
conflicting utilities by stylesheet order rather than by attribute order.

## Architecture

This crate is wrapped by framework adapters located in subdirectories:

- `leptos/` (`rustok-ui-leptos`): SSR-first Leptos 0.8 component adapter.
- `dioxus/` (`rustok-ui-dioxus`): Dioxus 0.6 component adapter for web and desktop hosts.

## Usage

```rust
use rustok_ui::{ButtonVariant, Size, button_classes, merge_classes};

let classes = button_classes(ButtonVariant::Default, Size::Md, None);
assert!(classes.contains("bg-primary"));

let combined = merge_classes(&["flex", "", " gap-2 "]);
assert_eq!(combined, "flex gap-2");
```

## Component state

Interactive state is carried by the rendered element, not by the resolver
signature: adapters emit the native `disabled` attribute (styled by the
`disabled:` utilities inside every resolver), `aria-invalid` for validation
errors, and `aria-checked` for switches/tabs. That keeps a single class list per
variant/size combination and avoids class churn on state transitions.

## Guarantees

- Class lists never contain leading, trailing, or doubled spaces.
- `extract_initials` returns at most two characters and falls back to `?`.
- `ButtonVariant`, `BadgeVariant`, `CardVariant`, `Size`, `Orientation`,
  `InputType`, and `SwitchSize` are `Serialize`/`Deserialize` including their
  display strings, so hosts can persist variant choices.
