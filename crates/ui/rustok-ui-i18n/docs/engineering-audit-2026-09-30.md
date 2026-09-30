# Инженерный аудит i18n-подсистемы — 2026-09-30

> Продолжение [`engineering-audit-2026-09-27.md`](./engineering-audit-2026-09-27.md).
> Прошлый аудит закрывал границу **самого crate**. Эта ревизия проверяет i18n
> как сквозную подсистему платформы: crate + 48 модульных каталогов + хосты
> (Leptos admin/storefront, next-admin, next-frontend) + `rustok-api` /
> `rustok-core` / `fly` / Page Builder + CI-гейты.

## Как это проверялось

```bash
node scripts/verify/verify-i18n-contract.mjs        # FAIL (exit 1)
node scripts/verify/verify-ui-i18n-parity.mjs       # PASS
node scripts/verify/verify-ui-i18n-keys.mjs --strict # PASS (987 вхождений / 50 пакетов)
```

Дополнительно написан одноразовый строгий FTL-анализатор (parity сообщений,
атрибутов, термов, наборов переменных, дубликатов и неразрешённых ссылок по
всем 112 `.ftl`) — **0 расхождений**, каталоги структурно консистентны.
Собранная статистика: 112 файлов, 9157 сообщений, 0 термов, 16 атрибутов,
351 использование переменных, **22** select-выражения.

**Rust-матрица не выполнялась**: в песочнице нет toolchain, а `static.rust-lang.org`
и `static.crates.io` недоступны (TLS handshake обрывается). Всё ниже —
статический анализ кода и исполнение JS-гейтов. `cargo fmt/clippy/test/check
--target wasm32` остаются за CI/мейнтейнером.

---

## Сводка

| # | Находка | Серьёзность | Владелец |
|---|---|---|---|
| A-01 | Гейт `verify:i18n:contract` красный на `main` | **Blocker** | `apps/next-frontend` |
| A-02 | Гейты parity/keys не запускаются ни одним workflow | **Blocker** | CI |
| A-03 | `validate()`/`prepare()` не вызываются ни одним из 48 модулей — strict-валидация мертва | **High** | модули + CI |
| A-04 | 3 модуля с осиротевшими каталогами и захардкоженным `ru/en` | **High** | brand, marketplace-listing, marketplace-seller |
| A-05 | `fly::normalize_locale_tag` лоукейсит локали и смешан с каноническим парсером в Page Builder | **High** | `fly`, `rustok-page-builder` |
| A-06 | Три расходящихся алгоритма locale-fallback | **High** | `rustok-ui-i18n`, `rustok-api`, `fly` |
| A-07 | Три разных предела длины locale tag (64 / 32 / 32) | Medium | те же |
| A-08 | `next-admin` переизобретает Accept-Language и игнорирует q-значения | **High** | `apps/next-admin` |
| A-09 | CLDR `parentLocales` не реализованы: `es-MX` → `en`, а не `es-419` | Medium | `rustok-ui-i18n` |
| A-10 | Плюрализация фактически не используется; подтверждённые грамматические дефекты в ru/en/ar | Medium | модульные каталоги |
| A-11 | Нет `NUMBER`/`DATETIME`: локале-зависимого форматирования чисел/дат нет вовсе | Medium | `rustok-ui-i18n` |
| A-12 | Дефолтный lenient-путь на первом lookup выполняет полный повторный парсинг каталога | Medium | `rustok-ui-i18n` |
| A-13 | Горячий путь `t()` реаллоцирует fallback-цепочку и заново парсит locale на каждый ключ | Medium | `rustok-ui-i18n` |
| A-14 | Схема термов требует переменные, которые невозможно передать | Medium (латентная) | `rustok-ui-i18n` |
| A-15 | `normalize_admin_locale` — host-политика `ru\|en` внутри «framework-agnostic» crate | Medium | `rustok-ui-i18n` |
| A-16 | Утечка типов `unic-langid` / `fluent-bundle` в публичные ошибки без ре-экспорта | Medium | `rustok-ui-i18n` |
| A-17 | `t!` несовместим с половиной собственных фасадов | Medium | `rustok-ui-i18n` |
| A-18 | Мёртвый workflow `next-fluent-verify.yml` (путь `packages/next-fluent` не существует) | Medium | CI |
| A-19 | Незапиненная GitHub-зависимость `@rustok/next-fluent` | Medium | Next-приложения |
| A-20 | Fail-soft загрузка FTL в Next + `__dirname` вне `try` | Medium | Next-приложения |
| A-21…A-32 | Гигиена гейтов, API, тестов, документации | Low | разное |

