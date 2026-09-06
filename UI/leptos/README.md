# iu-leptos

Leptos (Rust/WASM) component library for the RusToK design system.

Provides the same component API as `UI/next/components/` but implemented natively in Leptos for use in `apps/admin` and `apps/storefront`.

## Design system approach

`iu-leptos` follows the **shadcn/ui port pattern**: each component is implemented by directly porting the Tailwind class strings from the shadcn/ui open-source React components into Leptos `view!` macros. This guarantees visual parity without depending on any third-party Leptos UI crate.

The theming layer is a shared CSS variable contract (shadcn-compatible format) defined in `apps/admin/input.css` and `apps/next-admin/src/styles/globals.css`:

```css
/* Both apps define the same variables */
:root {
  --background: 0 0% 100%;
  --foreground: 222.2 84% 4.9%;
  --primary: 222.2 47.4% 11.2%;
  --primary-foreground: 210 40% 98%;
  /* ... full shadcn variable set */
}
```

The Tailwind configs in both apps extend the same color names (`background`, `foreground`, `primary`, `card`, `muted`, `accent`, `destructive`, `border`, `input`, `ring`, `sidebar-*`) resolving to these CSS variables via `hsl(var(--name))`.

## Why this approach

| Concern | Solution |
|---------|----------|
| Visual parity with Next.js admin | Identical Tailwind classes — same tokens, same utilities |
| No external Leptos UI library dependency | Components are plain Leptos code, no crates.io risk |
| Dark mode | `.dark` class strategy, same as shadcn/ui |
| Maintenance | When shadcn updates in Next.js app — copy the class strings to the Leptos version |

Complex interactive components (Select with popover, Dialog, DatePicker) may temporarily use Thaw or Leptonic as a pragmatic fallback while simple presentational components are manually ported.

## Purpose

- Implement shadcn/ui components natively in Leptos by porting class strings
- Use shared shadcn-compatible CSS custom properties for theming
- Expose the same prop contracts as the Next.js counterparts (see `UI/docs/api-contracts.md`)

## Responsibilities

- `Button`, `Input`, `Textarea`, `Select`, `Checkbox`, `Switch`, `Badge`, `Spinner` — base form/action primitives
- CSS-variable-based theming (no hardcoded Tailwind color classes — only semantic tokens)
- Re-exported by `crates/ui/leptos-ui` which adds domain-specific wrappers (`Card`, `Label`, `Separator`)

## Entry Points

- `src/lib.rs` — public API (`pub use` all components)
- `src/types.rs` — shared enums: `ButtonVariant`, `BadgeVariant`, `Size`

## Component Index

| Component | File | Status | shadcn reference |
|-----------|------|--------|-----------------|
| `Button` | `src/button.rs` | ✅ | `button.tsx` — all 6 variants |
| `Input` | `src/input.rs` | ✅ | `input.tsx` |
| `Textarea` | `src/textarea.rs` | ✅ | `textarea.tsx` |
| `Select` | `src/select.rs` | ✅ | `select.tsx` (native `<select>`) |
| `Checkbox` | `src/checkbox.rs` | ✅ | `checkbox.tsx` |
| `Switch` | `src/switch.rs` | ✅ | `switch.tsx` |
| `Badge` | `src/badge.rs` | ✅ | `badge.tsx` — all 6 variants incl. Success/Warning |
| `Spinner` | `src/spinner.rs` | ✅ | custom (shadcn has no Spinner) |

## CSS token migration note

Previous versions of this library used `--iu-*` CSS custom properties from `UI/tokens/base.css`. The library now uses the shadcn-compatible variable set (`--background`, `--primary`, `--destructive`, etc.) directly. The `UI/tokens/base.css` file is still imported for spacing/radius/font tokens (`--iu-radius-*`, `--iu-font-*`) which are not part of the shadcn contract.

## Interactions

- **Consumed by**: `crates/ui/leptos-ui` (re-exports), `apps/admin`, `apps/storefront`
- **Depends on**: `leptos` (workspace), `serde` (for derive on enums)
- **Tokens**: shadcn CSS variables defined in host app CSS entry point

## Links

- [IU API Contracts](../docs/api-contracts.md)
- [UI Tokens](../tokens/base.css)
- [leptos-ui crate](../../crates/ui/leptos-ui/) — thin re-export wrapper
- [Platform docs](../../docs/index.md)
