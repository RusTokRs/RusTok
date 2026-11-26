# Admin data-grid contract: honest sorting, one locale, no inline styles

- Date: 2026-11-26
- Decision status: Accepted
- Implementation status: Implemented
- Owners: `crates/ui/rustok-grid` (+ `leptos` adapter), `crates/ui/rustok-ui`, `apps/admin`
- Extends: None
- Supersedes: None
- Superseded by: None

## Context

`<DataGrid />` is the single table component of the admin surface: 43 call sites in
29 module crates render through it. An audit of the UI layer
(`ui-architecture-review-2026-11-26.md`) found that the component's *interface* and its
*behaviour* had drifted apart in three ways, and that each drift was invisible in
development and obvious in production:

1. **Sorting was advertised everywhere and worked nowhere.** `GridColumnDef::sortable`
   defaulted to `true`, so every header rendered a real `<button>` with `aria-sort`, but the
   grid never reordered rows: no call site passed `sort_state` (0/43), and nothing in the
   adapter could compare two rows anyway.
2. **The chrome spoke English regardless of the operator's locale.** The pager, page-size
   picker, empty/loading rows, filter labels and table name come from `GridMessages`, and
   exactly 1 of 43 call sites passed a catalog. The other 42 tables shipped English
   pagination inside a Russian admin.
3. **Column geometry and preview width were written as `style` attributes.** Both hosts
   serve UI documents with `style-src-attr 'none'`, so the browser dropped them: resizing
   was a no-op and a narrow editor preview spanned the whole pane. Because the source of
   truth for the policy lives in Rust middleware while the violation lived in a class
   string, no gate could see the pair.

## Decision

The admin grid has one contract, and the documentation of the component is the
specification of that contract.

### Sorting is offered only when it can change the rows

`<DataGrid />` accepts `sort_cell: (row, column_id) -> CellValue`. When it is provided the
grid orders its own rows (client-side mode) through `SortState::sort_items`, i.e. with the
same `CellValue` comparator the headless core already tests, keys resolved once per row, and
a stable sort so equal rows keep their input order. Ordering happens **before** slicing, and
a sort change resets the current page.

The header renders a sort control **only** when one of the following holds:

- `sort_cell` is provided — the grid reorders;
- `on_sort_change` is provided — the caller re-queries;
- the grid is in server-side pagination mode — the caller owns the query.

Otherwise the header is a plain label. A column never shows an affordance that does
nothing, and `aria-sort` never claims an order that was not applied.

### Grid chrome is localized once, by the host

`GridMessages` remains the canonical catalog. An explicit `messages` prop wins. Otherwise
`<DataGrid />` resolves the ambient `GridLocale`, which the host provides once through
`provide_grid_locale`. Rationale: the *page* owns the language, not the 43 tables. A
per-call-site `messages=` ritual is exactly the kind of boilerplate that was 42/43 missing.

Table *names* are page-specific, so when a page renders more than one table each table
passes its own `aria_label` (a catalog key, not a literal).

### UI code never styles itself through a `style` attribute

