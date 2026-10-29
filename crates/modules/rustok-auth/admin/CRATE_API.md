# rustok-auth-admin / CRATE_API

## Public surface

### Entry Point & Admin Pages
- `AuthAdmin`: module root admin page component for authentication control plane.
- `Login`: administrator login form and session initiator.
- `Register`: operator registration interface.
- `ResetPassword`: password reset request and recovery workflow.
- `Profile`: operator personal profile and settings interface.
- `Security`: account security, multi-factor authentication, and active session management.
- `Users`: tenant user directory listing, search, and status controls.
- `UserDetails`: detailed user view and role/attribute assignment.
- `OAuthAppsPage`: OAuth client applications management surface.
- `AcceptInvite`: invitation token validation and account onboarding.

### Core View Models & Helpers
- `AuthCopy`: view model for presentation headings and explanatory text.
- `LoginRequest`: structured login credentials request.
- `RegisterRequest`: structured registration request.
- `PasswordResetRequest`: password recovery initiation request.
- `ChangePasswordRequest`: password update command with verification token.

## Platform Capability Bindings
- **Mounted Admin Surface**: Registers `AuthAdmin` and user administration routes within `apps/admin`.
- **Fluid Frontend Architecture**: Strictly isolates Leptos rendering from framework-agnostic presentation logic in `core`.
- **RBAC & User Administration**: Manages user lifecycle states, password policies, and OAuth client credentials.

## Domain Collaborations
- **`rustok-auth`**: Core authentication module providing identity, hashing, tokens, and OAuth services.
- **`rustok-grid-leptos`**: Standard data grid component rendering the user directory with sorting and pagination.
- **`rustok-ui-core`**: Core UI text normalization and layout helpers.
- **`rustok-ui-i18n`**: Package-owned Fluent translations (`en.ftl`, `ru.ftl`) synchronized with host locale context.

## Minimum Contract Set

### Input DTOs/Commands
- `LoginRequest`: payload with tenant, email, and password.
- `RegisterRequest`: payload with tenant, email, password, and optional full name.
- `CreateUserInput`: operator command creating a new tenant user.
- `UpdateUserInput`: operator command modifying user properties and access status.
- `CreateOAuthAppInput`: payload defining new OAuth application parameters.
- `UpdateOAuthAppInput`: payload updating registered OAuth client properties.

### Domain Invariants
- **Non-Swallowed Errors**: Authentication and mutation failures must yield typed UI errors and avoid silent fallback.
- **Tenant Scope Enforcement**: User management and authentication requests strictly enforce tenant boundaries.
- **Credential Hygiene**: Passwords and secrets are never logged or exposed in client error states.

### Events / Outbox Side Effects
- Dispatches user creation, update, and suspension mutations producing outbox audit events.
- Emits OAuth client registrations and credential rotations through the native server adapter.

### Errors / Failure Codes
- `CreateUserInputError`: user creation validation failure (`MissingCredentials`).
- `AuthFormInputError`: input validation failure (`MissingRequiredFields`).
- `UiError`: presentation and RPC error states across authentication flows.
