---
id: doc://docs/concepts/plan-oauth2-app-connections.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# OAuth2 App Connections — connecting external applications

- Date: 2026-03-07
- Status: Draft
- Depends on: [Deployment Profiles ADR](../../DECISIONS/2026-03-07-deployment-profiles-and-ui-stack.md)

## Current implementation status

As of the current implementation:

- `embedded` and `first_party` applications are registered automatically from `modules.toml`.
- source of truth for standalone frontend OAuth settings:
  - `build.admin.public_url`
  - `build.admin.redirect_uris`
  - `build.storefront[*].public_url`
  - `build.storefront[*].redirect_uris`
- reconcile manifest-managed applications is performed:
  - during server bootstrap after manifest validation
  - after release activation
- browser install/login flow uses `GET /api/oauth/authorize`
- browser auth state for server-hosted consent flow is held in a temporary HttpOnly cookie,
  created via `POST /api/oauth/browser-session`
- server-hosted consent flow uses `POST /api/oauth/consent`
- JSON/API flow for SPA/native automation is preserved at `POST /api/oauth/authorize`
- `third_party` applications require consent
- `first_party` applications skip consent
- both admin UIs work through real GraphQL operations:
  - `createOAuthApp`
  - `updateOAuthApp`
  - `rotateOAuthAppSecret`
  - `revokeOAuthApp`
  - `oauthApps`
  - `myAuthorizedApps`
- manifest-managed applications are displayed in the UI as read-only for edit/revoke

## Why This Is Needed

Composable deployment layers (ADR v2) solve *how to assemble the platform*. But when
a storefront is deployed as a separate process (Next.js, mobile app, Telegram bot,
partner storefront), it needs a **secure, standard way** to connect to the API.

Currently we have JWT authentication tied to users. This is insufficient:

| Scenario | Current system | What's missing |
|---|---|---|
| Leptos admin (embedded) | Works via a single binary | — |
| Next.js admin (standalone) | JWT via GraphQL login | No app-level credentials, no scopes |
| Next.js storefront | JWT | No client_id, no scope restrictions |
| Mobile app | — | No OAuth2 flow |
| Telegram store bot | — | No machine-to-machine auth |
| Partner storefront | — | No limited access issuance |

**OAuth2 App Connections** is a mechanism for registering and authenticating **applications**
(not users), giving them controlled access to the GraphQL API.

## Core Concepts

### 1. What is an "App Connection"

```
┌──────────────────────────────────────────────────┐
│                  RusTok Platform                 │
│                                                  │
│  ┌────────────────────────────────────────────┐  │
│  │              App Registry                  │  │
│  │                                            │  │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐   │  │
│  │  │ Next.js │  │ Mobile  │  │ Telegram│   │  │
│  │  │ Store   │  │ App     │  │ Bot     │   │  │
│  │  │         │  │         │  │         │   │  │
│  │  │ client_ │  │ client_ │  │ client_ │   │  │
│  │  │ id: ... │  │ id: ... │  │ id: ... │   │  │
│  │  └────┬────┘  └────┬────┘  └────┬────┘   │  │
│  │       │            │            │         │  │
│  └───────┼────────────┼────────────┼─────────┘  │
│          │            │            │             │
│  ┌───────▼────────────▼────────────▼─────────┐  │
│  │          GraphQL API (Axum)               │  │
│  │   scope-based access control per app      │  │
│  └───────────────────────────────────────────┘  │
└──────────────────────────────────────────────────┘
```

**App Connection** = a DB record describing an **external application** that is
allowed to work with a specific tenant's API.

Each connection has:
- `client_id` — public identifier (UUID)
- `client_secret` — secret (stored as hash, shown once)
- `app_type` — application type (determines available OAuth2 flows)
- `scopes` — allowed operations
- `redirect_uris` — for Authorization Code flow
- Association with `tenant_id`

### 2. Application Types (App Types)

