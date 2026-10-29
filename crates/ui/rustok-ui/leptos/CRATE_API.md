# rustok-ui-leptos / CRATE_API

## Public surface

### Components
- **Actions & Controls**: `Button`, `Checkbox`, `Switch`, `Input`, `Textarea`, `Select`.
- **Feedback & Status**: `Alert`, `Badge`, `Progress`, `Spinner`, `Skeleton`.
- **Layout & Structure**: `Card` (`CardHeader`, `CardTitle`, `CardDescription`, `CardAction`, `CardContent`, `CardFooter`), `Label`, `Separator`, `Tabs` (`TabsList`, `TabsTrigger`, `TabsContent`).
- **Overlays**: `Dialog` (`DialogTrigger`, `DialogContent`, `DialogHeader`, `DialogTitle`, `DialogDescription`, `DialogFooter`, `DialogClose`, `DialogOverlay`, `DialogPortal`).
- **Data Display & Navigation**: `Avatar`, `TableOfContents`.

### Re-exported Core Types & Contracts
- All core types and tokens from `rustok-ui` (`ButtonVariant`, `AlertVariant`, `BadgeVariant`, `CardVariant`, `Size`, `DialogState`, `TabsState`, etc.).
- Prefixed `ui_*` aliases (`ui_button`, `ui_alert`, `ui_badge`, `ui_card`, `ui_dialog`, etc.) for consistent cross-module naming.

## Invariants

- **SSR-First Hydration Parity**: Component rendering and initial DOM markup are identical in server-side rendering (SSR) and client-side hydration.
- **Native Attribute State**: State flags (`disabled`, `aria-invalid`, `aria-checked`) are expressed via native HTML/WAI-ARIA attributes rather than class mutations.
- **Focus & Accessibility**: Interactive controls and modal dialogs follow Radix-grade accessible semantics including focus traps, Escape closing, and proper ARIA relationships.