---

## Статус починки (обновляется по мере работ)

Исправлено в ветке `arena/01a0f3f0-rustok` (коммиты `9a1fab7`, `a71f52f`,
`bb75810`, `33cd202`):

| # | Статус | Примечание |
|---|---|---|
| A-01 | ✅ | контракт проверяет реального владельца `i18n-config.ts` + требует ре-экспорт |
| A-02 | ✅ | parity и strict keys подключены к `ui-i18n-verify.yml`, path-фильтры синхронизированы |
| A-03 | ✅ | `declare_module_i18n!` генерирует `#[cfg(test)]`-контракт на `validate()` |
| A-08 | ✅ | q-совместимый парсер `apps/next-admin/src/i18n/accept-language.ts` |
| A-11 | 📝 | задокументировано в README «Known Limitations» |
| A-12 | ✅ | schema-валидация убрана с ленивого пути |
| A-14 | 📝 | задокументировано (латентно: 0 термов в репозитории) |
| A-15 | 📝 | задокументировано как legacy host policy |
| A-16 | ✅ | `FluentError` / `LanguageIdentifierError` ре-экспортированы |
| A-17 | ✅ | `t_for_locale` добавлен в `PreparedUiMessages` и `UiTranslator` |
| A-18 | ✅ | мёртвый `next-fluent-verify.yml` удалён |
| A-19 | ✅ | `@rustok/next-fluent` запинен на `14f870b` |
| A-20 | ✅ | fail-closed каталог + `import.meta.url` вместо `__dirname` |
| A-21…A-24 | ✅ | верификаторы переписаны на структурный FTL-парсер, мёртвый exclusion-list удалён |
| A-26 | ✅ | `MAX_LOCALE_TAG_LEN` публичный |
| A-27 | ✅ | prelude дополнен Accept-Language контрактом и константами |
| A-29 | ✅ | локаль берётся из имени файла, ошибки чтения не проглатываются |
| A-30 | ✅ | `deep-research-report (2).md` удалён (корневой (5) — вне i18n-скоупа, на него ссылается PRODUCTION_REMEDIATION_PLAN) |
| A-31 | ✅ | `skip_while` заменён на `filter` |
| A-10 | ✅ | 19 сообщений получили CLDR-селекторы (en/ru/ar), 13 — обоснованный `# plural-exempt` |
| A-33 | ⚠️ | **новая находка**: 105 сайтов обходили Fluent через `String::replace`; 59 переведены на `fluent_args!`, 46 под shrink-only baseline |
| — | ✅ | **новая находка**: 90 ключей использовались кодом, но отсутствовали в каталогах (русский UI показывал английский) |

Дополнительно исправлена ошибка заимствования, внесённая коммитом `a71f52f`:
семь сгенерированных `fluent_args!` передавали `&x.to_string()` — ссылку на
временное значение. Все семь — числовые поля, теперь передаются как числа.

Не начато: A-04 (осиротевшие каталоги brand / marketplace-listing /
marketplace-seller), A-05/A-06/A-07/A-32 (owner-level унификация локалей),
A-09 (CLDR `parentLocales`), A-25/A-28 (гигиена API).

**A-05/A-06 требуют компилятора.** Нижний регистр локалей (`ru-ru`) зашит не
только в `fly::normalize_locale_tag`, но и в тесты `fly/src/locale_policy.rs`,
`fly/src/translation.rs`, `fly/src/runtime_locale.rs`,
`rustok-page-builder/src/locale.rs` и
`rustok-page-builder/admin/src/editor/ssr_locale_policy.rs`. Смена конвенции
на канонический BCP-47 затрагивает ~8 файлов тестов и поведение артефактов
Page Builder — делать это вслепую нельзя.

---

## Blocker

### A-01 — Репозиторный i18n-контракт красный на `main`

```
$ node scripts/verify/verify-i18n-contract.mjs; echo $?
i18n contract drift detected:
- apps/next-frontend/src/i18n.ts: expected apps/next-frontend to use platform fallback locale 'en'
1
```

Причина: объявление уехало в `apps/next-frontend/src/i18n-config.ts:3`
(`export const defaultLocale = "en";`), а `i18n.ts:12` теперь только
ре-экспортирует его. Верификатор (`scripts/verify/verify-i18n-contract.mjs:61-65`)
ищет литеральную строку в `i18n.ts`.