```rust
pub enum AppType {
    /// Built into the binary (Leptos). Does not need OAuth2.
    /// Direct API access via shared state.
    Embedded,

    /// First party: our admin/storefront deployed as a separate process.
    /// Trusted — full set of scopes by default.
    /// Flow: Authorization Code + PKCE (for SPA), Client Credentials (for SSR).
    FirstParty,

    /// Mobile app: Authorization Code + PKCE.
    /// User context is required.
    Mobile,

    /// Machine-to-machine: bots, integrations, CI/CD.
    /// Flow: Client Credentials (without user context).
    Service,

    /// Third-party developers: limited access.
    /// Flow: Authorization Code + PKCE.
    /// Mandatory scope review.
    ThirdParty,
}
```

### 3. OAuth2 Flows

| App Type | Authorization Code + PKCE | Client Credentials | Implicit |
|---|---|---|---|
| FirstParty | Yes (SPA frontend) | Yes (SSR backend) | No |
| Mobile | Yes | No | No |
| Service | No | Yes | No |
| ThirdParty | Yes | No | No |

> **Implicit flow is not supported** — deprecated per OAuth 2.1.

### 4. Scopes

Scopes control which parts of the GraphQL API an application can access.

```
# Format: resource:action
# Wildcard: resource:* or *:*

# Read
catalog:read          # Products, categories, prices
content:read          # Content blocks, pages
orders:read           # Orders (own user or all — depends on context)
users:read            # User profiles

# Write
cart:write             # Cart, checkout
orders:write           # Create/update orders
content:write          # Create/edit content
users:write            # Update profiles

# Admin
admin:modules          # Module management
admin:tenants          # Tenant management
admin:users            # User management
admin:settings         # System settings
admin:builds           # Build triggers

# Special
storefront:*           # All storefront operations (for FirstParty storefront)
admin:*                # All admin operations (for FirstParty admin)
*:*                    # Full access (Embedded only)
```

**Rule**: scope defines the **maximum** application permissions. Within the application,
the user is still limited by their RBAC role. Effective access = `scopes ∩ RBAC`.

## Architecture

### 1. Relation to Deployment Profiles

```
modules.toml                    App Registry (DB)
─────────────                   ──────────────────

[build.server]                  ┌───────────────────────────┐
embed_admin = true   ──────►    │ App: "leptos-admin"       │
                                │ type: Embedded            │
                                │ scopes: *:*               │
                                │ (auto-created, no secret) │
                                └───────────────────────────┘

embed_storefront = false         (no auto-created app)

[[build.storefront]]            ┌───────────────────────────┐
id = "site-eu"       ──────►    │ App: "site-eu"            │
stack = "next"                  │ type: FirstParty          │
                                │ scopes: storefront:*      │
                                │ client_id: uuid           │
                                │ client_secret: ****       │
                                └───────────────────────────┘

[[build.storefront]]            ┌───────────────────────────┐
id = "site-us"       ──────►    │ App: "site-us"            │
stack = "next"                  │ type: FirstParty          │
                                │ scopes: storefront:*      │
                                │ client_id: uuid           │
                                │ client_secret: ****       │
                                └───────────────────────────┘
```

**Sync rule on `rustok rebuild`**:
1. Embedded (`embed_*=true`) → `Embedded` app is created automatically, without secret
2. Standalone Leptos/Next.js → `FirstParty` app, credentials generated on first creation
3. Previously registered apps removed from `modules.toml` → deactivated (soft delete)

### 2. Authentication Flow

#### A. Authorization Code + PKCE (SPA, Mobile, Third-Party)

