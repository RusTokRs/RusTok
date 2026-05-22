# RusTok: карта документации

Этот файл является канонической точкой входа в документацию репозитория. С него нужно начинать работу по правилам [AGENTS.md](../AGENTS.md).

Документация в `docs/` описывает платформу целиком. Локальные документы приложений и crate-ов лежат в `apps/*/docs/`, `crates/*/docs/` и `README.md` рядом с кодом.

## Как пользоваться картой

1. Сначала откройте обзор платформы и нужный архитектурный раздел.
2. Для изменений в модульной системе переходите в `docs/modules/*`.
3. Для UI-срезов используйте `docs/UI/*` и локальные docs приложений.
4. Для периодической верификации и quality-gates используйте `docs/verification/*` и `docs/guides/*`.
5. Для остаточного и будущего scope по platform contracts сверяйтесь с профильными live docs в `docs/architecture/*`, `docs/UI/*` и `apps/*/docs/*`, не смешивая это с периодической верификацией.
6. Для изменений конкретного модуля сверяйтесь с `docs/modules/registry.md` и локальными docs соответствующего crate.
7. Для принятых архитектурных решений и clean-cutover решений сверяйтесь с `DECISIONS/*`.

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
- [Спец-план rich-text и визуального page builder](./modules/tiptap-page-builder-implementation-plan.md)

Для исторических статусных деталей и cutover-хроники используйте профильные
implementation-plan документы в `docs/modules/*` и ADR в `DECISIONS/*`.

## UI и клиентские поверхности

- [Обзор UI](./UI/README.md)
- [GraphQL и Leptos server functions](./UI/graphql-architecture.md)
- [Контракт storefront](./UI/storefront.md)
- [Быстрый старт для Admin ↔ Server](./UI/admin-server-connection-quickstart.md)
- [Каталог Rust UI-компонентов](./UI/rust-ui-component-catalog.md)
- [Трек rich-text и визуального page builder](./modules/tiptap-page-builder-implementation-plan.md)
- [Архитектура i18n](./architecture/i18n.md)

Исторические status/cutover заметки по UI держим в профильных docs/ADR,
а этот индекс оставляем navigation-first.

## Архитектура и foundation

- [Диаграмма платформы](./architecture/diagram.md)
- [База данных](./architecture/database.md) — live DB/i18n storage contract: `base + translations + optional bodies`, `VARCHAR(32)` locale storage, `tenant_locales` policy layer, `flex` standalone schema translations, shared attached localized Flex values, live donor paths for `user`, `product`, `order`, and `topic`
- [ADR гибридного установщика](../DECISIONS/2026-04-26-hybrid-installer-architecture.md) — installer-core/CLI/web wizard layering, PostgreSQL production policy, explicit separation of build composition, schema composition and tenant enablement
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
- [ADR: control-plane lifecycle and migration ordering contracts](../DECISIONS/2026-05-18-control-plane-lifecycle-and-migration-contracts.md)
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
- Flex multilingual contract теперь имеет отдельный repo-side guardrail:
  `node scripts/verify/verify-flex-multilingual-contract.mjs`
- [Сводный план верификации](./verification/PLATFORM_VERIFICATION_PLAN.md)
- [Верификация foundation-слоя](./verification/platform-foundation-verification-plan.md)
- [Верификация API-поверхностей](./verification/platform-api-surfaces-verification-plan.md)
- [Верификация frontend-поверхностей](./verification/platform-frontend-surfaces-verification-plan.md)
- [Верификация целостности ядра](./verification/platform-core-integrity-verification-plan.md)
- [Верификация качества и эксплуатации](./verification/platform-quality-operations-verification-plan.md)

## AI, исследования и шаблоны

