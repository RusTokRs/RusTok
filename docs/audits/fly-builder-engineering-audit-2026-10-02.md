# Инженерный аудит билдера Fly

**Дата:** 2026-10-02
**Объём:** `crates/ui/fly/**` (крейты `fly`, `fly-ui`, `fly-web`, `fly-browser`, `fly-leptos`, `fly-dioxus`),
точка интеграции `crates/modules/rustok-page-builder/src/adapters/fly_service.rs`,
CI `.github/workflows/fly-page-builder.yml`, `scripts/verify/verify-fly-*.mjs`.
**Метод:** статический разбор всего исходника (33 993 строки Rust + JS), сверка кода с ADR
`DECISIONS/2026-07-13-fly-page-builder-architecture.md`, `DECISIONS/2026-07-16-fly-ssr-first-browser-runtime.md`
и `crates/ui/fly/README.md`.

> Ограничение окружения: в песочнице нет `cargo`, поэтому `cargo build/clippy/test/bench/audit`
> не запускались. Все выводы получены чтением кода; пункты, которые обязательно нужно
> подтвердить сборкой, помечены **[проверить сборкой]**.

---

## 1. Резюме

Fly — это зрелое по объёму и дисциплинированное по стилю ядро: 309 тестов, нет ни одного
`TODO`/`FIXME`/`todo!()`, нет ни одного блока `unsafe`, аккуратная доменная модель ошибок
(`FlyError`, 30+ вариантов), разделение слоёв по ADR формально соблюдено, lossless-сохранение
неизвестных полей GrapesJS реально реализовано через `#[serde(flatten)]` + `ComponentNode::Opaque`.

Но билдер **не готов к продакшену** по четырём независимым причинам:

| # | Область | Суть | Severity |
|---|---------|------|----------|
| 1 | Безопасность | Целостность снапшотов/бандлов/ревизий держится на **FNV-1a 64** (`ProjectHash`) — не криптостойкой хеш-функции. Подделка коллизии тривиальна. | **Critical** |
| 2 | Безопасность | `component.id` не валидируется по charset и попадает в CSS, который сырым вставляется в `<style>` → выход из `<style>` через `</style>` → XSS в SSR-рендере. | **Critical** |
| 3 | Производительность | `FlyEditor::apply()` делает **4 полных глубоких клона всего документа** + полную валидацию + полный пересчёт хеша на каждую команду; история хранит ещё по 2 полных документа × 100 записей. | **Critical** |
| 4 | Достоверность | README/ADR описывают полноценные редакторы на Leptos/Dioxus. Фактически `fly-leptos` = 95 строк, `fly-dioxus` = 72 строки пустых `<section>`-обёрток. «Готовность к извлечению в отдельный репозиторий» тоже не работает. | **High** |

Дополнительно: `fly-ui`, `fly-web`, `fly-leptos`, `fly-dioxus` **полностью отсутствуют в CI**
(ни тестов, ни clippy, ни fmt), а 8 contract-тестов `fly-browser` не запускаются из-за флага `--lib`.

---

## 1a. Статус реализации (обновлено 2026-10-02)

Первая волна исправлений выполнена в этой ветке. Таблица отражает фактическое состояние.

| ID | Пункт | Статус | Где |
|---|---|---|---|
| C-1 | Целостность снапшотов/бандлов на SHA-256 | ✅ Сделано | `src/digest.rs` (`ContentDigest`), `ProjectSnapshot::content_digest`, `ProjectBundle::content_digest`, `BundleDecodePolicy::verified()` |
| C-1 | `ProjectHash` документирован как **не** криптопримитив; убран тихий `unwrap_or_default` | ✅ Сделано | `src/command/model.rs` (`try_from_document`) |
| C-2 | XSS через `component.id` в `<style>` | ✅ Сделано | `escape_css_attribute` (allow-list), `escape_style_element_text`, `validate_identifier` |
| C-2 | Самовосстановление невалидных id (защита от блокировки редактора) | ✅ Сделано | `ProjectDocument::ensure_stable_ids` |
| C-3 | Клон документа при каждом `encode` | ✅ Сделано | `codec.rs::canonical_project` возвращает `&GrapesProject` |
| C-3 | 4 клона на команду, инкрементальная валидация, индекс компонентов | ⏳ Фаза 4 | требует Ф3 |
| H-1 | README приведён в соответствие (таблица статусов, блокеры извлечения) | ✅ Сделано | `crates/ui/fly/README.md` |
| H-1 | `standalone-Cargo.toml` — рабочий шаблон + проверка дрейфа | ✅ Сделано | `standalone-Cargo.toml`, `scripts/verify/verify-fly-standalone-workspace.mjs` |
| H-1 | Вынос `rustok-ui-i18n` за трейт | ✅ Сделано | `src/locale_resolver.rs`, фича `platform-i18n` |
| H-2 | Обход валидации через `Opaque` закрыт: лимиты и проверка id теперь тотальны | ✅ Сделано | `validation.rs::validate_opaque_components` |
| H-3 | Лимит глубины при декодировании | ✅ Сделано | `MAXIMUM_DECODE_DEPTH`, итеративный `ensure_depth_within_limit` |
| H-4 | Навигация по ответу сервера и intent-endpoint ограничены same-origin | ✅ Сделано | `fly-browser.js`: `sameOriginUrl()` |
| H-4 | Токен из `localStorage` → httpOnly-cookie | ⏳ Фаза 1 | требует изменений сервера (`apps/admin/src/main.rs` читает `x-fly-access-token`) |
| H-5 | Единый модуль политики URL | ✅ Сделано | `safe_url.rs` — оказалось **пять** реализаций, не три |
| H-6 | CI: добавлены `fly-ui`, `fly-web`, `fly-leptos`, `fly-dioxus`; убран `--lib`; wasm32-джоба; исправлены path-фильтры | ✅ Сделано | `.github/workflows/fly-page-builder.yml` |
| H-7 | Починены 4 сгнивших гейта; 6 сирот подключены в CI; мета-гейт против сирот | ✅ Сделано | `verify-fly-gates-are-wired.mjs` |
| H-7 | Замена grep-верификации на поведенческие тесты | ⏳ Фаза 3 | остаётся основная работа |
| M-1 | `strip_tags` удалён (потеря текста `5 < 10`) | ✅ Сделано | `render.rs::push_escaped_html` |
| M-5 | `#[derive(Copy)]` вместо ручного `impl` в `lib.rs` | ✅ Сделано | `dynamic.rs` |
| M-6 | Информативный `Display` для `FlyError::Validation` | ✅ Сделано | `error.rs::format_diagnostics` |
| M-8 | `History`: `VecDeque` + бюджет памяти | ✅ Сделано | `command/model.rs`, `FlyEditor::with_history_memory_budget` |
| L-4 | Однопроходное HTML-экранирование | ✅ Сделано | `push_escaped_html` / `push_escaped_attribute` |
| M-2 | `remap_ids` переписывал любую совпавшую строку | ✅ Сделано | `src/id_reference.rs` |
| L-1 | `#![forbid(unsafe_code)]` / `#![deny(...)]` во всех 6 крейтах | ✅ Сделано | `lib.rs` × 6 |
| L-5 | Магическое `maximum_nodes = 10_000` | ✅ Сделано | `DEFAULT_MAXIMUM_NODES` / `DEFAULT_MAXIMUM_DEPTH` |
| L-6 | Нет `SECURITY.md` | ✅ Сделано | `crates/ui/fly/SECURITY.md` |
| M-3 | `safe_style` блокировал весь `url(` | ✅ Сделано | безопасные `url()` разрешены, + deny-list свойств |
| M-7 | Сломанные вложенные repeater-ы + квадратичность | ✅ Сделано | `dynamic.rs::RuntimeExpander` |
| M-9 | Нет индекса id→узел | ✅ Сделано | `ComponentIndex` в `validate_style_rules` и `binding.rs` |
| M-11 | Несколько обходов в `validate_project` | ✅ Сделано | 4 обхода → 2 (`walk_component`) |
| C-3 | Клоны в `FlyEditor::apply`: 3 → 1, в `undo`/`redo`: 1 → 0 | ✅ Сделано | `Arc<ProjectDocument>` в `HistoryEntry` |
| M-10 | Property-тесты: было 1, стало 6 содержательных | ✅ Сделано | `src/tests.rs` |
| M-4 | 48 glob-реэкспортов | ⏳ Фаза 5 | механическая работа, большой диф |
| L-2 | Нет `[workspace.lints]` | ✅ Сделано | корень + 6 крейтов + зеркало в шаблоне |
| L-3 | Расхождение default-features адаптеров | ✅ Сделано | обосновано в манифесте + покрыто CI |
| L-4 | Нет `CHANGELOG` | ✅ Сделано | `crates/ui/fly/CHANGELOG.md` |
| L-7 | `cargo audit` не в CI | ✅ Сделано | джоба `advisories` (advisory-only) |
| — | Клоны правил стилей в предикатах | ✅ Сделано | `StyleRuleIdentity` |
| C-3 | История на инверсных командах | ⏳ Фаза 4 | уже не критично: память разделена |

