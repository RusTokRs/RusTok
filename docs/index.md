# RusTok: карта документации

Этот файл является канонической точкой входа в документацию репозитория. С него нужно начинать работу по правилам [AGENTS.md](../AGENTS.md).

Документация в `docs/` описывает платформу целиком. Локальные документы приложений и crate-ов лежат в `apps/*/docs/`, `crates/*/docs/` и `README.md` рядом с кодом.

## Как пользоваться картой

1. Сначала откройте обзор платформы и нужный архитектурный раздел.
2. Для изменений в модульной системе переходите в `docs/modules/*`.
3. Для UI-срезов используйте `docs/UI/*` и локальные docs приложений.
4. Для periodic verification и quality-gates используйте `docs/verification/*` и `docs/guides/*`.
5. Для residual/future scope по platform contracts сверяйтесь с профильными live docs в `docs/architecture/*`, `docs/UI/*` и `apps/*/docs/*`, не смешивая это с periodic verification.
6. Для изменений конкретного модуля сверяйтесь с `docs/modules/registry.md` и локальными docs соответствующего crate.

## Обязательные стартовые документы

- [Обзор платформы](./architecture/overview.md)
- [Архитектурные принципы](./architecture/principles.md)
- [API и surface-контракты](./architecture/api.md)
- [Маршрутизация](./architecture/routing.md)
- [Модульная архитектура](./architecture/modules.md)
- [Карта модулей и владельцев](./modules/registry.md)

## Модульная система

- [Обзор модульной платформы](./modules/overview.md)
- [План и текущее состояние module-system](./modules/module-system-plan.md)
- [Контракт `rustok-module.toml`](./modules/manifest.md)
- [Реестр crate-ов модульной платформы](./modules/crates-registry.md)
- [Индекс документации по модулям](./modules/_index.md)
- [Индекс UI-пакетов модулей](./modules/UI_PACKAGES_INDEX.md)
- [Quickstart по UI-пакетам](./modules/UI_PACKAGES_QUICKSTART.md)

## UI и клиентские поверхности

- [UI README](./UI/README.md)
- [GraphQL и Leptos server functions](./UI/graphql-architecture.md)
- [Storefront](./UI/storefront.md)
- [Быстрый старт для Admin ↔ Server](./UI/admin-server-connection-quickstart.md)
- [Каталог Rust UI-компонентов](./UI/rust-ui-component-catalog.md)
- [Архитектура i18n](./architecture/i18n.md) — request locale chain, shared locale normalization/validation contract, `verify:i18n:ui` + `verify:i18n:contract` gates, storefront locale-prefixed routes, outbound built-in auth email locale contract, manifest-level module UI bundle contract, временно без ecommerce locale alignment

## Архитектура и foundation

- [Диаграмма платформы](./architecture/diagram.md)
- [Database](./architecture/database.md) — live DB/i18n storage contract: `base + translations + optional bodies`, `VARCHAR(32)` locale storage, `tenant_locales` policy layer, `flex` standalone schema translations
- [Channels](./architecture/channels.md)
- [DataLoader](./architecture/dataloader.md)
- [Event flow contract](./architecture/event-flow-contract.md)
- [Matryoshka / composition model](./architecture/matryoshka.md)
- [Performance baseline](./architecture/performance-baseline.md)

## Руководства и стандарты

- [Quickstart](./guides/quickstart.md)
- [Testing](./guides/testing.md)
- [Observability quickstart](./guides/observability-quickstart.md)
- [Runtime guardrails](./guides/runtime-guardrails.md)
- [Input validation](./guides/input-validation.md)
- [Error handling](./guides/error-handling.md)
- [Security audit](./guides/security-audit.md)
- [Logging](./standards/logging.md)
- [Errors](./standards/errors.md)
- [Security](./standards/security.md)
- [Coding](./standards/coding.md)
- [RT JSON v1](./standards/rt-json-v1.md)

## Проверка платформы

- [Главный verification README](./verification/README.md)
- [Сводный verification plan](./verification/PLATFORM_VERIFICATION_PLAN.md)
- [Foundation verification](./verification/platform-foundation-verification-plan.md)
- [API surfaces verification](./verification/platform-api-surfaces-verification-plan.md)
- [Frontend surfaces verification](./verification/platform-frontend-surfaces-verification-plan.md)
- [Core integrity verification](./verification/platform-core-integrity-verification-plan.md)
- [Quality operations verification](./verification/platform-quality-operations-verification-plan.md)

## AI, исследования и шаблоны

- [AI context](./AI_CONTEXT.md)
- [AI session template](./ai/SESSION_TEMPLATE.md)
- [Известные pitfalls](./ai/KNOWN_PITFALLS.md)
- [Шаблон документации модуля](./templates/module_contract.md)
- [Исследования и ADR-черновики](./research/ADR-xxxx-grpc-adoption.md)

## Документация приложений

- [Server docs](../apps/server/docs/README.md)
- [Admin docs](../apps/admin/docs/README.md)
- [Storefront docs](../apps/storefront/docs/README.md)
- [Next Admin docs](../apps/next-admin/docs/README.md)
- [Next Frontend docs](../apps/next-frontend/docs/README.md)

## Документация crate-ов

- Для foundation и shared-инфраструктуры: `crates/rustok-core`, `crates/rustok-api`, `crates/rustok-events`, `crates/rustok-cache`, `crates/rustok-outbox`, `crates/rustok-telemetry`, `crates/rustok-tenant`.
- Для UI-библиотек: `crates/leptos-*`, `crates/flex`, `crates/leptos-ui`.
- Для доменных модулей: `crates/rustok-*` согласно [реестру модулей](./modules/registry.md).
- У каждого crate должен быть актуальный `README.md`, а при необходимости и `docs/`.

## Правила поддержки актуальности

- Центральные документы в `docs/` ведутся на русском языке.
- `README.md`, `AGENTS.md`, `CONTRIBUTING.md` и публичные контрактные документы ведутся на английском.
- Один файл — один язык.
- Не создавайте новый документ, если подходящий уже существует: расширяйте текущий.
- При изменении архитектуры, API, tenancy, routing, observability или module-system обновляйте и локальные docs компонента, и центральные документы в `docs/`.

## Architecture Decisions

- [ADR index](../DECISIONS/README.md)
