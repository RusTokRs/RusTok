# Архитектура Flutter-приложения для RusTok

## Executive summary

Доступный включённый коннектор в этой сессии — **GitHub**; анализ через него выполнен **только** по репозиторию `RusTokRs/RusTok`, как и требовалось. По результатам анализа видно, что RusTok уже мыслит платформу как **modular monolith** с явным `apps/server` как composition root, с host-приложениями, которые **монтируют** поверхности модулей, но не забирают у модулей ownership доменной логики и UI. Для UI-клиентов **каноническим transport-контрактом** объявлен GraphQL, а внешние/headless/mobile клиенты должны использовать GraphQL и/или REST, не полагаясь на внутренние Leptos `#[server]` функции. Это делает Flutter-клиент не «ещё одной папкой с экранами», а **новым host-клиентом платформы**, который должен уметь подключать module-owned mobile surfaces по manifest-driven правилам платформы. fileciteturn30file0L3-L3 fileciteturn14file0L3-L3 fileciteturn33file0L3-L3

Рекомендуемая стратегия — **feature-first modular Flutter app** с **Clean Architecture-lite**, где:
- **Riverpod** отвечает за state management и DI на уровне приложения и фич,
- **go_router** отвечает за shell-routing, deep links и typed query/path-параметры,
- **graphql_flutter** + **graphql_codegen** дают стандартное подключение к RusTok GraphQL, кэш, subscriptions и типизированные операции,
- module-owned мобильный UI живёт в отдельных пакетах вида `packages/modules/<slug>_mobile`, а общий каркас, токены, роут-контракты, locale/tenant/auth-контекст и GraphQL wiring выносятся в shared-пакеты. Такой подход лучше всего совпадает с текущей платформенной логикой RusTok: shared UI должен оставаться presentational, locale и routing-контракт должны быть host-owned, а модульные поверхности должны подключаться декларативно, а не жёстким копированием в host. fileciteturn17file0L3-L3 fileciteturn31file0L3-L3 fileciteturn32file0L3-L3 fileciteturn36file0L3-L3

С точки зрения Flutter-экосистемы выбор в пользу Riverpod/go_router выглядит наиболее прагматичным: Flutter прямо подчёркивает важность intentional architecture, MVVM/state management и dependency injection для масштабируемых приложений; Riverpod позиционируется как reactive caching/data-binding framework с compile-safety, автоматической обработкой loading/error и тестопригодностью; go_router остаётся официально публикуемым Flutter-пакетом для declarative routing, deep linking, ShellRoute и typed routes; а `graphql_flutter` поддерживает `GraphQLClient`, `AuthLink`, `GraphQLCache`, `HiveStore`, optimistic updates и subscriptions через split между HTTP и WebSocket link. citeturn11view4turn14view0turn14view2turn15view0turn15view2turn18view2turn9view1turn9view3turn9view4turn18view4

Ключевой вывод: **не стоит** строить Flutter-приложение как один большой `lib/features/...` без package boundaries. Для RusTok лучше подходит схема **host app + shared scaffold packages + module-owned mobile packages + generated contracts**. Это снижает UI drift, позволяет повторять параллельную структуру модулей платформы, упрощает поэтапную миграцию и делает mobile-клиент совместимым с manifest-driven будущим платформы. fileciteturn12file0L3-L3 fileciteturn15file0L3-L3

## Что показал анализ RusTok

В репозитории RusTok уже сформулированы почти все архитектурные правила, которые нужны для мобильного клиента. Платформа разделена на host-приложения (`apps/server`, `apps/admin`, `apps/storefront`, `apps/next-admin`, `apps/next-frontend`), платформенные модули и shared/capability crates; UI ownership остаётся у модуля, а host отвечает за routing, shell, locale propagation, auth/session UX и wiring модульных поверхностей. Иными словами, Flutter-клиент логичнее всего вводить как **ещё один host**, а не как набор хаотично скопированных экранов. fileciteturn30file0L3-L3 fileciteturn33file0L3-L3

У платформы уже есть сильный контрактный слой для UI. Manifest модуля (`rustok-module.toml`) может объявлять `provides.admin_ui` и `provides.storefront_ui`, route segment, i18n-пути, child pages и UI-classification; пример `rustok-blog` показывает `ui_classification = "dual_surface"`, admin/storefront route segments, child pages и отдельные locale bundles. Для Flutter это прямой сигнал: mobile-клиенту стоит ввести **аналогичную contract-модель**, например через `provides.mobile_ui`, либо через внешний generated JSON-реестр, который экспортируется из существующих manifest-файлов RusTok. fileciteturn11file0L3-L3 fileciteturn36file0L3-L3

Отдельно важен транспортный контракт. RusTok фиксирует GraphQL как **единый UI-facing surface** для admin/storefront/Next/module-owned UI, а REST — для интеграций, webhooks и ops. Канонические endpoints платформы: `/api/graphql` и `/api/graphql/ws`; серверный GraphQL host реально поднимает POST handler на `/api/graphql` и WebSocket handler на `/api/graphql/ws`. В schema builder уже заданы depth/complexity limits (`12` и `600`), а в UI-модуле `apps/admin/src/features/modules/api.rs` видны реальные query/mutation/subscription contracts, включая `BuildProgress` subscription. Для мобильного клиента это означает, что “стандартное подключение” должно сразу учитывать не только запросы/мутации, но и subscriptions, лимиты сложности и внятные fetch policies. fileciteturn14file0L3-L3 fileciteturn25file0L3-L3 fileciteturn27file0L3-L3 fileciteturn18file0L3-L3

Header-и auth-контракт тоже уже хорошо читаются из репозитория. В Next.js helper для GraphQL RusTok прокидывает `Authorization: Bearer ...`, `X-Tenant-Slug`, при возможности `X-Tenant-ID`, а также `Accept-Language`; GraphQL auth API использует GraphQL-мутции `signIn`, `signUp`, `refreshToken`, а также query `me` и `currentTenant`. Для WebSocket subscriptions сервер требует в `connection_init` как минимум `token` и `tenantSlug`, и опционально `locale`. Это прямой шаблон для mobile-клиента: **весь transport context должен собираться централизованно**, а не в каждой фиче отдельно. fileciteturn24file0L3-L3 fileciteturn37file0L3-L3 fileciteturn25file0L3-L3

RusTok также очень чётко разделяет shared UI и app-local UI. В каталоге UI сказано, что shared tokens и primitives живут отдельно, должны держать parity по назначению и базовому API, но не обязаны иметь зеркальную реализацию один-в-один; shared UI packages должны оставаться **presentational** и не владеть transport/auth/routing/domain behavior. Это критично для Flutter: дублировать нужно не исходный React/Leptos component tree, а **design contract, semantic roles и UX invariants**. То есть `Button`, `Badge`, `Input`, `Select`, `Card`, `Spinner` и layout primitives — в shared `ui_kit`; а сложные domain components вроде product editor, module registry dashboard, SEO panels, users list — в module-owned mobile packages. fileciteturn17file0L3-L3

Наконец, в RusTok уже зафиксированы два очень полезных host-level контракта, которые обязательно стоит перенести в mobile:
- **i18n**: effective locale выбирается host/runtime layer; module-owned UI не должен придумывать свой fallback-chain. fileciteturn31file0L3-L3
- **routing/query contract**: selection state — URL-owned source of truth; используются typed `snake_case` keys вроде `product_id`, `order_id`, `media_id`, `tab`, а generic `id` и camelCase-алиасы не считаются canonical. Для Flutter это значит: route params, deep links и screen selection state нужно проектировать по этим же правилам, чтобы later parity с web/admin оставалась естественной. fileciteturn32file0L3-L3

### Ключевые артефакты репозитория и их вывод для Flutter

| Наблюдение в RusTok | Вывод для Flutter | Источник |
|---|---|---|
| `apps/server` — composition root; hosts монтируют surfaces | Flutter должен быть новым host-клиентом | fileciteturn30file0L3-L3 |
| Module-owned UI остаётся у модуля | Экраны модулей выносить в отдельные mobile packages | fileciteturn12file0L3-L3 fileciteturn33file0L3-L3 |
| GraphQL — канонический UI-facing contract | Основа mobile transport — GraphQL, не REST-first | fileciteturn14file0L3-L3 |
| Сервер поднимает `/api/graphql` и `/api/graphql/ws` | Клиент нужен с HTTP + subscriptions | fileciteturn25file0L3-L3 |
| Shared UI должен быть presentational only | В shared-пакеты не класть auth/routing/domain behavior | fileciteturn17file0L3-L3 |
| Effective locale определяется host-слоем | Locale-provider — в app shell; не в модульных пакетах | fileciteturn31file0L3-L3 |
| Query keys typed и `snake_case` | Deep links и selection state делать по тем же правилам | fileciteturn32file0L3-L3 |
| Manifest объявляет `admin_ui`/`storefront_ui`, i18n и child pages | Для mobile нужен аналогичный registry/export layer | fileciteturn11file0L3-L3 fileciteturn36file0L3-L3 |

## Рекомендуемая архитектура

### Рекомендованный вариант

Для RusTok я рекомендую следующий стек архитектурных решений:

**Feature-first modular structure + Clean Architecture-lite + Riverpod + go_router + module-owned packages**.

Почему именно так:
- Flutter сам акцентирует важность intentional architecture, dependency injection, state management и testability в масштабируемых приложениях. citeturn11view4turn11view2
- Riverpod даёт compile-safety, native async/network patterns, loading/error handling, test readiness и хорошо работает в plain Dart и пакетной архитектуре. citeturn14view0turn14view2turn14view3
- go_router уже решает deep linking, ShellRoute, query/path parameters и redirect flows, что хорошо ложится на RusTok routing contract. citeturn15view0turn15view2
- RusTok сам уже мыслит UI как **host shell + module-owned surfaces**, поэтому modular package boundaries на клиенте естественны. fileciteturn12file0L3-L3 fileciteturn33file0L3-L3

Под **Clean Architecture-lite** я здесь имею в виду не академический “три десятка папок на каждую фичу”, а практический минимум:
- `presentation`
- `application`
- `data`
- `domain` только там, где действительно есть сложные правила и use cases

Для CRUD-heavy экранов доменный слой можно держать тонким. Для auth, permissions, module registry, workflows, SEO/publishing, pricing, offline drafts — наоборот, domain/application слой нужен обязательно.

### Сравнение вариантов