- [Контекст для AI](./AI_CONTEXT.md)
- [Шаблон AI-сессии](./ai/SESSION_TEMPLATE.md)
- [Известные pitfalls](./ai/KNOWN_PITFALLS.md)
- [Индекс MCP reference](./references/mcp/README.md)
- [Сравнение архитектуры RusTok и Medusa](./research/medusa-vs-rustok-architecture.md)
- [Fluid Frontend Architecture для RusTok](./research/fluid-frontend-architecture.md)
- [Fluid Backend Architecture для RusTok](./research/fluid-backend-architecture.md)
- [Единый план реализации Fluid Backend Architecture](./research/fluid-backend-architecture-unified-plan.md)
- [Historical input: deep research report (control plane/module lifecycle)](./research/deep-research-report%20(4).md)
- [План устранения недостатков control plane и module lifecycle](./research/control-plane-module-lifecycle-remediation-plan.md)
- [План исправления документации](./research/fix%20docs.md)
- [Исследования и ADR-черновики](./research/ADR-xxxx-grpc-adoption.md)

## Документация приложений

- [Документация Server](../apps/server/docs/README.md)
  Server docs теперь фиксируют live `flex` standalone GraphQL + REST surfaces, их tenant-scoped RBAC contract,
  а также reduced/headless build matrix с минимальным `--no-default-features` profile, optional `redis-cache`
  для Redis-backed runtime integrations, без обязательного `mod-commerce` и с compile-time feature ownership
  для embedded admin/storefront host-ов; content REST/OpenAPI fragments `blog/forum/pages` и content-only
  maintenance binary `migrate_legacy_richtext` там тоже зафиксированы как module-owned compile-time surfaces,
  а не как безусловный baseline `apps/server`.
- [Документация Admin](../apps/admin/docs/README.md)
- [Документация Storefront](../apps/storefront/docs/README.md)
- [Документация Next Admin](../apps/next-admin/docs/README.md)
- [Документация Next Frontend](../apps/next-frontend/docs/README.md)

## Документация crate-ов

- Для platform modules: `crates/rustok-*` согласно [реестру модулей и приложений](./modules/registry.md).
- Для foundation и shared libraries: `crates/rustok-core`, `crates/rustok-api`, `crates/rustok-events`, `crates/rustok-storage`, `crates/rustok-test-utils`, `crates/rustok-commerce-foundation`, `crates/rustok-seo/render`, `crates/rustok-seo-admin-support`.
- Для infrastructure и capability crates: `crates/rustok-installer`, `crates/rustok-iggy`, `crates/rustok-iggy-connector`, `crates/rustok-telemetry`, `crates/rustok-mcp`, `crates/rustok-ai`, `crates/alloy`, `crates/flex`, `crates/rustok-seo-targets`.
- Для UI-библиотек и host-shared UI support: `crates/leptos-*`, `crates/leptos-ui`.
- У каждого crate должен быть актуальный `README.md`, а при необходимости и `docs/`.

## Правила поддержки актуальности

- Центральные документы в `docs/` ведутся на русском языке.
- `README.md`, `AGENTS.md`, `CONTRIBUTING.md` и публичные контрактные документы ведутся на английском.
- Один файл — один язык.
- Не создавайте новый документ, если подходящий уже существует: расширяйте текущий.
- При изменении архитектуры, API, tenancy, routing, observability или модульной системы обновляйте и локальные docs компонента, и центральные документы в `docs/`.
- Любая новая схема проходит i18n-аудит: локализованные строки не храним в base-таблицах, display-поля живут только в `*_translations`. Module-owned UI пакеты не вводят package-local locale override и гидратят edit/detail формы по host-provided effective locale, а не по `first()` переводу сущности.
- Read-side/runtime locale resolution тоже живёт по общему contract: locale matching идёт через shared normalization (`requested -> tenant default -> first available`), а не через raw string equality вроде `ru` vs `ru-RU`.
- Любой новый module-owned admin UI обязан пройти route-selection audit: selection state хранится в URL,
  используются только typed `snake_case` query keys, локальный state остаётся производным от URL,
  а invalid/missing selection не должен silently fallback’иться на first-item auto-open.

## Architecture Decisions

- [Индекс ADR](../DECISIONS/README.md)

- [Security: RUSTSEC-2026-0045 remediation note](./security/aws-lc-rustsec-2026-0045.md)
- [Security: RUSTSEC-2026-0098 / 0099 / 0104 remediation note](./security/rustls-webpki-rustsec-2026-0099-0104.md)
- [Security: RUSTSEC-2023-0071 remediation note](./security/rsa-rustsec-2023-0071.md)
