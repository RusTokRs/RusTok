# Инженерный аудит `rustok-ui-i18n` — 2026-09-27

## Резюме

Проведена ревизия production-кода, публичного API, макросов, Fluent-схем, тестов,
CI и фактических потребителей `rustok-ui-i18n` в workspace. Аудит не опирается на
предположения из старого research-документа: каждый вывод ниже проверен по текущему
коду.

До этой ревизии фундамент уже корректно решал базовые задачи: потокобезопасная
инициализация через `OnceLock`, concurrent Fluent bundles, bidi-изоляция FSI/PDI,
ограниченный ввод locale/key, структурный fallback, строгая и мягкая сборка
каталогов, WASM-совместимый in-memory runtime. Критические гипотезы старого отчёта
про отключённую bidi-изоляцию, небезопасную lazy-инициализацию и `unwrap` на
production-парсинге **не подтвердились**.

Подтвердились четыре существенных функциональных/архитектурных пробела и четыре
проблемы эксплуатационного качества. Все подтверждённые проблемы в зоне
ответственности crate устранены этим изменением.

## Область проверки

Проверены:

- `src/bundle.rs`, `error.rs`, `locale.rs`, `messages.rs`, `macros.rs`, `prelude.rs`;
- все integration/unit/property/concurrency/stress-тесты crate;
- публичные root/prelude/module-macro поверхности;
- 50 module-owned UI packages и их Fluent-каталоги;
- workspace-валидаторы ключей, parity и i18n-контракта;
- `.github/workflows/ui-i18n-verify.yml`;
- реальный вызов интерполяции количества в Notifications storefront.

Не включены в ответственность crate: выбор locale из URL/cookie/header,
реактивность Leptos/Next, установка HTML `lang/dir`, бизнес-копирайтинг,
перевод контента из БД и runtime filesystem loading.

## Реестр находок и исправлений

### F-01 — Fluent attributes были недоступны через facade

- **Серьёзность:** высокая.
- **Статус до:** подтверждено.
- **Причина:** runtime разрешал только `message.value()`. Compound Fluent messages
  (`.placeholder`, `.aria-label`, `.title`, `.accesskey`) можно было создать, но
  нельзя было получить через `UiMessages`, `PreparedUiMessages`, `UiTranslator`
  или `UiLocaleTranslator`.
- **Последствия:** потребители вынуждены были дробить одну Fluent translation unit
  на искусственные ключи; отсутствовал независимый fallback атрибута; accessibility
  copy не имел полноценного API.
- **Исправление:** добавлены строгие и lenient attribute lookup/format API на всех
  facade-уровнях и низкоуровневые функции. Атрибут проходит тот же locale fallback,
  форматирование, bidi isolation и typed error contract, что и value.
- **Диагностика:** добавлены `AttributeNotFound` и `InvalidMessageAttribute`.
- **Проверка:** `compound_message_attributes_format_and_fall_back_independently`,
  `strict_attribute_errors_are_typed_and_lenient_calls_use_fallback`.

### F-02 — schema validation смешивала value/attributes и не видела зависимости

- **Серьёзность:** высокая.
- **Статус до:** подтверждено.
- **Причина 1:** переменные value и всех attributes объединялись в один set. Если
  локаль намеренно не переводила optional attribute и полагалась на fallback,
  strict startup мог ложно вернуть `MessageSchemaMismatch`.
- **Причина 2:** не обходились `MessageReference` и `TermReference`. Контракт
  `welcome = { base }` считался без аргументов даже когда `base` требовал `$name`.
- **Причина 3:** unresolved/cyclic references проходили startup и обнаруживались
  лишь при форматировании конкретного сообщения.
- **Исправление:** введён `MessageEntrySchema` с раздельными value/attribute
  контрактами; построен reference graph сообщений и terms; переменные вычисляются
  транзитивно; named term arguments исключают связанные внутренние параметры;
  unresolved/cyclic/duplicate graph entries завершают strict preparation typed
  ошибкой. Attribute parity валидируется отдельно, а отсутствие перевода по-прежнему
  допускается ради fallback.