```
┌──────────┐                          ┌──────────────┐
│  Browser │                          │ RusTok API   │
│  / App   │                          │              │
└────┬─────┘                          └──────┬───────┘
     │                                       │
     │  1. GET /oauth/authorize              │
     │     ?client_id=xxx                    │
     │     &redirect_uri=...                 │
     │     &response_type=code               │
     │     &code_challenge=...               │
     │     &code_challenge_method=S256       │
     │     &scope=catalog:read+cart:write    │
     │─────────────────────────────────────► │
     │                                       │
     │  2. Login page (if not authenticated) │
     │  ◄──────────────────────────────────  │
     │                                       │
     │  3. User authenticates + consents     │
     │─────────────────────────────────────► │
     │                                       │
     │  4. Redirect to redirect_uri          │
     │     ?code=AUTH_CODE                   │
     │  ◄──────────────────────────────────  │
     │                                       │
     │  5. POST /oauth/token                 │
     │     grant_type=authorization_code     │
     │     &code=AUTH_CODE                   │
     │     &code_verifier=...               │
     │     &client_id=xxx                   │
     │─────────────────────────────────────► │
     │                                       │
     │  6. { access_token, refresh_token }   │
     │  ◄──────────────────────────────────  │
     │                                       │
     │  7. GraphQL with Bearer token         │
     │─────────────────────────────────────► │
```

#### B. Client Credentials (Service, SSR backend)

```
┌──────────────┐                     ┌──────────────┐
│ Next.js SSR  │                     │ RusTok API   │
│ (server-side)│                     │              │
└──────┬───────┘                     └──────┬───────┘
       │                                    │
       │  1. POST /oauth/token              │
       │     grant_type=client_credentials  │
       │     &client_id=xxx                 │
       │     &client_secret=yyy             │
       │     &scope=storefront:*            │
       │──────────────────────────────────► │
       │                                    │
       │  2. { access_token }               │
       │     (no refresh_token,             │
       │      no user context)              │
       │  ◄─────────────────────────────── │
       │                                    │
       │  3. GraphQL with Bearer token      │
       │──────────────────────────────────► │
```

#### C. Combined (SSR + user)

Next.js storefront uses **both** flows:
- `client_credentials` — for public data (catalog, pages) without login
- `authorization_code` — when user logs in (cart, orders, profile)

```
┌──────────────┐
│ Next.js SSR  │
└──────┬───────┘
       │
       ├── getServerSideProps() ──► client_credentials token
       │   (catalog, SEO data)      scope: catalog:read
       │
       └── User clicks "Login" ──► authorization_code + PKCE
           (cart, profile)          scope: cart:write,orders:read
```

### 3. Token Claims (extension of current JWT structure)

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    // Existing fields
    pub sub: Uuid,                    // User ID (or app_id for client_credentials)
    pub tenant_id: Uuid,
    pub role: UserRole,
    pub session_id: Uuid,
    pub iss: String,                  // "rustok"
    pub aud: String,                  // "rustok-api"
    pub exp: usize,
    pub iat: usize,

    // New fields for OAuth2
    pub client_id: Option<Uuid>,      // App connection ID (None for embedded)
    pub scopes: Vec<String>,          // Granted scopes
    pub grant_type: GrantType,        // Which flow was used
}

pub enum GrantType {
    /// Direct access (embedded Leptos, current system)
    Direct,
    /// Authorization Code + PKCE (user context)
    AuthorizationCode,
    /// Client Credentials (app-level, without user)
    ClientCredentials,
    /// Refresh Token
    RefreshToken,
}
```

For `client_credentials` without user context:
- `sub` = `app_id` (not user_id)
- `role` = `Service` (new role, lower than Customer in privileges)
- `session_id` = `Uuid::nil()` (no session)

### 4. Middleware: scope enforcement

```rust
/// Checks that the current token has the required scope
pub fn require_scope(required: &str) -> impl Filter {
    // 1. Extract scopes from JWT claims
    // 2. Check: required ∈ scopes (including wildcards)
    // 3. If not — 403 Forbidden with description of missing scope
}

