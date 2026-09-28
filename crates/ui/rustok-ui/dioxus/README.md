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
