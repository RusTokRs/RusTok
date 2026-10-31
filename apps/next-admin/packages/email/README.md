# @rustok/email-admin

Next.js admin UI package for platform email delivery and SMTP configuration.

## Responsibilities

- Exposes the email settings dashboard page and form for `apps/next-admin`.
- Communicates directly with the RusToK GraphQL API via typed queries (`platformSettings(category: "email")`) and mutations (`updatePlatformSettings`).
- Provides dynamic configuration controls for email dispatch (enabled toggle, sender from address, password reset base URL, SMTP host, port, username, and password).
- Registers navigation items under Platform Settings with admin role access guards.