### Уточнение к H-1 (неиспользуемые зависимости)

`fly` и `fly-ui` в манифестах `fly-leptos`/`fly-dioxus` действительно не используются в коде, **но
они требуются намеренно** — `scripts/verify/verify-fly-dependency-boundaries.mjs` объявляет их
обязательными как фиксацию будущего направления зависимостей. Пункт 2.4 плана снимается: это не
дефект, а policy. Удалять их можно только вместе с правилом в скрипте границ.

### Что выяснилось при починке (вторая волна)

Три находки оказались хуже, чем зафиксировано в аудите:

1. **H-5: политик URL было пять, а не три.** Помимо `safe_url.rs`, `render.rs` и
   `validation.rs`, собственные копии примитивов держали `asset.rs` и `trait_model.rs`.
   При этом `render.rs` и `validation.rs` были **побайтовыми** копиями друг друга
   (отличались только инфиксом `_public_` в именах) — то есть любое ужесточение в одном
   месте гарантированно оставляло дыру в другом.

2. **M-2 ломался в обе стороны.** Помимо ложных срабатываний (компонент с id `title`
   переписывал `placeholder="title"`), были и ложные отрицания: ссылка внутри строки
   (`href="#hero"`, список `aria-labelledby="hero title"`) не находилась вовсе и
   оставалась висеть на оригинал после вставки.

3. **H-7: гейты противоречили тестам.** `verify-fly-admin-runtime.mjs` требовал наличия
   `transport::update_page` в `builder.rs`, тогда как Rust-тест
   `current_fly_tree_remains_the_only_document_authority` требует его **отсутствия**.
   Оба «проходили», потому что скрипт не был подключён к CI.

### Подтверждение H-7 на практике

При прогоне всех `verify-fly-*.mjs` на текущем `main` **четыре скрипта уже падают** и падали
до этих изменений:

- `verify-fly-admin-browser-runtime.mjs` — ищет `hydrate_page_components_from_frames`, `synchronize_first_frame`
- `verify-fly-admin-runtime.mjs` — то же
- `verify-fly-command-transactions.mjs` — ищет `pub fn set_schema_version`
- `verify-fly-snapshots.mjs` — ищет устаревший порядок имён в `pub use component_visit::{...}`

Ни один из них не был включён в CI-workflow, поэтому гниение осталось незамеченным. Это ровно тот
режим отказа, который описан в H-7.

Корневая причина измерима: из 18 гейтов **7 не запускались ни одним workflow**, и все четыре
сгнивших были среди этих семи. Корреляция полная. Шесть подключены в `fly-page-builder.yml`
(седьмой, `verify-fly-grapesjs-roundtrip.mjs`, легитимно живёт в `browser-e2e.yml`), а новый
`verify-fly-gates-are-wired.mjs` падает, если появится гейт, который никто не запускает.

Все 18 гейтов зелёные — впервые.

Кроме того, изменение CI сломало `verify-fly-actions-forms.mjs`, который требовал дословной строки
`cargo test -p fly-browser --lib` — то есть скрипт активно защищал тот самый баг, из-за которого
contract-тесты не запускались. Ассерт ослаблен до `cargo test -p fly-browser`.

### H-2 оказался обходом защиты, а не косметикой

`ComponentNode::visit` делает ранний выход на `Opaque`, поэтому **весь подграф под нераспознанным
компонентом полностью минует валидацию**: он не считается в `maximum_nodes`, его вложенность не
учитывается в `maximum_depth`, а id внутри не проверяются ни на дубликаты, ни на допустимый
charset. То есть документ, намеренно сломанный так, чтобы компонент не разобрался в типизированную
модель, обходит ресурсные лимиты и проверку идентификаторов.

`Opaque` **сохранён намеренно** — на нём держится lossless round-trip компонентов от неизвестных
Fly провайдеров, то есть смысл кодека. Тотальной сделана не модель, а валидация: добавлен обход
сырого JSON внутри opaque-подграфов, который считает узлы, меряет глубину и проверяет id
(и charset, и дубликаты — совместно с типизированным проходом). Закрыта и вторая слепая зона того
же рода: opaque *список детей* (`ComponentChildren::Opaque`), который `children()` отдавал как
пустой.

Тест `opaque_subtrees_cannot_be_used_to_evade_the_node_budget` фиксирует закрытие обхода.

Побочное наблюдение: сконструировать `Opaque` сложнее, чем кажется — из-за
`#[serde(flatten)] extensions` почти любой JSON-**объект** успешно разбирается в
`ComponentObject`. Деградация происходит на не-объектах и на неверном *типе* известного поля
(например `"id": 0`).

### Два гейта молчали там, где должны были говорить

