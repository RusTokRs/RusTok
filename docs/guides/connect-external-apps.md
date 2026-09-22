---
id: doc://docs/guides/connect-external-apps.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# How to Connect External Apps (OAuth2)

RusTok supports two OAuth app groups:

- `first_party` and `embedded` frontends are managed automatically from `modules.toml`.
- `third_party`, `mobile`, and `service` apps are managed manually through GraphQL, the admin UIs, or the CLI task.

## App Types

- `ThirdParty`: browser-based or partner integrations. Consent is required.
- `Mobile`: native/mobile clients that use PKCE.
- `Service`: machine-to-machine clients that use `client_credentials`.
- `FirstParty`: standalone Admin or Storefront frontends. These are manifest-managed and are not created manually.
- `Embedded`: frontends compiled into the server binary. These are manifest-managed and do not use OAuth login flows.

## 1. Manifest-Managed Frontends

Standalone frontends are registered from `modules.toml`. The manifest is the source of truth for:

- `build.admin.public_url`
- `build.admin.redirect_uris`
- `build.storefront[*].public_url`
- `build.storefront[*].redirect_uris`

Rules:

- `embedded` apps are created automatically when the surface is embedded into the server.
- standalone `first_party` admin/storefront apps are created automatically with `authorization_code` and `client_credentials`.
- manual edits and revocation are blocked for manifest-managed apps in both admin UIs.
- client secret rotation is allowed for standalone `first_party` apps.

App reconciliation runs:

- during server bootstrap after `modules.toml` validation
- after release activation, so the active runtime and OAuth registry stay aligned

## 2. Creating Manual Apps

You can manage manual OAuth apps from both admin UIs:

1. Open **App Connections**.
2. Click **Create New App**.
3. Choose `ThirdParty`, `Mobile`, or `Service`.
4. Fill in `redirectUris`, `scopes`, `grantTypes`, and `grantedPermissions` when the app uses `client_credentials`.
5. Save the displayed `clientSecret` immediately. It is shown only once.

The same operations are available via GraphQL:

- `createOAuthApp`
- `updateOAuthApp`
- `rotateOAuthAppSecret`
- `revokeOAuthApp`
- `oauthApps`
- `myAuthorizedApps`

## 3. Bootstrapping a Manual App via CLI

For local development you can create a test app with the built-in task:

```bash
cd apps/server
rustok-cli oauth create-app --name "My Postman Client" --slug postman-dev
```

This prints a one-time `client_secret`.

## 4. Browser Authorization Flow

Browser install/login flow uses `GET /api/oauth/authorize`.

Required query parameters:

- `response_type=code`
- `client_id`
- `redirect_uri`
- `code_challenge`
- `code_challenge_method=S256`
- optional `scope`
- optional `state`

Behaviour:

- `first_party` apps skip consent and redirect immediately with `code` and optional `state`
- `third_party` apps render a server-hosted consent page when consent is missing
- approving consent submits to `POST /api/oauth/consent`
- denying consent redirects back with `error=access_denied`

Current server-hosted consent pages expect an authenticated user access token. In practice this means:

- first-party frontends should call `POST /api/oauth/browser-session` with the current bearer token
- then open `GET /api/oauth/authorize` in the browser
- the temporary OAuth browser session cookie is cleared after the authorization redirect

## 5. Token Exchange

Use `POST /api/oauth/token` for machine/API steps:

- `authorization_code` + PKCE (`S256` only)
- `refresh_token`
- `client_credentials`

For `client_credentials`:

- OAuth `scopes` still gate transport/API intent.
- RusToK domain access is resolved separately from `oauth_apps.granted_permissions`.
- The server reads `granted_permissions` live on every service-token request, so app deactivation or permission edits apply immediately.

Related endpoints:

- `POST /api/oauth/revoke`
- `GET /api/oauth/userinfo`
- `GET /.well-known/oauth-authorization-server`
- `GET /.well-known/openid-configuration`

## 6. Operational Rules

- `authorization_code` apps must have at least one `redirect_uri`.
- `service` apps cannot use `authorization_code`.
- apps using `client_credentials` must declare at least one `granted_permission`.
- `first_party` and `embedded` apps are not created manually.
- both admin UIs display whether an app is manual or `managed by config/manifest`.

For deeper architecture notes, see [OAuth2 App Connections](../concepts/plan-oauth2-app-connections.md).
