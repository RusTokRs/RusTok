# apps/next-admin/packages/email / CRATE_API

## Purpose

`@rustok/email-admin` (`apps/next-admin/packages/email`) is the Next.js administration UI package for platform email transport and delivery configuration. It provides administrative controls for managing global email delivery switches, sender identity, password reset URL templates, and SMTP connection credentials in the Next.js admin dashboard.

## Architecture & Responsibilities

- **Module Registration:** Registers the email settings module with the host admin registry via `registerAdminModule` and supplies structured navigation items (`emailNavItems`) under Platform Settings with `admin` role access guards and keyboard shortcut `['e', 'm']`.
- **Settings Workspace:** Exposes `EmailSettingsPage` / `EmailSettingsAdminPage` and `EmailSettingsForm` for reviewing and modifying SMTP credentials, delivery toggles, and base URL configurations with optimistic toast feedback.
- **Dynamic Configuration Storage:** Integrates directly with the platform settings store via typed GraphQL queries (`platformSettings(category: "email")`) and mutations (`updatePlatformSettings(input: { category: "email", settings: "..." })`) without intermediate server proxies.
- **Fail-Safe Defaults:** Hardened against uninitialized or invalid platform settings JSON with canonical `DEFAULT_EMAIL_SETTINGS` fallbacks.

## Public Surface

- `emailNavItems`: Sidebar navigation metadata (`title: 'Platform Settings'`, `items: [{ title: 'Email', url: '/dashboard/email', shortcut: ['e', 'm'] }]`, `access: { role: 'admin' }`).
- `EmailSettingsPage` / `EmailSettingsAdminPage`: Server-rendered email configuration container page.
- `EmailSettingsForm`: Interactive form component managing SMTP and delivery parameters.
- `DEFAULT_EMAIL_SETTINGS`: Canonical default configuration fallback object (`enabled: false`, `provider: 'smtp'`, `from: 'noreply@example.com'`, `resetBaseUrl: 'https://example.com'`, `smtpPort: 587`).
- `PLATFORM_SETTINGS_QUERY`, `UPDATE_PLATFORM_SETTINGS_MUTATION`: Exported GraphQL query and mutation documents.
- `getEmailSettings(opts)`: Fetches and safely parses persisted email configuration settings.
- `updateEmailSettings(settings, opts)`: Updates and serializes email configuration payload to the platform settings store.
- `EmailSettings`, `EmailSettingsInput`, `GqlOpts`: TypeScript type definitions.

## Invariants & Guarantees

1. **GraphQL Contract Hygiene:** All operations execute through the canonical platform settings contract with category `'email'`.
2. **Safe Fallbacks:** Unparsable or missing platform settings JSON gracefully falls back to `DEFAULT_EMAIL_SETTINGS` without throwing or crashing the UI.
3. **Admin Guard:** Navigation and access strictly require the `admin` role (`access: { role: 'admin' }`).
4. **Credential Security:** Password field uses `type='password'` and `autoComplete='new-password'`, with a placeholder explaining that leaving it blank preserves the existing secret.
5. **Parity with Leptos Admin:** Email configuration capabilities and fields maintain functional parity with `apps/admin/src/pages/email_settings.rs`.

## Dependencies & Boundaries

- **Host Interface:** Depends on React 19, `@rustok/next-fluent`, `sonner`, `@/modules/registry`, and `@/shared/ui/shadcn` design system components from `apps/next-admin`.
- **Backend API:** Communicates with `apps/server` via GraphQL endpoint (`/api/graphql`).
- **No Direct Storage Access:** Never accesses configuration files or databases directly; all changes commit through the authenticated RusToK GraphQL gateway.