При добавлении `[workspace.lints]` обнаружилось, что `verify-fly-standalone-workspace.mjs`
остался зелёным, хотя шаблон после правки **не собрался бы**: каждый крейт Fly теперь объявляет
`[lints] workspace = true`, а Cargo это отвергает, если в корне воркспейса нет
`[workspace.lints]`. Регулярка гейта такое написание не ловила. Добавлена явная сверка политики
линтов между хостом и шаблоном, проверенная тремя негативными сценариями.

Обратный случай: `verify-fly-dependency-boundaries.mjs` упал на слове «leptos» **в комментарии**
манифеста `fly-dioxus`. Комментарий — не зависимость; гейт наказывал манифест за то, что тот себя
объясняет. Теперь комментарии вырезаются перед сканированием, но реальная зависимость
по-прежнему ловится (проверено негативным сценарием).

### H-1: инверсия, а не реимплементация

`rustok-ui-i18n` построен на ICU4X/CLDR. Переписывать его внутри Fly было бы и бесполезно, и
неверно, поэтому зависимость **инвертирована**: Fly объявляет, что ему нужно (трейт
`LocaleResolver` с двумя методами), а платформа это поставляет. Фича `platform-i18n` включена по
умолчанию, так что поведение хоста не изменилось ни на байт.

Самодостаточный `BasicLocaleResolver` честно теряет вывод likely-subtags (`zh-TW` больше не
доходит до `zh-Hant`), и это зафиксировано тестом `only_the_platform_resolver_infers_likely_subtags`,
а не спрятано. Согласованность двух реализаций там, где CLDR не нужен, проверяется
сверкой нормализации и **подпоследовательности** цепочек: платформенный резолвер вправе *вставить*
выведенную ветку, но не вправе потерять структурный шаг или переставить порядок.

Гейт `verify-fly-standalone-workspace.mjs` перестал быть списком «известных блокеров», который
просто подавлял ошибки: теперь исключение зависимости из шаблона разрешено, только если крейт
объявляет её `optional = true` — иначе извлечённый воркспейс не собрался бы. CI собирает и
тестирует `fly --no-default-features`, иначе этот путь сгнил бы так же, как сгнили гейты,
которые не запускал ни один workflow.

### M-11: 4 обхода дерева → 2

`validate_project` обходил дерево компонентов четырежды: `ComponentIndex::build`,
`visit_components` для типизированных проверок, отдельный проход по opaque-узлам (который я же и
добавил при закрытии H-2) и `validate_component_public_urls` для URL-атрибутов. Три из них
объединены в `walk_component`.

`validate_component_public_urls` остаётся публичной — её напрямую зовут `runtime_pipeline`,
`runtime_validation` и `landing_readiness`. Чтобы две реализации не разъехались, перкомпонентная
проверка вынесена в общий `check_component_public_urls`, а тест
`the_standalone_url_pass_agrees_with_the_unified_walk` сверяет их результаты.

Четвёртый обход (`ComponentIndex::build`) оставлен сознательно: вплетать построение индекса в
валидационный обход значило бы раскрыть его внутренности ради незначительного выигрыша.

### C-3: разделение вместо копирования

Документ, который становится `self.document`, побайтово совпадает с `after` записи истории, а
вытесняемый — с `before` следующей записи. За `Arc` это одна и та же аллокация, поэтому:

| Операция | Было глубоких копий | Стало |
|---|---|---|
| `apply` | 3 | 1 (спекулятивная, её нельзя избежать) |
| `undo` | 1 | 0 |
| `redo` | 1 | 0 |

Память истории примерно вдвое меньше. Публичное API не изменилось — `document()` по-прежнему
возвращает `&ProjectDocument` через deref-coercion.

Честная оговорка: `approximate_bytes` намеренно продолжает считать обе половины, хотя они
разделены. Оценка завышена примерно вдвое, то есть бюджет стал консервативным — удерживается
столько же записей, сколько раньше, но при вдвое меньшем реальном расходе. Научить счётчик
считать различные аллокации потребовало бы учёта по всему деку.

Переход на инверсные команды после этого перестал быть критичным: он убрал бы сериализацию в
`approximate_bytes`, но главная проблема — удвоение памяти — уже решена.

Тест `adjacent_history_entries_share_one_document_allocation` проверяет `Arc::ptr_eq` напрямую:
без него регрессия к независимым клонам прошла бы незамеченной — все поведенческие тесты
остались бы зелёными.

### Вычитка собственных правок

Поскольку `cargo` недоступен, проведён отдельный проход по всему внесённому Rust-коду на классы
ошибок, которые компилятор поймал бы сразу. Найдено и исправлено три:

1. **Настоящий конфликт заимствований** (`dynamic.rs`, `expand_child`). В let-chain
   `if let Some(conditions) = self.deferred_conditions.get(..) && ...` заимствование `self` живёт
   всё тело `if`, а внутри вызывались `&mut self`-методы. Условие вычисляется в `bool` до ветвления.
2. **Конструирование unit-структуры через псевдоним типа** (`locale_resolver.rs`). Псевдоним
   нельзя использовать в позиции выражения, поэтому `DefaultLocaleResolver` как значение не
   скомпилировался бы. Функция расписана по конфигурациям через `cfg`.
3. `UrlPolicy` пересобиралась на каждый `url()`-токен в объявлении — вынесена из цикла.

Проверено отдельно: неиспользуемых импортов нет (важно, так как CI линтует с `-D warnings`);
все вызываемые хелперы `dynamic.rs` определены ровно по разу; `collect_ids`/`remap_ids` доступны
как `pub(crate)`; `ComponentLocation` экспортируется через `pub use placement::*`;
`RenderPolicy::url_policy()` и поле `allow_data_images` существуют; `EmptyRepeaterBehavior`
выводит `PartialEq`.

Ранее тем же способом было поймано два случая выдуманного API: несуществующий
`PageCommand::UpsertPage` и неверное предположение, что объект с полем `provider` становится
`Opaque` (из-за `#[serde(flatten)] extensions` почти любой JSON-объект разбирается в
`ComponentObject`).

### Непроверенное сборкой

В песочнице нет `cargo`. Rust-правки вычитаны статически, JS проверен `node --check`, все
verify-скрипты прогнаны. Перед мержем обязательно:

```bash
cargo test -p fly -p fly-ui -p fly-web -p fly-browser --all-targets
cargo clippy -p fly -p fly-ui -p fly-web -p fly-leptos -p fly-dioxus --all-targets -- -D warnings
cargo fmt -p fly -p fly-ui -p fly-web -p fly-browser -p fly-leptos -p fly-dioxus -- --check
cargo check -p rustok-page-builder -p rustok-page-builder-admin -p rustok-pages-storefront
```

Наиболее вероятные места доработки после первой сборки: форматирование (`cargo fmt`) и
clippy-придирки в новом `digest.rs` / `render.rs`.

---

## 2. Карта кода (что реально есть)