// Usage in GraphQL resolvers:
impl QueryRoot {
    async fn products(&self, ctx: &Context<'_>) -> Result<Vec<Product>> {
        let auth = ctx.data::<AuthContext>()?;
        auth.require_scope("catalog:read")?;  // ← scope check
        // ... fetch products
    }
}
```

## DB Schema

### Table `oauth_apps`

```sql
CREATE TABLE oauth_apps (
    id              UUID PRIMARY KEY,
    tenant_id       UUID NOT NULL REFERENCES tenants(id),

    -- Identification
    name            VARCHAR(255) NOT NULL,          -- "Next.js Storefront EU"
    slug            VARCHAR(100) NOT NULL,          -- "site-eu"
    description     TEXT,
    app_type        VARCHAR(50) NOT NULL,           -- embedded/first_party/mobile/service/third_party
    icon_url        VARCHAR(500),

    -- Credentials
    client_id       UUID NOT NULL UNIQUE,           -- Public ID
    client_secret_hash VARCHAR(255),                -- Argon2 hash (NULL for Embedded)

    -- OAuth2 config
    redirect_uris   JSONB NOT NULL DEFAULT '[]',    -- ["https://store.example.com/callback"]
    scopes          JSONB NOT NULL DEFAULT '[]',     -- ["storefront:*"]
    grant_types     JSONB NOT NULL DEFAULT '[]',     -- ["authorization_code", "client_credentials"]

    -- Relation to modules.toml
    manifest_ref    VARCHAR(100),                   -- "storefront:site-eu" or NULL
    auto_created    BOOLEAN NOT NULL DEFAULT FALSE,  -- Created on rebuild?

    -- Status
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    revoked_at      TIMESTAMPTZ,
    last_used_at    TIMESTAMPTZ,

    -- Metadata
    metadata        JSONB NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(tenant_id, slug)
);