- **Диагностика:** `ExtraMessageValue`, `ExtraMessageAttribute`,
  `MessageAttributeSchemaMismatch`, `DuplicateEntry`, `UnresolvedReference`,
  `CyclicReference`.
- **Lenient runtime:** остаётся fail-soft, но его cached initialization report теперь
  сохраняет schema/reference defect для parseable catalog.
- **Проверка:** отдельные тесты value/attribute contracts, transitive references,
  term argument binding, missing/cyclic references.

### F-03 — `validate_message_key` не валидировал Fluent identifier

- **Серьёзность:** высокая (correctness + input hardening).
- **Статус до:** подтверждено.
- **Причина:** проверялись только пустота, длина и ASCII control bytes. Строки
  `" key"`, `.leading`, `key/value`, non-ASCII IDs и Unicode C1 controls принимались,
  хотя Fluent message ID их не поддерживает. Они превращались в неотличимый от
  обычного missing-key результат; Unicode controls противоречили документации.
- **Исправление:** ключ должен начинаться с ASCII letter и далее содержать только
  ASCII alphanumeric, `_`, `-` и документированный `.` alias. Проверка control
  characters выполняется по Unicode scalar values (`char::is_control`). Attribute
  IDs используют ту же bounded grammar без dot alias.
- **Диагностика:** `MessageKeyError::InvalidSyntax`.
- **Совместимость:** корректные существующие ключи workspace соответствуют grammar;
  ошибочные значения теперь получают явный typed error вместо cache miss.

### F-04 — runtime скрывал фактическую locale после fallback

- **Серьёзность:** средняя.
- **Статус до:** подтверждено.
- **Причина:** API возвращал только `String`. Потребителю, которому нужна provenance
  для telemetry/cache/UI metadata, приходилось повторять fallback-алгоритм и рисковать
  drift.
- **Исправление:** `ResolvedMessage { value, locale }` и provenance-aware strict/
  lenient методы. Locale — canonical catalog key, реально отдавший pattern.
  Facade-типы также предоставляют deterministic `available_locales()`.
- **Производительность:** старые string-only API не строят второй fallback chain;
  provenance извлекается из индекса уже найденного candidate.

### F-05 — module macro скрывал health/attribute API

- **Серьёзность:** средняя.
- **Статус до:** подтверждено.
- **Причина:** `declare_module_i18n!` генерировал только `t` и `format`, а `MESSAGES`
  оставался private. Внешний host не мог единообразно вызвать strict validation,
  посмотреть initialization diagnostics или форматировать attribute.
- **Исправление:** macro дополнительно генерирует `format_attribute`, `validate` и
  `initialization_diagnostics`, сохраняя прежние вызовы без изменений.

### F-06 — Notifications обходил Fluent interpolation

- **Серьёзность:** средняя.
- **Статус до:** подтверждено на реальном consumer.
- **Причина:** FTL экранировал `{count}` как literal, затем Rust делал
  `String::replace`. Это обходило Fluent args, plural/select semantics и единый
  bidi-safe formatting path.
- **Исправление:** каталоги используют `{ $count }`, consumer — `fluent_args!` и
  сгенерированный `format`.

### F-07 — focused CI имел неполные triggers и нестабильную WASM provisioning

- **Серьёзность:** средняя.
- **Статус до:** подтверждено.
- **Причина 1:** workflow запускался только при изменении директории crate или самого
  workflow. Изменение root `Cargo.toml`, `Cargo.lock` либо `rust-toolchain.toml`
  могло сломать crate без запуска focused gate.
- **Причина 2:** setup action устанавливал target для pinned `1.96.0`, но repository
  override `stable` выбирал для `cargo` другой rustup toolchain identity. Gate падал
  с `E0463 can't find crate for std/core`, не проверяя библиотеку.
- **Исправление:** root-файлы добавлены в push/PR path filters; job фиксирует
  `RUSTUP_TOOLCHAIN=1.96.0`, а перед WASM check выполняется идемпотентный `rustup
  target add` для этой же identity.

### F-08 — документация расходилась с реализацией

