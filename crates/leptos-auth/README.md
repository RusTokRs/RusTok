# leptos-auth

## Назначение

`crates/leptos-auth` — Leptos authentication library для RusToK с dual-path transport boundary: native Leptos `#[server]` first и сохранённый GraphQL fallback.

## Архитектура

**Главное правило:** Leptos auth surfaces идут через local API boundary. Native `#[server]` path добавляется параллельно, но GraphQL path не удаляется.

Эта библиотека предоставляет:
- Компоненты для защищённых маршрутов (`ProtectedRoute`, `GuestRoute`)
- Hooks для работы с аутентификацией (`use_auth`, `use_token`, `use_tenant`)
- Native-first API boundary для auth operations (`signIn`, `signUp`, `signOut`, `refresh`, `forgot password`, `current user`)
- LocalStorage helpers для сохранения сессии

## Взаимодействие

- `apps/admin` — использует для аутентификации
- `apps/storefront` — использует для аутентификации
- `crates/leptos-graphql` — GraphQL fallback transport layer
- `apps/server` — server functions на `/api/fn/*` и GraphQL mutations/queries на `/api/graphql`

### Transport contract

1. Native Leptos UI сначала вызывает local API / `#[server]` path.
2. Если native path недоступен, Leptos откатывается к GraphQL transport.
3. GraphQL остаётся обязательным параллельным контрактом для headless/Next.js и migration fallback.

## Структура

```
src/
├── lib.rs          ← Public API, типы (AuthUser, AuthSession, AuthError)
├── api.rs          ← GraphQL mutations & queries (signIn, signUp, signOut, me)
├── context.rs      ← AuthProvider component, AuthContext
├── hooks.rs        ← use_auth(), use_token(), use_tenant(), etc.
├── storage.rs      ← LocalStorage helpers
└── components.rs   ← ProtectedRoute, GuestRoute, RequireAuth
```

## Использование

### 1. Обернуть приложение в AuthProvider

```rust
// apps/admin/src/app.rs
use leptos_auth::AuthProvider;

#[component]
pub fn App() -> impl IntoView {
    view! {
        <AuthProvider>
            <Router>
                {/* routes */}
            </Router>
        </AuthProvider>
    }
}
```

### 2. Login page

```rust
use leptos::*;
use leptos_auth::{use_auth};

#[component]
pub fn Login() -> impl IntoView {
    let auth = use_auth();
    let (email, set_email) = create_signal(String::new());
    let (password, set_password) = create_signal(String::new());
    let (error, set_error) = create_signal(None::<String>);
    
    let login_action = create_action(|(email, password, tenant): &(String, String, String)| {
        let email = email.clone();
        let password = password.clone();
        let tenant = tenant.clone();
        
        async move {
            match auth.sign_in(email, password, tenant).await {
                Ok(_) => {
                    use leptos_router::use_navigate;
                    let navigate = use_navigate();
                    navigate("/dashboard", Default::default());
                }
                Err(e) => {
                    set_error.set(Some(format!("Login failed: {:?}", e)));
                }
            }
        }
    });
    
    view! {
        <form on:submit=move |ev| {
            ev.prevent_default();
            login_action.dispatch((
                email.get(),
                password.get(),
                "demo".to_string(),
            ));
        }>
            <input
                type="email"
                placeholder="Email"
                prop:value=email
                on:input=move |ev| set_email.set(event_target_value(&ev))
            />
            <input
                type="password"
                placeholder="Password"
                prop:value=password
                on:input=move |ev| set_password.set(event_target_value(&ev))
            />
            <button type="submit" disabled=move || login_action.pending().get()>
                {move || if login_action.pending().get() { "Logging in..." } else { "Login" }}
            </button>
            
            {move || error.get().map(|e| view! { <p class="error">{e}</p> })}
        </form>
    }
}
```

### 3. Protected routes

