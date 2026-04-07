# RusTok: карта документации

Этот файл — каноническая точка входа в документацию репозитория. С него начинают работу и люди, и автоматические агенты.

## Как пользоваться картой

1. Для общего контекста откройте архитектурный обзор и принципы платформы.
2. Для модульной платформы переходите в `docs/modules/*`.
3. Для UI-контуров и локализации используйте `docs/UI/*` и `docs/architecture/i18n.md`.
4. Для verification и quality gates используйте `docs/verification/*`.
5. Для конкретного модуля сверяйтесь одновременно с `docs/modules/registry.md`, `docs/modules/_index.md` и локальной документацией crate.

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
- [Индекс документации по модулям](./modules/_index.md)
- [Реестр crate-ов модульной платформы](./modules/crates-registry.md)
- [Индекс UI-пакетов модулей](./modules/UI_PACKAGES_INDEX.md)
- [Quickstart по UI-пакетам](./modules/UI_PACKAGES_QUICKSTART.md)
- [Исследование по единому стандарту модулей](./research/deep-research-modules.md)

## UI и клиентские поверхности

- [UI README](./UI/README.md)
- [GraphQL и Leptos server functions](./UI/graphql-architecture.md)
- [Storefront](./UI/storefront.md)
- [Быстрый старт для Admin ↔ Server](./UI/admin-server-connection-quickstart.md)
- [Каталог Rust UI-компонентов](./UI/rust-ui-component-catalog.md)
- [Архитектура i18n](./architecture/i18n.md)

## Архитектура и foundation

- [Диаграмма платформы](./architecture/diagram.md)
- [Database](./architecture/database.md)
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

## Правила актуальности

- Центральная документация в `docs/` ведётся на русском языке.
- `README.md`, `AGENTS.md`, `CONTRIBUTING.md` и публичные контрактные документы ведутся на английском.
- Один файл — один язык.
- Если подходящий документ уже существует, расширяйте его, а не создавайте дубликат.
- При изменении модулей, API, routing, tenancy, observability или UI контрактов обновляйте и локальные docs компонента, и центральные docs.

## Architecture Decisions

- [ADR index](../DECISIONS/README.md)