CREATE INDEX idx_oauth_apps_client_id ON oauth_apps(client_id);
CREATE INDEX idx_oauth_apps_tenant ON oauth_apps(tenant_id) WHERE is_active = TRUE;
```

### Table `oauth_authorization_codes`

```sql
CREATE TABLE oauth_authorization_codes (
    id              UUID PRIMARY KEY,
    app_id          UUID NOT NULL REFERENCES oauth_apps(id),
    user_id         UUID NOT NULL REFERENCES users(id),
    tenant_id       UUID NOT NULL REFERENCES tenants(id),

    code_hash       VARCHAR(255) NOT NULL UNIQUE,   -- SHA256 hash of code
    redirect_uri    VARCHAR(500) NOT NULL,
    scopes          JSONB NOT NULL,
    code_challenge  VARCHAR(255) NOT NULL,           -- PKCE S256
    code_challenge_method VARCHAR(10) NOT NULL DEFAULT 'S256',

    expires_at      TIMESTAMPTZ NOT NULL,            -- Short-lived: 10 minutes
    used_at         TIMESTAMPTZ,                     -- NULL = not used

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_oauth_codes_hash ON oauth_authorization_codes(code_hash)
    WHERE used_at IS NULL;
```

### Table `oauth_tokens`

```sql
CREATE TABLE oauth_tokens (
    id              UUID PRIMARY KEY,
    app_id          UUID NOT NULL REFERENCES oauth_apps(id),
    user_id         UUID,                           -- NULL for client_credentials
    tenant_id       UUID NOT NULL REFERENCES tenants(id),

    token_hash      VARCHAR(255) NOT NULL UNIQUE,   -- SHA256 hash of refresh token
    grant_type      VARCHAR(50) NOT NULL,           -- authorization_code / client_credentials
    scopes          JSONB NOT NULL,

    expires_at      TIMESTAMPTZ NOT NULL,
    revoked_at      TIMESTAMPTZ,
    last_used_at    TIMESTAMPTZ,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_oauth_tokens_hash ON oauth_tokens(token_hash)
    WHERE revoked_at IS NULL;
CREATE INDEX idx_oauth_tokens_app ON oauth_tokens(app_id, tenant_id)
    WHERE revoked_at IS NULL;
```

### Table `oauth_consent` (for Third-Party apps)

```sql
CREATE TABLE oauth_consents (
    id              UUID PRIMARY KEY,
    app_id          UUID NOT NULL REFERENCES oauth_apps(id),
    user_id         UUID NOT NULL REFERENCES users(id),
    tenant_id       UUID NOT NULL REFERENCES tenants(id),

    scopes          JSONB NOT NULL,                  -- User-approved scopes
    granted_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at      TIMESTAMPTZ,

    UNIQUE(app_id, user_id, tenant_id)
);
```

## GraphQL API

### Queries

```graphql
type Query {
  """List of connected applications (admin-only)"""
  oauthApps(
    tenantId: UUID!
    appType: AppType
    isActive: Boolean
  ): [OAuthApp!]!

  """Application details"""
  oauthApp(id: UUID!): OAuthApp

  """Active application tokens"""
  oauthAppTokens(appId: UUID!): [OAuthTokenInfo!]!

  """Applications the user has granted access to (for profile)"""
  myAuthorizedApps: [AuthorizedAppInfo!]!
}

type OAuthApp {
  id: UUID!
  name: String!
  slug: String!
  description: String
  appType: AppType!
  clientId: UUID!
  redirectUris: [String!]!
  scopes: [String!]!
  grantTypes: [String!]!
  manifestRef: String
  autoCreated: Boolean!
  isActive: Boolean!
  lastUsedAt: DateTime
  createdAt: DateTime!
  activeTokenCount: Int!
}

type OAuthTokenInfo {
  id: UUID!
  grantType: String!
  scopes: [String!]!
  userId: UUID
  lastUsedAt: DateTime
  expiresAt: DateTime!
  createdAt: DateTime!
}

type AuthorizedAppInfo {
  app: OAuthApp!
  scopes: [String!]!
  grantedAt: DateTime!
}
```

### Mutations

```graphql
type Mutation {
  """Create a new application (admin-only)"""
  createOAuthApp(input: CreateOAuthAppInput!): CreateOAuthAppResult!

  """Update application settings"""
  updateOAuthApp(id: UUID!, input: UpdateOAuthAppInput!): OAuthApp!

  """Regenerate client_secret"""
  rotateOAuthAppSecret(id: UUID!): RotateSecretResult!

  """Deactivate application (revoke all tokens)"""
  revokeOAuthApp(id: UUID!): OAuthApp!

  """Revoke a specific token"""
  revokeOAuthToken(tokenId: UUID!): Boolean!

  """User revokes application access to their account"""
  revokeAppConsent(appId: UUID!): Boolean!
}

input CreateOAuthAppInput {
  name: String!
  slug: String!
  description: String
  appType: AppType!
  redirectUris: [String!]
  scopes: [String!]!
  grantTypes: [GrantType!]!
}

type CreateOAuthAppResult {
  app: OAuthApp!
  """client_secret is shown ONCE at creation"""
  clientSecret: String!
}

type RotateSecretResult {
  app: OAuthApp!
  """New secret, shown once"""
  clientSecret: String!
}
```

## REST Endpoints (OAuth2 standard)

OAuth2 endpoints are implemented as REST (per RFC 6749/7636 standard):

```
POST /oauth/authorize          — Authorization endpoint
POST /oauth/token              — Token endpoint
POST /oauth/revoke             — Token revocation (RFC 7009)
GET  /oauth/userinfo           — User info (OpenID Connect)
GET  /.well-known/oauth-authorization-server  — Server metadata (RFC 8414)
```

> GraphQL — for **managing** applications (admin).
> REST — for **OAuth2 flows** (standard, client libraries expect REST).

## Admin UI

### "App Connections" Page (in Modules module → Connections section)

```
┌─────────────────────────────────────────────────────────────┐
│  Connected Applications                           [+ Add]  │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ 🔒 Next.js Storefront EU              FirstParty   │   │
│  │    client_id: 8a3f...b2c1                          │   │
│  │    Scopes: storefront:*                            │   │
│  │    Last used: 2 min ago        ● Active   [Manage] │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ 🔒 Next.js Storefront US              FirstParty   │   │
│  │    client_id: f1d2...a4e7                          │   │
│  │    Scopes: storefront:*                            │   │
│  │    Last used: 5 min ago        ● Active   [Manage] │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ 🤖 Telegram Order Bot                    Service   │   │
│  │    client_id: c7b8...d3f9                          │   │
│  │    Scopes: orders:read, catalog:read               │   │
│  │    Last used: 1 hour ago       ● Active   [Manage] │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ ⚙️ Leptos Admin (embedded)              Embedded   │   │
│  │    Scopes: *:*                                     │   │
│  │    Built-in — no credentials needed                │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Manage App Dialog

```
┌─────────────────────────────────────────────────────────┐
│  Next.js Storefront EU                                  │
│                                                         │
│  Client ID:     8a3f...b2c1             [Copy]         │
│  Client Secret: ●●●●●●●●               [Rotate]       │
│                                                         │
│  Redirect URIs:                                        │
│    https://eu.store.example.com/api/auth/callback       │
│    https://eu.store.example.com/oauth/callback          │
│                                                         │
│  Scopes:                                               │
│    ☑ catalog:read    ☑ content:read                    │
│    ☑ cart:write      ☑ orders:read                     │
│    ☑ orders:write    ☐ admin:*                         │
│    ☑ users:read      ☐ admin:modules                   │
│                                                         │
│  Active Tokens: 142                                    │
│  Created: 2026-03-01                                   │
│  Last Used: 2 min ago                                  │
│                                                         │
│  [Save Changes]    [Revoke All Tokens]    [Delete App] │
└─────────────────────────────────────────────────────────┘
```

## Integration with modules.toml

### Automatic synchronization on rebuild

```rust
/// Called in BuildService after successful build
async fn sync_app_connections(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    manifest: &ModulesManifest,
) -> Result<()> {
    let existing = OAuthApp::find_by_tenant(db, tenant_id).await?;

    // 1. Embedded apps
    if manifest.build.server.embed_admin {
        upsert_embedded_app(db, tenant_id, "leptos-admin", &["*:*"]).await?;
    }
    if manifest.build.server.embed_storefront {
        upsert_embedded_app(db, tenant_id, "leptos-storefront", &["*:*"]).await?;
    }

    // 2. Standalone storefronts → FirstParty apps
    for sf in &manifest.build.storefronts {
        if !manifest.build.server.embed_storefront || sf.stack == "next" {
            upsert_first_party_app(
                db, tenant_id,
                &sf.id,
                &format!("{} storefront", sf.id),
                &["storefront:*"],
                &["authorization_code", "client_credentials"],
            ).await?;
        }
    }

    // 3. Standalone admin → FirstParty app
    if !manifest.build.server.embed_admin {
        upsert_first_party_app(
            db, tenant_id,
            &format!("{}-admin", manifest.build.admin.stack),
            &format!("{} Admin", manifest.build.admin.stack),
            &["admin:*"],
            &["authorization_code", "client_credentials"],
        ).await?;
    }

    // 4. Deactivate apps removed from manifest
    deactivate_orphaned_apps(db, tenant_id, &manifest).await?;

    Ok(())
}
```

### Example: Next.js storefront configuration

On `rustok rebuild` with:
```toml
[[build.storefront]]
id = "site-eu"
stack = "next"
```

The system:
1. Creates an `oauth_apps` record: `slug="site-eu"`, `app_type=FirstParty`
2. Generates `client_id` and `client_secret`
3. Outputs in CLI:
```
✅ App connection created for storefront "site-eu"

   Client ID:     8a3f2d1c-...
   Client Secret: sk_live_a8b7c6d5e4f3...  ← SAVE THIS!

   Add to your Next.js .env:
   RUSTOK_CLIENT_ID=8a3f2d1c-...
   RUSTOK_CLIENT_SECRET=sk_live_a8b7c6d5e4f3...
   RUSTOK_API_URL=https://api.example.com
```

### Next.js SDK Integration

```typescript
// apps/next-frontend/src/lib/rustok-client.ts

import { RusTokClient } from '@rustok/sdk';

export const rustok = new RusTokClient({
  apiUrl: process.env.RUSTOK_API_URL!,
  clientId: process.env.RUSTOK_CLIENT_ID!,
  clientSecret: process.env.RUSTOK_CLIENT_SECRET!,

  // SSR: client_credentials for public data
  // Browser: authorization_code + PKCE for user data
  mode: 'hybrid',
});

// Server Component — client_credentials
export async function getProducts() {
  const token = await rustok.getServiceToken(['catalog:read']);
  return rustok.graphql(PRODUCTS_QUERY, {}, token);
}

// Client Component — authorization_code
export function useCart() {
  const { token } = useRusTokAuth(); // PKCE flow
  return rustok.graphql(CART_QUERY, {}, token);
}
```

## Security

### Secret Storage

- `client_secret` is stored as **Argon2 hash** (like user passwords)
- Shown **once** at creation or rotation
- Prefix `sk_live_` / `sk_test_` for visual distinction

### Rate limiting (per client_id)

```
/oauth/token    — 60 req/min per client_id
/oauth/authorize — 30 req/min per IP
GraphQL         — configurable per app (default: 1000 req/min)
```

### Token lifetimes

| Token | Lifetime | Renewable |
|---|---|---|
| Authorization code | 10 min | No (one-time use) |
| Access token (user) | 15 min | Yes (via refresh) |
| Access token (service) | 1 hour | No (request new one) |
| Refresh token | 30 days | Yes (rotation) |

### Audit log

All OAuth2 events are written to the audit log:
- `oauth_app.created` / `updated` / `revoked`
- `oauth_token.issued` / `refreshed` / `revoked`
- `oauth_consent.granted` / `revoked`
- `oauth_secret.rotated`

## Implementation Plan

### Phase 1: Core OAuth2 (MVP) - **Done**

- [x] DB migration: `oauth_apps`, `oauth_tokens`
- [x] `OAuthAppService` — CRUD for applications
- [x] `POST /oauth/token` — `client_credentials` flow
- [x] Scope enforcement in GraphQL middleware (`AuthContext::require_scope`)
- [x] Extended JWT Claims (`client_id`, `scopes`, `grant_type`)
- [x] GraphQL mutations: `createOAuthApp`, `rotateOAuthAppSecret`, `revokeOAuthApp`
- [x] Auto-sync on rebuild (`sync_app_connections`) — implemented in `services/oauth_app.rs`

**Result**: Next.js storefront connects via `client_credentials`.

### Phase 2: Authorization Code + PKCE - **Done**

- [x] `POST /oauth/authorize` — authorization endpoint
- [x] PKCE validation (S256) — with constant-time comparison
- [x] Authorization code storage (`oauth_authorization_codes`)
- [x] `authorization_code` grant type in `/oauth/token`
- [x] Refresh token rotation
- [x] `POST /oauth/revoke` (RFC 7009) — REST endpoint implemented

**Result**: Mobile apps and SPA can log in users.

### Phase 3: Consent & Third-Party - **Done**

- [x] `oauth_consents` table
- [x] Consent UI (scope confirmation page) - Backend API ready (`grantAppConsent`)
- [x] Third-party app registration flow - ThirdParty app_type supported
- [x] Scope review for third-party apps - Protection in `/oauth/authorize` (`interaction_required`)
- [x] User profile: "Connected apps" → revoke access - Query `myAuthorizedApps` and `revokeAppConsent`

**Result**: Third-party developers can create integrations.

### Phase 4: Admin UI & DX - **Done**

- [x] Leptos Admin: application management (CRUD + secret rotation) + FSD components (`apps/admin/src/{entities,features,widgets,pages}/oauth_apps*`)
- [x] Built-in SDK for frontend (`npm pkg @rustok/sdk`) - Moved to Next.js Admin integrations (`Next.js Admin OAuth UI`)
- [x] Instructions/documentation "How to connect a third-party application" - Added to `docs/guides/connect-external-apps.md`
- [x] CLI tools/scripts for quick app creation in dev environment (via Loco CLI Task `create_oauth_app`)
- [x] `/.well-known/oauth-authorization-server` metadata endpoint (+ `/openid-configuration`)
- [x] OpenID Connect basic support (`/oauth/userinfo`)
- [x] Documentation for module developers — included in `docs/guides/connect-external-apps.md`

**Result**: Full developer experience.

### Phase 5: RFC Compliance Tests - **Done**

- [x] RFC 6749 (OAuth 2.0) — scope validation, error codes, token response format, grant types (15 tests)
- [x] RFC 7636 (PKCE) — S256 transform, Appendix B test vector, constant-time comparison (7 tests)
- [x] RFC 7519 (JWT) — claims validation, expiration, issuer/audience/signature check (5 tests)
- [x] RFC 7009 (Token Revocation) — always-200 semantics, token_type_hint values (2 tests)
- [x] RFC 8414 (Metadata) — required fields, well-known paths, implementation match (3 tests)
- [x] OAuth2 scope enforcement — `AuthContext.require_scope()` for direct/OAuth2 tokens (7 tests)
- [x] Credential security — entropy, Argon2, SHA-256, salt uniqueness (6 tests)
- [x] Documentation — `docs/guides/testing-oauth2-rfc.md`

**Result**: 45 unit tests, no DB required. Full RFC compliance coverage.

### Verified: 2026-03-08 — all issues fixed

#### Issues found and fixed during verification

| # | Severity | Issue | Status | Fix |
|---|---|---|---|---|
| 1 | **Critical** | Case mismatch `"ThirdParty"` in consent check | **Fixed** | `controllers/oauth.rs` — replaced with `"third_party"` |
| 2 | **Critical** | Case mismatch `"ThirdParty"` in CLI task | **Fixed** | `tasks/create_oauth_app.rs` — replaced with `"third_party"` |
| 3 | **High** | `/oauth/revoke` not implemented | **Fixed** | Added `revoke_handler` + route + `revoke_token_by_hash` in service |
| 4 | **High** | `sync_app_connections` not implemented | **Fixed** | Full function implemented with upsert embedded/first-party + orphan deactivation |
| 5 | **High** | `oauth_tokens` missing `updated_at` | **Fixed** | Column added in migration + field in entity model |
| 6 | **Medium** | Workspace doesn't compile | **Fixed** | `leptos_i18n`/`leptos_i18n_build` updated to 0.6.1 |
| 7 | **Low** | Partial indexes without WHERE | **Fixed** | Migrations use raw SQL with WHERE clauses |
| 8 | **Medium** | `find_active_by_hash` signature mismatch | **Fixed** | Added `app_id` parameter in model |

## Relation to Other Plans

| Document | Relation |
|---|---|
| [Deployment Profiles ADR](../../DECISIONS/2026-03-07-deployment-profiles-and-ui-stack.md) | `modules.toml` defines which apps are created automatically |
| [Module authoring guide](../modules/module-authoring.md) | Rebuild triggers sync_app_connections; marketplace modules register OAuth apps |
| Security Standards (`docs/standards/security.md`) | OAuth2 extends existing OWASP protections |

## Summary

OAuth2 App Connections is the **bridge** between composable deployment layers and real
applications. Without this mechanism, standalone storefronts, mobile apps, and
third-party integrations cannot securely work with the API.

```
                        modules.toml
                            │
                    ┌───────┴────────┐
                    │  rustok rebuild │
                    └───────┬────────┘
                            │
                 ┌──────────┴──────────┐
                 │  sync_app_connections│
                 └──────────┬──────────┘
                            │
              ┌─────────────┼─────────────┐
              │             │             │
        ┌─────▼─────┐ ┌────▼────┐  ┌─────▼─────┐
        │ Embedded  │ │ First   │  │ Service   │
        │ (Leptos)  │ │ Party   │  │ (bots,    │
        │ no auth   │ │ (Next)  │  │  CI/CD)   │
        │ needed    │ │ OAuth2  │  │ client_   │
        │           │ │ PKCE +  │  │ credentials│
        └───────────┘ │ client_ │  └───────────┘
                      │ creds   │
                      └─────────┘
                           │
                    ┌──────┴──────┐
                    │   GraphQL   │
                    │   API       │
                    │  (scoped)   │
                    └─────────────┘
```