```
crates/ui/fly/                 33 993 LOC
├── src/                       ~25 000  ядро: AST, codec, команды, валидация, рендер,
│                                        dynamic/binding/context, landing, locale, snapshot
├── ui/                        ~4 000   FlyUiStateMachine, UiIntent/UiEffect, contribution-слой
├── web/                       ~1 500   геометрия, hit-test, iframe-мост, real-DOM inline
├── browser/  src 688 + js 1271         SSR JS-мост (fly-browser.js)
├── leptos/   95                        ПУСТАЯ оболочка
├── dioxus/   72                        ПУСТАЯ оболочка
└── fixtures/grapesjs/  3 файла, 1.2–1.8 КБ
```

Крупнейшие модули ядра: `context_schema.rs` (1618), `dynamic.rs` (1066), `render.rs` (1015),
`landing_contract.rs` (990), `landing_property.rs` (938), `trait_model.rs` (758), `validation.rs` (744).

Публичный API: **1299 `pub`-элементов**, все вынесены наружу 48 строками `pub use module::*;`
в `src/lib.rs`.

---

## 3. Критические дефекты

### C-1. `ProjectHash` — FNV-1a 64 используется как контроль целостности

`crates/ui/fly/src/command/model.rs:140-161`

```rust
pub struct ProjectHash(pub u64);
pub fn from_bytes(bytes: &[u8]) -> Self {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes { hash ^= u64::from(*byte); hash = hash.wrapping_mul(0x100000001b3); }
    Self(hash)
}
```

Где этот хеш используется как **гарантия неизменности**:

* `snapshot/model.rs:16-27` — `ProjectSnapshot::restore()` сверяет `project_hash` и бросает
  `SnapshotHashMismatch`. Это заявленная проверка целостности снапшота.
* `bundle.rs:172 bundle_hash()` + `FlyError::ProjectBundleHashMismatch`.
* `command/model.rs:202 RevisionState::acknowledge()` — оптимистическая блокировка (`RevisionConflict`).
* `context_json_schema.rs:94` — `contract_hash` контракта контекста.
* `runtime_scenario_release.rs:53,338` — **release-gate** сценариев рантайма.
* `runtime_scenario_render.rs:168-170` — `html_hash` / `css_hash` / `document_hash`.

FNV-1a 64 не является криптографической функцией: коллизию под заданное значение строят
за секунды. Любой, кто может подложить `project_data` в снапшот или бандл, обходит все
перечисленные гейты, включая релизный. Дополнительно 64 бита дают заметную вероятность
случайной коллизии (парадокс дней рождения: ~1 на 2³²).

Отягчающее: в том же крейте уже подключён `sha2` и **уже используется** в
`landing_contract.rs:9,263,508,610,661` (`sha256_hex`). То есть в проекте сосуществуют
две несовместимые схемы хеширования, и «слабая» применена именно к гейтам безопасности.

Ещё одна ошибка в том же месте — `ProjectHash::from_document` (`model.rs:143-147`):

```rust
let bytes = GrapesJsCodec::encode_vec(document)
    .unwrap_or_else(|_| serde_json::to_vec(&document.project).unwrap_or_default());
```

При двойном сбое сериализации получаем `&[]` → хеш пустого ввода. Все «несериализуемые»
документы получают один и тот же хеш, и `acknowledge()` считает их эквивалентными.

**Фикс:** `ProjectHash` → `[u8; 32]` на SHA-256, единый `sha256_hex` для всего крейта;
`from_document` возвращает `FlyResult<ProjectHash>` вместо проглатывания ошибки.
Версия хеша несовместима — нужна миграция хранимых снапшотов (см. план, Ф1).

---

### C-2. XSS: выход из `<style>` через неотвалидированный `component.id`

Цепочка:

1. `validation.rs:206-240` проверяет у `component.id` **только дубликаты и отсутствие**.
   Charset не проверяется нигде. `ensure_stable_ids` (`model.rs:215-245`) санирует только
   **генерируемые** id; существующие сохраняются как есть.
2. `render.rs:338-341` и `render.rs:374-377` строят CSS-селектор:
   ```rust
   let selector = format!("[data-fly-style-id=\"{}\"]", escape_css_attribute(&component_id));
   ```
3. `render.rs:710-716`:
   ```rust
   fn escape_css_attribute(value: &str) -> String {
       value.replace('\\', "\\\\").replace('"', "\\\"")
            .replace('\n', "\\a ").replace('\r', "\\d ")
   }
   ```
   Экранируются только `\`, `"`, CR/LF. Символы `<`, `>`, `/` проходят насквозь.
4. `render.rs:137-145` `compose_document_html` вставляет CSS **сырым** внутрь `<style>`:
   ```rust
   format!("...{}<style>{}</style>...", head.render_html(), css, body_html)
   ```

HTML-парсер завершает `<style>` на первом литеральном `</style>` независимо от CSS-синтаксиса.
Значит id вида `x</style><script>…</script><style>` с любым непустым `style` у компонента
даёт исполняемый скрипт в итоговом документе. Экранирование кавычек из шага 3 этому не мешает.

Тот же вектор через `StyleRuleScope::Media` менее вероятен: `safe_media_query`
(`render.rs:648-662`) ограничивает charset и `<`/`>` не пропускает.

**Фикс (три слоя, нужны все):**
* валидация: `component.id` и `page.id` — `^[A-Za-z0-9_:.-]{1,128}$`, severity `Error`;
* `escape_css_attribute` → полноценный CSS-ident/строковый escape (`\XX ` для всего, что
  вне безопасного множества);
* `compose_document_html` — отбивать `</` внутри CSS (`\00003c/`) и, независимо, добавить
  регрессионный тест на `</style>`, `</script>`, `<!--`.

---

### C-3. `FlyEditor::apply()` — четыре полных клона документа на команду

`crates/ui/fly/src/command/editor.rs:83-118`

```rust
let before = self.document.clone();        // клон 1
let mut after = before.clone();            // клон 2
self.apply_to_document(&mut after, &command)?;
after.ensure_stable_ids(&mut self.id_generator);          // полный обход
let report = extend_with_runtime_validation(&after,
    validate_project(&after, &self.registries, self.validation_limits)); // ≥4 полных обхода
...
self.document = after.clone();             // клон 3
self.history.push(HistoryEntry { command, before, after }); // хранит ещё 2 документа
self.revision.mark_changed(&self.document);                 // сериализация всего + хеш
```

Стоимость одного нажатия клавиши в property-панели для документа из N узлов:

* 3 глубоких клона дерева (`ComponentNode` рекурсивный, `serde_json::Value` внутри) + 1 в истории;
* `ensure_stable_ids` — полный обход с двумя `BTreeSet<String>` и аллокацией строки на каждый id;
* `validate_project` — независимые полные обходы в `validate_pages`, `validate_components`,
  `validate_component_public_urls`, `validate_runtime_extensions`, плюс построение
  `AssetCatalog` и `StyleRuleCatalog`;
* `GrapesProject::visit_components` (`model.rs:72-80`) на **каждом** узле делает
  `format!("{path}.components[{index}]")` — аллокация строки на узел на каждый обход;
* `mark_changed` → `serde_json::to_vec` всего проекта + FNV по всем байтам.