Усугубляющий фактор: `.github/workflows/ui-i18n-verify.yml` **не содержит**
`apps/next-frontend/**` в path-фильтрах. Гейт проверяет файл, изменение которого
его не запускает. Тот же разрыв для `apps/next-admin/src/i18n/config.ts`
(проверяется на строках 66-70, в фильтрах отсутствует) и для
`crates/modules/rustok-modules/src/static_package.rs`,
`crates/libs/rustok-core/src/field_schema.rs`, `crates/modules/rustok-ai/src/metrics.rs`.

Исправление — не подгонка регулярки: либо контракт должен проверять
`i18n-config.ts` как фактического владельца, либо верификатор должен резолвить
ре-экспорт. Плюс синхронизировать path-фильтры со списком проверяемых файлов.

### A-02 — Основные i18n-гейты не запускаются в CI

`package.json:28-30` объявляет `verify:i18n:ui`, `verify:i18n:keys`,
`verify:i18n:contract`. Грепом по `.github/workflows/` находится **только**
`verify-i18n-contract.mjs` (в `ui-i18n-verify.yml:76`).

- `verify-ui-i18n-parity.mjs` — единственная проверка parity ключей между
  локалями — не вызывается ни одним workflow.
- `verify-ui-i18n-keys.mjs --strict` — единственная проверка «ключ, который
  зовёт код, существует в каталоге» — тоже.
- `scripts/verify/verify-all.sh` (где они перечислены, строки 122-123) сам
  нигде в `.github/` не запускается.
- В `ci.yml` слова `i18n` / `locale` / `fluent` не встречаются вообще.

То есть удаление ключа из `ru.ftl` или опечатка в `t("...")` уедут в `main`
беззвучно.

---

## High

### A-03 — Strict-валидация каталогов не вызывается нигде

```bash
$ grep -rn "::prepare()\|i18n::validate\|initialization_diagnostics" --include=*.rs crates apps \
  | grep -v "rustok-ui-i18n/"
apps/admin/src/i18n.rs:16:        assert!(MESSAGES.initialization_diagnostics().is_empty());
apps/storefront/src/shared/local.rs:97:        assert!(MESSAGES.initialization_diagnostics().is_empty());
```

48 файлов вызывают `declare_module_i18n!`, макрос честно генерирует
`validate()` и `initialization_diagnostics()` — **ни один модуль их не зовёт**,
и `prepare()` не вызывается ни разу во всём репозитории.

Следствие: весь аппарат F-02 из прошлого аудита (`MessageSchemaMismatch`,
`ExtraMessageValue`, `ExtraMessageAttribute`, `MessageAttributeSchemaMismatch`,
`DuplicateEntry`, `UnresolvedReference`, `CyclicReference`) — код, который
никогда не исполняется на реальных каталогах платформы. Ни один CI-шаг не
проходит по всем 50 пакетам с `validate()`.

Дополнительно `static MESSAGES` в `macros.rs:217` не `pub`, поэтому внешний
harness не может собрать все каталоги в один тест — требуется либо `pub`
static, либо генерируемый `#[test]`, либо inventory-реестр.

**Рекомендация:** макрос должен генерировать `#[cfg(test)] #[test] fn
module_i18n_catalog_is_valid() { assert!(validate().is_ok()) }` — тогда гейт
появляется автоматически у всех 48 потребителей без ручной дисциплины.

### A-04 — Осиротевшие каталоги и захардкоженный `ru/en`

| Пакет | ключей в `en.ftl` | `declare_module_i18n!` | вхождений `russian` |
|---|---|---|---|
| `rustok-brand/admin` | 22 | нет | 37 |
| `rustok-marketplace-listing/admin` | 5 | нет | 43 |
| `rustok-marketplace-seller/admin` | 4 | нет | 44 |

Все три `src/i18n.rs` состоят из одной строки:

```rust
pub use rustok_ui_i18n::normalize_admin_locale;
```

а UI строит текст так (`.../admin/src/ui/leptos.rs:34`):

```rust
let locale = normalize_admin_locale(route_context.locale.as_deref());
let russian = locale == "ru";
```

`grep -rn "locales/" .../admin/src` даёт **0** — каталоги никогда не
подключаются `include_str!`. Они при этом собираются, версионируются, проходят
parity-гейт и рапортуются `verify-ui-i18n-keys.mjs` как «OK … (22 catalog keys)»,
потому что скрипт считает вхождения ключей, а не подключение каталога.

Это одновременно: мёртвый ассет, невыполнимая локализация за пределами `ru/en`
и ложный зелёный сигнал в обоих гейтах.

### A-05 — `fly::normalize_locale_tag` производит неканонические локали

`crates/ui/fly/src/runtime_locale.rs:257-270`:

