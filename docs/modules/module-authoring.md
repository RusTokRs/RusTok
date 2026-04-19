# Как писать модуль в RusToK

Этот документ — каноническая входная точка для разработчика и AI-агента, который пишет новый модуль или делает крупный module refactor. Он не дублирует архитектурные документы построчно, а фиксирует практический contract: как собирать backend и UI модуля так, чтобы не ломать platform boundaries.

Если нужен короткий ответ на вопрос «с чего начать», то порядок такой:

1. Определить ownership и runtime-роль модуля.
2. Собрать backend по platform contract.
3. Только потом добавлять UI как module-owned package.
4. Обновить local docs модуля и central docs.

## Перед стартом

Перед любыми изменениями по модулю обязательно сверяйтесь с:

- [Обзором модульной платформы](./overview.md)
- [Архитектурой модулей](../architecture/modules.md)
- [Контрактом `modules.toml` и `rustok-module.toml`](./manifest.md)
- [Схемой данных платформы](../architecture/database.md)
- [Архитектурой i18n](../architecture/i18n.md)
- [Архитектурой API](../architecture/api.md)
- [Документацией `apps/server`](../../apps/server/docs/README.md)
- [Документацией `apps/admin`](../../apps/admin/docs/README.md)
- [Workspace CLI `xtask`](../../xtask/README.md)

## Что считать модулем

В RusToK модуль — это не «любой crate в `crates/`». Канонический platform module:

- объявлен в `modules.toml`;
- имеет `slug`, ownership и runtime contract;
- проходит scoped validation через `cargo xtask module validate <slug>`;
- публикует backend и, при необходимости, module-owned UI surfaces через manifest.

Support/crate/capability слой может жить рядом с модулем, но это не делает его tenant-toggled модулем автоматически. Это особенно важно для `rustok-core`, `rustok-api`, `rustok-storage`, `rustok-mcp`, `rustok-ai`, `alloy`, `flex` и похожих foundation-слоёв.

## Backend

### 1. Сначала зафиксируйте runtime contract

Минимум для backend-модуля:

- запись в `modules.toml`;
- `rustok-module.toml` с корректными `module.slug`, `module.version`, `module.description`, `module.ui_classification`;
- root `README.md` на английском;
- local `docs/README.md` и `docs/implementation-plan.md` на русском.

Канон:

- composition и module taxonomy: [overview.md](./overview.md)
- manifest contract: [manifest.md](./manifest.md)
- ownership map: [registry.md](./registry.md)

### 2. Не придумывайте свой backend contract

Backend модуля должен встраиваться в общий platform flow:

- transport ownership идёт через `apps/server`, но business/domain contract остаётся у модуля;
- Leptos `#[server]` — default internal data layer для Leptos surfaces, но GraphQL остаётся параллельно;
- REST нужен только там, где действительно нужен явный HTTP contract: integrations, webhooks, ops, module-owned routes;
- нельзя делать package-local auth, locale, tenant или RBAC shortcuts.

Канон:

- API surfaces: [api.md](../architecture/api.md)
- routing и transport boundaries: [routing.md](../architecture/routing.md)
- server host contract: [apps/server/docs/README.md](../../apps/server/docs/README.md)

### 3. Данные и миграции пишутся по общему storage contract

Нельзя invent-ить свою схему хранения для текстов, locale и identity.

Базовые правила:

- language-agnostic state живёт в base tables;
- короткие локализуемые поля живут в `*_translations`;
- тяжёлый локализуемый контент при необходимости живёт в `*_bodies`;
- `locale` хранится нормализованно;
- audit payload и technical metadata не должны превращаться в business copy.

Канон:

- DB contract: [database.md](../architecture/database.md)
- i18n contract: [i18n.md](../architecture/i18n.md)

### 4. Не выносите platform rules в строки и ad-hoc JSON

Для backend-модуля запрещено:

- авторизовывать действия по недоверенным строкам или header-based actor model;
- строить live authority из display labels;
- держать canonical read contract в произвольном `details` JSON, если уже есть typed schema;
- смешивать public contract и internal audit storage.

Если нужен actor/principal/read-model, делайте typed contract, а не строковые эвристики.

### 5. Проверка backend-части

Минимальный check-list перед завершением работы:

1. `cargo xtask module validate <slug>`
2. `cargo check -p rustok-server --lib`
3. targeted `cargo test` по модулю и затронутому host/runtime
4. обновлены local docs модуля
5. обновлены central docs, если поменялся architecture/runtime contract

## UI

### 1. UI в RusToK — module-owned, а не host-owned by default

Если модуль публикует UI, этот UI должен жить рядом с модулем:

- Leptos admin/storefront — через `admin/` и `storefront/` sub-crates;
- Next.js surfaces — через соответствующие host packages, но ownership UI contract всё равно остаётся у модуля;
- host only mounts these surfaces and provides route/auth/locale/runtime context.

Канон:

- module composition: [modules.md](../architecture/modules.md)
- UI package map: [UI_PACKAGES_INDEX.md](./UI_PACKAGES_INDEX.md)
- quickstart по UI packages: [UI_PACKAGES_QUICKSTART.md](./UI_PACKAGES_QUICKSTART.md)