| Вариант | Плюсы | Минусы | Вывод |
|---|---|---|---|---|---|
| **Provider** | Очень простой, официальный экосистемный базис, мало порога входа | Отлично подходит для небольших приложений, но для крупного package-driven клиента быстро упирается в ручную дисциплину и слабее по ergonomics для async/state contracts | Подходит для MVP, не оптимален как основной фундамент |
| **BLoC** | Сильное разделение presentation и business logic, хорошие DI/repository widgets | Больше boilerplate, особенно если модулей и экранов много | Хорош для команд, уже живущих в event-driven стиле |
| **Riverpod** | Compile-safety, async-first ergonomics, test-ready, plain Dart, удобно раскладывается по пакетам | Нужна дисциплина в naming/provider design; команде без опыта понадобится onboarding | **Лучший базовый выбор** |
| **flutter_modular** | Встроенные modular routes + DI, удобен для package/module мышления | Смешивает routing и DI в отдельную мета-архитектуру; в RusTok это может дублировать Riverpod + go_router и делать систему тяжелее | Использовать только если команда уже стандартизировала всё на Modular |
| **Clean Architecture full** | Сильная изоляция и тестируемость | Риск переусложнения, лишняя многослойность для простых фич | Брать как принцип, но в стиле **lite** |

Основания для сравнения: `provider` — wrapper around `InheritedWidget`; `flutter_bloc` подчёркивает separation of presentation/business logic и DI widgets (`BlocProvider`, `RepositoryProvider`); Riverpod декларирует compile safety, async/error handling, test-ready и поддержку WebSocket/network scenarios; `flutter_modular` позиционируется как smart structure для modularized routes и DI. citeturn17view0turn16view0turn14view0turn14view2turn17view1

### Архитектурная схема

```mermaid
flowchart LR
    A[Flutter Host App] --> B[App Shell]
    B --> C[Routing Layer]
    B --> D[App Context]
    D --> D1[Tenant]
    D --> D2[Auth Session]
    D --> D3[Locale]
    D --> D4[Permissions]

    C --> E[Module Registry]
    E --> F1[blog_mobile]
    E --> F2[product_mobile]
    E --> F3[users_mobile]
    E --> F4[workflow_mobile]

    F1 --> G[Application Layer]
    F2 --> G
    F3 --> G
    F4 --> G

    G --> H[Repositories]
    H --> I[GraphQL Client]
    H --> J[Local Storage]
    I --> K[/api/graphql]
    I --> L[/api/graphql/ws]
```

### Практические trade-offs

Если команда RusTok уже сильно привыкла к BLoC, технической ошибки в выборе BLoC не будет. Но с учётом текущего устройства RusTok — manifest-driven, module-owned UI, host-level locale/routing contracts, много async GraphQL reads/writes/subscriptions — Riverpod даёт более дешёвую по когнитивной нагрузке архитектуру. BLoC здесь особенно хорош для очень сложных workflow/state machines, но как **универсальный** фундамент для десятков модульных surfaces Riverpod обычно быстрее и легче в сопровождении. Это мой архитектурный вывод на основании сочетания требований RusTok и свойств библиотек. fileciteturn12file0L3-L3 fileciteturn32file0L3-L3 citeturn14view0turn16view0

## Файловая структура и размещение UI-компонентов

### Базовый принцип размещения

Для RusTok я бы **не стал** делать один пакет `lib/features` с полным содержимым всех модулей платформы. Правильнее сделать monorepo из Flutter-пакетов:

- один **host app**,
- несколько **shared packages**,
- набор **module-owned mobile packages**.

Это буквально повторяет текущую философию RusTok, где host монтирует surfaces, а ownership UI остаётся у модуля. Shared UI должен содержать только presentational primitives и tokens; app shell должен держать routing, auth, locale, nav shell и module registry; module-owned mobile packages — собственные экраны и их data/application layers. fileciteturn17file0L3-L3 fileciteturn33file0L3-L3

### Рекомендуемое дерево репозитория

```text
rustok_mobile/
├── apps/
│   └── rustok_admin_mobile/
│       ├── lib/
│       │   ├── main.dart
│       │   ├── bootstrap.dart
│       │   ├── app.dart
│       │   ├── app_router.dart
│       │   ├── app_shell/
│       │   │   ├── presentation/
│       │   │   │   ├── app_shell.dart
│       │   │   │   ├── app_scaffold.dart
│       │   │   │   ├── app_navigation_bar.dart
│       │   │   │   ├── app_drawer.dart
│       │   │   │   └── app_error_view.dart
│       │   │   ├── application/
│       │   │   │   ├── current_tenant_controller.dart
│       │   │   │   ├── locale_controller.dart
│       │   │   │   └── auth_gate_controller.dart
│       │   │   └── domain/
│       │   │       ├── tenant_context.dart
│       │   │       └── user_session.dart
│       │   ├── routes/
│       │   │   ├── route_names.dart
│       │   │   ├── route_guards.dart
│       │   │   ├── route_codec.dart
│       │   │   └── deep_link_parser.dart
│       │   ├── registry/
│       │   │   ├── mobile_module_registry.dart
│       │   │   ├── generated/
│       │   │   │   └── mobile_manifest.g.dart
│       │   │   └── module_entry_adapter.dart
│       │   └── l10n/
│       │       ├── app_ru.arb
│       │       └── app_en.arb
│       ├── integration_test/
│       └── pubspec.yaml
├── packages/
│   ├── app_core/
│   │   └── lib/
│   │       ├── env/
│   │       ├── errors/
│   │       ├── logging/
│   │       ├── utils/
│   │       └── result/
│   ├── app_ui_kit/
│   │   └── lib/
│   │       ├── tokens/
│   │       ├── theme/
│   │       ├── atoms/
│   │       ├── molecules/
│   │       ├── organisms/
│   │       └── scaffolds/
│   ├── app_graphql/
│   │   └── lib/
│   │       ├── client/
│   │       │   ├── graphql_client_factory.dart
│   │       │   ├── graphql_headers_provider.dart
│   │       │   ├── graphql_error_mapper.dart
│   │       │   ├── graphql_retry_policy.dart
│   │       │   └── graphql_cache_policies.dart
│   │       ├── auth/
│   │       │   ├── auth_session_store.dart
│   │       │   ├── refresh_token_service.dart
│   │       │   └── secure_token_store.dart
│   │       └── generated/
│   │           └── schema.graphql
│   ├── app_route_contracts/
│   │   └── lib/
│   │       ├── query_keys.dart
│   │       ├── route_selection.dart
│   │       ├── route_sanitizer.dart
│   │       └── route_context.dart
│   ├── app_module_contracts/
│   │   └── lib/
│   │       ├── mobile_module_entry.dart
│   │       ├── mobile_nav_meta.dart
│   │       ├── mobile_surface_kind.dart
│   │       └── module_permissions.dart
│   ├── rustok_auth_mobile/
│   │   └── lib/
│   │       ├── presentation/
│   │       ├── application/
│   │       ├── data/
│   │       ├── graphql/
│   │       └── auth_mobile_module.dart
│   ├── rustok_modules_mobile/
│   │   └── lib/
│   │       ├── presentation/
│   │       ├── application/
│   │       ├── data/
│   │       ├── graphql/
│   │       └── modules_mobile_module.dart
│   ├── rustok_blog_mobile/
│   ├── rustok_product_mobile/
│   ├── rustok_users_mobile/
│   └── rustok_workflow_mobile/
├── tooling/
│   ├── build.yaml
│   ├── melos.yaml
│   └── scripts/
├── .github/
│   └── workflows/
└── pubspec.yaml
```

### Что лежит где и зачем

| Путь | Назначение |
|---|---|
| `apps/rustok_admin_mobile` | Host-приложение: запуск, shell, маршрутизация, registry wiring, глобальный контекст |
| `packages/app_core` | Базовые error/result/env/logging-инструменты без UI и domain-specific кода |
| `packages/app_ui_kit` | Общие design tokens, темы, базовые widgets и scaffold-компоненты |
| `packages/app_graphql` | Единая точка сборки GraphQL client, auth/tenant/locale headers, refresh token, mapping ошибок |
| `packages/app_route_contracts` | Typed route/query keys и sanitization по правилам RusTok |
| `packages/app_module_contracts` | Интерфейсы для подключения module-owned mobile packages |
| `packages/rustok_<slug>_mobile` | Экраны, application/data слои и GraphQL-документы конкретного модуля |
| `registry/generated/mobile_manifest.g.dart` | Generated-реестр модулей, child pages, nav metadata, permissions, route segments |
| `tooling/build.yaml` | Настройки `graphql_codegen` и других codegen-задач |
| `integration_test/` | E2E тесты host-приложения |

### Где размещать UI-компоненты и как дублировать UI модулей платформы

Правило должно быть таким.

**В `app_ui_kit`** живут только:
- tokens,
- темы,
- базовые кнопки и поля,
- карточки, бейджи, списочные контейнеры,
- layout/scaffold-примитивы,
- loading/empty/error views.

**В `rustok_<slug>_mobile`** живут:
- screen widgets,
- domain-specific forms,
- complex lists/tables/cards,
- route builders,
- feature controllers,
- GraphQL documents и mappers.

Это соответствует тому, как RusTok уже разделяет shared primitives и app-local/modular UI. fileciteturn17file0L3-L3

Для **дублирования UI существующих модулей** я рекомендую не делать визуальный copy-paste с web-host’ов, а придерживаться трёх уровней parity:

| Уровень parity | Что копировать | Что не копировать |
|---|---|---|
| **Contract parity** | Названия сущностей, permission gates, route semantics, empty/loading/error states, locale keys | Внутренние web-specific implementation details |
| **UX parity** | Информационная архитектура, порядок секций, action hierarchy, form semantics | Точное повторение desktop layouts на мобильном экране |
| **Visual parity** | Tokens, typography, colors, radius, iconography, component intent | Пиксель-в-пиксель сетки web-админки |

Иными словами, экран `modules` в Flutter должен воспроизводить **тот же product contract**, что и web `modules`: те же статусы, те же actions, те же constraints, тот же tenant/auth/locale context — но в мобильной layout-модели. RusTok сам уже подчёркивает parity discipline между Leptos и Next admin и фиксирует единые locale files / FSD-like structure для модульного UI; для Flutter нужно продолжить эту линию, а не строить третий, независимый UX. fileciteturn16file0L3-L3 fileciteturn33file0L3-L3

### Предлагаемый registry-driven поток подключения модулей

```mermaid
flowchart TD
    A[rustok-module.toml] --> B[Export step]
    B --> C[mobile_manifest.json]
    C --> D[codegen -> mobile_manifest.g.dart]
    D --> E[Host App Registry]
    E --> F[Navigation]
    E --> G[Feature Package Mount]
    E --> H[Permissions/Locale/Tenant Context]
```

Практически это означает: либо backend/CI RusTok публикует JSON-реестр модулей для mobile, либо Flutter-репозиторий периодически подтягивает manifest snapshots и генерирует Dart registry code. Второй вариант проще стартовать; первый — архитектурно чище в долгую.