Column geometry is a presentational `<colgroup><col width>` hint; runtime scalars that used
to be inline styles are `data-*` attributes consumed by a rule inside the CSS the host
already has to allow (a nonce'd `<style>` element). `style-src-attr 'none'` stays as is.

## Sources of truth and ownership

- `rustok-grid` (headless) owns: `SortState`, `CellValue`, the comparator, `GridMessages`,
  `ColumnWidths` clamping. No DOM, no Leptos.
- `rustok-grid-leptos` owns: prop surface, locale resolution, row ordering, and the
  rendering of the header/rows/pager.
- The **host** (`apps/admin`) owns: CSP, the ambient locale, the Tailwind `@source` roots.
- Call sites own: column definitions, `sort_cell` and, on multi-table pages, `aria_label`.

## Invariants

1. A sort control implies a comparator (`sort_cell`), a caller query (`on_sort_change`), or
   server-side pagination.
2. `aria-sort` reflects only an order that was applied to the rendered rows.
3. Sorting happens on the full client-side data set, never on the already-sliced page.
4. Changing sort or filters resets the page to 1.
5. Grid chrome is localized for every table, with no per-call-site opt-in required.
6. No `style` attribute exists in `crates/ui`, `apps/admin/src` or `apps/storefront/src`
   outside the (empty) CSP exception register.
7. Every class emitted by a host-reachable component is visible to that host's
   `@source`/`content` configuration.

### Allowed states

- Client-side pagination + `sort_cell`: grid sorts and slices.
- Server-side pagination + `on_sort_change`: caller sorts and slices, grid reports state.
- `sort_cell` absent, `on_sort_change` absent, server-side pagination: headers are labels.
- No `messages` prop, ambient `GridLocale` provided: localized chrome.
- No ambient locale either: the catalog default (`en`) applies.

### Forbidden states

- A sortable header on a grid that cannot reorder and has no callback.
- A table rendering chrome in a locale the host did not choose.
- Two tables on one page with the same accessible name.
- Any inline `style` attribute in a UI host outside the exception register.
- A class that exists in source but in no `@source` root (renders unstyled in production).

## Verification

| Contract | Gate |
| --- | --- |
| Sorting/aria agreement, locale resolution | `crates/ui/rustok-grid/leptos/README.md` + `verify-ui-grid-parity.mjs` |
| Chrome localization and multi-table naming | `npm run verify:ui:grid-messages` |
| No inline styles anywhere in the UI layer | `npm run verify:csp:inline-style-exceptions` |
| Class coverage of `@source`/`content` | `npm run verify:ui:tailwind-sources` (with tests) |

The three gates run in `.github/workflows/ui-i18n-verify.yml`.

## Data, transaction, and concurrency boundary

This contract is presentation-only and owns no persisted state. Grid state (sort, filters,
selection, page, widths) lives in signals owned by the page; nothing is written to storage
by the grid. The only durable artefact is the locale the host resolved, which the adapter
reads and never writes.

## Context dimensions

- `locale` — a tenant/admin UI locale; selects the `GridMessages` catalog.
- `page` / `page_size` — the operator's viewport into the result set; a change of sort or
  filter resets the page to 1.
- `column set` — visibility and width are per mount unless the page passes its own signals.
- `CSP policy` — the host's `style-src`/`style-src-attr` directives constrain *how* geometry
  may be expressed.

## Events and projections

- `on_sort_change(SortState)` — emitted when the operator changes the order, after the page
  reset, so a server-side caller sees the state it must query with.
- `on_filter_change(ColumnFilters)`, `on_page_change(usize)`, `on_page_size_change(usize)`,
  `on_selection_change(RowSelection)`, `on_column_widths_change(ColumnWidths)` — unchanged
  by this ADR; each reports the clamped, effective value, never a value the grid rejected.
- Projections: `pagination.total` is derived from `data` when the page does not supply an
  `on_page_change`; otherwise it is caller-owned.

## Failure semantics

- No comparator and no callback → headers are labels (fail closed, never a dead control).
- No explicit catalog and no ambient locale → catalog default `en`.
- Unknown `sort_cell` column (returns `CellValue::Empty`) → rows keep their input order.
- Out-of-range page request → clamped before it is reported to the caller.
- Inline style attempted anywhere in the UI layer → gate fails the build; the browser would
  have dropped the attribute silently.

## Migration and cutover

Already applied in this change: the adapter gained `sort_cell`/`sort_enabled`, the locale
context, and `<colgroup>` geometry; `rustok-auth/admin` is the reference consumer that
passes `sort_cell`; `apps/admin` provides the locale once. Remaining call sites need no
change to keep their current behaviour (labels instead of dead controls), and gain sorting
by adding one closure. No data migration, no feature flag: the change is additive at the
call site and strictly narrowing for the header affordance.

## Alternatives considered

- **Require `messages=` at every call site.** Rejected: 42 of 43 sites would repeat the same
  expression, and the failure mode (forgetting) is silent — the very defect being fixed.
- **Default `sortable: false` and gate `sortable ⇒ on_sort_change`.** Rejected as the only
  measure: it removes the lie but keeps 43 tables unsortable, whereas one comparator per
  table makes them sortable.
- **Add a CSP exception for the grid's `style` attributes.** Rejected: the register's ratchet
  is zero and inline geometry is avoidable for every current use.
- **CSS custom properties for column widths.** Rejected: setting a custom property still
  requires a `style` attribute, so the CSP outcome is identical.

## Non-goals

- Virtualization, sticky headers, multi-column sort, export, and width persistence: not
  implied by this ADR and not implemented. `GridColumnDef::pinned` remains an unimplemented
  field; the documentation no longer claims pinned columns.
- Client-side filtering semantics: filters are still a caller concern.
- Migrating the remaining raw `<button>`/`<input>`/`<select>` usage in module admin crates to
  the design system — tracked separately by the UI review (§7, P2-2).

## Consequences

- `GridHeader` gains a required `sort_enabled: Signal<bool>` and `GridRow` loses its
  `column_widths` prop: both are adapter-internal (the Dioxus twin is a separate crate).
- A module that wants sortable columns now supplies one closure, not a state machine.
- Administrators switching the language get translated grid chrome on the next mount;
  because chrome is resolved once per mount, an in-place language switch requires a remount.
- Removing `style` attributes costs one browser-compat caveat: `<col width>` is a *hint*,
  not a hard constraint, so a very long cell may still widen its column. Column resizing
  itself is unaffected because the stored width is clamped by `ColumnWidths`.
