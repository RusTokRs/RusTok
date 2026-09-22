# Unified design system: shadcn/ui CSS vars for all apps

- Date: 2026-02-25
- Status: Accepted

## Context

RusToK has four UI applications on two technology stacks:

| Application | Stack | Design system before |
|-----------|------|-------------------|
| `apps/admin` | Leptos CSR | custom `--iu-*` CSS vars |
| `apps/next-admin` | Next.js / React | shadcn/ui |
| `apps/storefront` | Leptos SSR | DaisyUI |
| `apps/next-frontend` | Next.js / React | custom Button, hardcoded colors |

During the review, three questions were raised:

1. Was the component unification in the admin panels done correctly?
2. Can the same approach be applied to the frontends?
3. Can the entire shadcn/ui be ported to Rust/Leptos?

### Problems with the old implementation

**`apps/admin`**: used `--iu-*` CSS custom properties — a custom naming scheme incompatible with shadcn. Visual parity with `apps/next-admin` was not achieved.

**`apps/storefront`**: DaisyUI — an old dependency from when the project had no unified design system. DaisyUI classes (`btn`, `badge-*`, `card-body`, `navbar`, `hero`, `stats`, `bg-base-*`) are incompatible with the other applications. Leptos SSR does not require DaisyUI — rendering is HTML strings, regular Tailwind classes are used.

**`apps/next-frontend`**: hardcoded colors (`bg-sky-600`, `border-slate-200`, `text-slate-*`) — does not follow any system.

## Decision

**Unified design system for all four applications: shadcn CSS variables + Tailwind.**

DaisyUI is completely removed from `apps/storefront`. All four applications:
- define a shadcn-compatible set of CSS custom properties (`--background`, `--foreground`, `--primary`, `--card`, `--muted`, `--accent`, `--destructive`, `--border`, `--input`, `--ring`, `--radius`)
- extend Tailwind using the same pattern `hsl(var(--name))`
- do not use hardcoded color values

### Leptos components (`UI/leptos/src/`)

Implemented as a **direct port of Tailwind classes from the shadcn/ui source**. This ensures visual parity without depending on external Leptos UI crates.

### Complete shadcn → Leptos port — on demand, not monolithic

| Category | Approach |
|-----------|--------|
| Presentation (Button, Badge, Input, Card, Label, Separator, Spinner) | ✅ Ported |
| Simple interactive (Switch, Checkbox, Select, Textarea) | ✅ Ported |
| Overlay (Dialog, Sheet, Popover, Tooltip) | Leptonic/Thaw → native port as needed |
| Complex data (Table, DatePicker, Combobox) | Leptonic/Thaw for now |
| Navigation (Breadcrumb, Tabs, Alert, Accordion) | Port natively — CSS-only, not complex |

Priority for next native ports:
1. `Alert` — CSS-only, needed for informational messages
2. `AlertDialog` — needed for confirm dialogs
3. `Tabs` — CSS + minimal JS, needed for settings pages
4. `Skeleton` — CSS-only, needed for loading states

## Consequences

### Positive
- Unified palette and styling pattern across all four applications
- No external UI libraries besides Tailwind — no versioning risks
- Leptos components are visually identical to React/shadcn counterparts
- When shadcn is updated in Next.js, only the class strings in Leptos need updating

### Negative / Trade-offs
- When shadcn is updated in Next.js, classes in Leptos versions must be manually synchronized
- Overlay components (Dialog, etc.) temporarily diverge visually (Leptonic/Thaw)

### Final state after migration

| Application | CSS system | Status |
|-----------|------------|--------|
| `apps/admin` | shadcn CSS vars + Tailwind | ✅ Migrated |
| `apps/next-admin` | shadcn/ui (React) | ✅ Already had it |
| `apps/next-frontend` | shadcn CSS vars + Tailwind | ✅ Migrated |
| `apps/storefront` | shadcn CSS vars + Tailwind (SSR) | ✅ Migrated, DaisyUI removed |