```rust
pub fn normalize_locale_tag(locale: &str) -> Option<String> {
    let locale = locale.trim().replace('_', "-").to_ascii_lowercase();
    ...
    Some(locale)
}
```

`ru-RU` → `ru-ru`, `zh-Hant` → `zh-hant`, `es-419` → `es-419`. Это четвёртая
реализация нормализации в репозитории и единственная, которая **ломает
каноническую форму BCP-47**.

`crates/modules/rustok-page-builder/src/locale.rs` смешивает её с каноническим
парсером **в одной структуре**:

```rust
use fly::{..., normalize_locale_tag};                       // :1  — лоукейс
use rustok_ui_i18n::accept_language_catalog_locales;        // :2  — канон ICU
...
let accepted = accept_language.map(accept_language_catalog_locales)...;  // ru-RU
let locale = route_locale.and_then(normalize_locale_tag)                 // ru-ru
```

Итоговый `$locale` / `$fallback_locales` в runtime-контексте и в
опубликованных артефактах — в нижнем регистре. Это расходится с
`rustok_api::RuntimeLocale`/`TenantLocale`/`StoredLocale` (`ru-RU`), с колонкой
`tenant_locales.locale` и с заголовком `Content-Language`, который
`rustok-pages/src/controllers/mod.rs:195` берёт прямо из `artifact.locale`.
Внутри `fly` рассинхрон маскируется тем, что `translation_for()` перенормализует
обе стороны, но на границе с API/БД сравнение идёт по строке.

### A-06 — Три расходящихся алгоритма locale-fallback

| Владелец | Цепочка |
|---|---|
| `rustok_ui_i18n::locale_candidates` (`locale.rs:142`) | exact → без вариантов → CLDR likely-script → без региона → без скрипта → язык → default → `en` |
| `rustok_api::build_locale_candidates` (`locale.rs:177`) | exact → primary language |
| `fly::push_locale_candidate` (`runtime_locale.rs:241`) | exact → primary language (свой дедуп, лоукейс) |

Для `zh-Hant-TW` UI пройдёт `zh-Hant-TW → zh-Hant → zh`, а доменные переводы
(`flex/attached.rs:282`, `rustok-pages/.../helpers.rs:144`,
`rustok-modules/governance/market_projections.rs:231`) —
`zh-Hant-TW → zh`, пропустив каталог `zh-Hant`. UI и контент одной страницы
могут прийти из разных языков.

`rustok-api/src/locale.rs:376` в леджере отмечен как «канонизирован через
`rustok-ui-i18n`» — канонизирована только *нормализация тега*, но не
*алгоритм кандидатов*.

### A-08 — `next-admin` заново реализует Accept-Language, игнорируя q

`apps/next-admin/src/i18n/request.ts:26-33`:

```ts
function resolveAcceptLanguage(value: string | null): Locale | undefined {
  return value
    ?.split(',')
    .map((item) => item.split(';')[0]?.trim())   // q-значения выброшены
    .filter(Boolean)
    .map((item) => matchSupportedLocale(item))
    .find((locale): locale is Locale => Boolean(locale));
}
```

Для `en;q=0.1, ru;q=0.9` вернёт `en`. Для `en;q=0` вернёт `en` (явный отказ
клиента проигнорирован). Это ровно дефект F-12 прошлого аудита, объявленный
устранённым: «Page Builder и admin удалили локальные parsers. Repository
verifier запрещает их возврат». Верификатор
(`verify-i18n-contract.mjs:81-90`) проверяет только `apps/admin/src/main.rs`
(Leptos), про `apps/next-admin` не знает.

Побочно: `matchSupportedLocale` продублирован в
`apps/next-admin/src/i18n/request.ts:13` и `apps/next-frontend/src/i18n-config.ts:5`
идентичным телом.

---

## Medium

### A-07 — Три предела длины locale tag

| Место | Предел | Что меряется |
|---|---|---|
| `rustok-ui-i18n/src/locale.rs:17` | 64 | сырой вход **до** trim (`pub(crate)`) |
| `rustok-api/src/locale.rs:13` | 32 | **нормализованный выход** |
| `verify-ui-i18n-parity.mjs:29` | 32 | сырое имя файла |

Локаль длиной 33-64 байта пройдёт i18n-слой и будет отвергнута API-слоем.
Плюс `MAX_LOCALE_TAG_LEN` в crate — `pub(crate)`, тогда как
`MAX_ACCEPT_LANGUAGE_LEN` и `MAX_MESSAGE_KEY_LEN` публичны, так что хост не
может проверить границу заранее.

### A-09 — CLDR `parentLocales` не реализованы