Память истории: `History::new(100)` (`editor.rs:29`) × `HistoryEntry { before, after }` =
до **200 полных копий документа** в RAM. Проект на 5 МБ → ~1 ГБ. Лимит задан в штуках
записей, бюджета по памяти нет.

Плюс `History::push` (`command/model.rs:122-128`) при переполнении делает `undo.remove(0)` —
O(n) сдвиг вектора из тяжёлых элементов вместо `VecDeque::pop_front`.

**Фикс:** перейти на обратимые команды (`InverseCommand`) вместо снапшотов «до/после»;
инкрементальная валидация затронутого поддерева с полным проходом только по явному запросу;
индекс `id -> путь` в `ProjectDocument`, инвалидируемый структурными правками;
`VecDeque` + бюджет истории в байтах; `visit_components` — передавать `&mut Vec<PathSegment>`
вместо `format!`.

---

## 4. Высокие риски

### H-1. Документация расходится с реальностью

* `README.md` обещает: «`fly-leptos`: Thin Leptos 0.8 components (`FlyFullEditor`, `FlyInlineEditor`,
  `FlyPreview`, `FlyReadOnly`)» и аналогично для Dioxus.
  Факт: `leptos/src/lib.rs` — 62 строки, четыре компонента вида
  `<section class="fly-editor--full">{children()}</section>`. Ни состояния, ни интентов,
  ни канваса, ни drag&drop. `dioxus/src/lib.rs` — то же самое, 72 строки.
  При этом `fly-leptos` и `fly-dioxus` объявляют зависимости на `fly` и `fly-ui`, которые
  в коде не используются (**[проверить сборкой]** `cargo udeps`).
* README описывает каталог `crates/ui/fly/locales/` — **его не существует**.
* README и ADR: «self-contained monorepo ready to be extracted». Факт:
  - `fly/Cargo.toml:9` зависит от `rustok-ui-i18n` (внутренний крейт RusTok,
    используется в `runtime_locale.rs:246,263`) — при извлечении сборка невозможна;
  - `standalone-Cargo.toml` содержит только `[workspace] members`, без
    `[workspace.package]` и `[workspace.dependencies]`, а все под-крейты используют
    `version.workspace = true`, `serde.workspace = true` и т. д. → шаблон нерабочий.
* ADR-2026-07-13 говорит «Dioxus support is deferred until `fly-ui` stabilizes», но
  `fly-dioxus` уже в workspace и в README как поддерживаемый.

### H-2. Молчаливая деградация при декодировании

`model.rs:160-164`:
```rust
#[serde(untagged)]
pub enum ComponentNode { Object(Box<ComponentObject>), Opaque(Value) }
```
`Opaque(Value)` матчится всегда. Если у узла хоть одно поле имеет неожиданный тип
(`traits` объектом вместо массива, `attributes` массивом, `components` строкой),
**весь узел с поддеревом** тихо становится `Opaque`: нередактируемым, невидимым для
валидации (`ComponentNode::visit` в `model.rs:206-220` делает `let Some(object) = … else { return }`),
частично невидимым для рендера. Диагностика не выдаётся ни одна.

`GrapesJsCodec::decode_value` (`codec.rs:17-24`) проверяет только `value.is_object()` на корне —
декодирование произвольного мусора «успешно». Ошибки serde для untagged-перечислений в принципе
нечитаемы, но здесь они даже не возникают.

**Фикс:** при построении `Opaque` из значения, которое *похоже* на компонент
(JSON-объект с `type`/`tagName`/`components`), эмитить
`ValidationDiagnostic { severity: Warning, code: "component_degraded_to_opaque" }`
с путём и причиной.

### H-3. Нет защиты от переполнения стека на глубоком документе

Рекурсивные без ограничения глубины: `ComponentNode::find` / `find_mut` (`model.rs:188-205`),
`visit` (206), `ensure_stable_ids` (222), `remove_descendant` (247), `collect_ids` (259),
`remap_ids` (269), `replace_value_references` (`model.rs:437`), `render_node` (`render.rs:201`),
`append_component_style_rules` (`render.rs:359`), `interpolate_node` (`dynamic.rs:748`).

`ValidationLimits::maximum_depth = 64` (`validation.rs:62-67`) проверяется **внутри**
`visit_components`, то есть уже после того, как обход рекурсивно дошёл до этой глубины.
Лимит диагностирует, но не защищает. Враждебный JSON с глубиной 100 000 кладёт процесс
по stack overflow (аборт, не panic — `catch_unwind` не спасёт). То же относится к
`Drop` для глубокого `serde_json::Value`.

**Фикс:** жёсткий `MAX_DEPTH` в `GrapesJsCodec::decode_*` до построения модели
(стриминговый счётчик глубины или `serde_stacker`), ошибка `FlyError::MaximumDepthExceeded`.
Конвертировать горячие обходы (`visit`, `collect_ids`, `render_node`) в явный стек.

### H-4. `fly-browser.js`: небезопасные дефолты и неконтролируемая навигация

`crates/ui/fly/browser/assets/fly-browser.js`

* **стр. 456-457** — `this.expectedOrigin = options.expectedOrigin || root.dataset.flyExpectedOrigin || "null";`
  Fail-open по умолчанию: строка `"null"` — это и есть origin opaque-iframe (`sandbox` без
  `allow-same-origin`). Отсутствие конфигурации не блокирует приём сообщений, а разрешает
  приём из песочницы. Должно быть fail-closed: нет origin → адаптер не стартует.
* **стр. 1153-1156** — ответ сервера напрямую управляет навигацией:
  ```js
  if (result.reload === true) globalThis.location.reload();
  else if (typeof result.location === "string") globalThis.location.assign(result.location);
  ```
  Схема не проверяется → `javascript:` / `data:` в `result.location` даёт XSS,
  внешний URL — open redirect. Нужна проверка «относительный путь или same-origin».
* **стр. 6, 461** — access-токен админки читается из `localStorage` (`rustok-admin-token`)
  и шлётся в заголовках. При любом XSS в админке токен угоняется. Усугубляется тем, что
  запрос уже идёт с `credentials: "same-origin"` (стр. 1108) — то есть httpOnly-cookie
  достаточно, а `Bearer` из localStorage избыточен.
* **стр. 1087-1089** — токен дублируется в двух заголовках (`authorization` и
  `x-fly-access-token`), что расширяет поверхность утечки без выигрыша.
* **стр. 458-459** — `intentEndpoint` берётся из `root.dataset.flyIntentEndpoint`, произвольный
  URL из DOM, и именно на него уходит токен. Нужно требовать относительный путь.

### H-5. Две независимые реализации политики URL

* `src/safe_url.rs` — `normalize_safe_url` (путь валидации).
* `src/render.rs:565-612` — `normalized_url_candidate` / `url_allowed` /
  `absolute_url_has_authority` / `relative_url_allowed` (путь рендера).
* `src/validation.rs` — третий набор: `PublicUrlAttributeKind` / `public_url_allowed`.