## Каркас проекта и интеграция GraphQL

### Базовый каркас приложения

RusTok уже задаёт правильную host-модель: shell, routing, locale propagation, auth UX, permissions и wiring модульных surfaces должны быть host-owned. Именно это и должно появиться в Flutter-`main.dart`, а не “голый MaterialApp с тремя экранами”. fileciteturn33file0L3-L3

```dart
// apps/rustok_admin_mobile/lib/main.dart
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'app.dart';
import 'bootstrap.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final container = await bootstrap();
  runApp(
    UncontrolledProviderScope(
      container: container,
      child: const RustokAdminMobileApp(),
    ),
  );
}
```

```dart
// apps/rustok_admin_mobile/lib/bootstrap.dart
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:app_core/env/env.dart';
import 'package:app_graphql/client/graphql_client_factory.dart';
import 'package:app_graphql/auth/secure_token_store.dart';

Future<ProviderContainer> bootstrap() async {
  final env = await Env.load();
  final tokenStore = SecureTokenStore();
  final container = ProviderContainer(
    overrides: [
      envProvider.overrideWithValue(env),
      secureTokenStoreProvider.overrideWithValue(tokenStore),
    ],
  );

  // Можно здесь прогреть сессию, tenant и locale.
  return container;
}
```

```dart
// apps/rustok_admin_mobile/lib/app.dart
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'app_router.dart';
import 'package:app_ui_kit/theme/app_theme.dart';

class RustokAdminMobileApp extends ConsumerWidget {
  const RustokAdminMobileApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final router = ref.watch(appRouterProvider);
    final locale = ref.watch(appLocaleProvider);

    return MaterialApp.router(
      title: 'RusTok Mobile',
      routerConfig: router,
      locale: locale,
      theme: buildLightTheme(),
      darkTheme: buildDarkTheme(),
      themeMode: ThemeMode.system,
      debugShowCheckedModeBanner: false,
    );
  }
}
```

Для UI-слоя разумно строиться на **Material 3**: Flutter указывает, что начиная с 3.16 `useMaterial3` включён по умолчанию, а переход к Material 3 предполагает новые компоненты, обновлённые визуальные значения и переход на `ColorScheme.fromSeed`. Для RusTok это хороший baseline: современный системный набор компонентов без лишнего кастомного низкоуровневого UI. citeturn13view3turn13view4

```dart
// packages/app_ui_kit/lib/theme/app_theme.dart
import 'package:flutter/material.dart';
import 'package:flex_color_scheme/flex_color_scheme.dart';

ThemeData buildLightTheme() {
  return FlexThemeData.light(
    useMaterial3: true,
    scheme: FlexScheme.indigo,
    visualDensity: VisualDensity.standard,
  );
}

ThemeData buildDarkTheme() {
  return FlexThemeData.dark(
    useMaterial3: true,
    scheme: FlexScheme.indigo,
    visualDensity: VisualDensity.standard,
  );
}
```

### Маршрутизация

Для RusTok особенно важно, чтобы routing был:
- declarative,
- deep-link friendly,
- shell-capable,
- совместим с typed query/path contract платформы.

Именно здесь `go_router` даёт наилучший баланс: URL-based API, deep links, redirects, sub-routes и `ShellRoute` для постоянного shell/navigation bar. citeturn15view0turn15view2

```dart
// apps/rustok_admin_mobile/lib/app_router.dart
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'app_shell/presentation/app_shell.dart';
import 'registry/mobile_module_registry.dart';
import 'routes/route_guards.dart';
import 'features/auth/presentation/sign_in_screen.dart';
import 'features/home/presentation/home_screen.dart';

final appRouterProvider = Provider<GoRouter>((ref) {
  final isSignedIn = ref.watch(isSignedInProvider);
  final registry = ref.watch(mobileModuleRegistryProvider);

  return GoRouter(
    initialLocation: '/home',
    redirect: (context, state) {
      final loggingIn = state.matchedLocation == '/sign-in';
      if (!isSignedIn && !loggingIn) return '/sign-in';
      if (isSignedIn && loggingIn) return '/home';
      return null;
    },
    routes: [
      GoRoute(
        path: '/sign-in',
        builder: (_, __) => const SignInScreen(),
      ),
      ShellRoute(
        builder: (_, __, child) => AppShell(child: child),
        routes: [
          GoRoute(
            path: '/home',
            builder: (_, __) => const HomeScreen(),
          ),
          ...registry.routes,
        ],
      ),
    ],
  );
});
```

### DI и registry-driven подключение модулей

В RusTok host-приложение не должно становиться владельцем модульной логики; оно должно только монтировать surfaces и прокидывать контекст. Ровно это и нужно сделать в Flutter через registry. fileciteturn33file0L3-L3

```dart
// packages/app_module_contracts/lib/mobile_module_entry.dart
import 'package:flutter/widgets.dart';
import 'package:go_router/go_router.dart';

abstract interface class MobileModuleEntry {
  String get slug;
  String get navLabel;
  int get navOrder;
  List<RouteBase> buildRoutes();
  Widget buildNavIcon(BuildContext context);
}
```

```dart
// apps/rustok_admin_mobile/lib/registry/mobile_module_registry.dart
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'package:rustok_blog_mobile/blog_mobile_module.dart';
import 'package:rustok_modules_mobile/modules_mobile_module.dart';

final mobileModuleRegistryProvider = Provider<MobileModuleRegistry>((ref) {
  final entries = [
    BlogMobileModule(),
    ModulesMobileModule(),
    // далее auto-generated wiring
  ]..sort((a, b) => a.navOrder.compareTo(b.navOrder));

  return MobileModuleRegistry(
    entries: entries,
    routes: entries.expand((e) => e.buildRoutes()).toList(),
  );
});

class MobileModuleRegistry {
  MobileModuleRegistry({
    required this.entries,
    required this.routes,
  });

  final List<dynamic> entries;
  final List<RouteBase> routes;
}
```

### Стандартное подключение GraphQL для RusTok

Официальные рекомендации GraphQL over HTTP задают очень понятную основу: GraphQL обычно работает через **один endpoint**, запросы идут по `POST` c `application/json`, а тело содержит `query`, `operationName`, `variables`, `extensions`; ответ использует top-level keys `data`, `errors`, `extensions`, причём partial success допустим как `data + errors`. RusTok со своей стороны фиксирует UI-facing endpoint `/api/graphql`, WebSocket endpoint `/api/graphql/ws` и использует дополнительные контекстные headers (`Authorization`, `X-Tenant-Slug`, `X-Tenant-ID`, `Accept-Language`) и payload для WS-инициализации (`token`, `tenantSlug`, `locale`). citeturn10view7turn10view8turn10view9turn10view0turn10view1 fileciteturn24file0L3-L3 fileciteturn25file0L3-L3

```mermaid
sequenceDiagram
    participant App as Flutter App
    participant Store as Secure Token Store
    participant GQL as GraphQL Client
    participant API as RusTok /api/graphql
    participant WS as RusTok /api/graphql/ws

    App->>Store: read access/refresh token
    App->>GQL: build headers/context
    GQL->>API: POST /api/graphql
    Note right of GQL: Authorization\nX-Tenant-Slug\nX-Tenant-ID\nAccept-Language
    API-->>GQL: { data, errors?, extensions? }

    App->>GQL: subscribe()
    GQL->>WS: connection_init
    Note right of GQL: token\ntenantSlug\nlocale
    WS-->>GQL: realtime events
```

```dart
// packages/app_graphql/lib/client/graphql_client_factory.dart
import 'package:graphql_flutter/graphql_flutter.dart';

class GraphqlClientFactory {
  GraphQLClient create({
    required String apiBaseUrl,
    required String wsBaseUrl,
    required Future<String?> Function() accessToken,
    required Future<String?> Function() tenantSlug,
    required Future<String?> Function() tenantId,
    required Future<String> Function() localeTag,
  }) {
    final httpLink = HttpLink('$apiBaseUrl/api/graphql');

    final authLink = AuthLink(
      getToken: () async {
        final token = await accessToken();
        return token == null ? '' : 'Bearer $token';
      },
    );

    final contextLink = Link.function((request, [forward]) async* {
      final headers = <String, String>{
        'Accept-Language': await localeTag(),
      };

      final slug = await tenantSlug();
      final id = await tenantId();

      if (slug != null && slug.isNotEmpty) {
        headers['X-Tenant-Slug'] = slug;
      }
      if (id != null && id.isNotEmpty) {
        headers['X-Tenant-ID'] = id;
      }

      final next = request.updateContextEntry<HttpLinkHeaders>(
        (existing) => HttpLinkHeaders(
          headers: {
            ...?existing?.headers,
            ...headers,
          },
        ),
      );

      yield* forward!(next);
    });

    final wsLink = WebSocketLink(
      '$wsBaseUrl/api/graphql/ws',
      config: SocketClientConfig(
        autoReconnect: true,
        initialPayload: () async => <String, dynamic>{
          'token': await accessToken(),
          'tenantSlug': await tenantSlug(),
          'locale': await localeTag(),
        },
      ),
    );

    final link = Link.split(
      (request) => request.isSubscription,
      wsLink,
      Link.from([authLink, contextLink, httpLink]),
    );

    return GraphQLClient(
      link: link,
      cache: GraphQLCache(store: HiveStore()),
      defaultPolicies: DefaultPolicies(
        query: Policies(
          fetch: FetchPolicy.cacheAndNetwork,
        ),
        mutate: Policies(
          fetch: FetchPolicy.noCache,
        ),
        watchQuery: Policies(
          fetch: FetchPolicy.cacheAndNetwork,
        ),
      ),
    );
  }
}
```

`graphql_flutter` требует `GraphQLClient` с `link` и `cache`, поддерживает `AuthLink`, `GraphQLCache`, `HiveStore`, optimistic mutations и subscriptions через split на subscription link и обычный terminating link. Это хорошо совпадает с RusTok transport-контрактом. citeturn18view2turn9view3turn9view4turn18view4turn9view1

### Авторизация и refresh

RusTok уже показывает GraphQL-мутции `signIn`, `signUp`, `refreshToken`, а также query `me` и `currentTenant`. На мобильном клиенте я рекомендую такую политику:

- `accessToken` и `refreshToken` — только в `flutter_secure_storage`,
- `tenantSlug` и non-sensitive UI prefs — в `shared_preferences`,
- refresh — централизованный сервис в `app_graphql/auth`,
- повтор запроса — **один раз** после успешного refresh,
- при провале refresh — hard logout и очистка кэша/сессии. fileciteturn37file0L3-L3 citeturn6view4turn6view5