`crates/ui/rustok-ui-i18n/tests/world_language_tests.rs:146` фиксирует это как
ожидаемое поведение:

```rust
assert_eq!(prepared.t(Some("es-MX"), "hello", "fallback"), "Hello");  // не "Hola"
```

При этом документация макроса (`macros.rs:117`) прямо предлагает
`locales = ["en", "ar", "de", "es-419", "ja", "zh-Hant"]`. Модуль, поступивший
по инструкции, отдаст английский всем `es-MX`, `es-AR`, `es-CO`, `es-CL`…

Отсутствующие переходы: `es-AR → es-419`, `en-GB → en-001`,
`zh-Hant-MO → zh-Hant-HK`, `pt-AO → pt-PT`. Структурный peeling (region/script)
их не покрывает — нужна таблица CLDR `parentLocales`
(`icu_locale` даёт её через `LocaleFallbacker`, который уже есть в дереве
зависимостей).

### A-10 — Плюрализация не применяется; подтверждённые грамматические дефекты

9157 сообщений против **22** select-выражений. 75 сообщений содержат числовую
переменную без селектора. Выборочно подтверждённые дефекты:

| Ключ | Локаль | Текущее | Результат |
|---|---|---|---|
| `search-results-summary` | ru | `{ $count } результатов…` | «1 результатов» |
| `forum-topic-unreadCount` | ru | `{ $count } непрочитанных` | «1 непрочитанных» |
| `media-asset-bytes` | ru | `{ $count } байт` | «2 байт» |
| `comments-threads-count` | en | `{ $count } comments` | «1 comments» |
| `comments-threads-total` | en | `{ $count } matching threads` | «1 matching threads» |
| `forum-topic-unreadCount` | **ar** | `{ $count } غير مقروء` | неверно для 5 из 6 категорий |
| `forum-thread-repliesTotal` | **ar** | `الإجمالي { $count }` | то же |

Арабский каталог `rustok-forum` — единственный не-en/ru в проде, и в нём нет
ни одного селектора при шести CLDR-категориях. `forum/admin/src/i18n.rs:31`
проверяет только совпадение набора ключей с `en.ftl`.

Часть каталогов обходит проблему конструкцией «Существительное: { $count }»
(`notifications-navigation-unread` ru), что грамматически безопасно, но не
масштабируется на языки без такой конструкции и оставляет английскую сторону
(`{ $count } unread notifications`) сломанной.

Движок это умеет (`world_language_tests.rs:64` проверяет все шесть арабских
категорий) — проблема в контенте и в отсутствии гейта: parity-скрипт сравнивает
только имена ключей, набор переменных и наличие селектора не сравнивает никто
(strict-валидатор, который сравнивает переменные, не вызывается — A-03).

### A-11 — Нет локале-зависимого форматирования чисел и дат

```bash
$ grep -rn "add_function" --include=*.rs .     # пусто
$ grep -rn "NUMBER(\|DATETIME(" --include=*.ftl .   # пусто
```

`fluent-bundle` не регистрирует `NUMBER`/`DATETIME` сам — их обязан добавить
хост через `FluentBundle::add_function`. Сейчас:

- любой FTL с `NUMBER($x)` упадёт в `I18nError::FormattingFailed`;
- `{ $count }` печатается как есть — `1234567`, без разрядных разделителей
  ни в `en` (`1,234,567`), ни в `ru` (`1 234 567`);
- цены/даты/проценты форматируются вручную вне i18n-слоя.

В прошлом аудите это отнесено в «осознанно отложенные», но при заявленном
«world-language surface» и наличии `icu_locale` в зависимостях это дыра
контракта, а не аккуратно очерченная граница. Минимум — задокументировать в
README в разделе «Known Limitations».

### A-12 — Lenient-путь оплачивает полную strict-валидацию на первом lookup

`messages.rs:944-960`:

```rust
} else if report.is_clean()
    && let Err(error) = validate_catalog_schemas(self.bundles, &default_locale)
```

`validate_catalog_schemas` заново парсит **все** FTL через
`fluent_syntax::parser::parse` и строит граф ссылок — поверх уже построенных
`FluentResource`. Для `rustok-search/admin` (232 ключа) и
`rustok-page-builder/admin` (230) это двойной парсинг каталога на первый
`t()` в процессе, на дефолтном пути всех 48 модулей.

Второй дефект той же строки: валидация схем выполняется **только если**
`report.is_clean()`. Если хотя бы одна локаль не собралась, дефекты схем в
остальных локалях не диагностируются вообще.

### A-13 — Аллокации на горячем пути

