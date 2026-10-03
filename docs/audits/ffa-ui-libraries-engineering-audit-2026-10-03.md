# Инженерный аудит FFA- и UI-библиотек

- **Дата:** 2026-10-03
- **Объём:** `crates/ui/*` (rustok-ui-core, rustok-ui-i18n, rustok-ui-transport, rustok-ui-auth,
  rustok-ui + адаптеры leptos/dioxus, rustok-forms, rustok-grid, rustok-graphql, leptos-auth,
  leptos-ui, leptos-ui-routing, fly — выборочно), `crates/libs/rustok-fba`, FFA-реестр хоста
  `apps/next-admin`, Fluent-каталоги хоста, verify-гейты FFA/i18n.
- **Метод:** статическая вычитка исходников + прогон всех node-verify-гейтов как
  исполняемого слоя проверки. **Rust-тулчейн в песочнице недоступен** (сетевые блокировки
  rustup/crates.io), поэтому `cargo check/test/clippy` не выполнялись — все правки Rust-кода
  сделаны минимально-инвазивными и вычитаны вручную; их нужно прогнать в CI.
- **Контекст:** fly уже проходил отдельный аудит 2026-10-02
  (`docs/audits/fly-builder-engineering-audit-2026-10-02.md`); здесь fly проверялся только
  интеграционно (7 собственных гейтов + контракт locale-резолвера).

## 1. Резюме

Ядро FFA в хорошем состоянии. Контрактные крейты (`rustok-ui-core`, `rustok-ui-i18n`,
`rustok-ui-transport`, `rustok-ui-auth`, `rustok-fba`) вычитаны целиком: границы слоёв
соблюдены, зависимостей на фреймворки в ядре нет, snake_case-контракт query-ключей
выдержан, error-safety-паттерн (статические публичные конверты + внутренняя диагностика)
применяется последовательно. Маркеров `TODO`/`FIXME`/`unimplemented!` в объёме аудита нет.

Главная системная проблема — **не код, а протухшие verify-гейты**: три гейта из
FFA/i18n-набора проверяли состояние репозитория, которого больше нет (или никогда не было),
и падали на корректном коде. Это ровно тот класс дефектов, который fly-аудит зафиксировал
как H-7 («verify-скрипты проверяют текст, а не поведение»): текстовые ожидания дрейфуют
от кода и начинают наказывать правильные архитектурные решения. Четвёртая находка —
реальная недоделка: при добавлении страниц каталога/авторизации в `next-admin` в оба
Fluent-каталога был вслепую дописан блок из 18 уже существующих ключей, что валило
`verify:i18n:ui`.

В Rust-коде найдены два реальных дефекта уровня M (паникабельные пути в `rustok-ui-core`)
и набор нитов уровня L, зафиксированных без правок (раздел 6).

**Все найденные дефекты исправлены в этом же изменении.** После правок зелёные: агрегат
`verify:ffa:ui:migration` (41 подгейт), `verify:i18n:ui`, `verify:i18n:keys`,
`verify:i18n:contract`, `verify-ui-i18n-parity`, `verify:frontend:host-ffa-contract`,
все 7 fly-гейтов.

## 2. Статус-борд

| ID | Серьёзность | Где | Суть | Статус |
|----|-------------|-----|------|--------|
| G-1 | High | `scripts/verify/verify-frontend-host-ffa-contract.mjs` | Гейт требовал относительные импорты `../../packages/*/src`, хотя реестр хоста давно переведён на npm-алиасы `@rustok/*-admin` | Исправлено |
| G-2 | High | `apps/next-admin/messages/{en,ru}.ftl` | 18 дублированных ключей (`auth-*`, `register-*`), дописанных в конец обоих каталогов; `verify:i18n:ui` и parity-гейт падали | Исправлено |
| G-3 | High | `scripts/verify/verify-i18n-contract.mjs` | Гейт требовал прямые вызовы `rustok_ui_i18n::*` в `fly/src/runtime_locale.rs`, не зная об инверсии через `LocaleResolver` (сделанной по итогам fly-аудита H-1) | Исправлено |
| G-4 | High | `scripts/verify/verify-product-storefront-catalog-native-error-safety.mjs` | Гейт требовал SSR-only `dep:tracing` по шаблону region/marketplace-listing, хотя product storefront логирует GraphQL-сбои на всех профилях (pricing-конвенция) | Исправлено |
| M-1 | Medium | `rustok-ui-core/src/pagination.rs` | Паника на underflow/делении на ноль при `page == 0` / `per_page == 0` (поля публичные и `Deserialize`) | Исправлено |
| M-2 | Medium | `rustok-ui-core/src/navigation.rs` | Паника `&seg[1..]` на не-ASCII сегменте пути в хлебных крошках (`/модули/блог`) | Исправлено |
| D-1 | Low | `docs/UI/README.md`, `docs/modules/crates-registry.md` | Документация заявляла поддержку JSON-каталогов в `rustok-ui-i18n` — крейт Fluent-only | Исправлено |
| D-2 | Low | `docs/UI/module-package-implementation.md` | Ссылка на несуществующий тип `LeptosUiMessages` (реальные: `UiMessages`, `LazyUiMessages`) | Исправлено |
| D-3 | Low | `rustok-ui-core/src/css.rs` | Единственный файл крейта без лицензионного заголовка | Исправлено |
| N-* | Nit | разное | Зафиксированы без правок, см. раздел 6 | Отчёт |

