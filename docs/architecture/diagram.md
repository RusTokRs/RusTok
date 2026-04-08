# Диаграммы платформы

Этот документ содержит только актуальные верхнеуровневые диаграммы RusToK.
Детали ownership, manifests и local docs описаны в `docs/modules/*` и
`docs/architecture/*`.

## Общая схема платформы

```mermaid
graph TD
    subgraph Hosts["Host applications"]
        SERVER["apps/server"]
        ADMIN["apps/admin"]
        STOREFRONT["apps/storefront"]
        NEXT_ADMIN["apps/next-admin"]
        NEXT_FRONT["apps/next-frontend"]
    end

    subgraph Core["Core modules"]
        AUTH["auth"]
        CACHE["cache"]
        CHANNEL["channel"]
        EMAIL["email"]
        INDEX["index"]
        SEARCH["search"]
        OUTBOX["outbox"]
        TENANT["tenant"]
        RBAC["rbac"]
    end

    subgraph Optional["Optional modules"]
        CONTENT["content"]
        CART["cart"]
        CUSTOMER["customer"]
        PRODUCT["product"]
        PROFILES["profiles"]
        REGION["region"]
        PRICING["pricing"]
        INVENTORY["inventory"]
        ORDER["order"]
        PAYMENT["payment"]
        FULFILLMENT["fulfillment"]
        COMMERCE["commerce"]
        BLOG["blog"]
        FORUM["forum"]
        COMMENTS["comments"]
        PAGES["pages"]
        TAXONOMY["taxonomy"]
        MEDIA["media"]
        WORKFLOW["workflow"]
    end

    subgraph Support["Shared / capability crates"]
        CORE["rustok-core"]
        API["rustok-api"]
        EVENTS["rustok-events"]
        STORAGE["rustok-storage"]
        TESTS["rustok-test-utils"]
        COM_FOUND["rustok-commerce-foundation"]
        TELEMETRY["rustok-telemetry"]
        IGGY["rustok-iggy"]
        MCP["rustok-mcp"]
        AI["rustok-ai"]
        ALLOY["alloy"]
        FLEX["flex"]
    end

    SERVER --> AUTH
    SERVER --> CACHE
    SERVER --> CHANNEL
    SERVER --> EMAIL
    SERVER --> INDEX
    SERVER --> SEARCH
    SERVER --> OUTBOX
    SERVER --> TENANT
    SERVER --> RBAC
    SERVER --> CONTENT
    SERVER --> CART
    SERVER --> CUSTOMER
    SERVER --> PRODUCT
    SERVER --> PROFILES
    SERVER --> REGION
    SERVER --> PRICING
    SERVER --> INVENTORY
    SERVER --> ORDER
    SERVER --> PAYMENT
    SERVER --> FULFILLMENT
    SERVER --> COMMERCE
    SERVER --> BLOG
    SERVER --> FORUM
    SERVER --> COMMENTS
    SERVER --> PAGES
    SERVER --> TAXONOMY
    SERVER --> MEDIA
    SERVER --> WORKFLOW

    SERVER --> CORE
    SERVER --> API
    SERVER --> EVENTS
    SERVER --> STORAGE
    SERVER --> TELEMETRY
    SERVER --> IGGY
    SERVER --> MCP
    SERVER --> AI
    SERVER --> ALLOY
    SERVER --> FLEX

    ADMIN --> SERVER
    STOREFRONT --> SERVER
    NEXT_ADMIN --> SERVER
    NEXT_FRONT --> SERVER

    COMMERCE --> CART
    COMMERCE --> CUSTOMER
    COMMERCE --> PRODUCT
    COMMERCE --> REGION
    COMMERCE --> PRICING
    COMMERCE --> INVENTORY
    COMMERCE --> ORDER
    COMMERCE --> PAYMENT
    COMMERCE --> FULFILLMENT

    BLOG --> CONTENT
    BLOG --> COMMENTS
    BLOG --> TAXONOMY
    FORUM --> CONTENT
    FORUM --> TAXONOMY
    PAGES --> CONTENT
    PRODUCT --> TAXONOMY
    PRODUCT --> COM_FOUND
    PRICING --> COM_FOUND
    INVENTORY --> COM_FOUND
    MEDIA --> STORAGE
    OUTBOX --> EVENTS
    OUTBOX --> IGGY
```

## Runtime-композиция

```mermaid
flowchart TD
    MANIFEST["modules.toml"] --> REGISTRY["ModuleRegistry"]
    MANIFEST --> VALIDATE["manifest/runtime validation"]
    VALIDATE --> SERVER["apps/server"]

    SERVER --> GRAPHQL["GraphQL"]
    SERVER --> REST["REST"]
    SERVER --> SERVER_FN["Leptos #[server] functions"]
    SERVER --> HEALTH["health / metrics / ops"]

    SERVER --> MODULES["platform modules"]
    SERVER --> SUPPORT["shared/support/capability crates"]

    MODULES --> OUTBOX["transactional outbox"]
    OUTBOX --> EVENTS["event flow"]
    EVENTS --> INDEX["read-side / indexing"]
```

## UI-композиция

```mermaid
graph LR
    subgraph Module["Module-owned UI"]
        ADMIN_UI["admin/ sub-crate"]
        STORE_UI["storefront/ sub-crate"]
        DOCS["README.md + docs/README.md"]
        MODULE_MANIFEST["rustok-module.toml"]
    end

    subgraph Hosts["Hosts"]
        ADMIN["apps/admin"]
        STOREFRONT["apps/storefront"]
        NEXT_ADMIN["apps/next-admin"]
        NEXT_FRONT["apps/next-frontend"]
    end

    MODULE_MANIFEST --> ADMIN_UI
    MODULE_MANIFEST --> STORE_UI
    DOCS --> MODULE_MANIFEST

    ADMIN --> ADMIN_UI
    STOREFRONT --> STORE_UI
    NEXT_ADMIN --> ADMIN_UI
    NEXT_FRONT --> STORE_UI
```

## Поток write / event / read

```mermaid
sequenceDiagram
    participant Client as Client
    participant Host as apps/server
    participant Module as Module service
    participant DB as Write model
    participant Outbox as rustok-outbox
    participant Consumer as Consumers / indexers
    participant Read as Read model

    Client->>Host: request
    Host->>Module: validated call
    Module->>DB: write transaction
    Module->>Outbox: publish_in_tx(...)
    Outbox->>DB: persist sys_events
    DB-->>Host: committed result
    Outbox-->>Consumer: domain event
    Consumer->>Read: update projections / indexes
    Host-->>Client: response
```

## Tenant lifecycle

```mermaid
stateDiagram-v2
    [*] --> PlatformComposition
    PlatformComposition --> OptionalEnabled: tenant enables optional module
    OptionalEnabled --> OptionalDisabled: tenant disables optional module
    OptionalDisabled --> OptionalEnabled: tenant re-enables module

    note right of PlatformComposition
        Core modules are always present
        Capability crates are not tenant-toggled modules
    end note
```

## Связанные документы

- [Обзор архитектуры платформы](./overview.md)
- [Архитектура модулей](./modules.md)
- [Обзор модульной платформы](../modules/overview.md)
- [Реестр модулей и приложений](../modules/registry.md)
- [Контракт `rustok-module.toml`](../modules/manifest.md)
