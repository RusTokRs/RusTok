# leptos-ui

DSD-style UI components for Leptos applications (shadcn approach).

## Features

- **Copy-paste friendly** — не требует npm install
- **Variants-based API** — размеры, цвета через enum
- **Tailwind-first** — использует Tailwind CSS
- **Accessibility** — ARIA attributes, keyboard navigation
- **Type-safe** — Rust типы для всех props

## Installation

```toml
[dependencies]
leptos-ui = { path = "../../crates/leptos-ui" }
```

## Components (Phase 1)

### Button

```rust
use leptos::*;
use leptos_ui::{Button, ButtonVariant, ButtonSize};

#[component]
fn App() -> impl IntoView {
    view! {
        <Button variant=ButtonVariant::Primary size=ButtonSize::Md>
            "Click me"
        </Button>
        
        <Button variant=ButtonVariant::Outline loading=true>
            "Loading..."
        </Button>
    }
}
```

**Props:**
- `variant`: Primary | Secondary | Outline | Ghost | Destructive
- `size`: Sm | Md | Lg
- `disabled`: bool
- `loading`: bool
- `class`: Option<&'static str> — дополнительные CSS классы

### Input

```rust
use leptos_ui::Input;

view! {
    <Input 
        type="email"
        placeholder="you@example.com"
        value=email
        on_input=move |ev| set_email(event_target_value(&ev))
        error=Some("Invalid email")
    />
}
```

**Props:**
- `type`: text | email | password | number
- `placeholder`: &'static str
- `value`: Signal<String>
- `on_input`: Callback
- `error`: Option<&'static str>
- `disabled`: bool

### Card

```rust
use leptos_ui::{Card, CardHeader, CardContent, CardFooter};

view! {
    <Card>
        <CardHeader>
            <h2 class="text-2xl font-bold">"Title"</h2>
        </CardHeader>
        <CardContent>
            <p>"Content here"</p>
        </CardContent>
        <CardFooter>
            <Button>"Action"</Button>
        </CardFooter>
    </Card>
}
```

### Label

```rust
use leptos_ui::Label;

view! {
    <Label required=true>"Email"</Label>
    <Input type="email" />
}
```

### Alert

```rust
use leptos_ui::{Alert, AlertVariant};

view! {
    <Alert variant=AlertVariant::Warning>
        "The Iggy module is not enabled. Enable it on the Modules page."
    </Alert>

    <Alert variant=AlertVariant::Destructive title="Save failed".to_string()>
        "Could not reach the server."
    </Alert>

    <Alert variant=AlertVariant::Info>
        "Restart required to apply transport changes."
    </Alert>
}
```

**Props:**
- `variant`: `Default | Info | Warning | Destructive | Success`
- `title`: `Option<String>` — необязательный жирный заголовок
- `class`: `Option<String>` — дополнительные CSS классы

### Badge

```rust
use leptos_ui::{Badge, BadgeVariant};

view! {
    <Badge variant=BadgeVariant::Success>"Active"</Badge>
    <Badge variant=BadgeVariant::Warning>"Pending"</Badge>
}
```

**Variants:** Default | Primary | Success | Warning | Danger

### Separator

```rust
use leptos_ui::Separator;

view! {
    <div class="space-y-4">
        <p>"Section 1"</p>
        <Separator />
        <p>"Section 2"</p>
    </div>
}
```

## Design Principles

1. **DSD approach** (Domain-Specific Design)
   - Copy-paste friendly (не через npm)
   - Variants over composition
   - Tailwind-first

2. **Accessibility**
   - ARIA attributes
   - Keyboard navigation
   - Focus management

3. **Consistency**
   - Единый API для всех компонентов
   - Shared types (Size, Variant)

## Roadmap

### Phase 1 (Current)
- [x] Button
- [x] Input
- [x] Label
- [x] Card (Card, CardHeader, CardContent, CardFooter)
- [x] Badge
- [x] Separator
- [x] Alert

### Phase 2
- [ ] Table primitives
- [ ] Dropdown menu
- [ ] Dialog (Modal)
- [ ] Tabs
- [ ] Checkbox
- [ ] Textarea
- [ ] Select/Combobox

### Phase 3
- [ ] Command palette
- [ ] Toast notifications
- [ ] Progress bar
- [ ] Skeleton loader
- [ ] Avatar
- [ ] Tooltip

## License

MIT OR Apache-2.0