## 3. Протухшие гейты (G-1…G-4)

### G-1. Хостовый FFA-гейт против npm-алиасов

`apps/next-admin/src/modules/index.ts` регистрирует админ-пакеты модулей через
`file:`-слинкованные пакеты (`@rustok/blog-admin` → `packages/blog` и т.д.) — это
канонический контракт: хост импортирует **точку входа пакета**, а не его внутренности.
`package.json` хоста подтверждает все алиасы. Гейт же требовал семь относительных путей
вида `../../packages/blog/src`, то есть ровно тот антипаттерн, от которого ушли.

Исправление: список ожиданий переведён на полный набор из 11 алиасов
(`blog`, `cache`, `commerce`, `email`, `events`, `forum`, `iggy-connector`, `rbac`,
`product`, `translation`, `workflow`), плюс добавлен запрет `../../packages/` в реестре —
чтобы регрессия к относительным импортам ловилась, а не поощрялась. Проверки для
`next-frontend` (там относительные пути пока легитимны) не тронуты.

### G-2. Дубликаты ключей во Fluent-каталогах хоста

При добавлении страниц каталога товаров и авторизации в `next-admin` в конец `en.ftl`
и `ru.ftl` (строки 662–680) был дописан блок из 8 ключей `auth-*` и 10 ключей
`register-*`, полностью дублирующих определения со строк 99–169. Fluent при дубликате
берёт последнее определение, поэтому UI «работал», но `verify:i18n:ui` и
`verify-ui-i18n-parity` падали на обоих языках. Соседний блок `modules-*` из того же
коммита дубликатом не был — оставлен.

Исправление: дописанные блоки удалены из обоих файлов; канонические определения
на строках 99–169 покрывают все ключи новых страниц (имена совпадают).

### G-3. i18n-контракт против инверсии зависимости в fly

По итогам fly-аудита (H-1, извлекаемость) fly спрятал зависимость от платформенного
i18n за трейт `LocaleResolver`: `runtime_locale.rs` делегирует в
`default_locale_resolver().normalize_tag()/.fallback_chain()`, а прямые вызовы
`rustok_ui_i18n::normalize_locale_tag()/locale_fallback_chain()` живут в
`locale_resolver.rs` (`PlatformLocaleResolver` за фичей `platform-i18n`, включённой по
умолчанию; `BasicLocaleResolver` — структурный BCP-47-фоллбек для standalone-сборки).
Это правильная архитектура, но `verify-i18n-contract.mjs` продолжал искать прямые вызовы
в `runtime_locale.rs` и падал.

Исправление: ожидания делегирования в `rustok_ui_i18n` перенаправлены на
`locale_resolver.rs`, добавлены проверки, что `runtime_locale.rs` ходит только через
`default_locale_resolver()`; запрет `to_ascii_lowercase()` сохранён. Теперь гейт
фиксирует и инверсию, и конечное делегирование.

### G-4. Product storefront: SSR-only tracing из чужого шаблона

Гейт требовал `tracing = { workspace = true, optional = true }` + `"dep:tracing"` в фиче
`ssr` — шаблон, корректный для region/marketplace-listing, где tracing нужен только на
сервере. Но product storefront, как и pricing storefront, логирует сбои GraphQL-транспорта
в `graphql_error_safety.rs` **на всех профилях сборки** (включая CSR-путь), поэтому его
`Cargo.toml` правильно держит безусловный `tracing.workspace = true`. Pricing-гейты уже
прошли эту эволюцию и прямо запрещают SSR-only-форму как «stale». Падал весь агрегат
`verify:ffa:ui:migration`, причём из-за `&&`-цепочки в npm-скрипте провал маскировался
более ранними падениями G-1.

Исправление: гейт приведён к pricing-конвенции (требуется all-profile tracing,
SSR-only-форма запрещена). Манифест и код не менялись — они были правильными.

## 4. Дефекты Rust-кода (M-1, M-2)

### M-1. `UiPaginationState`: паника на внеконтрактных значениях