```dart
abstract interface class SessionStore {
  Future<AuthSession?> read();
  Future<void> write(AuthSession session);
  Future<void> clear();
}

class AuthSession {
  const AuthSession({
    required this.accessToken,
    required this.refreshToken,
    required this.tenantSlug,
  });

  final String accessToken;
  final String refreshToken;
  final String tenantSlug;
}
```

### Кэширование, обработка ошибок и subscriptions

Для RusTok я бы рекомендовал **не обещать офлайн-first**, если это отдельно не подтверждено требованиями. Сейчас разумный baseline такой:

- persisted GraphQL cache для чтений;
- secure session store;
- local drafts только для реально нужных фич;
- subscriptions там, где есть live surfaces: build progress, возможно notifications, audit/event streams;
- сложную offline-синхронизацию отложить до explicit product requirement.

Это соответствует и природе platform admin-клиента, и общим механизмам GraphQL demand control/security. GraphQL Foundation отдельно рекомендует demand control через pagination, depth limiting, breadth/batch limiting и rate limiting; RusTok уже ограничивает depth/complexity на сервере, значит мобильному клиенту не стоит плодить «fat queries» ради удобства UI. citeturn10view3turn10view4turn10view5 fileciteturn27file0L3-L3

### Пример GraphQL-документов и маппинга

```graphql
# packages/rustok_modules_mobile/lib/graphql/list_modules.graphql
query ListModules {
  moduleRegistry {
    moduleSlug
    name
    description
    version
    kind
    dependencies
    enabled
    ownership
    trustLevel
    recommendedAdminSurfaces
    showcaseAdminSurfaces
  }
}
```

```graphql
# packages/rustok_auth_mobile/lib/graphql/sign_in.graphql
mutation SignIn($input: SignInInput!) {
  signIn(input: $input) {
    accessToken
    refreshToken
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

```graphql
# packages/rustok_modules_mobile/lib/graphql/build_progress.graphql
subscription BuildProgress {
  buildProgress {
    buildId
    status
    stage
    progress
    releaseId
    errorMessage
  }
}
```

```yaml
# tooling/build.yaml
targets:
  $default:
    builders:
      graphql_codegen:
        options:
          clients:
            - graphql
          scalars:
            DateTime:
              type: DateTime
              fromJsonFunctionName: dateTimeFromJson
              toJsonFunctionName: dateTimeToJson
              import: package:app_core/utils/scalars.dart
```

```dart
// packages/rustok_modules_mobile/lib/data/modules_repository.dart
import 'package:graphql/client.dart';
import '../graphql/list_modules.graphql.dart';

class ModulesRepository {
  ModulesRepository(this._client);

  final GraphQLClient _client;

  Future<List<ModuleSummary>> listModules() async {
    final result = await _client.query$ListModules(
      Options$Query$ListModules(
        fetchPolicy: FetchPolicy.cacheAndNetwork,
      ),
    );

    if (result.hasException) {
      throw mapGraphqlException(result.exception);
    }

    final items = result.parsedData?.moduleRegistry ?? const [];
    return items.map(ModuleSummary.fromGql).toList();
  }
}

class ModuleSummary {
  const ModuleSummary({
    required this.slug,
    required this.name,
    required this.description,
    required this.enabled,
  });

  final String slug;
  final String name;
  final String description;
  final bool enabled;

  factory ModuleSummary.fromGql(Query$ListModules$moduleRegistry gql) {
    return ModuleSummary(
      slug: gql.moduleSlug,
      name: gql.name,
      description: gql.description,
      enabled: gql.enabled,
    );
  }
}
```

### Шаблоны экранов и виджетов

```dart
// packages/app_ui_kit/lib/scaffolds/app_screen.dart
import 'package:flutter/material.dart';

class AppScreen extends StatelessWidget {
  const AppScreen({
    super.key,
    required this.title,
    required this.body,
    this.fab,
  });

  final String title;
  final Widget body;
  final Widget? fab;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: Text(title)),
      body: SafeArea(child: body),
      floatingActionButton: fab,
    );
  }
}
```

```dart
// packages/app_ui_kit/lib/scaffolds/async_screen.dart
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

class AsyncScreen<T> extends StatelessWidget {
  const AsyncScreen({
    super.key,
    required this.value,
    required this.data,
  });

  final AsyncValue<T> value;
  final Widget Function(T data) data;

  @override
  Widget build(BuildContext context) {
    return value.when(
      data: data,
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (error, stack) => Center(
        child: Text('Ошибка: $error'),
      ),
    );
  }
}
```

```dart
// packages/rustok_modules_mobile/lib/presentation/modules_screen.dart
class ModulesScreen extends ConsumerWidget {
  const ModulesScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final modules = ref.watch(modulesControllerProvider);

    return AppScreen(
      title: 'Modules',
      body: AsyncScreen(
        value: modules,
        data: (items) => ListView.builder(
          itemCount: items.length,
          itemBuilder: (_, index) {
            final item = items[index];
            return ListTile(
              title: Text(item.name),
              subtitle: Text(item.description),
              trailing: Switch(
                value: item.enabled,
                onChanged: (_) {},
              ),
            );
          },
        ),
      ),
    );
  }
}
```

## Библиотеки и самописные модули

### Рекомендуемые готовые библиотеки

Ниже — рекомендуемый набор библиотек для стартового production-scaffold. Версии ниже взяты с официальных страниц библиотек по состоянию на **22 мая 2026 года**. Для state/routing/DI это `flutter_riverpod 3.3.1`, `go_router 17.2.3`, `get_it 9.2.1`, `flutter_bloc 9.1.1`, `provider 6.1.5+1`, `flutter_modular 6.4.1`, `auto_route 11.1.0`; для data/GraphQL — `graphql_flutter 5.3.0`, `graphql_codegen 3.0.1`, `build_runner 2.15.0`, `dio 5.9.2`, `flutter_secure_storage 10.2.0`, `shared_preferences 2.5.5`, `drift 2.33.0`, `isar 3.1.0+1`, `json_serializable 6.14.0`; для UI/testing — `flutter_svg 2.3.0`, `flex_color_scheme 8.4.0`, `skeletonizer 2.1.3`, `mocktail 1.0.5`, `patrol 4.6.0`. Отдельно важно, что `golden_toolkit 0.15.0` на pub.dev помечен как **discontinued**, поэтому для нового проекта я бы его не брал. citeturn6view1turn6view0turn6view6turn16view0turn17view0turn17view1turn17view2turn6view2turn8view5turn8view4turn6view3turn6view4turn6view5turn8view1turn8view0turn8view2turn8view7turn8view8turn8view9turn7view0turn8view10turn6view7

| Категория | Рекомендовано | Почему | Альтернатива | Когда выбрать альтернативу |
|---|---|---|---|---|---|
| State management | **flutter_riverpod** | Лучший баланс async ergonomics, modularity, testability | `flutter_bloc`, `provider` | BLoC — если команда уже на event-driven модели; Provider — для маленького MVP |
| Routing | **go_router** | Deep links, ShellRoute, query/path params, стабильность | `auto_route`, `flutter_modular` | `auto_route` — если нужна сильная codegen-типизация маршрутов |
| DI | **Riverpod providers** как primary DI; `get_it` только точечно | Меньше дублирования DI-механизмов | `get_it` | Только для низкоуровневых сервисов без UI-context |
| GraphQL client | **graphql_flutter** | Прямо покрывает link/cache/auth/subscriptions | `ferry_flutter` | Если нужна более жёсткая typed-архитектура и команда готова к steeper setup |
| GraphQL codegen | **graphql_codegen** + `build_runner` | Типизированные операции и маппинг schema/documents | ручной gql client | Только на очень маленьком проекте |
| HTTP fallback/REST | **dio** | Interceptors, cancelation, adapters | `http` | Если нужен совсем простой клиент |
| Secure storage | **flutter_secure_storage** | Правильное место для токенов и секретов | — | Без альтернативы в baseline |
| Preferences | **shared_preferences** | Простые non-critical настройки | — | Только для prefs, не для критичных данных |
| Local DB | **drift** | Более свежая stable-линейка и хороший контроль SQL/queries | `isar` | `isar` — если нужен object-store стиль и команда уже его знает |
| JSON models | **json_serializable** | Стабильный официальный путь к сериализации | ручной код | Если моделей мало |
| SVG assets | **flutter_svg** | Практический стандарт для иконографии | PNG-only | Если SVG вообще не используются |
| Theming | **flex_color_scheme** | Быстрый production-grade старт поверх Material 3 | только ThemeData вручную | Если нужен предельно минималистичный theming |
| Skeleton loading | **skeletonizer** | Быстро даёт consistent loading states | самописные placeholders | Если дизайн-система совсем уникальна |
| Unit/widget mocks | **mocktail** | Не требует code generation | `mockito` | Если команде нужен именно Mockito flow |
| Integration/E2E | **integration_test** baseline + `patrol` optional | Patrol закрывает native interactions, где `integration_test` неудобен | только `integration_test` | Если native prompts/permissions не тестируются |

### Почему выбранные лучше именно для RusTok

Для RusTok главный критерий — не “самая модная библиотека”, а **совместимость с архитектурой модульной платформы**. Поэтому:
- `go_router` лучше `flutter_modular` как базовый роутинг, потому что он решает routing-плоскость без навязывания отдельной DI/meta-архитектуры поверх RusTok host-shell.
- Riverpod лучше Provider как основной state/DI фундамент, потому что RusTok явно тяготеет к package boundaries, async GraphQL flows, strict contracts и тестопригодности.
- `graphql_flutter + graphql_codegen` лучше ручного GraphQL, потому что платформа уже богата schema-driven поверхностями, subscriptions и сложными query/mutation contracts; ручной string-based клиент быстро станет источником drift и ошибок.
- `drift` как optional offline/data-db выглядит надёжнее для long-lived production mobile, чем брать object-store только потому, что он “быстрее стартует”. При этом если офлайн-режим ограничен лишь persisted GraphQL cache + drafts, отдельная БД может вообще не понадобиться на первом этапе. Это уже продуктовый выбор, а не обязательный обязательный слой.

### Предлагаемые самописные библиотеки и модули

Вот набор **самописных** пакетов, которые я бы считал не “лишней абстракцией”, а архитектурным минимумом.

| Пакет/модуль | Ответственность | Почему нужен именно как отдельный пакет |
|---|---|---|
| `app_core` | env, Result/Failure, logging, retry utils, typed errors | Чтобы не дублировать инфраструктуру по фичам |
| `app_ui_kit` | tokens, themes, base widgets, scaffolds | Чтобы держать parity с RusTok shared UI contract |
| `app_graphql` | client factory, headers/ws payload, refresh policy, error mapping | Чтобы auth/tenant/locale transport context не расползся по всем фичам |
| `app_route_contracts` | query keys, route sanitization, deep-link parsing | Чтобы mirror-ить RusTok route-selection contract |
| `app_module_contracts` | интерфейсы module entry, nav metadata, permissions wiring | Чтобы модульные mobile packages были truly pluggable |
| `generated_module_registry` | generated wiring из manifest/export | Чтобы host не редактировался вручную при каждом новом модуле |

### Примеры публичных интерфейсов

```dart
abstract interface class GraphqlHeadersProvider {
  Future<Map<String, String>> buildHttpHeaders();
  Future<Map<String, Object?>> buildWsPayload();
}
```

```dart
abstract interface class RouteSelectionSanitizer {
  Map<String, String> sanitize(String routeName, Map<String, String> raw);
}
```

```dart
abstract interface class MobileModuleRegistryEntry {
  String get slug;
  bool get adminSurface;
  List<RouteBase> routes();
  List<MobileNavItem> navItems();
}
```

```dart
abstract interface class FailureMapper {
  AppFailure map(Object error, StackTrace stackTrace);
}
```

```dart
sealed class AppFailure {
  const AppFailure();
}

