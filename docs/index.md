# RusTok: карта документации

Этот файл является канонической точкой входа в документацию репозитория.
С него нужно начинать работу по правилам [AGENTS.md](../AGENTS.md).

Документация в `docs/` описывает платформу целиком.
Локальные документы приложений и crate-ов лежат в `apps/*/docs/`,
`crates/*/docs/` и `README.md` рядом с кодом.

## Как пользоваться картой

1. Откройте обзор платформы и нужный архитектурный раздел.
2. Для модулей используйте `docs/modules/*` и `docs/modules/registry.md`.
3. Для UI используйте `docs/UI/*` и локальные docs приложений.
4. Для проверок и quality-gates используйте `docs/verification/*`
   и `docs/guides/*`.
5. Для архитектурных решений используйте `DECISIONS/*`.

## Обязательные стартовые документы

- [Обзор платформы](./architecture/overview.md)
- [Архитектурные принципы](./architecture/principles.md)
- [API и surface-контракты](./architecture/api.md)
- [Маршрутизация](./architecture/routing.md)
- [Модульная архитектура](./architecture/modules.md)
- [Карта модулей и владельцев](./modules/registry.md)

## Модульная система

- [Обзор модульной платформы](./modules/overview.md)
- [Как писать модуль в RusToK](./modules/module-authoring.md)
- [Контракт `rustok-module.toml`](./modules/manifest.md)
- [Реестр модулей и приложений](./modules/registry.md)
- [Реестр crate-ов модульной платформы](./modules/crates-registry.md)
- [Индекс документации по модулям](./modules/_index.md)
- [Реестр implementation plans](./modules/implementation-plans-registry.md)
- [Шаблон документации модуля](./templates/module_contract.md)
- [Индекс UI-пакетов модулей](./modules/UI_PACKAGES_INDEX.md)
- [Быстрый старт по UI-пакетам](./modules/UI_PACKAGES_QUICKSTART.md)
- [Спец-план rich-text и визуального page builder](
  ./modules/tiptap-page-builder-implementation-plan.md)

## UI и клиентские поверхности

- [Обзор UI](./UI/README.md)
- [GraphQL и Leptos server functions](./UI/graphql-architecture.md)
- [Контракт storefront](./UI/storefront.md)
- [Быстрый старт для Admin ↔ Server](./UI/admin-server-connection-quickstart.md)
- [Каталог Rust UI-компонентов](./UI/rust-ui-component-catalog.md)
- [Трек rich-text и визуального page builder](
  ./modules/tiptap-page-builder-implementation-plan.md)
- [Архитектура i18n](./architecture/i18n.md)

## Архитектура и foundation

- [Диаграмма платформы](./architecture/diagram.md)
- [База данных](./architecture/database.md) — live DB/i18n storage contract: `base + translations + optional bodies`, `VARCHAR(32)` locale storage, `tenant_locales` policy layer, `flex` standalone schema translations, shared attached localized Flex values, live donor paths for `user`, `product`, `order`, and `topic`
- [ADR гибридного установщика](../DECISIONS/2026-04-26-hybrid-installer-architecture.md) — installer-core/CLI/web wizard layering, PostgreSQL production policy, explicit separation of build composition, schema composition and tenant enablement
- [ADR lifecycle hook phases/retry contract](../DECISIONS/2026-05-22-module-lifecycle-hook-phases-and-retry-contract.md) — `validated/running/committed/failed`, explicit `pre/post` hooks и retryable post-hook failures без частичного rollback
- [Каналы](./architecture/channels.md)
- [DataLoader](./architecture/dataloader.md)
- [Контракт event flow](./architecture/event-flow-contract.md)
- [Matryoshka / модель композиции](./architecture/matryoshka.md)
- [Базовая производительность](./architecture/performance-baseline.md)

## Примеры и smoke-сценарии

- [Каталог исполняемых примеров](./examples/README.md)

## Руководства и стандарты

- [Быстрый старт](./guides/quickstart.md)
- [Тестирование](./guides/testing.md)
- [Быстрый старт по observability](./guides/observability-quickstart.md)
- [Runtime guardrails](./guides/runtime-guardrails.md)
- [ADR: control-plane lifecycle and migration ordering contracts](
  ../DECISIONS/2026-05-18-control-plane-lifecycle-and-migration-contracts.md)
