# rustok-ui-dioxus

Dioxus 0.6 design system component adapter for the RusToK platform.

## Overview

`rustok-ui-dioxus` provides Dioxus 0.6 implementations of the RusToK UI component suite. It wraps the framework-agnostic `rustok-ui` core crate, ensuring identical styling tokens, WAI-ARIA compliance, and semantic parity across web and desktop hosts.

## Components

- **Actions & Controls**: `Button`, `Checkbox`, `Switch`, `Input`, `Textarea`, `Select`.
- **Feedback & Status**: `Alert`, `Badge`, `Spinner`, `Skeleton`.
- **Layout & Structure**: `Card`, `Label`, `Separator`, `Tabs`.
- **Overlays**: `Dialog` (Modal).
- **Data Display**: `Avatar`.

## Behaviour notes

- The component surface, class resolvers, and `ui_*` aliases mirror
  `rustok-ui-leptos` so a host can switch renderers without changing markup
  decisions.
- `Checkbox` has no `indeterminate` prop: HTML defines no `indeterminate`
  attribute (it is a DOM property) and `dioxus-html` 0.6 declares no such
  constant, so a tri-state checkbox has to be driven from host JavaScript.
- `Dialog` closes through the backdrop; keyboard `Escape`/focus management is
  host-provided in this adapter.
- `Spinner` exposes an optional `aria_label` (default `Loading`) for localized
  accessible names, and the `value` prop of `Select` is rendered on the
  `<select>` element in addition to per-option selection state.
- `TabsList` and `Separator` announce their `aria-orientation`.

## Usage

```rust
use dioxus::prelude::*;
use rustok_ui_dioxus::{Button, ButtonVariant, Size, Alert, AlertVariant};

#[component]
pub fn MyDioxusView() -> Element {
    rsx! {
        Alert { variant: AlertVariant::Info,
            "Configuration saved."
        }
        Button {
            variant: ButtonVariant::Default,
            size: Size::Md,
            "Submit"
        }
    }
}
```