```rust
use leptos::*;
use leptos_router::*;
use leptos_auth::ProtectedRoute;

#[component]
pub fn App() -> impl IntoView {
    view! {
        <AuthProvider>
            <Router>
                <Routes>
                    <Route path="/login" view=Login />
                    <Route path="/register" view=Register />
                    
                    {/* Protected routes */}
                    <ParentRoute path="" view=ProtectedRoute>
                        <Route path="/dashboard" view=Dashboard />
                        <Route path="/profile" view=Profile />
                    </ParentRoute>
                </Routes>
            </Router>
        </AuthProvider>
    }
}
```

### 4. Use auth hooks

```rust
use leptos::*;
use leptos_auth::{use_current_user, use_is_authenticated};

#[component]
pub fn Dashboard() -> impl IntoView {
    let is_authenticated = use_is_authenticated();
    let current_user = use_current_user();
    
    view! {
        <div>
            <h1>"Dashboard"</h1>
            {move || {
                if is_authenticated.get() {
                    if let Some(user) = current_user.get() {
                        view! { <p>"Welcome, " {user.email} "!"</p> }.into_view()
                    } else {
                        view! { <p>"Loading user..."</p> }.into_view()
                    }
                } else {
                    view! { <p>"Not authenticated"</p> }.into_view()
                }
            }}
        </div>
    }
}
```

### 5. Logout

```rust
use leptos::*;
use leptos_auth::use_auth;

#[component]
pub fn LogoutButton() -> impl IntoView {
    let auth = use_auth();
    
    let logout_action = create_action(|_| async move {
        match auth.sign_out().await {
            Ok(_) => {
                use leptos_router::use_navigate;
                let navigate = use_navigate();
                navigate("/login", Default::default());
            }
            Err(e) => {
                log::error!("Logout failed: {:?}", e);
            }
        }
    });
    
    view! {
        <button on:click=move |_| logout_action.dispatch(())>
            "Logout"
        </button>
    }
}
```

## API Reference

### GraphQL Mutations

#### `signIn(input: SignInInput!): AuthPayload!`

Login with email and password.

**GraphQL:**
```graphql
mutation SignIn($input: SignInInput!) {
    signIn(input: $input) {
        accessToken
        refreshToken
        tokenType
        expiresIn
        user {
            id
            email
            name
            role
            status
        }
    }
}
```

**Rust API:**
```rust
use leptos_auth::api;

let (user, session) = api::sign_in(
    "admin@local".to_string(),
    "admin12345".to_string(),
    "demo".to_string(),
).await?;
```

---

#### `signUp(input: SignUpInput!): AuthPayload!`

Register new user.

**GraphQL:**
```graphql
mutation SignUp($input: SignUpInput!) {
    signUp(input: $input) {
        accessToken
        refreshToken
        user {
            id
            email
            name
        }
    }
}
```

**Rust API:**
```rust
use leptos_auth::api;

let (user, session) = api::sign_up(
    "user@example.com".to_string(),
    "password123".to_string(),
    Some("John Doe".to_string()),
    "demo".to_string(),
).await?;
```

---

#### `signOut: SignOutPayload!`

Logout (invalidate session).

**GraphQL:**
```graphql
mutation SignOut {
    signOut {
        success
    }
}
```

**Rust API:**
```rust
use leptos_auth::api;

api::sign_out(
    session.token.clone(),
    "demo".to_string(),
).await?;
```

---

#### `refreshToken(input: RefreshTokenInput!): AuthPayload!`

Refresh access token.

**GraphQL:**
```graphql
mutation RefreshToken($input: RefreshTokenInput!) {
    refreshToken(input: $input) {
        accessToken
        refreshToken
        user {
            id
            email
        }
    }
}
```

**Rust API:**
```rust
use leptos_auth::api;

let new_session = api::refresh_token(
    old_refresh_token,
    "demo".to_string(),
).await?;
```

---

#### `forgotPassword(input: ForgotPasswordInput!): ForgotPasswordPayload!`

Request password reset.

