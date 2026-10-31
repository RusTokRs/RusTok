# @rustok/iggy-connector-admin

Next.js admin UI package for managing the Iggy streaming message broker connector.

## Responsibilities

- Exposes the Iggy broker connection management workspace for `apps/next-admin`.
- Supports switching between bundled (embedded) and external operator-managed broker modes (`bundled`, `external`).
- Configures external cluster connection endpoints, credentials resolver, and TLS parameters.
- Dispatches typed GraphQL queries (`iggyConnectorConfiguration`) and mutations (`updateIggyConnectorConfiguration`).
- Registers navigation items under Infrastructure with admin role access guards.