`UiMessages::format` → `resolve_fluent_message` → `locale_candidates(locale, default)`
на **каждый ключ**: ICU-парсинг локали, `LocaleCanonicalizer::new_extended()`
(`locale.rs:44,64`), `LocaleExpander::new_extended()` (`locale.rs:189`),
`Vec<String>` из 3-6 элементов, плюс `to_string()` на fallback.

Сгенерированные макросом `t`/`format` (`macros.rs:220-232`) всегда идут этим
путём и никогда не используют `for_locale`/`UiLocaleTranslator`, ради которого
эта оптимизация и делалась. Пример стоимости:
`apps/storefront/src/shared/local.rs::locale_strings` — 21 вызов подряд,
`featured_products` — ещё 11, все с одной и той же локалью.

Дешёвое улучшение: мемоизировать цепочку по `(locale, default)` в
thread-local/`OnceLock`-кэше либо сгенерировать в макросе `for_locale`-вариант
и перевести на него batch-места.

### A-14 — Схема термов требует непередаваемые переменные (латентно)

`messages.rs:546-566`: для `TermReference` переменные терма, не переданные
явными именованными аргументами, пробрасываются в контракт вызывающего
сообщения. В `fluent-bundle` терм резолвится с `scope.local_args = Some(args)`,
то есть **видит только свои явные аргументы** — переменную из внешнего
`FluentArgs` он не увидит никогда.

Итог: `validate_catalog_schemas` потребует переменную, передать которую
невозможно, а `extract_locale_schemas` сообщит её как часть внешнего контракта.
Сейчас в репозитории 0 термов, поэтому дефект не проявляется, но он сработает
на первом же `-brand = …{ $tenant }…`.

### A-15 — `normalize_admin_locale`: host-политика внутри библиотеки

`locale.rs:71` жёстко схлопывает весь мир в `"ru" | "en"` и экспортируется из
корня crate. README того же crate требует обратного:

> Do not own the host's query/cookie/header/tenant precedence or supported-locale policy here.

Реальные потребители — три модуля из A-04, которые именно из-за этой функции и
остались на `if russian`.

### A-16 — Типы зависимостей в публичных ошибках без ре-экспорта

```rust
BundleBuildError::InvalidLocale   { source: unic_langid::LanguageIdentifierError }  // error.rs:61
BundleBuildError::AddResource     { errors: Vec<fluent_bundle::FluentError> }       // error.rs:75
I18nError::FormattingFailed       { errors: Vec<fluent_bundle::FluentError> }       // error.rs:249
```

Ни `FluentError`, ни `LanguageIdentifierError` из корня не ре-экспортированы, а
`LanguageIdentifier` ре-экспортирован **и помечен `#[deprecated]`** (`lib.rs:22`).
Чтобы обработать эти варианты, потребитель обязан завести прямую зависимость на
`fluent-bundle`/`unic-langid` и синхронизировать их версии вручную — это и есть
скрытая часть semver-контракта.

### A-17 — `t!` несовместим с половиной собственных фасадов

`macros.rs:51-69`:

- рукав без аргументов зовёт `$messages.t_for_locale(...)` — метода нет у
  `PreparedUiMessages`, `UiTranslator`, `UiLocaleTranslator`, `LazyUiLocaleTranslator`;
- рукава с аргументами зовут `$messages.format(locale, key, args, fallback)` —
  у `UiLocaleTranslator`/`LazyUiLocaleTranslator` `format` трёхарный
  (`key, args, fallback`).

При этом `prelude` (`prelude.rs:275-280`) экспортирует `t!` вместе со всеми
этими типами, а `public-api.md` перечисляет их в одном ряду. Либо добавить
`t_for_locale`/унифицировать `format`, либо явно задокументировать, что `t!`
применим только к `UiMessages`/`LazyUiMessages`.

### A-18 — Мёртвый workflow `next-fluent-verify.yml`

Триггеры и `working-directory` указывают на `packages/next-fluent/**`.
В репозитории `packages/` содержит только `richtext` и `rustok-ui-auth` —
пакет вынесен во внешний репозиторий. Workflow никогда не сработает на push/PR,
а `workflow_dispatch` упадёт на `npm ci` в несуществующей директории.

### A-19 — Незапиненная GitHub-зависимость

`apps/next-admin/package.json:81` и `apps/next-frontend/package.json:22`:

```json
"@rustok/next-fluent": "github:RusTokRs/next-fluent"
```

