<div align="center">

# <img src="assets/rustok-logo-512x512.png" width="72" align="center" /> RusTok

**Событийная модульная платформа на Rust**

*Один репозиторий для сервера, интегрированных Leptos host-приложений и headless/экспериментальных Next.js host-приложений.*

[![CI](https://github.com/RustokCMS/RusToK/actions/workflows/ci.yml/badge.svg)](https://github.com/RustokCMS/RusToK/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](https://www.rust-lang.org)
[![PRs Welcome][def]](CONTRIBUTING.md)

**[English version](README.md)** | **[РљСЂР°С‚РєР°СЏ СЃРїСЂР°РІРєР° РїРѕ РїР»Р°С‚С„РѕСЂРјРµ](PLATFORM_INFO_RU.md)**

</div>

RusToK сейчас представляет собой Rust-first modular monolith для мультитенантных продуктов, где сочетаются контент, commerce, workflow и интеграции. Текущий центр платформы — `apps/server` как composition root, сборка модулей через manifest, событийное разделение путей записи и чтения и две стратегии UI-host'ов: Leptos как основной интегрированный путь и Next.js как headless или экспериментальный контур.

<a id="table-of-contents"></a>

## Оглавление

- [Обзор](#overview)
- [Возможности](#features)
- [Производительность и экономия](#performance-and-economy)
- [Почему Rust](#why-rust)
- [AI-Native Architecture](#ai-native-architecture)
- [Сравнение](#comparison)
- [Снимок архитектуры](#architecture-snapshot)
  - [Приложения](#applications)
  - [Таксономия модулей](#module-taxonomy)
- [Система модулей](#module-system)
- [Быстрый старт](#quick-start)
- [Документация](#documentation)
- [Разработка](#development)
- [Текущий фокус](#current-focus)
- [Благодарности](#acknowledgments)
- [Лицензия](#license)

<a id="overview"></a>

## Обзор

Текущие сильные стороны платформы:

- Сборка через manifest от [`modules.toml`](modules.toml) до рантайма и host-приложений.
- Явные границы между `Core`, `Optional` и capability/support crates.
- Гибридная API-модель: GraphQL для доменных поверхностей, ориентированных на UI, REST для operational и integration flows, WebSocket там, где нужен live runtime.
- Событийное разделение путей записи и чтения через transactional outbox, `rustok-index` и `rustok-search`.
- Две стратегии UI-host'ов: `apps/admin` и `apps/storefront` как интегрированные Leptos hosts, `apps/next-admin` и `apps/next-frontend` как headless/экспериментальные контуры.

Корневой README намеренно короткий. Его задача — дать точку входа в репозиторий, а не заменить полную архитектурную спецификацию.

<a id="features"></a>

## Возможности

### Core Platform

- Мультитенантный runtime с tenant-aware контрактами
- Гибридная API-модель с GraphQL, REST и WebSocket там, где это нужно
- Manifest-driven composition модулей и per-tenant enablement
- Событийное разделение путей записи и чтения через transactional outbox
- Встроенные основы для локализации, observability и RBAC

### Режимы деплоя

| Режим | Как работает | Аутентификация | Сценарий |
|------|-------------|----------------|----------|
| **Монолит** | Сервер плюс интегрированные Leptos admin/storefront hosts | Серверные сессии и общий runtime context | Self-hosted сайт, встроенный backoffice и storefront |
| **Headless** | `apps/server` отдаёт API, а frontend живёт отдельно | OAuth2, sessions или смешанный контракт в зависимости от клиента | Мобильные приложения, внешние фронтенды, интеграции |
| **Смешанный** | Интегрированные Leptos hosts и внешние клиенты поверх одного рантайма | Оба | Встроенная админка плюс внешние приложения и интеграции |

### Матрица возможностей

| Возможность | WordPress | Shopify | Strapi | Ghost | **RusToK** |
|---|---|---|---|---|---|
| Монолитный деплой | да | нет | нет | да | **да** |
| Headless API surface | частично | да | да | частично | **да** |
| Смешанный integrated + headless режим | костыли | частично | частично | ограниченно | **да** |
| Мультитенантный runtime | multisite | ограниченно | нет | нет | **нативно** |
| Compile-time composition модулей | нет | нет | нет | нет | **да** |
| Rust-first integrated UI path | нет | нет | нет | нет | **да** |

### Developer Experience

- Loco.rs как foundation для общего server runtime
- Rust crates как явные границы модулей
- Module-owned transport и UI slices вместо giant central app
- Живая документация, индексируемая из `docs/index.md`

### Тестирование и качество

- Workspace-wide Rust test flow через `cargo nextest`
- Проверки manifest и dependency hygiene через `cargo machete`
- Планы верификации платформы для architecture, frontend и quality контуров

### Наблюдаемость и безопасность

- Prometheus-style метрики и tracing stack
- Typed RBAC и permission-aware runtime contracts
- Tenant-aware request context и channel-aware request flow
- Общие validation и outbox/event-runtime guardrails

<a id="performance-and-economy"></a>

## Производительность и экономия

Точные числа зависят от deployment profile и состава модулей, но позиционирование платформы по-прежнему строится вокруг эффективности compiled runtime и денормализованных read paths.

### Бенчмарки (симулированные)

| Метрики | WordPress | Strapi | RusToK |
|---------|-----------|--------|--------|
| **Req/sec** | 60 | 800 | **45,000+** |
| **P99 Latency**| 450ms | 120ms | **8ms** |
| **Cold Boot** | N/A | 8.5s | **0.05s** |

<a id="why-rust"></a>

## Почему Rust

### Проблемы с текущими CMS-решениями

| Проблема | WordPress | Node.js CMS | RusToK |
|----------|-----------|-------------|--------|
| **Runtime Errors** | Fatal errors крашат сайт | Неотловленные исключения | Гарантии времени компиляции |
| **Memory Leaks** | Частые с плагинами | GC паузы и раздувание памяти | Модель владения предотвращает |
| **Безопасность** | Большая поверхность атак через плагины | npm supply-chain риски | Скомпилированные и аудируемые зависимости |
| **Масштабирование** | Требуются внешние слои кеширования | В основном горизонтальное | Вертикальное и горизонтальное |

### Преимущество Rust

```rust
let product = Product::find_by_id(db, product_id)
    .await?
    .ok_or(Error::NotFound)?;
```

Даже после переработки архитектуры базовая ценность остаётся той же:

- больше ошибок ловится на этапе компиляции;
- доменные контракты остаются явными между crate-ами;
- runtime-поведение предсказуемо и не зависит от интерпретатора.

<a id="ai-native-architecture"></a>

## AI-Native Architecture

RusToK по-прежнему ориентирован на agent-assisted работу, но в практическом смысле: репозиторий опирается на явные контракты, карту документации, module manifests и предсказуемые component boundaries, а не на магию генераторов.

Практические AI-facing точки входа:

- [Карта документации](docs/index.md)
- [Системный манифест](RUSTOK_MANIFEST.md)
- [Реестр модулей](docs/modules/registry.md)
- [Правила агентов](AGENTS.md)

<a id="comparison"></a>

## Сравнение

### vs. WordPress + WooCommerce

| Аспект | WordPress | RusToK |
|--------|-----------|--------|
| Язык | PHP 7.4+ | Rust |
| Plugin System | Runtime | Compile-time и manifest-driven |
| Type Safety | Нет | Полная |
| Multi-tenant | Multisite | Нативный |
| API | REST | GraphQL + REST |
| Admin UI | PHP templates | Leptos host |

Лучше для: команд, которым нужны более строгие контракты, чем даёт plugin-first PHP stack.

### vs. Strapi (Node.js)

| Аспект | Strapi | RusToK |
|--------|--------|--------|
| Язык | JavaScript/TypeScript | Rust |
| Моделирование контента | UI-based | Code- и module-based |
| Plugin Ecosystem | npm | crates и workspace modules |
| Cold Start | Выше | Ниже |

Лучше для: команд, которым нужна type safety и явное владение доменами.

### vs. Medusa.js (E-commerce)

| Аспект | Medusa | RusToK |
|--------|--------|--------|
| Фокус | Только e-commerce | Commerce плюс content/community/workflow |
| Язык | TypeScript | Rust |
| Архитектура | Microservices encouraged | Модульный монолит |
| Storefront | Next.js templates | Leptos host плюс Next.js companion paths |

Лучше для: команд, которым нужны commerce и non-commerce домены в одной платформе.

### vs. Directus / PayloadCMS

| Аспект | Directus/Payload | RusToK |
|--------|------------------|--------|
| Подход | Database-first | Schema-first и module-first |
| Type Generation | Build step | Нативные Rust types |
| Custom Logic | Hooks (JS) | Rust modules |
| Self-hosted | Да | Да |
| "Full Rust" | Нет | Да |

Лучше для: команд, строящих платформу вокруг Rust-стека.

<a id="architecture-snapshot"></a>

## Снимок архитектуры

<a id="applications"></a>

### РџСЂРёР»РѕР¶РµРЅРёСЏ

| РџСѓС‚СЊ | Р РѕР»СЊ |
|---|---|
| `apps/server` | Composition root, общий HTTP/GraphQL runtime host, wiring auth/session/RBAC, event runtime, проверка manifest |
| `apps/admin` | Основной Leptos admin host |
| `apps/storefront` | Основной Leptos storefront host |
| `apps/next-admin` | Headless или экспериментальный Next.js admin host |
| `apps/next-frontend` | Headless или экспериментальный Next.js storefront host |

<a id="module-taxonomy"></a>

### РўР°РєСЃРѕРЅРѕРјРёСЏ РјРѕРґСѓР»РµР№

`modules.toml` — источник истины по модульному составу платформы.

Core-модули:

`auth` · `cache` · `channel` · `email` · `index` · `search` · `outbox` · `tenant` · `rbac`

Optional-модули:

- Контент и community: `content`, `blog`, `comments`, `forum`, `pages`, `media`, `workflow`
- Commerce family: `cart`, `customer`, `product`, `profiles`, `region`, `pricing`, `inventory`, `order`, `payment`, `fulfillment`, `commerce`

Вспомогательные и capability-crates находятся вне таксономии `Core` / `Optional`:

- Shared/support: `rustok-core`, `rustok-api`, `rustok-events`, `rustok-storage`, `rustok-commerce-foundation`, `rustok-test-utils`, `rustok-telemetry`
- Capability/runtime layers: `rustok-mcp`, `alloy`, `alloy-scripting`, `flex`, `rustok-iggy`, `rustok-iggy-connector`

Ключевые границы доменов:

- `rustok-content` теперь shared helper и orchestration layer. Это больше не product-facing storage или transport owner для `blog`, `forum` и `pages`.
- `rustok-comments` — отдельный generic comments module для классических комментариев вне forum domain.
- Commerce surface разделён на профильные family modules, а `rustok-commerce` работает как umbrella/root module и orchestration layer.
- Channel-aware поведение уже входит в live request/runtime pipeline через `rustok-channel` и общие request-context contracts.

<a id="module-system"></a>

## Система модулей

Текущий модульный поток управляется через manifest:

```text
modules.toml
  -> build.rs генерирует wiring для host-приложений
  -> apps/server проверяет manifest
  -> ModuleRegistry / bootstrap рантайма
  -> per-tenant enablement для optional modules
```

Важные правила:

- Не считать ручную регистрацию маршрутов в `app.rs` основным способом интеграции модулей.
- Host-приложения подключают optional modules через generated contracts, производные от `modules.toml` и module manifests.
- Build composition и tenant enablement — разные уровни:
  - build composition определяет, что попадает в артефакт;
  - tenant enablement определяет, какие optional modules активны для конкретного tenant.
- Leptos hosts уже потребляют module-owned UI packages через manifest-driven wiring.
- Next.js hosts остаются manual/headless entry points и не должны описываться так, будто у них уже есть тот же generated host contract.

Полная карта текущего runtime описана в:

- [Обзоре архитектуры](docs/architecture/overview.md)
- [Реестре модулей](docs/modules/registry.md)
- [Индексе модульной документации](docs/modules/_index.md)
- [Документе про manifest и rebuild lifecycle](docs/modules/manifest.md)

<a id="quick-start"></a>

## Р‘С‹СЃС‚СЂС‹Р№ СЃС‚Р°СЂС‚

Актуальное руководство быстрого старта для локальной разработки находится в [docs/guides/quickstart.md](docs/guides/quickstart.md).

Типовой сценарий:

```bash
./scripts/dev-start.sh
```

Текущий guide покрывает полный локальный стек:

- backend на `http://localhost:5150`
- Next.js admin на `http://localhost:3000`
- Leptos admin на `http://localhost:3001`
- Next.js storefront на `http://localhost:3100`
- Leptos storefront на `http://localhost:3101`

Если нужен не корневой обзор, а контекст конкретного приложения, начинайте с:

- [документации apps/server](apps/server/docs/README.md)
- [документации apps/admin](apps/admin/docs/README.md)
- [документации apps/storefront](apps/storefront/docs/README.md)
- [документации apps/next-admin](apps/next-admin/docs/README.md)
- [документации apps/next-frontend](apps/next-frontend/docs/README.md)

<a id="documentation"></a>

## Документация

Канонические точки входа:

- [Карта документации](docs/index.md)
- [Обзор архитектуры](docs/architecture/overview.md)
- [Реестр модулей и приложений](docs/modules/registry.md)
- [Индекс модульной документации](docs/modules/_index.md)
- [Справочный пакет MCP](docs/references/mcp/README.md)
- [Руководство по тестированию](docs/guides/testing.md)
- [Как писать модуль в RusToK](docs/modules/module-authoring.md)
- [Главный план верификации платформы](docs/verification/PLATFORM_VERIFICATION_PLAN.md)
- [Системный манифест](RUSTOK_MANIFEST.md)
- [Правила агентов](AGENTS.md)

<a id="development"></a>

## Р Р°Р·СЂР°Р±РѕС‚РєР°

Рекомендуемый минимум окружения:

- Rust toolchain из конфигурации репозитория
- PostgreSQL для локального рантайма
- Node.js или Bun для Next.js host-приложений
- `trunk` для Leptos host-приложений

Полезные команды:

```bash
# полный локальный стек
./scripts/dev-start.sh

# Rust tests
cargo nextest run --workspace --all-targets --all-features

# doc-тесты
cargo test --workspace --doc --all-features

# format и lint
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings

# dependency и policy checks
cargo deny check
cargo machete
```

Общие правила для контрибьюторов и агентов описаны в [CONTRIBUTING.md](CONTRIBUTING.md) и [AGENTS.md](AGENTS.md).

<a id="current-focus"></a>

## Текущий фокус

Актуальные приоритеты ведутся в живых platform docs, а не в отдельном root roadmap-файле:

- [Как писать модуль в RusToK](docs/modules/module-authoring.md)
- [Главный план верификации платформы](docs/verification/PLATFORM_VERIFICATION_PLAN.md)
- [Архитектурные решения](DECISIONS/README.md)

Верхнеуровнево текущий кодовый фокус такой:

- держать честные module boundaries по мере роста платформы;
- развивать module-owned transport и UI surfaces, не превращая `apps/server` в доменную свалку;
- сохранять manifest-driven composition для server и Leptos hosts;
- синхронизировать channel-aware, multilingual и event-driven contracts между доменами.

<a id="acknowledgments"></a>

## Благодарности

Платформа опирается на такие open-source основы, как:

- Loco.rs
- Leptos
- SeaORM
- async-graphql
- Axum

<a id="license"></a>

## Р›РёС†РµРЅР·РёСЏ

RusToK СЂР°СЃРїСЂРѕСЃС‚СЂР°РЅСЏРµС‚СЃСЏ РїРѕ [Р»РёС†РµРЅР·РёРё MIT](LICENSE).

RusToK распространяется по [лицензии MIT](LICENSE).


[def]: https://img.shields.io/badge/PRs-welcome-brightgreen.svg