- [Валидация входных данных](./guides/input-validation.md)
- [Обработка ошибок](./guides/error-handling.md)
- [Аудит безопасности](./guides/security-audit.md)
- [Логирование](./standards/logging.md)
- [Ошибки](./standards/errors.md)
- [Безопасность](./standards/security.md)
- [Правила кодирования](./standards/coding.md)
- [Стандарт RT JSON v1](./standards/rt-json-v1.md)

## Проверка платформы

- [Инструмент workspace CLI `xtask`](../xtask/README.md)
- [Главный README по верификации](./verification/README.md)
- [Проверка Flex multilingual contract](
  ../scripts/verify/verify-flex-multilingual-contract.mjs)
- [Сводный план верификации](./verification/PLATFORM_VERIFICATION_PLAN.md)
- [Верификация foundation-слоя](
  ./verification/platform-foundation-verification-plan.md)
- [Верификация API-поверхностей](
  ./verification/platform-api-surfaces-verification-plan.md)
- [Верификация frontend-поверхностей](
  ./verification/platform-frontend-surfaces-verification-plan.md)
- [Верификация целостности ядра](
  ./verification/platform-core-integrity-verification-plan.md)
- [Верификация качества и эксплуатации](
  ./verification/platform-quality-operations-verification-plan.md)

## AI, исследования и шаблоны

- [Контекст для AI](./AI_CONTEXT.md)
- [Шаблон AI-сессии](./ai/SESSION_TEMPLATE.md)
- [Известные pitfalls](./ai/KNOWN_PITFALLS.md)
- [Индекс MCP reference](./references/mcp/README.md)
- [Сравнение архитектуры RusTok и Medusa](./research/medusa-vs-rustok-architecture.md)
- [Fluid Frontend Architecture для RusTok](./research/fluid-frontend-architecture.md)
- [Fluid Backend Architecture для RusTok](./research/fluid-backend-architecture.md)
- [Архитектура Flutter-приложения для RusTok](./research/flutter.md)
- [Единый план реализации Fluid Backend Architecture](./research/fluid-backend-architecture-unified-plan.md)
- [Historical input: deep research report (control plane/module lifecycle)](./research/deep-research-report%20(4).md)
- [План устранения недостатков control plane и module lifecycle](./research/control-plane-module-lifecycle-remediation-plan.md)
- [План исправления документации](./research/fix%20docs.md)
- [Исследования и ADR-черновики](./research/ADR-xxxx-grpc-adoption.md)

## Документация приложений

- [Документация Server](../apps/server/docs/README.md)
- [Документация Admin](../apps/admin/docs/README.md)
- [Документация Storefront](../apps/storefront/docs/README.md)
- [Документация Next Admin](../apps/next-admin/docs/README.md)
- [Документация Next Frontend](../apps/next-frontend/docs/README.md)
- [Документация Flutter Admin Mobile](../rustok_mobile/apps/rustok_admin_mobile/README.md)

## Документация crate-ов

- Для platform modules: `crates/rustok-*` согласно
  [реестру модулей и приложений](./modules/registry.md).
- Для foundation/shared libraries см. `crates/rustok-*`
  и соответствующие `README.md`.
- Для infrastructure/capability crates см. `crates/*`
  и `docs/modules/crates-registry.md`.
- Для UI-библиотек используйте `crates/leptos-*`, `crates/leptos-ui`.
- У каждого crate должен быть актуальный `README.md`,
  а при необходимости и `docs/`.

## Правила поддержки актуальности

- Центральные документы в `docs/` ведутся на русском языке.
- `README.md`, `AGENTS.md`, `CONTRIBUTING.md`
  и публичные контрактные документы ведутся на английском.
- Один файл — один язык.
- Не создавайте новый документ, если подходящий уже существует:
  расширяйте текущий.
- При изменении архитектуры, API, tenancy, routing, observability
  или модульной системы обновляйте и локальные docs компонента,
  и центральные документы в `docs/`.
- Любая новая схема проходит i18n-аудит;
  локализованные display-поля живут в `*_translations`.
- Read-side locale matching использует shared normalization
  (`requested -> tenant default -> first available`).
- Module-owned admin UI хранит selection state в URL
  с typed `snake_case` query keys.

## Architecture Decisions

- [Индекс ADR](../DECISIONS/README.md)

- [Security: RUSTSEC-2026-0045 remediation note](
  ./security/aws-lc-rustsec-2026-0045.md)
- [Security: RUSTSEC-2026-0098 / 0099 / 0104 remediation note](
  ./security/rustls-webpki-rustsec-2026-0099-0104.md)
- [Security: RUSTSEC-2023-0071 remediation note](
  ./security/rsa-rustsec-2023-0071.md)