Без тега/SHA это HEAD дефолтной ветки: невоспроизводимые сборки, отсутствие
integrity-хеша, отсутствие аудита в `npm audit`. При этом next-fluent —
рантайм-владелец всей i18n обоих Next-приложений.

### A-20 — Fail-soft загрузка каталогов в Next

`apps/next-frontend/src/i18n.ts:36` и `apps/next-admin/src/i18n/request.ts:63`
при отсутствии файла возвращают `""` и печатают `console.warn`. Приложение
стартует с пустым каталогом и рендерит ключи/fallback без единого сигнала в
мониторинг.

Плюс `apps/next-admin/src/i18n/request.ts:43-44` вычисляет
`path.resolve(__dirname, ...)` **вне** `try` — в ESM-сборке `__dirname`
не определён, и `ReferenceError` вылетит до цикла с `try/catch`.

---

## Low / гигиена

**A-21. Мёртвый exclusion-list в parity-гейте.**
`verify-ui-i18n-parity.mjs:7-19` исключает `/crates/rustok-commerce`,
`/crates/rustok-order` и т.д., а реальные пути — `/crates/modules/rustok-commerce`.
Ни одно исключение не срабатывает (вывод содержит
`OK crates/modules/rustok-pricing/admin/locales`). Список — остаток от
реорганизации в `modules/`: либо удалить, либо починить.

**A-22. Пробелы `verify-ui-i18n-keys.mjs`.**
- `rawContent.slice(0, rawContent.indexOf("#[cfg(test)]"))` (строка 42) —
  весь продовый код после первого inline-тест-модуля не сканируется;
- регулярка (строка 46) ловит только `t(` / `t!(`; вызовы `format(...)`,
  `format_attribute(...)`, `module_t!` не проверяются (пример:
  `notifications-navigation-unread` резолвится через `format()` в
  `rustok-notifications/storefront/src/i18n.rs` и остаётся вне гейта);
- сканируются только `crates/modules/*/{admin,storefront}` — вне охвата
  `apps/*`, `crates/ui/*`, `rustok-reactions-storefront` (плоская раскладка).

**A-23. FTL парсится построчной регуляркой.**
Оба JS-гейта (`^([a-zA-Z][a-zA-Z0-9_-]*)\s*=`) не видят атрибуты
(`.aria-label`) и термы (`-brand`). В сочетании с A-03 это значит, что parity
атрибутов не проверяется **нигде**, хотя F-01 прошлого аудита их специально
добавлял.

**A-24. Пятая реализация нормализации локали** — `normalizeLocaleTag`
в `verify-ui-i18n-parity.mjs:27`, с собственными правилами регистра и
пределом 32.

**A-25.** `#[macro_use] pub mod macros;` (`lib.rs:17-18`) — `#[macro_use]`
избыточен при `#[macro_export]`; публичный модуль `macros` пуст в rustdoc.

**A-26.** `MAX_LOCALE_TAG_LEN` — `pub(crate)` на фоне публичных
`MAX_ACCEPT_LANGUAGE_LEN`/`MAX_MESSAGE_KEY_LEN` (см. A-07).

**A-27.** Прелюдия неполна относительно `public-api.md`: нет
`MAX_ACCEPT_LANGUAGE_LEN`, нет Accept-Language API (объявленного «каноническим
общим механизмом»), нет `AcceptLanguagePreference`, `FluentValue`.

**A-28.** `normalize_default_locale` (`messages.rs:1104`) дублирует
`parse_language_identifier` (`bundle.rs:66`) — та же логика, другой тип ошибки.
При этом обе, в отличие от публичной `normalize_locale_tag`, не принимают
полные Unicode-локали, так что фраза «accepted consistently with
`normalize_locale_tag`» в доке `build_fluent_bundle` неточна:
`normalize_locale_tag("en-US-u-nu-latn")` → `Some("en-US")`, а
`build_fluent_bundle("en-US-u-nu-latn", …)` → `Err(InvalidLocale)`.

**A-29.** Unit-тест `workspace_module_ftl_files_parse_cleanly` (`lib.rs:255`):
- ходит в файловую систему (`../../modules`) из crate, который декларирует
  «zero runtime filesystem access» и публикуемость/WASM-пригодность;
- `if let Ok(entries) = fs::read_dir(dir)` молча пропускает нечитаемые ветки;
- выбирает локаль по префиксу имени файла (`starts_with("ru")`, иначе `"en"`),
  поэтому `ar.ftl` собирается как английский bundle и арабские plural-правила
  не проверяются.