**GraphQL:**
```graphql
mutation ForgotPassword($input: ForgotPasswordInput!) {
    forgotPassword(input: $input) {
        success
        message
    }
}
```

**Rust API:**
```rust
use leptos_auth::api;

let message = api::forgot_password(
    "user@example.com".to_string(),
    "demo".to_string(),
).await?;
```

---

### GraphQL Queries

#### `me: User`

Get current authenticated user.

**GraphQL:**
```graphql
query CurrentUser {
    me {
        id
        email
        name
        role
        status
    }
}
```

**Rust API:**
```rust
use leptos_auth::api;

let user = api::fetch_current_user(
    session.token.clone(),
    "demo".to_string(),
).await?;
```

---

### Hooks

#### `use_auth() -> AuthContext`

Get auth context (includes all methods and signals).

**Example:**
```rust
let auth = use_auth();
auth.sign_in(email, password, tenant).await?;
auth.sign_out().await?;
```

---

#### `use_current_user() -> Signal<Option<AuthUser>>`

Get current user signal.

---

#### `use_is_authenticated() -> Signal<bool>`

Check if user is authenticated.

---

#### `use_is_loading() -> Signal<bool>`

Check if auth is loading (initial check).

---

#### `use_token() -> Signal<Option<String>>`

Get current access token.

---

#### `use_tenant() -> Signal<Option<String>>`

Get current tenant slug.

---

### Components

#### `<ProtectedRoute />`

Wraps routes that require authentication. Redirects to `/login` if not authenticated.

**Props:**
- `children: Children` — child routes/components
- `redirect_path: Option<String>` — redirect path if not authenticated (default: `/login`)

---

#### `<GuestRoute />`

Wraps routes for guests only (e.g., login, register). Redirects to `/dashboard` if authenticated.

**Props:**
- `children: Children` — child routes/components
- `redirect_path: Option<String>` — redirect path if authenticated (default: `/dashboard`)

---

#### `<RequireAuth />`

Conditionally render content if authenticated.

**Props:**
- `children: Children` — content to show if authenticated
- `fallback: Option<View>` — fallback content if not authenticated

**Example:**
```rust
<RequireAuth fallback=move || view! { <p>"Please sign in"</p> }>
    <p>"Secret content"</p>
</RequireAuth>
```

---

## Types

### `AuthUser`

```rust
pub struct AuthUser {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
}
```

### `AuthSession`

```rust
pub struct AuthSession {
    pub token: String,
    pub tenant: String,
}
```

### `AuthError`

```rust
pub enum AuthError {
    Unauthorized,
    InvalidCredentials,
    Network,
    Http(u16),
}
```

---

## Environment Variables

### WASM (Browser)

**`window.location.origin`** — используется как API base URL (auto-detected)

Результирующий GraphQL endpoint: `${origin}/api/graphql`

### SSR (Server)

**`RUSTOK_API_URL`** — API base URL (default: `http://localhost:5150`)

**Example:**
```bash
RUSTOK_API_URL=http://localhost:5150
```

Результирующий GraphQL endpoint: `${RUSTOK_API_URL}/api/graphql`

---

## Testing

### Unit Tests

```bash
cargo test -p leptos-auth
```

### Integration Testing (Manual)

```bash
# 1. Start server
cd apps/server && cargo run

# 2. Test GraphQL mutation via curl
curl -X POST http://localhost:5150/api/graphql \
  -H "Content-Type: application/json" \
  -H "X-Tenant-Slug: demo" \
  -d '{
    "query": "mutation SignIn($input: SignInInput!) { signIn(input: $input) { accessToken user { email } } }",
    "variables": {
      "input": {
        "email": "admin@local",
        "password": "admin12345"
      }
    }
  }'

# Expected response:
{
  "data": {
    "signIn": {
      "accessToken": "eyJ...",
      "user": {
        "email": "admin@local"
      }
    }
  }
}
```

---

## Dependencies