Правила уже разъехались: `safe_url.rs` безусловно режет любой `data:`, а `render.rs`
разрешает `data:image/*;base64,` при `policy.allow_data_images`; лимит длины 2048 есть
только в рендере. Любая будущая правка одной копии оставит дыру в двух других.

**Фикс:** один модуль `url_policy` с типизированным `UrlPolicy`, остальные — тонкие обёртки;
общий корпус тестов, прогоняемый против всех точек входа.

### H-6. CI не покрывает половину билдера

`.github/workflows/fly-page-builder.yml`

* Тестируются/линтятся только `fly`, `fly-browser` и модули page-builder.
  **`fly-ui` (~4000 LOC), `fly-web` (~1500 LOC), `fly-leptos`, `fly-dioxus` не собираются,
  не тестируются и не линтятся вообще.**
* `cargo test -p fly --lib` и `cargo test -p fly-browser --lib`: флаг `--lib` исключает каталог
  `tests/`. Восемь contract-тестов `crates/ui/fly/browser/tests/*.rs`
  (`lifecycle_contract`, `response_order_contract`, `intent_timeout_contract`, …)
  **никогда не запускаются**.
* Path-фильтры ссылаются на несуществующие каталоги `crates/ui/fly-ui/**`,
  `crates/ui/fly-browser/**`, `crates/ui/fly-leptos/**` (реальные пути — `crates/ui/fly/ui` и т. д.).
  Сейчас спасает более широкий `crates/ui/fly/**`, но фильтры вводят в заблуждение.
* Нет сборки под `wasm32-unknown-unknown`, хотя `fly-web`/`fly-leptos` имеют
  wasm-only код под `cfg(target_arch = "wasm32")` — он не компилируется **ни в одной** джобе.
  **[проверить сборкой]** подозрение: `[target.'cfg(target_arch="wasm32")'.dependencies]`
  + `dep:js-sys` в фиче `wasm-client` может не резолвиться на хостовой цели.
* Нет `cargo deny` / `cargo audit` / покрытия / бенчмарков для Fly
  (в `ops/benches` бенчей Fly нет вовсе).

### H-7. Verify-скрипты проверяют текст, а не поведение

20 скриптов `scripts/verify/verify-fly-*.mjs` — это `readFile` + `source.includes(marker)`.
Пример `verify-fly-ssr-first.mjs`: 24 захардкоженных пути и набор `requireMarker` /
`rejectMarker` по подстрокам.

Последствия: любой корректный рефактор (переименование, вынос функции, смена форматирования)
ломает «верификацию»; и наоборот — закомментированный маркер проходит проверку. Это создаёт
ложную уверенность и прямо блокирует рефакторинг, необходимый для C-3.

**Фикс:** заменить на архитектурные тесты в Rust (`cargo test --test architecture`:
проверка графа зависимостей по `cargo metadata`), snapshot-тесты (`insta`) для SSR-вывода
и Playwright-тесты для браузерных контрактов. Grep-скрипты оставить только для того,
что действительно является текстовым инвариантом (например, запрет импорта).

---

## 5. Средние дефекты

### M-1. `strip_tags` молча уничтожает легальный текст
`render.rs:684-696`. Наивный автомат по `<`/`>`. Текст `"5 < 10 и 3 > 2"` превращается в
`"5  2"`. При этом функция избыточна: результат всё равно проходит через `escape_html`
(`render.rs:290`, `render.rs:678`), который и так обезвреживает разметку. Это чистая потеря данных.
**Фикс:** убрать `strip_tags`, оставить только экранирование.

### M-2. `remap_ids` подменяет любую строку, совпавшую с id
`model.rs:269-296`: при клонировании repeater-шаблона маппинг применяется ко **всем** строковым
значениям в `attributes`, `style`, `traits`, `extensions` рекурсивно. Текстовое содержимое,
совпавшее с id компонента (например `content: "hero"`), будет подменено на `hero--rep-0`.
**Фикс:** переименовывать только поля, семантически являющиеся ссылками (`id`, `for`,
`aria-*`, `href="#…"`, объявленные в реестре trait-ы типа `ComponentRef`).

### M-3. `safe_style` блокирует `url(` целиком
`render.rs:614-646`. Побочный эффект: `background-image`, `list-style-image`, `@font-face`,
`mask-image` невозможны в принципе. Для визуального билдера это функциональный блокер.
**Фикс:** разрешить `url()` с проверкой содержимого через единый `url_policy` (H-5).

### M-4. Глобальные ре-экспорты как публичный API
`src/lib.rs`: 48 строк `pub use module::*;` на 1299 `pub`-элементов. Нет различия между
доменным API и внутренними деталями; любое добавление `pub` в любом модуле — потенциальный
breaking change по именам; высок риск коллизий при добавлении модулей.
**Фикс:** явный `pub use` с курированным prelude, остальное — `pub(crate)`.

### M-5. `impl Copy` вне модуля-владельца
`src/lib.rs:95-96`:
```rust
impl Copy for ConditionOperator {}
impl Copy for EmptyRepeaterBehavior {}
```
Трейт реализуется в `lib.rs`, а не рядом с типом в `dynamic.rs`. Сигнал, что `#[derive(Copy)]`
не добавили из-за конфликта и обошли вручную. Переносится в `derive` у типа.

### M-6. `FlyError::Validation` теряет содержимое в `Display`
`error.rs:63-64`: `#[error("project validation failed")] Validation(Vec<ValidationDiagnostic>)`.
В логах и в HTTP-ответах пользователь увидит строку без единой диагностики. Нужно
форматировать первые N диагностик с кодами и путями.

### M-7. Квадратичность в `expand_repeater`
`dynamic.rs:567-645`: на каждую итерацию — `component_location` (полный обход),
`remove_component` (полный обход), `insert_component` (поиск родителя полным обходом),
`collect_ids` + `BTreeMap` маппинг + `clone` шаблона. Для 100 элементов и дерева 1000 узлов —
сотни тысяч лишних шагов. Плюс вложенные repeater-ы: каталог
`DynamicCatalog::from_document` строится **до** разворачивания, поэтому определения
вложенных repeater-ов ссылаются на старые id и после `remap_ids` не находятся.
**Нужен регрессионный тест на вложенный repeater.**

### M-8. `History` без бюджета памяти и с `remove(0)`
См. C-3. Отдельным пунктом, потому что чинится независимо: `VecDeque` + лимит в байтах.

### M-9. Отсутствие индекса компонентов
`GrapesProject::component/component_mut` (`model.rs:58-70`),
`ProjectDocument::contains_component`, `component_location` (`placement.rs:35`),
`is_component_descendant_of` (`placement.rs:87`) — все O(n) с полным обходом.
Пакетная команда из K операций → O(K·N).

### M-10. Крайне тонкое покрытие свойств
309 `#[test]`, но **один** `proptest` (`src/tests.rs:249-261`), проверяющий round-trip
одного скалярного поля верхнего уровня. Для крейта, чья главная гарантия — «lossless
bidirectional codec», это недостаточно. Фикстуры GrapesJS: 3 файла по 1.2–1.8 КБ,
одна реальная браузерная (`browser-current.json`, GrapesJS 0.23.6).