**A-30.** Мусорные артефакты: `crates/ui/rustok-ui-i18n/deep-research-report (2).md`
и корневой `deep-research-report (5).md` — имена с пробелами и скобками
(ломают shell-пайплайны и `.gitattributes`-правила). Первый — 10-строчный
маркер «superseded», его место в CHANGELOG, а не в дереве crate.

**A-31.** `PageBuilderLocaleContext::from_request` (`locale.rs:54`):
`.skip_while(|candidate| locale.as_deref() == Some(candidate.as_str()))`
останавливается на первом несовпадении, поэтому дубликат убирается не им, а
повторной нормализацией в `new()`. Мёртвая логика, вводящая в заблуждение.

**A-32. Легаси `rustok-core::i18n` не мигрировано.**
ADR [`2026-04-03-system-i18n-fluent-migration`](../../../DECISIONS/2026-04-03-system-i18n-fluent-migration.md)
объявил первый Fluent-срез для auth/validation/system-ошибок. Спустя ~6 месяцев
`crates/libs/rustok-core/src/i18n.rs` — это 6 match-таблиц × 27 ключей
(проверено: parity между таблицами полная, дубликатов нет) с:
- закрытым enum `Locale { En, Ru, Es, De, Fr, Zh }` без `#[non_exhaustive]`;
- собственным `Locale::parse` (`to_lowercase().split('-')`) — шестым
  нормализатором локали;
- смешанной конвенцией ключей (`invalid_kind` vs `auth.email_already_exists`),
  расходящейся с kebab-конвенцией UI-каталогов;
- без интерполяции, без плюрализации, с `String`-аллокацией на каждый вызов;
- без CI-гейта на parity таблиц (сейчас сходятся, но ничем не удерживаются).

В леджере это LOCALE-LEGACY-01, при этом `FS-22.04.10` помечен `[x]` —
формально закрыт, фактически legacy-модель системных сообщений жива и
расходится с «world-language» контрактом UI-слоя.

---

## Что подтвердилось как корректное

Перепроверено независимо и **не** является дефектом:

1. Каталоги структурно консистентны: 112 файлов, 0 расхождений по сообщениям,
   атрибутам, наборам переменных, 0 дубликатов, 0 неразрешённых ссылок.
2. `verify-ui-i18n-keys.mjs --strict` — 987 вхождений, 50 пакетов, 0 промахов.
3. Сгенерированные `src/types/i18n.d.ts` синхронны с `messages/en.ftl`
   (660/660 для next-admin, 49/49 для next-frontend).
4. `rustok-api::request::extract_requested_locale` (`request.rs:141-146`)
   реализует ADR-цепочку `query → x-medusa-locale → cookie → Accept-Language`
   точно и делегирует парсинг в `rustok-ui-i18n`.
5. `set_use_isolating(true)`, `OnceLock`/`Once`-инициализация, bounded-вход,
   отсутствие `unwrap` на продовом парсинге — как и в аудите 2026-09-27.
6. `with_kebab_key` безопасен: стековый буфер трогает только байт `0x2E`,
   который в UTF-8 не встречается в продолжающих байтах; индексация
   ограничена `key.len() <= STACK_KEY_BUF_SIZE`.
7. Арифметика q-значений в `parse_quality` точная и без float-краёв;
   сортировка стабильная с явным tie-break по исходному порядку.
8. Цепочка `zh-TW-u-ca-chinese → zh-TW → zh-Hant-TW → zh-Hant → zh`
   действительно работает и соответствует README.

---

## Предлагаемый порядок устранения

1. **A-01, A-02, A-18** — вернуть CI в рабочее состояние (иначе любое
   исправление ниже не удерживается).
2. **A-03** — генерировать `#[test] validate()` прямо в `declare_module_i18n!`;
   это автоматически закрывает A-04 (осиротевшие каталоги станут видны) и даёт
   гейт для A-10 в части переменных.
3. **A-05, A-06, A-07, A-32** — один owner-level заход: `rustok-ui-i18n`
   становится единственным нормализатором и единственным построителем цепочки
   кандидатов; `fly`, `rustok-api::build_locale_candidates`,
   `rustok-core::i18n::Locale::parse` делегируют ему; предел длины — один.
4. **A-08, A-19, A-20** — Next-хосты: убрать локальный парсер, запинить
   зависимость, сделать отсутствие каталога fail-closed.
5. **A-09, A-10, A-11** — контент и CLDR: `LocaleFallbacker` для parentLocales,
   селекторы в 75 сообщениях, регистрация `NUMBER`/`DATETIME` либо явная
   фиксация ограничения в README.
6. **A-12…A-17** — качество API и производительности crate.
7. **A-21…A-31** — гигиена гейтов, тестов и документации.
