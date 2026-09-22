# Navigation and RBAC Contract

## Purpose

This document defines the current sidebar/navigation contract in `apps/next-admin`.
Navigation is a UX filter, not a security boundary: server pages, API routes,
GraphQL, and server actions must perform their own access checks.

## Active Files

- `src/shared/config/nav-config.ts` defines host-owned core navigation.
- `src/modules/index.ts` imports module-owned entrypoints and assembles the registry.
- `src/modules/registry.ts` exposes registered module-owned items.
- `src/shared/hooks/use-nav.ts` filters items by session role and enabled modules.
- `src/widgets/app-shell/app-sidebar.tsx` groups filtered items into the shell.
- `src/widgets/command-palette/index.tsx` recursively indexes the same filtered items.

## Data Sources

- User role comes from `next-auth` session: `session.user.role`.
- Enabled modules are read via the host hook `useEnabledModules()`.
- Module-owned navigation must come from the module/package entrypoint, not from
  a host-owned feature folder.
- Starter-only routes like `billing`, `exclusive`, `workspaces`, and `workspaces/team`
  must not appear in RusTok's public navigation.

## Filtering

`useFilteredNavItems()` applies two synchronous UX filters:

1. If an item has a `moduleSlug`, it is shown only when that slug is present in the
   enabled modules list.
2. If an item has `access.role`, the user's role must be at least the specified one
   in the hierarchy `customer < manager < admin < super_admin`.

`access.requireOrg` is currently considered a legacy starter field and hides the item. New
items must not use `requireOrg`, `permission`, `plan`, or `feature` without
updating the actual runtime contract.

Filtering is applied recursively: items and children are checked first, then
empty container items with `url: '#'` are hidden. This prevents showing empty
collapsible sections when all child routes are unavailable by role or disabled by
`moduleSlug`.

## Sidebar Grouping

The shell groups already-filtered items via the `group` field, falling back to
`moduleSlug` for module-owned items:

- `Overview` — `Dashboard`.
- `Management` — collapsible `Access`, `Platform`, `Operations`.
- `Module Plugins` — collapsible module-owned containers `Blog`, `Forum`, `Catalog`, `Workflows`.
- `Account` — `Profile`; `Sign Out` remains in the footer user menu.

Core Next Admin navigation uses `i18nKey` for all host-owned labels.
The sidebar and command palette take localized labels from `messages/en.json` and
`messages/ru.json`. Module-owned items may pass `i18nKey`, but must not
introduce their own locale fallback-chain above the host/runtime locale.

Active state is computed recursively from the current pathname, so detail routes like
`/dashboard/product/:id` highlight the parent item `/dashboard/product`.

## Adding an Item

A host-owned platform screen is added to `coreNavItems` only when it is genuinely
a host shell responsibility. A module-owned screen must register from the package/module
entrypoint via `registerAdminModule()`.

Example host-owned item:

```ts
{
  title: 'Access',
  url: '#',
  i18nKey: 'access',
  group: 'management',
  icon: 'users',
  items: [
    {
      title: 'Users',
      url: '/dashboard/users',
      i18nKey: 'users',
      access: { role: 'manager' }
    }
  ]
}
```

Example module-owned item should live in the module package:

```ts
registerAdminModule({
  slug: 'product',
  navItems: [
    {
      title: 'Catalog',
      url: '#',
      i18nKey: 'catalog',
      group: 'modulePlugins',
      icon: 'product',
      moduleSlug: 'product',
      items: [
        {
          title: 'Products',
          url: '/dashboard/product',
          i18nKey: 'products'
        }
      ]
    }
  ]
});
```

## Verification

After navigation changes, run:

- `npm run typecheck` in `apps/next-admin`;
- visual sidebar check for active state and grouping;
- verify that disabled module slugs do not show module-owned items.
- verify command palette: child routes are indexed recursively and named
  in the current language.