### M-11. Повторяющиеся полные обходы в валидации
`validate_project` (`validation.rs:69-99`) вызывает 4+ независимых полных обхода дерева,
каждый из которых заново строит строковые пути через `format!`. Объединяется в один проход
с визитором-коллекцией правил.

### M-12. `canonical_project` клонирует весь документ
`codec.rs:42-47` возвращает `document.project.clone()` только чтобы сериализовать.
Сериализация должна идти по ссылке (`&GrapesProject`), клон не нужен.

---

## 6. Низкие замечания

* `L-1` — `crates/ui/fly/Cargo.toml` не имеет секции `[lints]`, в корневом `Cargo.toml` нет
  `[workspace.lints]`. Нет `#![forbid(unsafe_code)]`, `#![deny(missing_docs)]` при 1299 pub-элементах.
* `L-2` — `leptos/src/root.rs:1-2`: `#[path = "lib.rs"] mod foundation;` при `[lib] path = "src/root.rs"`.
  Файл `lib.rs`, который не является крейт-корнем, — ловушка для IDE и новых разработчиков.
* `L-3` — `SequentialIdGenerator` (`ids.rs`) предсказуем; для многопользовательского
  редактирования нужен ULID/UUIDv7, иначе параллельные сессии дают конфликты id.
* `L-4` — `escape_html` (`render.rs:697-702`) делает 3 прохода `String::replace` с 3 аллокациями;
  на горячем пути рендера заменяется однопроходной записью в `&mut String`.
* `L-5` — `GrapesProject::component` возвращает первое совпадение по всем страницам; при
  дубликатах id между страницами (валидатор ловит, но только как отчёт) поведение недетерминировано
  по отношению к намерению пользователя.
* `L-6` — `ValidationLimits::maximum_nodes = 10_000` — жёсткий дефолт без обоснования и без
  документированной связи с реальными лендингами.
* `L-7` — нет `CHANGELOG`/`SECURITY.md` внутри `crates/ui/fly/`, при заявке на отдельный OSS-репозиторий.
* `L-8` — `fly-dioxus` имеет `default = []`, то есть по умолчанию собирается `dioxus` без рендерера;
  в отличие от `fly-leptos` (`default = ["ssr"]`). Асимметрия без объяснения.

---

## 7. План устранения

Фазы упорядочены так, чтобы безопасность и CI шли первыми, а тяжёлый рефактор ядра
опирался уже на настоящие тесты, а не на grep-скрипты.

### Фаза 0 — восстановить обратную связь (1 неделя)

| Задача | Детали | Критерий готовности |
|---|---|---|
| 0.1 | Добавить в CI `fly-ui`, `fly-web`, `fly-leptos`, `fly-dioxus`: `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check` | все 6 крейтов в каждой джобе |
| 0.2 | Убрать `--lib` из `cargo test -p fly` и `-p fly-browser` | 8 contract-тестов `browser/tests/` реально исполняются |
| 0.3 | Исправить path-фильтры workflow (`crates/ui/fly/ui/**` вместо `crates/ui/fly-ui/**`) | фильтры соответствуют дереву |
| 0.4 | Джоба `cargo check --target wasm32-unknown-unknown -p fly-web -p fly-leptos --features wasm-client` | wasm-код компилируется в CI |
| 0.5 | `[workspace.lints]` + `#![forbid(unsafe_code)]` во всех крейтах Fly | clippy зелёный с pedantic-подмножеством |
| 0.6 | `cargo deny check` для поддерева Fly | джоба зелёная |

### Фаза 1 — безопасность (1–2 недели, блокирует релиз)

| Задача | Детали |
|---|---|
| 1.1 **C-1** | `ProjectHash` → SHA-256 (`[u8;32]`). Единая функция `fly::hash::sha256_hex`, удалить дублирующий `sha256_hex` из `landing_contract.rs`. `from_document` → `FlyResult<ProjectHash>`, убрать `unwrap_or_default`. |
| 1.2 **C-1** | Миграция: `ProjectSnapshot`/`ProjectBundle` получают поле `hash_algorithm`; читатель принимает старый `fnv1a64` в режиме «только чтение, с warning-диагностикой», писатель всегда пишет `sha256`. Срок выключения совместимости — следующий мажор модуля (по политике ADR). |
| 1.3 **C-2** | Валидация charset `component.id` / `page.id` (`^[A-Za-z0-9_:.-]{1,128}$`, severity `Error`). |
| 1.4 **C-2** | Полный CSS-escape в `escape_css_attribute`; нейтрализация `</` при вставке CSS в `<style>` в `compose_document_html`. Тесты: `</style>`, `</script>`, `<!--`, `]]>`. |
| 1.5 **H-3** | `MAX_DECODE_DEPTH` (512) в `GrapesJsCodec::decode_slice/decode_value` до построения модели; новый `FlyError::MaximumDepthExceeded`. Fuzz-тест глубокого JSON. |
| 1.6 **H-4** | `fly-browser.js`: fail-closed `expectedOrigin` (нет значения → `start()` бросает); whitelist схем для `result.location` (только относительный путь либо same-origin); `intentEndpoint` только относительный; убрать чтение токена из `localStorage` и дублирующий `x-fly-access-token`, перейти на httpOnly-cookie + CSRF-токен. |
| 1.7 **H-5** | Выделить `src/url_policy.rs` как единственный источник правды; `safe_url.rs`, `render.rs::url_allowed`, `validation.rs::public_url_allowed` — тонкие обёртки. Общий табличный тест-корпус (≥60 кейсов), прогоняемый через все три точки входа. |
| 1.8 | `cargo fuzz` таргет на `GrapesJsCodec::decode_slice` (или `arbitrary` + proptest, если fuzz инфраструктуры нет). |

### Фаза 2 — честная документация и границы (1 неделя, параллельно Ф1)

| Задача | Детали |
|---|---|
| 2.1 **H-1** | Переписать `crates/ui/fly/README.md`: таблица «Статус» со значениями `stable / foundation-only / planned`. `fly-leptos` и `fly-dioxus` явно помечаются `foundation-only (layout shells)`. Убрать упоминание несуществующего `locales/`. |
| 2.2 **H-1** | Либо убрать `rustok-ui-i18n` из `fly` (вынести `locale_fallback_chain`/`normalize_locale_tag` за трейт `LocalePolicyProvider`, реализация — в RusTok), либо удалить заявку на извлекаемость из README и ADR. Рекомендация: **первое** — это 2 функции в `runtime_locale.rs:246,263`. |
| 2.3 **H-1** | Дописать `standalone-Cargo.toml`: `[workspace.package]` + `[workspace.dependencies]`, чтобы шаблон реально собирался. Джоба CI, копирующая поддерево во временный каталог и делающая `cargo check`. |
| 2.4 **H-1** | Удалить неиспользуемые зависимости `fly`/`fly-ui` из `fly-leptos`/`fly-dioxus` либо начать их использовать (`cargo machete` / `cargo udeps` в CI). |
| 2.5 **M-4** | Курированный публичный API: `pub mod prelude`, явные `pub use`, всё остальное `pub(crate)`. Это breaking change — выполнять одним коммитом с обновлением `rustok-page-builder`. |
| 2.6 **M-5** | `#[derive(Copy)]` у `ConditionOperator` и `EmptyRepeaterBehavior`, удалить ручные `impl` из `lib.rs`. |
| 2.7 **M-6** | Сделать `Display` для `FlyError::Validation` информативным. |

