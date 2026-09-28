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