- **Серьёзность:** низкая/средняя.
- **Статус до:** подтверждено.
- `build_fluent_catalog` назывался логирующим, хотя функция намеренно pure;
- prelude-документ не перечислял уже экспортированные validators/constants;
- speculative research report выдавал неподтверждённые риски за дефекты;
- compound messages, provenance и реальная глубина schema validation не были
  описаны.
- **Исправление:** README, implementation plan, public API policy и этот аудит
  синхронизированы с кодом. Старый отчёт помечен superseded.

## Проверенные инварианты, не требующие изменения

1. **Bidi:** `set_use_isolating(true)` вызывается при каждой сборке Rust bundle;
   lenient formatting не возвращает частичный malformed output.
2. **Concurrency:** `UiMessages` использует `OnceLock<FluentCatalogBuildReport>` и
   `Once` для one-time logging; каталог содержит concurrent `FluentBundle`.
3. **Locale model:** extension-bearing tags намеренно отклоняются; Rust identity —
   `LanguageIdentifier` (language/script/region/variants), а extension policy остаётся
   у host.
4. **Fallback:** exact → variants removed → region removed → script removed →
   language → default hierarchy → `en`; candidates canonical и deduplicated.
5. **No I/O:** production crate не сканирует filesystem и пригоден для WASM.
6. **Strict/lenient split:** strict preparation fail-closed; legacy rendering
   fail-soft с typed cached diagnostics.
7. **Catalog collision:** normalized locale duplicates и duplicate Fluent entries
   не перезаписываются молча.
8. **Bounded diagnostics:** oversized locale/key payload не копируется целиком.

## Архитектурная граница после исправлений

```text
host/runtime
  └─ выбирает effective locale (route/cookie/header/user policy)
     └─ rustok-ui-i18n
        ├─ canonical locale + structural fallback
        ├─ immutable concurrent Fluent catalog
        ├─ value / attribute formatting + bidi isolation
        ├─ strict startup schema/reference validation
        ├─ lenient rendering + cached diagnostics
        └─ resolved-locale provenance
           └─ Leptos/Dioxus/other UI code renders String
```

Crate по-прежнему не должен зависеть от Leptos, Axum, GraphQL, router, cookies,
environment или module business copy.

## Верификация

Обязательная матрица:

```text
cargo fmt -p rustok-ui-i18n -- --check
cargo test -p rustok-ui-i18n --all-features
cargo clippy -p rustok-ui-i18n --all-targets --all-features -- -D warnings
cargo check -p rustok-ui-i18n --all-features --target wasm32-unknown-unknown
npm run verify:i18n:ui
npm run verify:i18n:keys -- --strict
```

Дополнительно проверяется Notifications storefront consumer.

На sandbox без установленного Rust toolchain локально выполнены JS gates, а Rust
матрица выполнена focused GitHub workflow:

- `cargo fmt -p rustok-ui-i18n -- --check` — PASS;
- `cargo test -p rustok-ui-i18n --all-features` — PASS;
- `cargo clippy -p rustok-ui-i18n --all-targets --all-features -- -D warnings` — PASS;
- UI catalog parity — PASS;
- strict UI key inventory — PASS (987 occurrences, 50 packages, 0 missing);
- repository i18n contract — имеет независимый baseline failure в
  `apps/admin/build.rs` (ожидаемый `Config::new("en")?` отсутствует); изменение
  `rustok-ui-i18n` эту проверку не ухудшает.

WASM check выявил дефект provisioning самого workflow (`wasm32-unknown-unknown`
не был установлен несмотря на setup input); gate исправлен явной идемпотентной
установкой target и повторно выполнен перед merge.

## Осознанно отложенные направления (не дефекты текущего контракта)

- locale-aware currency/date/number functions требуют отдельного API-design решения
  и не должны вноситься как скрытая host policy;
- native fuzz harness добавляется только вместе с общей repository fuzz
  infrastructure; property/stress tests уже остаются обязательными;
- сужение compatibility-only exports требует сохранённого результата
  `cargo xtask i18n-api-inventory` и отдельного pre-1.0 migration window;
- benchmark numbers должны сохраняться при конкретном hot-path изменении, а не
  генерироваться без сравниваемого baseline.

Эти пункты не оставляют незавершённого runtime поведения в заявленной текущей зоне
ответственности библиотеки.