final class UnauthorizedFailure extends AppFailure {
  const UnauthorizedFailure();
}

final class NetworkFailure extends AppFailure {
  const NetworkFailure(this.message);
  final String message;
}

final class GraphqlFailure extends AppFailure {
  const GraphqlFailure(this.message, {this.code});
  final String message;
  final String? code;
}
```

## CI/CD, риски и рекомендации по миграции

### Что стоит перенять из текущего CI RusTok

Текущий CI RusTok уже очень строгий: formatting, clippy/check, platform contract validation, audit/deny, documentation, unused dependencies, coverage, SBOM, сборка серверных и UI-артефактов, а также отдельные jobs для Next-приложений. Для Flutter-репозитория имеет смысл взять **тот же уровень дисциплины**, а не ограничиться `flutter test`. fileciteturn29file0L3-L3

### Рекомендуемый pipeline для Flutter

Минимальный production pipeline:
1. `dart format --set-exit-if-changed .`
2. `flutter analyze`
3. codegen-check: `dart run build_runner build --delete-conflicting-outputs` + `git diff --exit-code`
4. unit/widget tests + coverage
5. integration tests
6. Android build
7. iOS build
8. артефакты + релизная публикация для internal channels
9. contract checks: GraphQL schema snapshot, generated registry snapshot, route/query contract tests

### Шаблон GitHub Actions

```yaml
name: Flutter CI

on:
  push:
    branches: ["**"]
  pull_request:
    branches: ["**"]
  workflow_dispatch:

