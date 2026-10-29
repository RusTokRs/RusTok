# rustok-ui-dioxus / CRATE_API

## Public surface

### Components
- **Actions & Controls**: `Button`, `Checkbox`, `Switch`, `Input`, `Textarea`, `Select`.
- **Feedback & Status**: `Alert`, `Badge`, `Progress`, `Spinner`, `Skeleton`.
- **Layout & Structure**: `Card` (`CardHeader`, `CardTitle`, `CardDescription`, `CardContent`, `CardFooter`, `CardAction`), `Label`, `Separator`, `Tabs` (`TabsList`, `TabsTrigger`, `TabsContent`).
- **Overlays**: `Dialog` (`DialogHeader`, `DialogTitle`, `DialogDescription`, `DialogFooter`).
- **Data Display**: `Avatar`, `TableOfContents`.

### Canonical Re-exports
- Core design tokens, variant enums, and class resolvers re-exported from `rustok-ui`:
  `ButtonVariant`, `AlertVariant`, `BadgeVariant`, `CardVariant`, `AvatarSize`, `SkeletonVariant`, `Size`, `SwitchSize`, `Orientation`, `InputType`, `SelectOption`, `TabItem`, `DialogState`, `TabsState`, `TocItem`.
- `ui_` prefixed aliases for all components (`ui_button`, `ui_alert`, `ui_badge`, `ui_card`, `ui_dialog`, etc.).

## Invariants

- **Component FFA Parity**: Wraps pure styling resolvers from `rustok-ui` with zero internal styling divergence from `rustok-ui-leptos`.
- **WAI-ARIA Accessibility**: Standard ARIA attributes (`role`, `aria-label`, `aria-checked`, `aria-selected`, `aria-orientation`, `aria-valuenow`, `aria-valuemin`, `aria-valuemax`) enforced across interactive elements.
- **SSR & Desktop Portability**: Usable on desktop (Dioxus desktop), SSR, and web targets.