### Фаза 3 — заменить grep-верификацию на настоящие тесты (2 недели)

| Задача | Детали |
|---|---|
| 3.1 **H-7** | Архитектурный тест на Rust: парсинг `cargo metadata`, проверка запрещённых рёбер графа (`fly -/-> fly-ui`, `rustok-page-builder -/-> fly-leptos`, …). Заменяет `verify-fly-dependency-boundaries.mjs`. |
| 3.2 **H-7** | Snapshot-тесты (`insta`) на SSR-вывод: `render_page` + `compose_document_html` для набора фикстур. Заменяет `verify-fly-ssr-first.mjs`, `verify-fly-ssr-assets.mjs`, `verify-fly-snapshots.mjs`. |
| 3.3 **H-7** | Playwright-тесты (уже есть каркас в `apps/next-admin/tests/e2e/fly-browser-*.spec.ts`) как единственный источник проверки браузерных контрактов. Заменяет `verify-fly-admin-browser-runtime.mjs`, `verify-fly-interaction-capabilities.mjs`. |
| 3.4 **H-7** | Оставшиеся `verify-fly-*.mjs` удалить из CI и из репозитория; то, что действительно текстовый инвариант, перенести в один `verify-fly-source-invariants.mjs` с явным списком и комментарием «почему grep». |
| 3.5 **M-10** | Расширить GrapesJS-фикстуры: ≥10 реальных браузерных захватов (`scripts/capture/capture-fly-grapesjs-fixture.mjs` уже есть), включая preset-webpage, формы, таблицы, кастомные плагины, большой лендинг (>500 узлов). |
| 3.6 **M-10** | Property-тесты: генератор произвольных `GrapesProject` + инварианты `decode(encode(x)) == x`, `apply(undo(apply(cmd))) == identity`, «валидный документ рендерится без паники», «любой id проходит через рендер без выхода из контекста». |

### Фаза 4 — производительность ядра (3–4 недели, самая крупная)

| Задача | Детали |
|---|---|
| 4.1 **C-3** | Обратимые команды: `trait Reversible { fn invert(&self, before: &ProjectDocument) -> InverseCommand }`. `HistoryEntry` хранит `(command, inverse)` вместо двух документов. Ожидаемое сокращение памяти истории — на 2–3 порядка. |
| 4.2 **C-3** | Убрать клоны в `apply()`: применять команду к `&mut self.document` под «транзакцией» (при ошибке — откат через `inverse`), вместо `before/after`. |
| 4.3 **M-9** | `ComponentIndex`: `HashMap<Arc<str>, ComponentPath>` внутри `ProjectDocument`, перестраивается лениво и инвалидируется структурными правками. `component`, `component_mut`, `contains_component`, `component_location`, `is_component_descendant_of` — O(1)/O(depth). |
| 4.4 **C-3/M-11** | Единый обход в `validate_project`; инкрементальная валидация затронутого поддерева в `apply()`, полный проход — только в явном `validate()` и перед публикацией. |
| 4.5 **C-3** | `visit_components` — путь как `&[PathSegment]`, `format!` только при создании диагностики. |
| 4.6 **M-8** | `History` → `VecDeque` + бюджет в байтах (`with_memory_budget`). |
| 4.7 **M-12** | `canonical_project` без клона; сериализация по ссылке. |
| 4.8 **M-7** | Переписать `expand_repeater` на «вырезать шаблон один раз → построить все клоны → вставить одним срезом». Тест на вложенные repeater-ы. |
| 4.9 **L-4** | Однопроходный `escape_html`/`escape_attribute` с записью в `&mut String`. |
| 4.10 | Бенчмарки в `ops/benches`: `fly_apply_command`, `fly_render_page`, `fly_codec_roundtrip`, `fly_validate_project` на документах 100 / 1 000 / 10 000 узлов. Порог регрессии в CI. |

### Фаза 5 — функциональные долги (по мере приоритета продукта)

| Задача | Детали |
|---|---|
| 5.1 **M-1** | Удалить `strip_tags`, оставить экранирование. Тест на `"5 < 10"`. |
| 5.2 **M-2** | Точечный `remap_ids` только по полям-ссылкам. |
| 5.3 **M-3** | Разрешить `url()` в inline-стилях через `url_policy`. |
| 5.4 **H-2** | Диагностика `component_degraded_to_opaque` при молчаливой деградации узла. |
| 5.5 **L-3** | ULID/UUIDv7 вместо `SequentialIdGenerator` для многопользовательских сессий (трейт `IdGenerator` уже есть — смена реализации неинвазивна). |
| 5.6 **H-1** | Решение по Leptos/Dioxus: либо довести `fly-leptos` до реального редактора (канвас, drag&drop, панели, биндинг к `FlyUiStateMachine`), либо отметить адаптеры как экспериментальные и убрать из workspace до готовности. Без решения README останется недостоверным. |

---

## 8. Предлагаемая очерёдность

```
Ф0 (неделя 1)  ────────────────┐
Ф1 (недели 1–3) ───────────────┤── релизный блокер
Ф2 (неделя 2)  ────────────────┘
Ф3 (недели 3–5) ──────────────────── снимает блок с рефакторинга
Ф4 (недели 5–9) ──────────────────── требует Ф3
Ф5 (постоянно) ─────────────────────
```

**Нельзя начинать Ф4 до завершения Ф3.** Сейчас единственная «проверка» архитектуры —
grep по исходникам; любой рефактор `apply()` или `visit_components` сломает десяток
`verify-fly-*.mjs` и не будет при этом проверен по существу.

---

## 9. Что обязательно подтвердить сборкой

1. `cargo clippy --workspace -p fly -p fly-ui -p fly-web -p fly-leptos -p fly-dioxus -p fly-browser --all-targets -- -D warnings` — объём предупреждений неизвестен (CI линтит только `fly --lib`).
2. `cargo check --target wasm32-unknown-unknown -p fly-web --features wasm-client` — подозрение на нерезолвимый `dep:js-sys` из target-specific секции.
3. `cargo test -p fly-browser` без `--lib` — 8 contract-тестов никогда не исполнялись, их фактический статус неизвестен.
4. `cargo udeps` / `cargo machete` — подтвердить неиспользуемые `fly`/`fly-ui` в адаптерах.
5. `cargo tree -d` по поддереву Fly — дубли версий.
6. Профиль `apply()` на документе 10 000 узлов — зафиксировать базовую линию до Ф4.
