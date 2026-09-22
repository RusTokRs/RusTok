---
id: doc://docs/research/flutter-ffa-architecture-article.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# FFA for Flutter: Why a Platform Product Needs More Than "Ordinary" Mobile Architecture

> Article-note for external publication in Medium style. The material explains
> why the RusTok Flutter client is developed as a Fluid Frontend Architecture
> host, rather than a standalone application with a set of screens.

## In Short

For a small Flutter application, a typical `lib/features/*` structure is often
sufficient: there are screens, services, a common router and a few API clients.
But for a platform like RusTok, this is not enough.

RusTok already lives as a modular platform: there is a backend composition root,
platform modules, several frontend hosts, storefront/admin surfaces,
GraphQL/headless contracts, module manifests and generated registries. Therefore,
Flutter here should not become a "third independent frontend." It should
be another host client of the platform.

This is exactly what FFA — Fluid Frontend Architecture — provides.

FFA for Flutter is not an attempt to use Leptos `#[server]` functions from
Dart, nor a way to reuse Rust UI components. It is a way to maintain one
product contract across different frontend runtimes:

- Leptos can live close to Rust server functions.
- Next.js can be a headless web host.
- Dioxus can become a Rust UI runtime.
- Flutter remains a Dart/mobile runtime.
- But module ownership, routes, permissions, locale, tenant context and backend
  contracts do not diverge.

## What Would Be in an Ordinary Flutter Architecture

A typical Flutter project often looks like this:

```text
lib/
  main.dart
  router.dart
  features/
    auth/
    catalog/
    cart/
    profile/
    modules/
  services/
    graphql_client.dart
    cart_service.dart
    auth_service.dart
  widgets/
  models/
```

This is a normal scheme for a single product. It is simple, fast to start and well
understood by the team.

The problem appears when the product is not a single application but a platform:

- a web storefront already exists;
- an admin web already exists;
- a Next-host is evolving in parallel;
- backend modules have their own manifests;
- UI surfaces belong to modules;
- tenant, locale, auth, routing and permissions are platform
  contracts;
- GraphQL/REST/WS must be the canonical headless boundary.

In such an environment, an ordinary mobile architecture quickly starts living its own life.
The catalog feature creates its own API client. The cart feature creates its own storage.
The profile feature creates its own locale fallback. The router gets a manual list
of modules. Within months, the mobile app is no longer just a different UI but a different
product.

## What FFA Changes

FFA proposes a different view:

```text
backend/platform
  owns canonical behavior and contracts

host app
  owns shell, routing, auth, tenant, locale, transport, storage, registry wiring

module-owned package
  owns screens, widgets, UI states, forms and user intents
```

For Flutter, this translates to:

```text
rustok_mobile/
  apps/
    rustok_admin_mobile/       # admin/operator host
    rustok_frontend_mobile/    # customer storefront host
  packages/
    app_graphql/               # shared transport
    app_route_contracts/       # typed route/query contracts
    app_module_contracts/      # module mounting contracts
    rustok_catalog_mobile/     # module-owned storefront UI
    rustok_modules_mobile/     # module-owned admin/operator UI
```

A Flutter package can own a product card, a cart screen, an empty state,
a loading state and a user intent like "add to cart." But it should not own the
GraphQL client, tenant resolver, locale fallback or durable cart storage.

The host receives an intent from the package and executes it through the canonical backend
contract.

## Example: Cart/Catalog

An ordinary implementation might take the shortcut:

```text
CatalogScreen -> CartService -> /mobile/cart/add -> local cart storage
```

This is fast. But it creates a mobile-only API, mobile-only storage and a separate
cart flow semantics.

In the FFA variant, the flow looks different:

```text
rustok_catalog_mobile
  owns:
    - ProductCard
    - CartLineTile
    - EmptyCartSurface
    - intents: add, start, update, remove

rustok_frontend_mobile
  owns:
    - GraphQL client
    - tenant/locale/auth headers
    - StorefrontCartIdStore
    - canonical cart mutations

backend
  owns:
    - storefrontCart
    - createStorefrontCart
    - addStorefrontCartLineItem
    - updateStorefrontCartLineItem
    - removeStorefrontCartLineItem
```

That is, the module package says: "the user wants to add a product." The host
decides which cart id to use, which headers to send, which canonical
GraphQL mutation to call and where to store the cart id.

This is an important difference. The package remains a UI package, not a small application
inside an application.

## Why Such an Architecture Is Better for RusTok

### 1. It reduces UI drift

When Leptos, Next and Flutter evolve independently, they quickly start to
diverge:

- different empty states;
- different action labels;
- different permission gates;
- different route semantics;
- different locale keys;
- different errors and loading states.

FFA says: layout can differ, but the product contract must be one.
Mobile is not required to copy desktop pixel-perfect. But it must preserve the same
entities, constraints, actions, permissions and states.

### 2. It protects against Flutter-only API

The most tempting path for mobile is to ask the backend for a convenient
endpoint:

```text
/mobile/catalog
/mobile/cart/add
/mobile/me
/mobile/modules
```