### 2. Для Leptos UI сначала `#[server]`, потом всё остальное

Для module-owned Leptos UI действует обязательное правило:

- internal data layer по умолчанию строится через native `#[server]` functions;
- GraphQL остаётся параллельным transport contract и не удаляется;
- нельзя заменять уже существующий GraphQL только потому, что появился `#[server]` path.

Канон:

- UI/GraphQL/server-functions: [graphql-architecture.md](../UI/graphql-architecture.md)
- admin host contract: [apps/admin/docs/README.md](../../apps/admin/docs/README.md)

### 3. UI не выбирает locale сам

Module-owned UI package не имеет права invent-ить свою locale chain.

Правило:

- effective locale приходит от host/runtime;
- Leptos packages читают host-provided `UiRouteContext.locale`;
- Next packages используют host/runtime locale providers;
- query/header/cookie fallback chain на уровне пакета запрещена.

Канон:

- i18n contract: [i18n.md](../architecture/i18n.md)
- UI host contract: [apps/admin/docs/README.md](../../apps/admin/docs/README.md)

### 4. UI wiring идёт через manifest, а не через «магическое наличие crate»

Сам факт существования `admin/` или `storefront/` каталога ещё не означает, что surface интегрирован корректно. Канонический source of truth здесь — manifest.

Нужно:

- объявить UI surface в `rustok-module.toml`;
- держать `module.ui_classification` в соответствии с реальным wiring;
- не оставлять orphaned host dependency или feature entry после рефакторинга.

Канон:

- manifest/UI wiring: [manifest.md](./manifest.md)
- module registry/index: [registry.md](./registry.md), [_index.md](./_index.md)

### 5. Проверка UI-части

Минимальный check-list:

1. `cargo xtask module validate <slug>`
2. targeted `cargo check` для UI crate и host app
3. `npm.cmd run verify:i18n:ui`, если тронуты locale bundles или locale wiring
4. UI package docs и host docs обновлены, если поменялся surface contract

## Что запрещено

При написании модуля нельзя:

- считать любой crate модулем без `modules.toml`;
- invent-ить package-local i18n contract;
- переносить module-owned domain/UI ownership в host app без явной причины;
- делать runtime authority из строковых акторов, display labels или недоверенных headers;
- хранить локализуемый business text прямо в base rows, если модуль уже идёт по multilingual contract;
- заменять typed public contract сырым `details` JSON;
- обновлять только код без local/central docs, если изменился контракт.

## Быстрый шаблон решения

Если агенту или разработчику нужно быстро принять решение, используйте такой порядок:

1. Это platform module или support/capability crate?
2. Какой у него backend contract: GraphQL, REST, `#[server]`, events, migrations?
3. Какие данные language-agnostic, а какие localized?
4. Есть ли у модуля module-owned UI surface?
5. Как host даёт ему auth, locale, routing и tenant context?
6. Какие docs и verification gates должны измениться вместе с кодом?

## PR / Review Checklist

Этот checklist нужен для любого нового модуля, крупного module refactor или изменения module contract. Его можно прогонять перед PR или во время review.

### Backend checklist

1. Модуль действительно объявлен в `modules.toml`, а не существует только как crate.
2. `rustok-module.toml` синхронизирован с реальным runtime contract.
3. `module.slug`, `module.version`, `module.description` и `module.ui_classification` не расходятся с кодом и wiring.
4. Backend не invent-ит свой auth, tenant, locale или RBAC contract.
5. Leptos `#[server]` добавлен как internal data layer там, где это нужно, но GraphQL не удалён и не подменён скрыто.
6. REST добавлен только там, где действительно нужен явный HTTP contract.
7. Language-agnostic state хранится в base tables, локализуемые поля вынесены в `*_translations` или `*_bodies`.
8. Typed public contract не заменён строковыми эвристиками, `details` JSON или header-based authority.
9. Миграции, read-model и transport обновлены согласованно, без half-migrated contract.
10. Пройдены `cargo xtask module validate <slug>` и targeted `cargo check` / `cargo test`.

### UI checklist

1. UI surface остаётся module-owned, а не расползается в host app без явной причины.
2. UI wiring описан в manifest, а не держится на «магическом» наличии crate или route.
3. `module.ui_classification` соответствует реальным admin/storefront surfaces.
4. Leptos UI использует native `#[server]` path как default internal data layer.
5. GraphQL transport остаётся параллельно, если модуль уже публикует GraphQL surface.
6. UI package не invent-ит свой locale selection и потребляет host-provided effective locale.
7. Host даёт модулю только context и mounting, а не становится владельцем domain UI contract.
8. Нет orphaned host dependencies, feature flags или устаревшего wiring после рефакторинга.
9. Обновлены local docs UI package и host docs, если поменялся surface contract.
10. Пройдены targeted UI checks и `verify:i18n:ui`, если тронут locale bundles или locale wiring.

### Docs checklist

1. Обновлён root `README.md` компонента.
2. Обновлён local `docs/README.md`.
3. Обновлён local `docs/implementation-plan.md`, если менялся roadmap или target state.
4. Обновлён `docs/index.md`, если поменялась карта документации.
5. Нет дублирующего нового документа, если подходящий уже существовал.