- `leptos` — reactive framework
- `leptos_router` — routing
- `leptos-graphql` — GraphQL transport layer (HTTP client)
- `serde`, `serde_json` — serialization
- `gloo-storage` — LocalStorage wrapper
- `web-sys` — WASM window.location (только для auto-detect URL)

---

## Implementation Notes

### Why GraphQL for Auth?

**Consistency:** Все через один endpoint `/api/graphql`.

**Type Safety:** GraphQL schema гарантирует type-safe queries.

**Performance:** DataLoader для efficient queries, batch requests.

**Developer Experience:** Один клиент (leptos-graphql) вместо двух (REST + GraphQL).

**⚠️ Avoid Mixing:** Смешивать REST и GraphQL — плохая практика! Выбирайте один подход.

---

## Backend GraphQL Schema

На сервере (`apps/server/src/graphql/auth/`) реализованы:

### Mutations

```graphql
type Mutation {
    signIn(input: SignInInput!): AuthPayload!
    signUp(input: SignUpInput!): AuthPayload!
    signOut: SignOutPayload!
    refreshToken(input: RefreshTokenInput!): AuthPayload!
    forgotPassword(input: ForgotPasswordInput!): ForgotPasswordPayload!
    resetPassword(input: ResetPasswordInput!): ResetPasswordPayload!
}
```

### Input Types

```graphql
input SignInInput {
    email: String!
    password: String!
}

input SignUpInput {
    email: String!
    password: String!
    name: String
}

input RefreshTokenInput {
    refreshToken: String!
}

input ForgotPasswordInput {
    email: String!
}

input ResetPasswordInput {
    token: String!
    newPassword: String!
}
```

### Response Types

```graphql
type AuthPayload {
    accessToken: String!
    refreshToken: String!
    tokenType: String!
    expiresIn: Int!
    user: AuthUser!
}

type AuthUser {
    id: String!
    email: String!
    name: String
    role: String!
    status: String!
}

type SignOutPayload {
    success: Boolean!
}

type ForgotPasswordPayload {
    success: Boolean!
    message: String!
}
```

### Queries

```graphql
type Query {
    me: User
    authHealth: String!
}
```

---

## Troubleshooting

### "Network error" on login

**Check:**
1. Server is running: `curl http://localhost:5150/api/health`
2. GraphQL endpoint accessible: `curl http://localhost:5150/api/graphql -d '{"query":"query{health}"}'`
3. Tenant header is correct: `X-Tenant-Slug: demo`

### "Mutation not found"

**Check:**
1. Backend GraphQL schema includes auth mutations
2. Server recompiled after adding auth module
3. GraphQL introspection shows mutations: `curl http://localhost:5150/api/graphql -d '{"query":"{ __schema { mutationType { fields { name } } } }"}'`

### "Unauthorized" error

**Check:**
1. Credentials are correct: `admin@local` / `admin12345`
2. Token is not expired
3. Token is in GraphQL context (automatic via leptos-graphql)

---

## Roadmap

- [x] GraphQL mutations (signIn, signUp, signOut, refreshToken)
- [x] Auth context & hooks
- [x] Protected routes
- [x] LocalStorage persistence
- [ ] Token auto-refresh on expiry
- [ ] Password reset flow (complete implementation)
- [ ] Email verification flow
- [ ] 2FA support
- [ ] SSR support (server-side auth)

---

## Status

✅ **Ready to use** (GraphQL implementation)

**Last updated:** 2026-02-13  
**Version:** 0.1.0

---

## ⚠️ Important: REST API Deprecated

**Previous versions** used REST API (`/api/auth/login`, `/api/auth/register`) — **this is deprecated**.

**Current version** uses **only GraphQL** (`/api/graphql`).

**Why?** Mixing REST and GraphQL is bad practice. Choose one approach and stick to it.

If you need REST API for other clients (mobile apps, etc.), use a separate service or reverse proxy, but **admin panel should use GraphQL only**.