Initially, this speeds up development. Within a year, it becomes a set of
parallel backend contracts that need separate maintenance.

FFA requires: if a contract is needed by the product, it must be a platform-level
contract, not a Flutter-only shortcut.

### 3. It preserves module ownership

In a platform product, the module should own its UI surface. The host should not
become the place where all domain-specific presentation logic lives.

In FFA, the host mounts surfaces but does not take ownership. This is especially important for
RusTok, where modules already have manifests, route segments, permissions and UI
classification.

### 4. It makes module connection declarative

Without FFA, a new module often means a manual list of changes:

1. add a screen;
2. add a route;
3. add a nav item;
4. add a permission check;
5. add locale keys;
6. add a deep link;
7. do not forget parity with web.

With FFA, the path is different:

```text
rustok-module.toml
  -> mobile manifest snapshot
  -> generated Dart registry
  -> host registry adapter
  -> mounted module-owned package
```

This does not eliminate all work but turns module connection into a verifiable
contractual process.

### 5. It centralizes tenant, locale, auth and storage

Tenant, locale, auth/session and cart storage are not details of a specific feature.
They are platform context.

If each feature starts selecting its own locale, reading the tenant from its own place, and
storing the cart id in its own way, the application becomes unpredictable. FFA maintains these
rules at the host/runtime level.

### 6. It makes architectural drift verifiable

FFA is not just principles but also artifacts:

- generated registry;
- manifest snapshots;
- codegen freshness checks;
- route contract tests;
- package boundary tests;
- documentation evidence blocks;
- readiness boards.

This translates architectural discipline from verbal code review into a verifiable
workflow.

## What Problems FFA Solves

### UI drift

Flutter does not become a separate UX product. It remains the mobile expression of the
same product contract.

### Transport drift

Feature packages do not create their own GraphQL clients, headers, retry policies,
locale chains and auth refresh logic.

### Ownership drift

The host remains a host. The module package remains a module package. The backend remains
the source of canonical behavior.

### API drift

Mobile-only shortcuts do not turn into a second backend contract.

### Routing drift

Route semantics and query keys remain part of the platform contract, not a local
agreement of a specific Flutter router.

### Locale drift

Effective locale is chosen by the host/runtime layer and propagated to UI surfaces.
Module packages do not invent their own fallback chains.

### Registry drift

The list of available module surfaces comes through manifest/codegen, not through a manual
list of screens in the mobile host.

## Disadvantages

FFA is not free.

### 1. More complexity at startup

For a small application, this is overengineering. Instead of a single `CartService`,
there are repository boundaries, host implementation, DTOs, GraphQL operations,
cart id stores, tests and docs.

### 2. More boilerplate

Even a simple user action may go through several layers:

```text
Widget -> repository interface -> host adapter -> GraphQL client -> backend
```

This takes longer than calling a service directly from a feature.

### 3. Slower for quick experiments

FFA restricts quick shortcuts. You cannot simply create a Flutter-only endpoint
or save a cart id in package-local storage if it breaks the platform contract.

### 4. Requires team discipline

The architecture only works if the team understands ownership boundaries:

- what is host-owned;
- what is module-owned;
- what is backend-owned;
- what is shared;
- what cannot be placed in a package;
- when docs and manifests need updating.

Without this discipline, FFA becomes a set of abstractions without benefit.

### 5. Risk of excessive modularity

FFA does not mean "create a package for every button." Boundaries should reflect
ownership, not the desire to divide everything into the maximum number of directories.

### 6. Harder debugging

A bug in the cart flow may pass through a widget, provider, repository boundary,
host storage, GraphQL client, request context and backend resolver. The stack is longer
than in a typical mobile app.

### 7. Flutter does not get Rust runtime benefits

For Leptos/Dioxus, part of the benefit may be at the runtime level: Rust components,
server functions, closer to the service layer. Flutter does not get this. For Flutter,
FFA is about contract convergence, not code/runtime convergence.

## When to Use FFA

FFA is a good fit if you have:

| Condition | Why It Matters |
|---|---|
| Multiple frontend hosts | Need to maintain parity |
| Modular platform | Need to preserve module ownership |
| Headless/mobile clients | Need a canonical API contract |
| Manifest/codegen | Need declarative mounting |
| Admin and storefront | Cannot mix UX and RBAC |
| Long product lifespan | Drift will be expensive |
| Multiple teams | Need verifiable boundaries |

If the application is small, there is one team, web parity is not needed and there are no modules,
an ordinary Flutter architecture may be better.

## Conclusion

FFA for Flutter is a more advanced architecture not in the sense of "simpler" but in the sense of
"more resilient to platform growth."

Ordinary Flutter architecture optimizes the development speed of a single application.
FFA Flutter architecture optimizes platform integrity, modularity and
compatibility across multiple frontend hosts.

For RusTok, this is a justified trade-off: we pay with complexity at the start but
gain protection against UI/API/transport/routing drift, preserve module ownership and
keep Flutter as part of the platform, not a separate product.

Short formula:

> Flutter in FFA is not a separate mobile app. It is a mobile host of the platform
> that expresses the same product contract in a different runtime.
