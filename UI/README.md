# IU

Internal UI  workspace for shared design tokens and parallel component implementations.

## Structure

- `tokens/`: design tokens (colors, typography, spacing, radius, shadows, themes).
- `next/`: Next.js (React) component implementation.
- `leptos/`: Leptos (Rust) component implementation.
- `docs/`: component API contracts and usage guidelines.

## Component scope (initial list)

- Button
- Input
- Textarea
- Select
- Checkbox
- Switch
- Badge / Tag
- Table
- Modal / Dialog
- Toast / Notification
- Sidebar / Navigation
- Header / Topbar

## Principles

- Keep API parity across Next and Leptos components.
- Base styling on shared tokens and support light/dark themes.
- Avoid direct duplication of shadcn by treating it as a reference for Next only.

