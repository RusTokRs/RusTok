# rustok-ui

Framework-agnostic design system primitives, variants, contracts, and styling resolvers for the RusToK platform.

## Overview

`rustok-ui` is a pure FFA (Framework-Free Architecture) foundational crate with zero DOM, zero web framework, and zero runtime dependencies beyond standard serialization. It serves as the single source of truth for:

- Component variants, sizes, and states (`ButtonVariant`, `AlertVariant`, `BadgeVariant`, `CardVariant`, `AvatarSize`, `SkeletonVariant`, `Size`, `SwitchSize`, `Orientation`, `InputType`).
- Deterministic CSS class generation for Tailwind CSS / shadcn styling.
- Headless state models and contracts (`SelectOption`, `TabItem`, `DialogState`, `TabsState`, `extract_initials`).
- Design system tokens (colors, focus rings, elevation, border radiuses, animation transitions).

## Architecture

This crate is wrapped by framework adapters located in subdirectories:

- `leptos/` (`rustok-ui-leptos`): SSR-first Leptos 0.8 component adapter.
- `dioxus/` (`rustok-ui-dioxus`): Dioxus 0.6 component adapter for web and desktop hosts.

## Usage

```rust
use rustok_ui::{ButtonVariant, Size, button_classes};

let classes = button_classes(ButtonVariant::Default, Size::Md, false, false, None);
assert!(classes.contains("bg-primary"));
```
