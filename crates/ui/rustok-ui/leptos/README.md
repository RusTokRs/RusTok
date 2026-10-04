# rustok-ui-leptos

SSR-first Leptos 0.8 design system component adapter for the RusToK platform.

## Overview

`rustok-ui-leptos` provides Leptos 0.8 implementations of the RusToK UI component suite. It wraps the framework-agnostic `rustok-ui` core crate, ensuring identical styling tokens, WAI-ARIA compliance, and semantic parity across frameworks.

## Components

- **Actions & Controls**: `Button`, `Checkbox`, `Switch`, `Input`, `Textarea`, `Select`.
- **Feedback & Status**: `Alert`, `Badge`, `Progress`, `Spinner`, `Skeleton`.
- **Layout & Structure**: `Card`, `Label`, `Separator`, `Tabs`.
- **Overlays**: `Dialog` (Modal).
- **Data Display**: `Avatar`.

## Behaviour notes

- `Button` accepts `on_click`, `disabled`, `loading` (shows a `Spinner`), and a
  native `r#type` (default `button`). `disabled`/`loading` are rendered through
  the native attribute, not through extra classes.
- `Select` is controlled through `value`/`set_value`; the component binds the
  `value` property and emits `on:change` with `event_target_value`.
- `Dialog` is a compound primitive: `Dialog` owns controlled (`open` plus
  `on_open_change`) or uncontrolled (`default_open`) state, `DialogTrigger`
  toggles it, and `DialogContent` portals its panel and overlay into the body.
  `DialogClose` and the overlay close it; Escape closes it; modal content locks
  body scrolling, focuses on open, and traps `Tab`/`Shift+Tab`. Connect an
  explicit `DialogContent id` to a trigger with `aria_controls`. Focus returns to
  the last mounted trigger; programmatic opens without a trigger remain
  host-owned. Provide a name with `aria_label` or `aria_labelledby` plus a matching
  `DialogTitle` `id`; `aria_describedby` can reference a `DialogDescription` id.
  The adapter is still being brought to full Radix parity; known gaps include
  `asChild`, background isolation, complete outside-interaction behavior,
  generated title/description IDs, and exit-animation lifecycle.
- `Progress` accepts an optional reactive `value` (defaults to zero), reactive
  `max` (defaults to `100`), `orientation`, `aria_label` or `aria_labelledby`,
  and `aria_value_text`. Values are clamped to `0..=max`; non-finite values map
  to zero, and the same normalized value drives the SVG and `aria-valuenow`.
- `Spinner` exposes `aria_label` (default `Loading`) so hosts can pass localized
  text; `TabsList` and `Separator` announce their `aria-orientation`.
- `Input`, `Textarea`, `Select`, `Checkbox`, `Switch`, and `Progress` accept an
  optional `class` string that is appended to the resolved class list.

## Usage

```rust
use leptos::prelude::*;
use rustok_ui_leptos::{Alert, AlertVariant, Button, ButtonVariant, Progress, Size};

#[component]
pub fn MyView() -> impl IntoView {
    let upload_progress = Signal::derive(|| 62.5);
    view! {
        <Alert variant=AlertVariant::Info>
            "Configuration saved."
        </Alert>
        <Button variant=ButtonVariant::Default size=Size::Md>
            "Submit"
        </Button>
        <Progress value=upload_progress aria_label="Upload progress" />
    }
}
```

## Dialog composition

```rust
use leptos::prelude::*;
use rustok_ui_leptos::{
    Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger,
};

#[component]
pub fn ConfirmDialog() -> impl IntoView {
    view! {
        <Dialog>
            <DialogTrigger aria_controls="confirm-dialog">"Open confirmation"</DialogTrigger>
            <DialogContent
                id="confirm-dialog"
                aria_labelledby="confirm-title"
                aria_describedby="confirm-description"
            >
                <DialogHeader>
                    <DialogTitle id="confirm-title">"Confirm action"</DialogTitle>
                    <DialogDescription id="confirm-description">
                        "This action cannot be undone."
                    </DialogDescription>
                </DialogHeader>
            </DialogContent>
        </Dialog>
    }
}
```