jobs:
  static:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5

      - uses: subosito/flutter-action@v2
        with:
          channel: stable

      - name: Install dependencies
        run: flutter pub get

      - name: Format
        run: dart format --set-exit-if-changed .

      - name: Codegen
        run: dart run build_runner build --delete-conflicting-outputs

      - name: Verify generated files committed
        run: git diff --exit-code

      - name: Analyze
        run: flutter analyze

  tests:
    runs-on: ubuntu-latest
    needs: static
    steps:
      - uses: actions/checkout@v5
      - uses: subosito/flutter-action@v2
        with:
          channel: stable

      - name: Install dependencies
        run: flutter pub get

      - name: Unit and widget tests
        run: flutter test --coverage

      - name: Upload coverage
        uses: actions/upload-artifact@v4
        with:
          name: coverage
          path: coverage/lcov.info

  integration-android:
    runs-on: macos-latest
    needs: tests
    steps:
      - uses: actions/checkout@v5
      - uses: subosito/flutter-action@v2
        with:
          channel: stable

      - name: Install dependencies
        run: flutter pub get

      - name: Run integration tests
        run: flutter test integration_test

  build-android:
    runs-on: ubuntu-latest
    needs: tests
    steps:
      - uses: actions/checkout@v5
      - uses: subosito/flutter-action@v2
        with:
          channel: stable

      - name: Install dependencies
        run: flutter pub get

      - name: Build Android App Bundle
        run: flutter build appbundle --release --dart-define=FLAVOR=prod

      - name: Upload Android bundle
        uses: actions/upload-artifact@v4
        with:
          name: android-aab
          path: build/app/outputs/bundle/release/*.aab

  build-ios:
    runs-on: macos-latest
    needs: tests
    steps:
      - uses: actions/checkout@v5
      - uses: subosito/flutter-action@v2
        with:
          channel: stable

      - name: Install dependencies
        run: flutter pub get

      - name: Build iOS without codesign
        run: flutter build ipa --release --no-codesign --dart-define=FLAVOR=prod

      - name: Upload iOS artifact
        uses: actions/upload-artifact@v4
        with:
          name: ios-ipa
          path: build/ios/ipa/*.ipa
```

### Риски и митигации

| Риск | Почему реален для RusTok | Митигация |
|---|---|---|
| **UI drift** между Flutter и web-host’ами | RusTok уже поддерживает несколько UI-стеков | Единые tokens, locale keys, route/query contracts, review checklist на parity |
| **Разъезд transport-контрактов** | Платформа уже богата GraphQL/REST/WS surfaces | Весь transport context только через `app_graphql`, никаких feature-local клиентов |
| **Over-modularization** | Легко превратить всё в слишком большое число пакетов | Пакет выделять только если есть ownership boundary или повторное использование |
| **Schema drift / codegen drift** | GraphQL surface живой и растущий | Schema snapshot + codegen CI + generated files checked in |
| **Сложность офлайна** | Не указано, нужен ли offline-first | Начать с persisted cache + secure session; Drift/outbox только после подтверждения требований |
| **Сложный routing state** | В RusTok route/query contract уже строгий | Единый sanitizer и typed route keys package |
| **Auth/tenant bugs** | У RusTok контекст tenant/auth/locale обязателен | HTTP/WS заголовки и payload собирать централизованно и контрактно тестировать |

### Фазный план реализации (без дубляжа)

Ниже — единый implementation plan, который **ссылается на уже описанные разделы** документа вместо повторения одних и тех же решений.


_Легенда статусов: `⬜ Planned` — не начато, `🟡 In progress` — в работе, `✅ Done` — выполнено._

| Статус | Фаза | Объём работ | На что опираемся в этом документе | Definition of Done |
|---|---|---|---|---|
| 🟡 In progress | **Phase 0 — Foundation** | Создать `host app`, app shell, тему, auth session store, GraphQL client factory, route contracts и сразу зафиксировать FFA-baseline для Flutter (единый product/capability contract без Flutter-specific API). | [Базовый каркас приложения](#базовый-каркас-приложения), [Маршрутизация](#маршрутизация), [Стандартное подключение GraphQL для RusTok](#стандартное-подключение-graphql-для-rustok), [Авторизация и refresh](#авторизация-и-refresh) | Работают login + `me` + `currentTenant`, есть базовый shell и deep-link вход в защищённый экран; FFA-baseline зафиксирован с начала разработки. |
| 🟡 In progress | **Phase 1 — Pilot module** | Внедрить один module-owned пакет (рекомендовано: `modules` или `blog`) с реальным E2E флоу. | [Файловая структура и размещение UI-компонентов](#файловая-структура-и-размещение-ui-компонентов), [DI и registry-driven подключение модулей](#di-и-registry-driven-подключение-модулей), [Шаблоны экранов и виджетов](#шаблоны-экранов-и-виджетов) | Один бизнес-сценарий модуля проходит end-to-end в мобильном host без feature-local transport-клиентов. |
| 🟡 In progress | **Phase 2 — Registry/codegen** | Подключить generated mobile registry из manifest/export и убрать ручное wiring в host. Заложить расширяемость только под mobile-safe host metadata: nested routes, nav, locale namespace и permission gates. | [Предлагаемый registry-driven поток подключения модулей](#предлагаемый-registry-driven-поток-подключения-модулей), [Предлагаемые самописные библиотеки и модули](#предлагаемые-самописные-библиотеки-и-модули) | Новый модуль подключается через manifest/codegen без правок в навигационном каркасе host; registry не содержит server-side FBA/provider metadata. |
| ⬜ Planned | **Phase 3 — Parity expansion** | Перенести остальные high-value модули, закрепить route/i18n/permission parity и единые error/loading/empty паттерны. | [Где размещать UI-компоненты и как дублировать UI модулей платформы](#где-размещать-ui-компоненты-и-как-дублировать-ui-модулей-платформы), [Кэширование, обработка ошибок и subscriptions](#кэширование-обработка-ошибок-и-subscriptions) | Покрыты основные operator flows; контракты query keys/locale/permissions не расходятся с web-host правилами. |
| ⬜ Planned | **Phase 4 — Hardening & release** | E2E, performance, observability, crash reporting, release pipeline (Android/iOS), rollout gates. | [Рекомендуемый pipeline для Flutter](#рекомендуемый-pipeline-для-flutter), [Шаблон GitHub Actions](#шаблон-github-actions), [Риски и митигации](#риски-и-митигации) | Готовы alpha/beta релизы, pipeline стабилен, критичные риски закрыты митигациями. |
| ⬜ Planned | **Phase 5 — Offline/advanced sync (optional)** | Добавить офлайн-сценарии только после продуктового подтверждения требований. | [Open questions и ограничения](#open-questions-и-ограничения), [Риски и митигации](#риски-и-митигации) | Есть утверждённые offline requirements и реализована целевая стратегия sync/outbox. |


#### Операционный статус плана (обновлено: 2026-06-13, storefront GraphQL evidence hardening)

- **FFA в плане отмечен:** ✅ Да. FFA-baseline явно зафиксирован в `Phase 0 — Foundation` и отдельно закреплён в anti-drift guardrail разделе.
- **Текущий фокус выполнения:** `Phase 1 — Pilot module` остаётся `🟡 In progress` для admin/operator flow, но его ключевые seams уже закрыты: module-owned package, shared GraphQL transport, `toggleModule`, hydrated `me.permissions`, retry/compensation actions и dedicated operation history/recovery screen используют существующие lifecycle GraphQL contracts без feature-local transport-клиентов. `Phase 2 — Registry/codegen` работает как supporting track: admin и storefront registry/codegen проверяются локальными deterministic checks, а storefront registry уже используется для package discovery и generated home navigation.
- **Storefront track:** `rustok_frontend_mobile` отделён от admin/operator host; catalog/cart package работает через host-owned repository seam, durable cart id seam и source-backed GraphQL contract verifier для `storefrontSearch`, `storefrontCart` и create/add/update/remove cart mutations.
- **Следующая точка контроля:** превратить source-backed storefront verifier в live schema/test-server CI job, когда доступен Flutter SDK или lightweight server harness; до этого не расширять Flutter API surface и не добавлять package-local transport/storage fallback chains.

#### Ближайший execution backlog (Phase 1 pilot)

1. **Registry schema freeze (FFA-safe):** зафиксировать минимальный mobile registry contract (`module_slug`, `surface_kind`, `route_segment`, `child_pages`, `permissions`, `locale_namespace`) без Flutter-only полей.
2. **Codegen pipeline:** добавить reproducible генерацию `mobile_manifest.g.dart` из manifest snapshot + CI-проверку diff generated-файлов (в работе: локальная verify-команда уже фиксирует stale-state для manifest + snapshot).
3. **Host integration seam:** подключить registry через единый adapter-слой (`module_entry_adapter`) и убрать ручное перечисление модулей в shell routing/nav.
4. **Pilot gate:** `Phase 1 — Pilot module` переведён в `🟡 In progress`; первый mutation-backed operator action реализован через `toggleModule`, permission gate подключён к hydrated `me.permissions`, retryable post-hook failures показывают recovery feedback и retry/compensation actions; operation history/recovery screen добавлен как следующий audit-oriented слой pilot-флоу.



#### Sprint continuation (FFA-first, ближайшие 2 PR)

| PR | Цель | Обязательные артефакты | FFA-критерий приёмки |
|---|---|---|---|
| **PR-A: Registry contract freeze** | Зафиксировать минимальный mobile contract без Flutter-only расширений | `mobile_manifest` schema snapshot, таблица compatibility-правил, changelog полей | Contract описывает capability/surface, а не runtime-детали Flutter; отсутствуют mobile-exclusive API поля. |
| **PR-B: Codegen + host seam** | Сделать deterministic codegen и единый adapter в host | reproducible generation pipeline, diff-check generated файлов в CI, `module_entry_adapter` как единственная точка подключения | Новый модуль подключается декларативно через registry без ручной правки shell-routing; route/locale/auth контекст остаётся host-owned. |

**Правило перехода в `Phase 1`:** после merge `PR-A` и `PR-B` взять pilot-флоу `modules` (предпочтительно) или `blog`, и зафиксировать первое E2E-доказательство parity (login → module list/detail → обратно в shell) без feature-local transport-клиентов.

#### Inline comments resolution log (update 2026-05-24)

Чтобы снять замечания по предыдущему PR и избежать «плана ради плана», фиксируем обязательные выходы по каждому ближайшему шагу:

- **PR-A считается закрытым только при наличии артефактов в репозитории**: snapshot schema, compatibility matrix и field changelog в одном месте документации трека.
- **PR-B считается закрытым только при проверяемом CI-сигнале**: deterministic codegen + `git diff --exit-code` для generated-файлов после генерации.
- **Переход в Phase 1 запрещён без evidence-блока**: ссылка на конкретный E2E прогон pilot-флоу (`modules`/`blog`) и отметка FFA-checklist без исключений.

#### Concrete deliverables (Phase 2 execution board)

| Deliverable | Owner zone | Verification command / signal | Status |
|---|---|---|---|
| `mobile_manifest` minimal schema snapshot | `rustok_mobile/tooling` + docs трека | schema snapshot обновлён и закоммичен | ✅ Done |
| Compatibility matrix (`required/optional/deprecated`) | `docs/research/flutter.md` | матрица заполнена для всех contract-полей | ✅ Done |
| Deterministic codegen job | mobile CI pipeline | `dart run build_runner build --delete-conflicting-outputs` + `git diff --exit-code` | ⬜ Planned |
| Local deterministic codegen check | `rustok_mobile/tooling/scripts/check_mobile_codegen.py` | generator CLI runs into temp outputs and diffs against committed manifest/snapshot | ✅ Done |
| Generated-file diff diagnostics | `rustok_mobile/tooling/scripts/verify_mobile_manifest.py` | stale manifest/snapshot failures print unified diff + regeneration command | ✅ Done |
| Host adapter seam (`module_entry_adapter`) | `apps/rustok_admin_mobile` | registry подключается без ручного списка модулей в shell-router | ✅ Done |
| Manifest-driven nav icon mapping | `apps/rustok_admin_mobile` | host nav использует `nav.icon` из generated manifest и fallback по module metadata без ручного списка routes | ✅ Done |
| Pilot E2E evidence (modules/blog) | `rustok_mobile/apps/rustok_admin_mobile/test/pilot_modules_flow_test.dart` + `rustok_mobile/packages/rustok_modules_mobile/test/modules_mobile_screen_test.dart` | authenticated shell → GraphQL-backed module list → module detail route → shell back; package widget test verifies `toggleModule` action refresh and operation history/recovery actions | 🟡 In progress |
| Storefront catalog/cart GraphQL source verifier | `rustok_mobile/tooling/scripts/verify_storefront_graphql_contract.py` + `rustok_mobile/tooling/tests/test_storefront_cart_graphql_contract.py` | `python3 rustok_mobile/tooling/scripts/verify_storefront_graphql_contract.py` verifies catalog/cart operation documents against existing server-owned surfaces | ✅ Done |
| Storefront live schema/test-server promotion | mobile CI pipeline + server test harness | execute the same catalog/cart operation set against a live schema or test server, then keep source verifier as fast preflight | ⬜ Planned |
| Storefront package mapping expansion gate | `apps/rustok_frontend_mobile` + module-owned packages | add a new package-backed route only when a module-owned package exists and consumes a host-provided repository | ⬜ Planned |

#### PR-D evidence pack (Flutter storefront mobile host)

**Storefront mobile host:** `rustok_mobile/apps/rustok_frontend_mobile`.

Добавлен отдельный customer-facing Flutter host, потому что web storefront уже существует как `apps/storefront` + `apps/next-frontend`, а мобильный storefront не должен смешиваться с admin/operator приложением:
- host-owned runtime context — `StorefrontRuntimeContext` и `storefrontGraphQlConfigProvider` собирают `tenantSlug`, `locale` и `/api/graphql` endpoint централизованно;
- route contract — shell содержит `home/catalog/cart/profile`, монтирует module-owned catalog/cart package и использует generated storefront registry для reserved `/modules/:routeSegment`;
- FFA rule — новый host не вводит Flutter-only backend API и не копирует canonical routing/storage logic из web storefront;
- evidence — `rustok_mobile/apps/rustok_frontend_mobile/test/storefront_router_test.dart` проверяет home runtime context, catalog/cart routes и generic module placeholder route.

Следующий storefront mobile шаг: довести cart-часть до shared-transport backend implementation и подключить storefront packages через generated registry, синхронно с `docs/UI/storefront.md` и web storefront parity rules.

#### PR-C evidence pack (Phase 1 pilot modules flow)

**Pilot package:** `rustok_mobile/packages/rustok_modules_mobile`.

Минимальный Phase 1 pilot теперь закреплён как module-owned mobile package для surface `modules`:
- data boundary — `ModulesRepository`, где host обязан передать shared GraphQL client;
- transport implementation — `GraphQlModulesRepository` использует существующий platform query `moduleRegistry`;
- UI entry point — `ModulesMobileScreen` с loading/error/empty состояниями и переходом в generated host route;
- host seam — `apps/rustok_admin_mobile` overrides `modulesRepositoryProvider` через `graphQlClientProvider`, сохраняя auth/tenant/locale context на host-слое;
- evidence — widget E2E `pilot_modules_flow_test.dart` проверяет authenticated shell → module list → generated detail route → return to `/modules`.

FFA-ограничение для этого шага: пакет `rustok_modules_mobile` не создаёт собственный GraphQL client, auth/session store, tenant resolver или locale fallback chain. Он только потребляет host-provided repository и existing GraphQL surface.

#### PR-E evidence pack (Phase 1 modules mutation action)

**Mutation-backed operator action:** `toggleModule` в `rustok_mobile/packages/rustok_modules_mobile`.

Pilot-флоу `modules` расширен от read-only list/detail evidence к первому operator action:
- data boundary — `ModulesRepository.toggleModule(...)` добавляет write-side контракт рядом с `listModules()`;
- transport implementation — `GraphQlModulesRepository` использует существующую canonical GraphQL mutation `toggleModule(moduleSlug, enabled)` и возвращает typed `ModuleToggleResult`;
- UI action — `ModulesMobileScreen` показывает enable/disable action только для optional modules и GraphQL-hydrated `modules:manage` capability context;
- refresh evidence — после успешной mutation UI инвалидирует `modulesControllerProvider` и перечитывает `moduleRegistry`;
- test evidence — `modules_mobile_screen_test.dart` проверяет mutation request, refresh state, recovery feedback и retry/compensation actions without feature-local GraphQL client.

Permission source теперь hydrated через GraphQL `me.permissions` в host bootstrap probe; retryable lifecycle failures читаются через existing query `failedModuleOperationRecoveryPlans`, а recovery выполняется через existing mutations `retryFailedModuleOperationPostHook` и `compensateFailedModuleOperation`. Это не новый Flutter-specific backend API и не feature-local fallback chain.

#### PR-F evidence pack (Phase 1 lifecycle recovery actions)

**Recovery actions:** `retryFailedModuleOperationPostHook` и `compensateFailedModuleOperation` в `rustok_mobile/packages/rustok_modules_mobile`.

Pilot-флоу `modules` теперь не только показывает recovery feedback, но и даёт оператору выполнить canonical recovery actions:
- data boundary — `ModulesRepository.retryFailedPostHook(...)` и `ModulesRepository.compensateFailedOperation(...)` добавляют write-side recovery contract рядом с `toggleModule(...)`;
- transport implementation — `GraphQlModulesRepository` использует существующие lifecycle GraphQL mutations без Flutter-only API;
- UI action — `_RecoveryPlanNotice` показывает `Retry post-hook` и `Compensate`, блокирует retry для non-retryable plans и инвалидирует `modulesControllerProvider` после recovery;
- test evidence — `modules_mobile_screen_test.dart` проверяет retry и compensation action requests, очистку recovery notice и отсутствие feature-local GraphQL client.

Следующий шаг после PR-F был закрыт отдельным audit-oriented слоем: inline recovery notice теперь может вести в dedicated operation history/recovery screen без feature-local GraphQL client.

#### PR-G evidence pack (Phase 1 operation history/recovery screen)

**Operation history screen:** `ModulesRecoveryScreen` в `rustok_mobile/packages/rustok_modules_mobile`.

Pilot-флоу `modules` расширен от inline recovery notice к отдельному экрану истории failed lifecycle operations:
- UI boundary — `ModulesRecoveryScreen` показывает список `failedModuleOperationRecoveryPlans` с `operationId`, requested/previous state, `issue`, `recommendedAction`, `correlationId`, `requestedBy` и `errorMessage`;
- host route — `apps/rustok_admin_mobile` монтирует `/modules/recovery/:moduleSlug` внутри существующего shell и передаёт тот же host-provided `ModulesRepository`;
- action parity — экран выполняет те же canonical `retryFailedModuleOperationPostHook` и `compensateFailedModuleOperation`, инвалидирует `modulesControllerProvider` и перечитывает recovery history;
- FFA guardrail — recovery screen не добавляет Flutter-only API, не создаёт feature-local GraphQL client и остаётся потребителем existing lifecycle GraphQL contracts;
- test evidence — `modules_mobile_screen_test.dart` проверяет metadata/history rendering и recovery action requests, а `pilot_modules_flow_test.dart` проверяет переход из recovery feedback в shell-mounted recovery route.

Следующий шаг после PR-G: закрепить operation history/recovery screen в CI/codegen evidence и продолжить storefront mobile track с первым module-owned catalog/cart package.


#### PR-H evidence pack (storefront catalog/cart mobile package)

**Storefront catalog/cart package:** `rustok_mobile/packages/rustok_catalog_mobile`.

В storefront mobile-треке появился первый module-owned package вместо catalog/cart placeholder-экранов:
- UI-boundary — `StorefrontCatalogScreen` и `StorefrontCartScreen` живут вне host-а и владеют customer-facing loading/error/empty состояниями;
- data-boundary — `StorefrontCatalogRepository` передаётся host-ом через Riverpod, поэтому package не создаёт GraphQL client, tenant resolver, auth/session store или locale fallback chain;
- shared-transport wiring — `apps/rustok_frontend_mobile` использует host-owned `GraphQlStorefrontCatalogRepository`, который читает каталог через существующий GraphQL `storefrontSearch`;
- host route — `apps/rustok_frontend_mobile` монтирует package на `/catalog` и `/cart`, а shell navigation, runtime `tenantSlug`/`locale` и GraphQL-конфигурация остаются host-owned;
- test evidence — `catalog_screens_test.dart` проверяет package UI states с host-provided repositories, а `storefront_router_test.dart` проверяет catalog/cart mounting внутри customer storefront shell.

FFA guardrail для этого шага: это только mobile client/package boundary. Изменение не добавляет Flutter-specific backend API, feature-local transport client или дублирующую canonical routing/storage logic.

Следующий storefront шаг: довести cart-часть до canonical shared-transport reads/writes, когда соответствующая GraphQL/REST surface будет готова.


#### PR-I evidence pack (storefront mobile registry/codegen)

**Storefront registry:** `rustok_mobile/apps/rustok_frontend_mobile/lib/registry/storefront_mobile_manifest.g.dart`.

Storefront mobile получил generated registry рядом с admin mobile registry:
- codegen surface — `generate_mobile_manifest.py --surface storefront` читает `provides.storefront_ui` из `rustok-module.toml` и выставляет `MobileSurfaceKind.storefront`;
- snapshot evidence — `rustok_mobile/tooling/snapshots/storefront_mobile_manifest.snapshot.json` фиксирует `storefront_mobile` contract поля `module_slug`, `route_segment`, `permissions`, `locale_namespace` и `child_pages`;
- host route — generic `/modules/:routeSegment` теперь показывает manifest-backed title для storefront modules вместо raw segment-only placeholder;
- verification — `check_mobile_codegen.py` и `verify_mobile_manifest.py` принимают `--surface storefront`, поэтому admin и storefront generated outputs можно проверять одинаковым CI-friendly сигналом.

FFA guardrail для этого шага: registry содержит только mobile-safe surface metadata и не переносит server-side FBA/provider details в Flutter host.

Следующий storefront шаг: довести cart-часть до canonical shared-transport reads/writes и расширять manifest-to-package mapping по мере появления новых module-owned storefront packages.


#### PR-J evidence pack (storefront registry package discovery)

**Storefront package discovery:** `rustok_mobile/apps/rustok_frontend_mobile/lib/registry/storefront_surface_registry.dart`.

Generated storefront registry теперь используется не только для placeholder-title metadata, но и для выбора mounted package surface:
- registry adapter — `StorefrontSurfaceRegistry` нормализует `route_segment` из generated manifest и возвращает typed `catalog`, `cart` или `generic` surface match;
- host route — `/modules/products` монтирует `StorefrontCatalogScreen`, а `/modules/cart` монтирует `StorefrontCartScreen` через тот же host-provided `StorefrontCatalogRepository`;
- generic fallback — остальные storefront module routes продолжают показывать manifest-backed placeholder без feature-local transport clients;
- test evidence — `storefront_router_test.dart` проверяет generic `blog`, package-backed `products` и package-backed `cart` routes внутри shell.

FFA guardrail для этого шага: mapping остаётся host-side adapter logic поверх generated mobile-safe registry и не переносит server-side provider/FBA metadata в Flutter package.

Следующий storefront шаг: добавить canonical cart write actions (create/add/update/remove) и расширять manifest-to-package mappings только при появлении соответствующих module-owned packages.


#### PR-K evidence pack (storefront cart read transport)

**Cart read path:** `rustok_mobile/apps/rustok_frontend_mobile/lib/data/storefront_catalog_repository.dart`.

Storefront catalog/cart package теперь получает cart data через host-owned shared GraphQL transport:
- runtime context — `StorefrontRuntimeContext.cartId` берётся из optional `RUSTOK_STOREFRONT_CART_ID` и не создаёт package-local storage contract;
- data adapter — `GraphQlStorefrontCatalogRepository.cartLines()` вызывает existing GraphQL `storefrontCart(id)` и маппит `lineItems` в `StorefrontCartLine`;
- empty fallback — если host не передал cart id, cart surface остаётся empty state без Flutter-only API или локального fallback resolver;
- FFA guardrail — catalog/cart package по-прежнему потребляет только host-provided repository и не создаёт собственный GraphQL client, tenant resolver или locale fallback chain.

#### PR-L evidence pack (storefront cart write transport)

**Cart write path:** `rustok_mobile/apps/rustok_frontend_mobile/lib/data/storefront_catalog_repository.dart` + `rustok_mobile/packages/rustok_catalog_mobile/lib/src/catalog_repository.dart`.

Storefront catalog/cart package теперь выполняет customer cart write actions через тот же host-owned repository seam:
- repository contract — module-owned package объявляет create/add/update/remove intents, но не создаёт GraphQL client, tenant resolver, locale fallback или package-local cart storage;
- data adapter — host repository вызывает canonical GraphQL mutations `createStorefrontCart`, `addStorefrontCartLineItem`, `updateStorefrontCartLineItem` и `removeStorefrontCartLineItem`, повторно используя shared GraphQL headers/context;
- UX actions — catalog cards могут добавить товар только при наличии backend-provided `variantId`, empty cart может стартовать cart, line items могут увеличить/уменьшить quantity или удалить строку;
- runtime guardrail — созданный cart id хранится в host-owned `StorefrontCartIdStore`; durable storage остаётся отдельным host-owned product decision, а не package-local contract;
- contract tests — `rustok_mobile/tooling/tests/test_storefront_cart_contract.py` фиксирует, что package не fallback-ит product id в variant id и что host repository использует host cart id store.

Следующий storefront шаг был закрыт host-owned persistence seam и schema-backed contract evidence: cart id больше не привязан к package-local или repository-local памяти, а Flutter cart operations проверяются против существующего commerce GraphQL surface.

#### PR-M evidence pack (storefront cart durable host seam + schema contract)

**Cart persistence and schema evidence:** `rustok_mobile/apps/rustok_frontend_mobile/lib/app_shell/storefront_context.dart` + `rustok_mobile/tooling/tests/test_storefront_cart_graphql_contract.py`.

Storefront cart write path получил host-owned durable seam без расширения Flutter-specific API:
- persistence boundary — `DurableStorefrontCartIdStore` работает через `StorefrontCartIdPersistence`, поэтому package `rustok_catalog_mobile` по-прежнему не создаёт cart storage contract;
- host adapter — `FileStorefrontCartIdPersistence` может хранить cart id в host-provided JSON file через `RUSTOK_STOREFRONT_CART_ID_FILE`, а previews/tests сохраняют in-memory persistence;
- runtime guardrail — storage key задаётся host-слоем (`RUSTOK_STOREFRONT_CART_STORAGE_KEY`), tenant/locale/auth context остаётся в shared GraphQL config;
- schema-backed evidence — `test_storefront_cart_graphql_contract.py` сверяет Flutter cart operations и input types с существующими resolver/input declarations в `crates/rustok-commerce`;
- FFA guardrail — durable seam не добавляет `/api/flutter`, `/api/mobile` или feature-local GraphQL client и не переносит storage ownership в module-owned catalog package.

Следующий storefront шаг частично закрыт registry-driven home navigation: storefront host теперь показывает все generated manifest routes на главной поверхности, разделяя package-backed и manifest-only entries без ручного hard-code blog route. Более глубокий integration/e2e сигнал поверх реального schema snapshot или test server остаётся следующим CI-шагом, когда Flutter SDK будет доступен в окружении.

#### PR-N evidence pack (storefront generated module navigation)

**Generated storefront module links:** `rustok_mobile/apps/rustok_frontend_mobile/lib/routes/storefront_router.dart`.

Storefront host начал использовать generated registry не только для direct `/modules/:routeSegment` resolution, но и для home navigation discovery:
- navigation boundary — `StorefrontHomePage` рендерит список `storefrontSurfaceRegistry.entries`, поэтому новые storefront manifest entries становятся видимыми без hard-coded home links;
- package/fallback signal — package-backed `products`/`cart` routes помечаются как `package`, остальные manifest-only surfaces остаются generic `manifest` entries;
- route parity — tap по generated entry ведёт на тот же `/modules/<route_segment>` seam, который уже монтирует `StorefrontCatalogScreen`, `StorefrontCartScreen` или generic placeholder;
- test evidence — `storefront_router_test.dart` проверяет generated home links и переход в manifest-backed `blog`, а `test_storefront_home_registry_contract.py` фиксирует отсутствие hard-coded `/modules/blog` home route.

Следующий storefront шаг частично закрыт source-backed сигналом для catalog/cart GraphQL path: Flutter operation documents теперь сверяются с существующими storefront/search API и server runtime parity flow. Live schema/test-server CI остаётся следующим усилением, когда Flutter SDK и тестовый server harness будут доступны в целевом окружении. Package mappings по-прежнему расширяются только при появлении новых module-owned storefront packages.

#### PR-O evidence pack (storefront catalog/cart GraphQL integration signal)

**Source-backed integration signal:** `rustok_mobile/tooling/scripts/verify_storefront_graphql_contract.py` + `rustok_mobile/tooling/tests/test_storefront_cart_graphql_contract.py`.

Storefront track получил первый детерминированный CI-friendly сигнал для catalog/cart GraphQL path без добавления Flutter-specific API:
- catalog contract — переиспользуемая CLI-проверка извлекает mobile `StorefrontMobileCatalog` query из Dart raw-string const и сверяет её с существующей surface `storefrontSearch(input: $input)` и `SearchPreviewInput` в `crates/rustok-search/storefront/src/api.rs`;
- cart contract — CLI-проверка извлекает mobile `StorefrontMobileCart` read и create/add/update/remove mutation documents, проверяет operation/root-field markers и сверяет их с canonical commerce operation names;
- server-backed evidence — cart operation documents дополнительно привязаны к `crates/rustok-commerce/tests/graphql_runtime_parity_test.rs`, где те же create/add/query/update/remove steps выполняются через `schema.execute(Request::new(...))` и проверяют отсутствие GraphQL errors;
- FFA guardrail — CLI-проверка не вводит `/api/flutter`, `/api/mobile`, feature-local transport client или package-local tenant/locale/cart ownership и запрещает `tenantId` в mobile operation documents, потому что tenant/locale остаются host-provided GraphQL context;
- environment note — это source-backed contract/integration evidence для текущего окружения без Flutter SDK; CLI умеет печатать JSON evidence со списком server evidence paths через `--json`, а live schema/test-server job остаётся следующим CI-усилением, а не новым архитектурным контрактом.

Следующий storefront шаг: поднять этот сигнал до live schema/test-server job в CI, когда будет доступен Flutter SDK/test server harness, и расширять package mappings только при появлении новых module-owned storefront packages.

#### PR-P execution slice (storefront live-schema promotion plan)

**Цель:** перевести существующий source-backed verifier в CI-grade live сигнал без изменения mobile/backend API contract.

Так как текущая проверка уже доказывает совпадение Flutter operation documents с server-owned source и runtime parity test paths, следующий PR должен быть не новым architectural layer, а promotion-шагом:
- preflight — оставить `verify_storefront_graphql_contract.py` как быстрый обязательный шаг до запуска Flutter/server harness;
- live execution — поднять test schema/server и выполнить тот же набор operations: `StorefrontMobileCatalog`, `StorefrontMobileCart`, `StorefrontMobileCreateCart`, `StorefrontMobileAddCartLine`, `StorefrontMobileUpdateCartLine`, `StorefrontMobileRemoveCartLine`;
- evidence output — сохранять JSON с operation name, root field, source marker, server evidence path и результатом live execution;
- failure policy — падение live job не должно компенсироваться Flutter fallback endpoint-ом; исправлять нужно schema/resolver/client document drift;
- package expansion gate — новые package-backed storefront routes разрешены только после появления module-owned package и host-owned repository adapter, а manifest-only routes остаются generic placeholders.

**Не делать в PR-P:** не добавлять `/api/flutter`, `/api/mobile`, package-local GraphQL clients, tenant/locale/cart fallback chains или server-side FBA metadata в Flutter registry. Это hardening existing contract, а не расширение platform surface.

#### PR-A evidence pack (registry contract freeze)

**Snapshot source (canonical):** `rustok_mobile/tooling/snapshots/mobile_manifest.snapshot.json`.

Минимальный schema contract для mobile registry фиксируется следующими полями:
- `module_slug` — required;
- `surface_kind` — required;
- `route_segment` — required;
- `nav_icon` — required в snapshot/codegen output, optional на уровне исходных module manifests с fallback `module`;
- `child_pages` — optional (по умолчанию пустой список);
- `permissions` — optional (по умолчанию пустой список);
- `locale_namespace` — optional.

##### Compatibility matrix (`required/optional/deprecated`)

| Поле | Статус | Правило совместимости | Примечание FFA |
|---|---|---|---|
| `module_slug` | required | Запрещено переименование без migration-слоя в codegen | Идентификатор capability-поверхности, не runtime-деталь Flutter |
| `surface_kind` | required | Допустимо расширение enum только backward-compatible значениями | Нормализует тип surface для host-клиентов |
| `route_segment` | required | Изменение требует явного redirect/mapping в host routing | Поддерживает единый routing contract между host-ами |
| `nav_icon` | required in snapshot / optional in source manifests | Отсутствие в manifest нормализуется в `module`; новые значения должны быть backward-compatible с host fallback mapping | Visual parity metadata без Flutter-specific API или transport contract |
| `child_pages` | optional | Отсутствие трактуется как `[]`; новые элементы добавляются additive | Нужен для nested mobile/admin surfaces без server-side FBA metadata |
| `permissions` | optional | Отсутствие трактуется как `[]`; новые permission strings additive | Capability-level gate, без mobile-only API |
| `locale_namespace` | optional | Отсутствие означает fallback на module slug namespace | Сохраняет host-owned locale policy без feature-local fallback |

##### Field changelog (PR-A freeze)

- Добавлено явное требование использовать только capability-поля, без Flutter-specific transport/UI API.
- Зафиксированы правила default/fallback для optional-полей (`child_pages`, `permissions`, `locale_namespace`).
- Закреплён запрет на breaking rename для `module_slug` и обязательный redirect/mapping для `route_segment`.
- Runtime-contract в `app_module_contracts` синхронизирован со snapshot: `surface_kind`, `module_slug/route_segment` и нормализованный `nav_icon` обязательны в generated snapshot, `child_pages`/`permissions` имеют default `[]`, `locale_namespace` optional.

**Execution rule:** каждый следующий PR в этом треке должен обновлять таблицу статусов выше и добавлять ссылку на проверяемое evidence (commit, CI job или test log).

#### FFA-проверка для каждого PR в этом треке

- [x] Нет Flutter-specific API-контрактов поверх платформы (только consumption существующих platform contracts).
- [x] Route/query keys соответствуют canonical `snake_case` правилам RusTok.
- [x] Locale/tenant/auth context собирается host-слоем централизованно (без feature-local fallback chains).
- [x] Registry/codegen изменения не ломают возможность module-owned surfaces подключаться декларативно.

#### Чек-лист anti-duplication для PR

- Не дублировать архитектурные решения в новых docs: ссылаться на разделы этого файла.
- Новые детали добавлять только там, где им логически место (routing — в routing разделе, GraphQL — в transport разделе).
- При изменении фаз обновлять только эту таблицу и связанные разделы, а не создавать параллельные «планы-повторы».

### Open questions и ограничения


### Scope clarification: клиенты, а не третий web frontend

Для текущего трека Flutter scope фиксируется так:
- не создавать «третий web frontend» параллельно `apps/admin` / `apps/next-admin` / `apps/storefront` / `apps/next-frontend`;
- развивать **headless-клиенты** (mobile и desktop) как host-приложения поверх существующего backend-контракта;
- держать admin/operator и customer storefront как отдельные Flutter host-приложения (`rustok_admin_mobile` и `rustok_frontend_mobile`), а не смешивать их навигацию и UX;
- переиспользовать общий client-core (auth/session, tenant/locale context, GraphQL transport, route/query contracts) между mobile и desktop поверхностями.

Это сохраняет platform parity и снижает стоимость сопровождения по сравнению с отдельным web-host fork.

### Зависимости между планами (anti-drift guardrail)

- Flutter host, базовый app shell и module-owned mobile UI пакеты нужно развивать в логике FFA (один product contract при разных runtime/topology), но **без** Flutter-специфичных API-контрактов поверх платформы.
- FBA/provider-consumer metadata относится к серверным модулям и backend governance. Flutter registry/codegen не должен читать или дублировать `fba.*` manifest sections, `builder_contract_version`, `consumer_min_version`, machine-readable builder registry, provider capabilities или toggle profiles.
- Для практики это означает:
  - Flutter держит только mobile-safe host metadata: route segment, nav, child pages, locale namespace и permissions;
  - canonical правила builder/state/validation/RBAC остаются на backend и в общем page-builder плане;
  - parity проверяется по UI-facing GraphQL/REST contracts и host navigation semantics, а не по server-side FBA manifest fields.
- Если когда-нибудь появится отдельный UI-facing page-builder contract для мобильного host, его нужно вводить отдельным Flutter-планом и не смешивать с server-side FBA metadata.
- Чекпойнт реализации (2026-05-31): сгенерированный Flutter-реестр переносит из `rustok-module.toml` только mobile-safe host metadata: маршруты, навигацию, дочерние страницы, `locale_namespace` и `permissions`. FBA/provider-consumer metadata и `crates/rustok-page-builder/contracts/page-builder-fba-registry.json` остаются server/module concern и не попадают во Flutter snapshot; мобильный host не вводит `builder_surface` или page-builder-specific route guards.
- Чекпойнт синхронизации Page Builder (2026-06-13): synthetic Wave 0 observability evidence (`metrics` + минимум 2 `trace_samples`) остаётся server/module artifact в `crates/rustok-page-builder/contracts/evidence/pages-wave0-dry-run-evidence.json`; Flutter adapters обязаны предоставлять runtime/device evidence только на Wave hand-off, не дублируя FBA registry или control-plane toggle semantics в mobile registry.
- Чекпойнт синхронизации Page Builder (2026-06-13, SLO): provider health/SLO metadata (`ready/degraded/unavailable`, degradation reasons, pilot thresholds) остаётся server/module artifact в `crates/rustok-page-builder/contracts/page-builder-fba-registry.json`; Flutter Wave hand-off должен приложить device/runtime evidence, но не дублировать эти FBA thresholds в mobile registry.


Ниже — то, что в постановке **не указано**, поэтому в отчёте я дал только разумные варианты, а не жёсткие требования:

| Параметр | Статус | Разумный вариант по умолчанию |
|---|---|---|
| Целевые платформы | **Уточнено** | Mobile-first (iOS + Android), затем desktop (macOS/Windows/Linux) на общем client-core; без запуска нового web-host |
| Минимальные OS/SDK требования | **Не указано** | Брать актуальную stable-ветку Flutter и согласовать minima после freeze набора пакетов |
| Offline support | **Не указано** | Стартовать без offline-first; только persisted cache + secure auth + drafts |
| Тип мобильного приложения | **Уточнено** | Admin/operator и customer storefront ведутся как отдельные host-приложения: `rustok_admin_mobile` и `rustok_frontend_mobile` |
| Способ получения manifest/export для mobile registry | **Не указано** | На первом этапе — snapshot/codegen из RusTok repo; в долгую — backend/export endpoint |
| Distribution model | **Не указано** | Internal/TestFlight/Play Internal на первых этапах |
| Live schema/test-server harness для Flutter storefront | **Запланировано** | Использовать source-backed verifier как preflight; live job должен выполнять те же catalog/cart operations против test schema/server без новых Flutter-specific endpoints |

Итоговая рекомендация остаётся высокой уверенности: **строить Flutter-клиент как новый host платформы RusTok, а не как набор экранов без ownership-границ**. Именно это лучше всего соответствует текущей архитектуре репозитория и лучше всего масштабируется в сопровождении. fileciteturn30file0L3-L3 fileciteturn33file0L3-L3
