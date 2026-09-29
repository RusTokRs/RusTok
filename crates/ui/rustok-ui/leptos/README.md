# rustok-ui-leptos

SSR-first Leptos 0.8 design system component adapter for the RusToK platform.

## Overview

`rustok-ui-leptos` provides Leptos 0.8 implementations of the RusToK UI component suite. It wraps the framework-agnostic `rustok-ui` core crate, ensuring identical styling tokens, WAI-ARIA compliance, and semantic parity across frameworks.

## Components

- **Actions & Controls**: `Button`, `Checkbox`, `Switch`, `Input`, `Textarea`, `Select`.
- **Feedback & Status**: `Alert`, `Badge`, `Spinner`, `Skeleton`.
- **Layout & Structure**: `Card`, `Label`, `Separator`, `Tabs`.
- **Overlays**: `Dialog` (Modal).
- **Data Display**: `Avatar`.

## Behaviour notes

- `Button` accepts `on_click`, `disabled`, `loading` (shows a `Spinner`), and a
  native `r#type` (default `button`). `disabled`/`loading` are rendered through
  the native attribute, not through extra classes.
- `Select` is controlled through `value`/`set_value`; the component binds the
  `value` property and emits `on:change` with `event_target_value`.
- `Dialog` renders the backdrop without `aria-hidden` (it is clickable to
  dismiss), focuses the dialog on open, and closes on `Escape`.
- `Spinner` exposes `aria_label` (default `Loading`) so hosts can pass localized
  text; `TabsList` and `Separator` announce their `aria-orientation`.
- `Input`, `Textarea`, `Select`, `Checkbox`, and `Switch` accept an optional
  `class` string that is appended to the resolved class list.

## Usage

```rust
use leptos::prelude::*;
use rustok_ui_leptos::{Button, ButtonVariant, Size, Alert, AlertVariant};

#[component]
pub fn MyView() -> impl IntoView {
    view! {
        <Alert variant=AlertVariant::Info>
            "Configuration saved."
        </Alert>
        <Button variant=ButtonVariant::Default size=Size::Md>
            "Submit"
        </Button>
    }
}
```