Поля `page`/`per_page` публичны и десериализуются из внешних данных (query, конфиг),
минуя клампинг в `new()`. При `page == 0` `offset()` делал `page - 1` (underflow —
паника в debug), при `per_page == 0` `total_pages()` делил на ноль; `total_pages` также
молча обрезал `u64 → u32` через `as`. Исправлено: `saturating_sub(1)`,
`per_page.max(1)`, `u32::try_from(...).unwrap_or(u32::MAX)`, `saturating_add` в
`next_page()`. Добавлен тест `pagination_tolerates_out_of_contract_field_values`
(включая путь через `serde_json::from_str`).

### M-2. Хлебные крошки: байтовый срез по UTF-8

Фоллбек-капитализация сегмента пути делала `first.to_uppercase() + &seg[1..]` —
срез по байту 1 паникует, если первый символ многобайтный. Для платформы, где русская
локаль первична, `/модули/блог` — реалистичный маршрут. Исправлено итератором
`chars()` без срезов; добавлен тест
`test_build_ui_breadcrumbs_handles_non_ascii_segments` на кириллице.

## 5. Что вычитано и признано чистым

- **rustok-ui-core** (целиком, кроме исправленного): contracts/route-query/busy —
  последовательные, без паник; `css.rs` — осознанно узкая грамматика цветов с
  задокументированной мотивацией (XSS).
- **rustok-ui-i18n** (глубоко: locale, messages, bundle, accept_language, macros, lazy) —
  дефектов нет; `LazyUiMessages` — аккуратный fail-soft-дизайн с документированными
  компромиссами. Крейт Fluent-only — это норма, документация приведена к коду (D-1).
- **rustok-ui-transport, rustok-ui-auth, rustok-fba** — целиком чистые; `rustok-fba`
  маленький и строго контрактный (схема provider-registry-v1 согласована с типами).
- **leptos-auth** — двойной транспорт через `UiTransportPath` (CSR→GraphQL,
  hydrate/SSR→`#[server]`), `ServerAuthSnapshot` для SSR, авто-signout/refresh на wasm —
  архитектура согласована с FFA-контрактом. Ниты — в разделе 6.
- **rustok-forms, rustok-grid** (validation/state/sanitize/dirty, pagination/filter +
  оба адаптера) — чистые.
- **rustok-ui + leptos/dioxus-адаптеры** — a11y в dialog/tabs корректна
  (фокус-трап, aria-атрибуты, roving tabindex).
- **leptos-ui, leptos-ui-routing, rustok-graphql** — чистые, ниты в разделе 6.
- **fly** — интеграционно: все 7 гейтов зелёные, `verify-fly-gates-are-wired` 18/18.

## 6. Ниты (зафиксированы, правки не вносились)

1. `rustok-graphql::graphql_endpoint_from_base` отбрасывает путь базового URL
   (`https://host/sub` → `https://host/api/graphql`); `is_mutation()` — наивный
   `starts_with` без учёта комментариев/пробелов. Терпимо для текущих вызовов.
2. `leptos-auth`: токены в `LocalStorage` на wasm (XSS-поверхность; митигируется
   коротким TTL и авто-refresh, но стоит рассмотреть httpOnly-cookie-путь);
   `get_api_url` синтаксически завязан на суффикс `/api/graphql`.
3. `rustok-grid`: leptos/dioxus-адаптеры пагинации хардкодят английские подписи
   («Previous»/«Next») вместо ключей `rustok-ui-i18n`.
4. `rustok-ui-core::format_ui_price` предполагает 2 десятичных знака минорных единиц —
   неверно для JPY (0) и KWD (3); нужен справочник экспонент валют, если такие валюты
   станут поддерживаемыми.
5. `rustok-ui-core::ui_href_is_active`: пустой `href` совпадает с любым путём.
6. `verify:ffa:ui:migration` — цепочка из 41 `&&`: первый провал маскирует остальные
   (так G-4 прятался за G-1). Стоит перевести агрегат на скрипт, собирающий все провалы.

## 7. Верификация

Прогнано в песочнице (node v22.22.3), всё с кодом выхода 0 **после** правок:

```
npm run verify:ffa:ui:migration          # агрегат, 41 подгейт
npm run verify:i18n:ui                   # 60 OK
npm run verify:i18n:keys                 # 3241 вхождение / 55 пакетов
npm run verify:i18n:contract
npm run verify:frontend:host-ffa-contract
node scripts/verify/verify-ui-i18n-parity.mjs
node scripts/verify/verify-fly-*.mjs     # все 7
```

Не проверено сборкой (нет тулчейна в песочнице) — требует CI:
`cargo check/test -p rustok-ui-core` (правки M-1/M-2 + новые тесты), `cargo fmt --check`.
Правки ограничены выражениями со стандартными API (`saturating_*`, `div_ceil`,
`u32::try_from`, `chars().chain()`), вычитаны построчно.

---

## 8. Фаза 2: триаж 44 красных CI-гейтов репозитория (продолжение «копаем»)

