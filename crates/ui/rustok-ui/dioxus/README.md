# rustok-ui-dioxus

Dioxus 0.6 design system component adapter for the RusToK platform.

## Overview

`rustok-ui-dioxus` provides Dioxus 0.6 implementations of the RusToK UI component suite. It wraps the framework-agnostic `rustok-ui` core crate, ensuring identical styling tokens, WAI-ARIA compliance, and semantic parity across web and desktop hosts.

## Components

- **Actions & Controls**: `Button`, `Checkbox`, `Switch`, `Input`, `Textarea`, `Select`.
- **Feedback & Status**: `Alert`, `Badge`, `Progress`, `Spinner`, `Skeleton`.
- **Layout & Structure**: `Card`, `Label`, `Separator`, `Tabs`.
- **Overlays**: `Dialog` (Modal).
- **Data Display**: `Avatar`.

## Behaviour notes

- The framework-neutral class resolvers and core contracts are shared with
  `rustok-ui-leptos`; component APIs are renderer-specific where required, and
  dialog interaction parity is not yet complete in this adapter.
- `Checkbox` has no `indeterminate` prop: HTML defines no `indeterminate`
  attribute (it is a DOM property) and `dioxus-html` 0.6 declares no such
  constant, so a tri-state checkbox has to be driven from host JavaScript.
- `Dialog` closes through the backdrop; provide an accessible name with
  `aria_label` or `aria_labelledby` and give `DialogTitle` the matching `id`.
  Keyboard `Escape`/focus management is host-provided in this adapter.
- `Spinner` exposes an optional `aria_label` (default `Loading`) for localized
  accessible names, and the `value` prop of `Select` is rendered on the
  `<select>` element in addition to per-option selection state.
- `TabsList` and `Separator` announce their `aria-orientation`.
- `Progress` defaults `value` to zero and `max` to `100`, supports orientation,
  `aria_labelledby`, and `aria_value_text`, clamps values to `0..=max`, and uses
  the normalized value for `aria-valuenow` and the corresponding bar percentage.

## Usage

```rust
use dioxus::prelude::*;
use rustok_ui_dioxus::{Alert, AlertVariant, Button, ButtonVariant, Progress, Size};

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
        Progress {
            value: 62.5,
            aria_label: Some("Upload progress".to_string()),
        }
    }
}
```
