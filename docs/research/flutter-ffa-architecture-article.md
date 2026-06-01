# FFA для Flutter: зачем платформенному продукту не «обычная» мобильная архитектура

> Статья-заметка для внешней публикации в стиле Medium. Материал объясняет,
> почему Flutter-клиент RusTok развивается как Fluid Frontend Architecture
> host, а не как самостоятельное приложение с набором экранов.

## Коротко

Для небольшого Flutter-приложения обычная структура `lib/features/*` часто
достаточна: есть экраны, сервисы, общий router и несколько API-клиентов.
Но для платформы вроде RusTok этого мало.

RusTok уже живёт как модульная платформа: есть backend composition root,
платформенные модули, несколько frontend-host-ов, storefront/admin поверхности,
GraphQL/headless contracts, module manifests и generated registries. Поэтому
Flutter здесь не должен становиться «третьим независимым frontend-ом». Он должен
быть ещё одним host-клиентом платформы.

Именно это даёт FFA — Fluid Frontend Architecture.

FFA для Flutter — это не попытка использовать Leptos `#[server]` functions из
Dart и не способ переиспользовать Rust UI components. Это способ удержать один
product contract при разных frontend runtime:

- Leptos может жить близко к Rust server functions.
- Next.js может быть headless web host.
- Dioxus может стать Rust UI runtime.
- Flutter остаётся Dart/mobile runtime.
- Но module ownership, routes, permissions, locale, tenant context и backend
  contracts не расходятся.

## Что было бы в обычной Flutter-архитектуре

Типичный Flutter-проект часто выглядит так:

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

Это нормальная схема для одного продукта. Она проста, быстра на старте и хорошо
понятна команде.

Проблема появляется, когда продукт — не одно приложение, а платформа:

- web storefront уже существует;
- admin web уже существует;
- Next-host развивается параллельно;
- backend modules имеют свои manifests;
- UI surfaces принадлежат модулям;
- tenant, locale, auth, routing и permissions являются платформенными
  контрактами;
- GraphQL/REST/WS должны быть canonical headless boundary.

В такой среде обычная mobile-архитектура быстро начинает жить своей жизнью.
Catalog feature заводит свой API client. Cart feature заводит своё storage.
Profile feature заводит свой locale fallback. Router получает ручной список
модулей. Через несколько месяцев mobile уже не просто другой UI, а другой
продукт.

## Что меняет FFA

FFA предлагает другой взгляд:

```text
backend/platform
  owns canonical behavior and contracts

host app
  owns shell, routing, auth, tenant, locale, transport, storage, registry wiring

module-owned package
  owns screens, widgets, UI states, forms and user intents
```

Для Flutter это выражается так:

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

Flutter package может владеть карточкой товара, экраном корзины, empty state,
loading state и user intent «добавить в корзину». Но он не должен владеть
GraphQL client, tenant resolver, locale fallback или durable cart storage.

Host получает intent от package и выполняет его через canonical backend
contract.

## Пример: cart/catalog

Обычная реализация могла бы пойти по короткому пути:

```text
CatalogScreen -> CartService -> /mobile/cart/add -> local cart storage
```

Это быстро. Но это создаёт mobile-only API, mobile-only storage и отдельную
семантику cart flow.

В FFA-варианте поток выглядит иначе:

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

То есть module package говорит: «пользователь хочет добавить товар». Host
решает, какой cart id использовать, какие headers отправить, какой canonical
GraphQL mutation вызвать и где хранить cart id.

Это важная разница. Package остаётся UI package, а не маленьким приложением
внутри приложения.

## Почему такая архитектура лучше для RusTok

### 1. Она снижает UI drift

Когда Leptos, Next и Flutter развиваются независимо, они быстро начинают
расходиться:

- разные empty states;
- разные названия действий;
- разные permission gates;
- разные route semantics;
- разные locale keys;
- разные ошибки и loading states.

FFA говорит: layout может быть разным, но product contract должен быть одним.
Mobile не обязан копировать desktop pixel-perfect. Но он обязан сохранить те же
сущности, constraints, actions, permissions и состояния.

### 2. Она защищает от Flutter-only API

Самый соблазнительный путь для mobile — попросить backend сделать удобный
endpoint:

```text
/mobile/catalog
/mobile/cart/add
/mobile/me
/mobile/modules
```

На старте это ускоряет разработку. Через год это превращается в набор
параллельных backend contracts, которые нужно поддерживать отдельно.

FFA требует: если contract нужен продукту, он должен быть platform-level
contract, а не Flutter-only shortcut.

### 3. Она сохраняет module ownership

В платформенном продукте модуль должен владеть своей UI surface. Host не должен
становиться местом, где живёт вся domain-specific presentation logic.

В FFA host монтирует surfaces, но не забирает ownership. Это особенно важно для
RusTok, где модули уже имеют manifests, route segments, permissions и UI
classification.

### 4. Она делает подключение модулей декларативным

Без FFA новый модуль часто означает ручной список изменений:

1. добавить screen;
2. добавить route;
3. добавить nav item;
4. добавить permission check;
5. добавить locale keys;
6. добавить deep link;
7. не забыть parity с web.

С FFA путь другой:

```text
rustok-module.toml
  -> mobile manifest snapshot
  -> generated Dart registry
  -> host registry adapter
  -> mounted module-owned package
```

Это не убирает всю работу, но превращает подключение модуля в проверяемый
контрактный процесс.

### 5. Она централизует tenant, locale, auth и storage

Tenant, locale, auth/session и cart storage — это не детали конкретной feature.
Это platform context.

Если каждая feature начнёт выбирать locale сама, читать tenant из своего места и
хранить cart id по-своему, приложение станет непредсказуемым. FFA удерживает эти
правила на уровне host/runtime.

### 6. Она делает архитектурный drift проверяемым

FFA — это не только принципы, но и артефакты:

- generated registry;
- manifest snapshots;
- codegen freshness checks;
- route contract tests;
- package boundary tests;
- documentation evidence blocks;
- readiness boards.

Это переводит архитектурную дисциплину из устного code review в проверяемый
workflow.

## Какие проблемы FFA решает

### UI drift

Flutter не становится отдельным UX-продуктом. Он остаётся mobile expression того
же product contract.

### Transport drift

Feature packages не создают свои GraphQL clients, headers, retry policies,
locale chains и auth refresh logic.

### Ownership drift

Host остаётся host-ом. Module package остаётся module package. Backend остаётся
источником canonical behavior.

### API drift

Mobile-only shortcuts не превращаются во второй backend contract.

### Routing drift

Route semantics и query keys остаются частью platform contract, а не локальной
договорённостью конкретного Flutter router.

### Locale drift

Effective locale выбирается host/runtime слоем и прокидывается в UI surfaces.
Module packages не придумывают свои fallback chains.

### Registry drift

Список доступных module surfaces идёт через manifest/codegen, а не через ручной
список экранов в mobile host.

## Недостатки

FFA не бесплатна.

### 1. Больше сложности на старте

Для маленького приложения это overengineering. Вместо одного `CartService`
появляются repository boundary, host implementation, DTO, GraphQL operation,
cart id store, tests и docs.

### 2. Больше boilerplate

Даже простая user action может пройти через несколько слоёв:

```text
Widget -> repository interface -> host adapter -> GraphQL client -> backend
```

Это дольше, чем вызвать service напрямую из feature.

### 3. Медленнее быстрые эксперименты

FFA ограничивает быстрые shortcuts. Нельзя просто сделать Flutter-only endpoint
или сохранить cart id в package-local storage, если это ломает platform contract.

### 4. Требуется дисциплина команды

Архитектура работает только если команда понимает ownership boundaries:

- что host-owned;
- что module-owned;
- что backend-owned;
- что shared;
- что нельзя класть в package;
- когда нужно обновлять docs и manifests.

Без этой дисциплины FFA превращается в набор абстракций без пользы.

### 5. Риск избыточной модульности

FFA не означает «создать package на каждую кнопку». Границы должны отражать
ownership, а не желание всё разделить на максимальное число директорий.

### 6. Сложнее debugging

Баг в cart flow может проходить через widget, provider, repository boundary,
host storage, GraphQL client, request context и backend resolver. Stack длиннее,
чем в обычном mobile app.

### 7. Flutter не получает Rust runtime benefits

Для Leptos/Dioxus часть выгоды может быть runtime-level: Rust components,
server functions, ближе к service layer. Flutter этого не получает. Для Flutter
FFA — это contract convergence, а не code/runtime convergence.

## Когда FFA стоит использовать

FFA хорошо подходит, если у вас есть:

| Условие | Почему это важно |
|---|---|
| Несколько frontend-host-ов | Нужно сохранять parity |
| Модульная платформа | Нужно сохранять module ownership |
| Headless/mobile clients | Нужен canonical API contract |
| Manifest/codegen | Нужен декларативный mounting |
| Admin и storefront | Нельзя смешивать UX и RBAC |
| Долгая жизнь продукта | Drift будет дорогим |
| Несколько команд | Нужны проверяемые boundaries |

Если же приложение маленькое, команда одна, web parity не нужен и modules нет,
обычная Flutter-архитектура может быть лучше.

## Вывод

FFA для Flutter — это более совершенная архитектура не в смысле «проще», а в
смысле «устойчивее к росту платформы».

Обычная Flutter-архитектура оптимизирует скорость разработки одного приложения.
FFA Flutter-архитектура оптимизирует целостность платформы, модульность и
совместимость нескольких frontend-host-ов.

Для RusTok это оправданный trade-off: мы платим сложностью на старте, но
получаем защиту от UI/API/transport/routing drift, сохраняем module ownership и
оставляем Flutter частью платформы, а не отдельным продуктом.

Короткая формула:

> Flutter в FFA — это не отдельный mobile app. Это mobile host платформы,
> который выражает тот же product contract в другом runtime.