После аудита FFA/UI-библиотек проверены **все 151** npm-гейта `verify:*`;
на старте фазы падали **44** (main фактически красный). Каждый гейт разобран
по существу: «протух гейт» против «реальный дефект кода». Итог:
**151/151 зелёные** (sweep `xargs -P8` по полному списку из package.json).

### 8.1. Классы дрейфа гейтов (расширение G-1…G-4)

- **G-2 (файл → директория модуля):** `rustok-modules/src/{data,governance}.rs`,
  `rustok-pages/src/services/page.rs`, product `ports.rs → ports/` и др.
  Лекарство — dir-aware `read()`, конкатенирующий `*.rs` рекурсивно.
- **G-3 (реестры/evidence отстали от кода):** comments-реестр v5 потерял
  `idempotency_principal_bound` у 4 write-операций (код связывает принципала —
  `bind_idempotency_actor` — ассерты восстановлены); order-реестр и evidence
  декларировали фантомную операцию `adopt_legacy` (в trait её нет — удалена);
  смешанные порты (payment/order/pricing/inventory/cart) получили явные
  `read_operations`/`write_operations`, а гейт — модель «неидемпотентный write
  с `PortCallPolicy::write()`» (fulfillment.select_shipping_option).
- **G-4 (централизация логики):** alloy authoring-сервис, региональная política
  `require_region_read_policy`, comments-trait в контракт-крейте
  `rustok-comments-api`, forum channel-gating в `graphql/mod.rs`,
  OCI-публикация в `rustok-build-publication`. Гейты перенацелены на
  транспортную делегацию + центральные инварианты.
- **G-5 (новый класс — сломанные regex самих гейтов):** незакрытые сканы
  `[\s\S]*?` через границы scope давали ложные срабатывания. Так «дыра» DLQ
  (`DlqQuery.tenant_id`) оказалась багом regex — структура чистая; проверки
  ограничены телом struct/fn.

### 8.2. Реальные дефекты кода, найденные и исправленные

- **rustok-tax (утечка данных провайдера в публичные ошибки).**
  `tax_result_error(detail: impl Display)` прокидывал `format!`-строки с
  сырыми значениями внешнего провайдера (`provider_id`, `currency_code`,
  идентификаторы, суммы) в `PortError::invariant_violation` — публичную
  деталь ошибки. Сигнатура ужесточена до `&'static str`, 7 call-site'ов
  переведены на статические bounded-сообщения (лог и так писал только
  `detail_present`/`detail_length`). Требует `cargo check -p rustok-tax` в CI.

### 8.3. Ложные тревоги, закрытые разбором

- **DLQ tenant-изоляция** — ложное срабатывание G-5 (regex), код чист.
- **Region policy guard** — присутствует и усилен (обёртка с admission-логом).
- **Blog storefront comment create** — ревалидация видимости после внешнего
  create не пропала, а *усилена*: идёт через
  `ensure_public_comment_creation_allowed` (режим комментариев + видимость),
  с компенсирующим идемпотентным delete при гонке.
- **Marketplace listing provenance** — `actor_id`/`locale` на месте (рефактор
  в params-структуру + нормализация locale перед записью).

### 8.4. Реестры приведены к честности

- `translation-surfaces.json#product_option_copy`: `provider_status` переведён
  `registered → not_registered` — провайдер option-переводов **отсутствует в
  кодовой базе** (остались только журнальная миграция `m20260908_000024`,
  guard в dispatcher и перманентно красный CI-workflow). Блокер переписан:
  нужна реимплементация провайдера (вне scope без компилятора).
- Order: фантомная операция `adopt_legacy` удалена из реестра и evidence.

### 8.5. Оставшийся долг (не блокирует зелёный статус)

1. `verify-ecommerce-public-port-error-safety-v2.test.mjs` (фикстурный
   самотест верификатора) падал и **до** правок: фикстура не содержит
   половину файлов, которые читает верификатор (ENOENT). Нужна регенерация
   фикстуры.
2. `catalog_command_port.rs` (product) логирует сырой `tenant_id = %context.tenant_id`
   и `internal_code = %error.code`, тогда как read-порт дисциплинированно пишет
   только bounded-факты. Несоответствие конвенции — стоит выровнять.
3. `verify-fly-grapesjs-roundtrip` требует `@playwright/test` — окружение CI.
4. Все Rust-правки фазы 2 (rustok-tax) вычитаны построчно, но не собраны
   компилятором (тулчейна нет) — обязательный `cargo check/test` в CI.

### 8.6. Верификация фазы 2

```
grep -o '"verify:[^"]*"' package.json | tr -d '"' | sort -u \
  | xargs -P 8 -I{} bash -c 'npm run -s "{}" >/dev/null 2>&1 || echo "{} FAIL"'
# → пусто: 151/151 OK (node v22.22.3)
```
