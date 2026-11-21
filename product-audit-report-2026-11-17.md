# Аудит логики модуля управления продуктами (rustok-product) — ошибки, косяки и рекомендации

**Дата:** 2026-11-17
**Ветка:** `arena/fd611cdb-rustok` (base `58ccfb8`)
**Объём:** `crates/modules/rustok-product` (~106,5k строк Rust: `src` 67,9k, `admin` 28,6k, `storefront` 5,6k, `ports` 4,3k, тесты 6,3k), интеграционные поверхности `crates/modules/rustok-commerce` (GraphQL-резолверы каталога, REST-контроллеры), `apps/server/src/services/product_*`, Next.js-пакеты `apps/next-admin/packages/rustok-product` и `apps/next-frontend/packages/rustok-product`, ADR 2026-03-25 / 2026-07-01 / 2026-07-11 / 2026-10-17 / 2026-11-04 / 2026-11-05 / 2026-11-11.
**Метод:** статический проход по исполняемым путям (транзакции/рицы, жизненный цикл, варианты и оси, EAV-значения, чтение сторфронта, фасеты, Media-retention, транспорт native↔GraphQL, UI-логика), проверка 6 Universal Logic Dimensions (AGENTS.md §2.1). Первоначально — только ревью; по итогам аудита (раунды 4–12) применены точечные фиксы к 19 .rs-файлам (см. §4–12) при том же ограничении: `cargo check/test/clippy`, E2E и компиляцию `apps/server` не запускал (в песочнице нет Rust-тулчейна). Сводка кампании правок и финальное состояние — §16.3. Все ссылки на файлы/строки проверены по текущему HEAD.

> Отчёт — не фикс. Приоритеты: **P1** — функциональный баг / потеря данных / конкурентность, **P2** — качество, целостность контрактов, UX-опасное поведение, **P3** — заметки и гигиена.

---

## 1. Executive Summary

Product — самый «тяжёлый» из аудированных мной модулей репозитория, и его транзакционный каркас сделан на редкость серьёзно: owner-рицы с admission/replay/finalization, отложенный rollback через `Drop`-перехват (`write_transaction.rs:39-45, 567-579`), транзакционный outbox, tenant-проверки в каждом native server-fn, параметризованный EAV-фильтринг, keyset-сканы, at-least-once release-очередь Media со защитой от stale-записей. Инфраструктурных P0 (инъекции, обход тенанта, потеря записей при коммите) не найдено.

Тем не менее найден **1 дефект уровня P1** (с двумя режимами отказа) и ряд P2/P3. Главная тема:

* **Публичный «заголовок варианта» — это машинная строка `combination_identity` из пар UUID**, а не человекочитаемые значения опций. Она пишется в `variant_translations.title` при создании, отдаётся в `VariantResponse.title`, рендерится в гриде Leptos-админки и запрашивается Next-сторфронтом. Человекочитаемый генератор заголовков существует, но **мёртв** (0 вызовов). Хуже того: `update_variant` при любой правке осевых значений **перезаписывает заголовки всех локалей** этой строкой, уничтожая переведённые оператором названия.
* **Media-retain выполняется до транзакции и не компенсируется** при её откате — «осиротевшие» hold'ы Media только диагностируются (`orphan_media_hold`), но не высвобождаются.
* **Поиск по каталогу регистрозависим на PostgreSQL** (`LIKE` без `ILIKE`/`lower`) и не экранирует wildcard'ы `%`/`_` — и в сторфронте, и в админке, и в фасетах.
* **Фильтр по категории не включает дочерние категории** (только точное совпадение `primary_category_id`) — и в списках, и в счётчиках фасет.
* **Невалидный ввод количества в админке молча превращается в 0** и уходит в сохранение (обнулённый сток / скачок позиции изображения).

Ниже — находки с путями и строками, затем рекомендации.

---

## 2. Находки

### P1. Заголовок варианта — это `combination_identity` (пары UUID), а не название; правка осей затирает переводы

**Корень.** `generate_variant_title` возвращает сырую `combination_identity`:

- `src/services/catalog/helpers.rs:124-129` — `match &variant.combination_identity { Some(identity) => identity.clone(), _ => "Default" }`;
- `src/services/catalog/commands.rs:2569-2582` — identity формируется как `format!("{}:{}", attribute_id, option_id)`, отсортированные пары склеиваются через `;` — то есть строка вида `019a…f3:019a…b7;019a…c2:019a…d9`.

**Куда утекает:**

1. **Сидируется в `variant_translations.title` при создании**: `commands.rs:407-423` (`create_product`), `commands.rs:1455-1470` (`create_variant`), `commands.rs:833-845` (бэкфилл новой локали в `update_product`).
2. **Перезаписывает все локали при правке осей**: `commands.rs:1653-1665` — `update_variant` при `Some(axis_values)` делает `update_many` по **всем** `variant_translation` этого варианта, выставляя `title` = identity.
3. **Публичный API**: `VariantResponse.title` = identity — `src/services/catalog/projection.rs:800-812` и `:1031-1043`.
4. **Грид админки** рендерит этот `title`: `admin/src/core.rs:2061-2068` (`build_variant_row_view_models`), рядом отдельной колонкой считается человекочитаемый `options_summary` (`admin/src/core.rs:2038-2055`, `admin/src/core/combinations.rs:134-160`).
5. **Next-сторфронт** запрашивает и рендерит это поле: `apps/next-frontend/packages/rustok-product/src/api/products.ts` (в `STOREFRONT_PRODUCT_QUERY`: `variants { id title … }`).

**Потеря данных.** Модуль Translation даёт оператору авторить человекочитаемые заголовки вариантов по локалям: `src/services/catalog/variant_translation.rs:242-310` (тест на `:666` прямо показывает «Petit / Marine»). Любой последующий `update_variant` с `axis_values` (например, смена SKU-опции или позиции в матрице) **стирает эти переводы**, заменяя их UUID-строкой. Это и нарушение границы владения (Translation-ресурс перезаписывается каталогом), и невосполнимая потеря локализованных данных.

**Мёртвый код.** Человекочитаемый генератор `generate_variant_title_from_axis_values` (`helpers.rs:131-137`, формат «Значение / Значение») существует, но **не вызывается ни разу** во всём воркспейсе — его просто не подключили.

**Режимы отказа:** (а) во всех UI пользователь видит `019a…:019a…;…` вместо «Красный / M»; (б) переведённые названия вариантов уничтожаются любой правкой осей; (в) в SEO/teaser-проекциях и письмах фигурируют UUID.

**Рекомендация:**
- `generate_variant_title` должен собирать заголовок из лейблов опций разрешённых осей (по образцу `retained_labels` в `admin/src/core/combinations.rs:134-160`), с fallback на код опции, и только в крайнем случае — на identity;
- подключить (или удалить) `generate_variant_title_from_axis_values`;
- в `update_variant` не перезаписывать существующие непустые `variant_translation.title`; генерить заголовок только для локалей, где его нет;
- добавить миграцию/бэкфилл, восстанавливающий заголовки из лейблов опций там, где `title == combination_identity`.

---

### P2-1. Media-retain до транзакции не компенсируется при откате Product-транзакции

`add_product_image` сначала вызывает `retain_product_image` (внешний Media-сервис), и только потом открывает owner-транзакцию:

- `commands.rs:1820-1834` — retain выполняется **до** `ProductWriteTransaction::begin` (precheck продукта на `self.db` — `:1823-1827` — тоже вне транзакции);
- после retain есть множество точек отказа: валидация позиции `:1858-1862` (`insertion_index > existing_images.len()` — это Validation-ошибка **после** retain), вставки, `mark_image_media_provisioned`/`mark_media_retention_ready`, receipt-комплетion (который по конструкции откатывает транзакцию — `write_transaction.rs:244-292`).

Release-работы ставятся в очередь **только для закоммиченных удалений** (`commands.rs:2088`, `:1299`). Реконсиляция же сознательно диагностическая: `src/media_reference.rs:598` — «in-flight retain can legitimately look orphaned. **NEVER release from here**», `:688` — код `orphan_media_hold`.

**Режим отказа:** транзакция после успешного retain откатилась (например, оператор прислал position больше количества картинок, или конкурентное удаление продукта, или сбой на коммите). Hold Media на `image_id` остаётся навсегда: повтор с тем же idempotency-ключом реплеит терминальную ошибку (`catalog_command_port.rs:443-459`), повтор с новым ключом создаёт **новый** `image_id` и новый retain — старый осиротеет окончательно. Накопление «висящих» hold'ов блокирует удаление ассетов в Media и требует ручной чистки.

**Рекомендация:** после отката транзакции, которой предшествовал успешный retain, ставить компенсирующую release-запись (в ту же очередь `product_media_release`, с маркером «compensation») — семантика at-least-once уже это допускает; либо вести persistent-таблицу pending-retain'ов и подчищать её тем же воркером. Диагностику `orphan_media_hold` повысить до аларма.

---

### P2-2. Поиск по заголовку регистрозависим на PostgreSQL; `%`/`_` не экранируются

- `src/services/catalog/queries.rs:516-544` — `product_title_search_condition`: `pt.title LIKE $1` с паттерном `format!("%{search}%")`;
- тот же код в админке: `src/services/catalog/admin_queries.rs:199-227`;
- фасеты переиспользуют его: `src/services/catalog/facets.rs:684-690`.

**Режимы отказа:** на PostgreSQL `LIKE` регистрозависим (колонка `title` — обычный TEXT/VARCHAR, без citext/ICU-коллации в миграциях) — запрос «iphone» не найдёт «iPhone». Символы `%` и `_` в пользовательском вводе не экранируются: поиск «100%» превращается в паттерн `%100%%` и матчит почти всё; «_» матчит любой символ.

**Рекомендация:** для Postgres — `ILIKE` либо `lower(pt.title) LIKE lower($1)` (+ функциональный индекс по `lower(title)`), экранировать `%`/`_`/`\`; на перспективу — pg_trgm/FTS. Тест на кейс-чувствительность и wildcards добавить в контракт-тесты списка.

---

### P2-3. Фильтр по категории не включает дочерние категории

- сторфронт-лист: `src/services/catalog/queries.rs:97-99` — `PrimaryCategoryId.eq(category_id)`;
- админ-лист: `src/services/catalog/admin_queries.rs:73-75`;
- счётчики фасет: `src/services/catalog/facets.rs:675-683`.

При этом таксономия иерархична (`path`, дерево строится в `admin/src/core.rs:2500-2533`), и селектор категорий в админке предлагает узлы с путём. Выбор родительской категории не покажет товары из дочерних — ни в списке, ни в фасетах; поведение одинаково «неправильное» во всех трёх местах (по крайней мере консистентно).

**Рекомендация:** подтвердить продуктовое намерение. Если требуется агрегация по поддереву — фильтровать по `path`-префиксу (или по таблице замыканий), и синхронно поменять все три места + фасеты. Если текущая семантика задумана — зафиксировать в README/CRATE_API, потому что UX-ожидание обратное.

---

### P2-4. Невалидный ввод количества в админке молча становится `0`

- `admin/src/core.rs:1999-2001` — `value.trim().parse().unwrap_or(0)`;
- поведение закреплено тестом `:3750-3757` («not-a-number» → 0);
- подключено к инпуту стока: `admin/src/ui/leptos.rs:1172` и к позиции изображения: `:2085`.

**Режим отказа:** оператор набирает «1O0» (буква O вместо нуля) или вставляет пробел внутри числа → поле мгновенно становится 0 → `validate_editor_variant_draft` (`admin/src/core.rs:102-104`) пропускает (0 ≥ 0) → сохранение записывает `inventory_quantity = 0` через `update_initial_quantity_in_tx` (`commands.rs:1692-1696`) — **обнуление стока без единого предупреждения**. Для позиции изображения невалидный ввод молча двигает картинку в начало.

Это анти-паттерн silent-default (родственник `P-JSON-DECODE-DEFAULT` из `docs/standards/logic-hygiene-patterns.md`) в месте, где default меняет данные.

**Рекомендация:** парсить в `Option<i32>` и (а) игнорировать ввод, оставляя последнее валидное значение, плюс (б) блокировать сохранение с ошибкой валидации, пока поле невалидно (fail-closed). Тест на `:3750` переписать соответственно.

---

### P2-5. Публичное чтение карточки пересчитывает и принудительно проверяет инварианты осей на каждый запрос

`get_published_product_by_id_with_locale_fallback` (`src/services/catalog/queries.rs:412-451`) на **каждом** публичном чтении:
- пересобирает оси/туплы из проекции и вызывает `validate_variant_axis_tuples`;
- пересчитывает `combination_identity` для каждого варианта и при расхождении возвращает Validation-ошибку «public SKU identity projection is inconsistent; repair the catalog before publishing».

Инвариант уже поддерживается на уровне БД: constraint-триггер `trg_maintain_combination_identity` (`src/migrations/m20260918_000033_create_variant_axes_and_invariants.rs:216-261`) пересчитывает identity при изменении опций. Повторная проверка на чтении — O(варианты × оси) на каждый просмотр карточки, а при реальной рассинхронизации превращает страницу товара в публичную ошибку вместо деградации.

**Рекомендация:** оставить fail-closed только на путях записи/публикации и в диагностике; на публичном чтении доверять триггеру (или кэшировать результат проверки по `index_revision`). Как минимум — протестировать стоимость на каталогах с сотнями вариантов.

---

### P3-1. Legacy-поверхности и мёртвый код (напряжение с zero-legacy, AGENTS.md §13)

- `queries.rs:243-365` — `list_legacy_storefront_products_with_locale_fallback` с fallback'ом shipping-профиля из `metadata` (`:328-333`);
- `admin_queries.rs:30-43` — `list_admin_products_with_compatibility_query` с `raw_status: Option<&str>`, сравниваемым с enum-колонкой как строка (`:59-60`): значения не в lower-case (например «ACTIVE») молча дадут пустой список;
- `admin/src/catalog_transport.rs:269-271` — `fetch_product_pricing` под `#[allow(dead_code)]`;
- `storefront/src/catalog_controls.rs:92-112` — три `#[allow(dead_code)]`-хелпера фильтров;
- `src/services/catalog/storefront_attributes.rs:73-74`, `src/services/catalog/types.rs:142-143`, `attribute_validation.rs:81-83` — ещё `allow(dead_code)`;
- `helpers.rs:131-137` — мёртвый `generate_variant_title_from_axis_values` (см. P1).

AGENTS.md прямо запрещает compatibility-обёртки и dual read paths в `main`. Рекомендация: перенести legacy-параметры в контракт адаптера вызывающей стороны или удалить; `allow(dead_code)` либо убрать, либо заменить на явные реэкспорты контракта.

### P3-2. Мелкие дефекты качества

- **`DuplicateSku("")`** при уникальном нарушении без SKU: `src/services/catalog.rs:108-111` — `sku.unwrap_or_default()`. Оператор получает бессмысленное сообщение.
- **Пустой `update_product` не является no-op**: guard `commands.rs:587-601` срабатывает только при «нет изменений И явно передан тот же статус». Полностью пустой инпут (все `Patch::Keep`, без статуса) проходит дальше, поднимает `revision` и публикует `ProductUpdated` (`:703`, `:900-905`) — холостой churn событий/ревизий и гонка с другими редакторами.
- **`.ok()`-проглатывание декода**: `src/services/catalog/translation_changes.rs:175-176` — сбой чтения `resource_revision`/`lifecycle` молча трактуется как «нет предыдущего значения» (journal dedupe может пропустить изменение).
- **`scan_published_product_ids` не ограничивает `limit` сверху** (`queries.rs:27-62`, только `max(1)`); текущие вызовы ограничены (`seo_targets.rs:22` `BULK_FETCH_SIZE = 48`), но контракт открыт для внешнего вызывающего.
- **Хардкод `$N`-плейсхолдеров** рядом с существующим хелпером `sql_placeholder` (`helpers.rs:584-589`): `commands.rs:2594-2607`, `:972-976`, `:1619-1647`. На SQLite эти запросы работают лишь потому, что параметры идут по порядку; повторное использование `$N` в одном запросе молча сломается. Привести к хелперу.
- **`validate_primary_category` выполняется на `self.db` вне owner-транзакции** (`commands.rs:962-991`): между проверкой и вставкой категорию может удалить Taxonomy → падение по FK вместо типизированной ошибки. Некритично (транзакция откатится), но сообщение хуже и это check-then-act.
- **Дуализм создания/обновления цен**: `create_product`/`create_variant` принимают channel-scoped цены (`commands.rs:425-436`, `:1472-1492`), а `update_variant` их отвергает (`:1539-1550`). Задокументировано в implementation-plan, но асимметрия путает клиентов.
- **`bump_product_index_revision` молча пропускает не-Postgres** (`write_transaction.rs:521-523`) — на SQLite индексная ревизия не двигается (осознанный dual-db компромисс, но стоит явно тестировать).
- **`sync_product_brands_in_tx` — delete+insert** (`commands.rs:107-153`): UUID строк `product_brand` и `created_at` пересоздаются при каждом апдейте продукта (churn идентичности).
- **In-memory retry-identity для Leptos-мутаций** (`admin/src/transport/retry.rs:17-20`, `OnceLock<Mutex<HashMap>>`): ключ теряется при перезагрузке страницы; Next-поверхность имеет durable journal (`write-journal.ts`), Leptos — session journal только для осей (по write-boundary). Межповерхностная асимметрия durability.
- **`has_unverified_media_images` на каждом добавлении картинки** (`commands.rs:1828-1832`) — tenant-wide gate: один непрогнанный backfill навсегда блокирует добавление изображений всему тенанту (задокументировано, но это availability-ловушка; нужен явный статус/алерт в админке).

### P3-3. Документация

- `README.md` модуля содержит **пять** секций «Known Limitations / Pending Implementation» (строки 33, 466, 586, 603, 636) — дублирование затрудняет понимание актуального списка ограничений.
- `docs/README.md` описывает generic-GraphQL `variants.prices` как «compatibility snapshot» и `shipping_profile.slug` как «backward-compatible normalization» — при §13 zero-legacy формулировки «backward compatible» в `main` выглядят как задокументированный долг; стоит либо убрать формулировки, либо завести явный долг с ADR.

---

## 3. Что проверено и сделано хорошо

Чтобы оценка была честной, перечислю проверенные места, где дефектов **не** найдено:

- **Owner-рицы** (`src/services/write_transaction.rs`, `src/catalog_command_port.rs`): admission/replay до операции, фактический публичный результат кодится **до** коммита, completion — в той же транзакции, детерминированные отказы терминальны, unknown-commit оставляет reclaimable-lease, откат ожидается перед финализацией. Тесты на все ветки есть в самом файле.
- **Транзакционный outbox + refresh-леджери**: `publish_internal` (`write_transaction.rs:333-514`) атомарно пишет событие, translation-journals, locale/variant refresh-записи; сбой любой части откатывает мутацию.
- **Тенант-изоляция в native server-fn**: каждый `#[server]`-хендлер проверяет permission + `tenant_id != tenant.id` (например, `admin/src/transport/native_server_adapter.rs:700-722, 1095-1105`); GraphQL выводит тенант из auth-контекста.
- **Паритет ключей идемпотенции между native и GraphQL**: обе поверхности фреймят ключ одной функцией `scoped_caller_idempotency_key` (`src/catalog_schema_write_port.rs:36-55`; native — `native_server_adapter.rs:638-676`, GraphQL — `rustok-commerce/src/graphql/mutations/catalog.rs:54-103`), поэтому native→GraphQL fallback (`admin/src/transport.rs:101-107, 322-345`) не порождает дублей — второй вызов реплеится по рице. Остаточный риск — маскировка ошибки первого транспорта, не двойное исполнение.
- **EAV-фильтры** (`src/services/catalog/attribute_filters.rs`): параметры всегда связаны, фильтр только по `is_filterable`, неархивированным, tenant+detached-гвард в каждом условии.
- **Фасеты** (`src/services/catalog/facets.rs:727-836`): buckets читаются нативными типами, `+1` строка для отличия «ровно лимит» от «усечено», tenant-гвард в join.
- **Media release-воркер** (`src/media_reference.rs:762-855`): at-least-once, bounded backoff, защита от stale-обновлений (CAS по attempts), подтверждение через lookup.
- **Index refresh relay** (`src/services/index_refresh_relay.rs:283-362`): cursor под `FOR UPDATE`, CAS-продвижение — повторная публикация исключена.

---

## 4. Рекомендации (по приоритету)

1. **P1, немедленно:** починить генерацию заголовков вариантов (лейблы опций вместо identity), убрать перезапись непустых переводов в `update_variant`, написать бэкфилл испорченных заголовков. Добавить контракт-тест «title never equals combination_identity».
2. **P2:** компенсация Media-retain при откате транзакции (или persistent pending-retain + свипер); аларм на `orphan_media_hold`.
3. **P2:** регистронезависимый поиск + экранирование wildcards в `product_title_search_condition`/`admin_product_title_search_condition` (одна общая функция) с тестами на PG.
4. **P2:** решение по семантике фильтра категорий (поддерево vs точное) и синхронное изменение трёх мест + фасет.
5. **P2:** fail-closed парсинг числовых полей в админке (сток/позиция) вместо `unwrap_or(0)`.
6. **P2:** убрать пересчёт инвариантов осей из горячего пути публичного чтения (оставить на записи/публикации/диагностике).
7. **P3:** чистка legacy-поверхностей и `#[allow(dead_code)]`; устранение дублей «Known Limitations»; unify `$N`-плейсхолдеры через `sql_placeholder`; empty-update no-op; типизированная ошибка при валидации категории внутри транзакции.

## 5. Ограничения аудита

- Проверял только границы модуля продукта и его интеграций; полный аудит `rustok-commerce`, `rustok-ai-product`, `rustok-product-bundles/relations` вне скоупа (их ledger-записи отмечены как completed).
- Ничего не компилировалось и не запускалось: выводы — по чтению исходников на HEAD `58ccfb8`. Перед фиксом P1/P2 нужны тесты на PG (в модуле они помечены как «written but not executed» для ряда поверхностей).
- Не проверял рантайм-поведение миграций (включая `m20261117_000044_order_product_channel_convergence_commits`) — только их исходники на консистентность инвариантов.

---

## 6. Перепроверка сделанных правок (2026-11-17, финальный проход)

Полный прогон всех 1175 статических verify-скриптов репо в двух деревьях (наша ветка vs базовый `58ccfb8` в отдельном worktree) с диффом **сообщений**, а не только exit-кодов; затем точечный разбор 49 скриптов, читающих изменённые нами файлы.

**Итог для 49 скриптов наших файлов:** 25 PASS/PASS; 22 падают с идентичными сообщениями на базе (предсуществующие, не наши); 1 гейт, красный на базе, закрыт нами (`verify-product-next-data-table-host` — конверсия `brands-table.tsx` на `DataTableStatic`); 1 (`verify-frontend-host-ffa-contract`) падает ENOENT на отсутствующем `apps/admin/src/features/workflow/mod.rs` **в обоих** деревьях — предсуществующее.

**Найденные и устранённые самовнесённые регрессии гейтов (из §4-фиксов раунда 1):**

1. **Экранирование LIKE-wildcards — ОТКАЧЕНО** (правка рек. №3): `escape_like_pattern` в owner'е (`queries.rs`/`admin_queries.rs`) расходила владельца с shadow-адаптером (`rustok-distribution/.../storefront_shadow.rs` → `FilterExpr::TextLike`, где `%`/`_` — осознанная wildcard-семантика, зафиксированная 5+ гейтами и reference-тестом `text_like_matches_postgres_wildcards_and_escape`). Trim был избыточен — `normalize_storefront_product_search` уже тримит. Уточнение к рек. №3: экранирование возможно только синхронно в owner+shadow+reference одним скоупированным изменением; регистронезависимость по-прежнему не реализована. Хелпер и тест удалены.
2. **Поддерево категорий — ОСТАВЛЕНО, маркеры гейтов обновлены** (рек. №4): `PrimaryCategoryId.eq(category_id)` → `PrimaryCategoryId.is_in(subtree)` в `verify-product-storefront-category-sort`, `verify-product-admin-category-sort`, `verify-product-catalog-controls-plan-sync` (гейты обновляются вместе с кодом — конвенция подтверждена историей репо).
3. **Бэкенд-зависимые плейсхолдеры — ОСТАВЛЕНЫ, маркер обновлён** (рек. №7): `verify-product-catalog-schema` теперь пинит `VALUES ({tenant_ph}, {value_ph}, {option_ph})` вместо жёстких `$1..$3` (ломались на SQLite).
4. **Stage/commit разбиение `add_product_image` — ОСТАВЛЕНО, гейт перестроен**: `verify-product-media-retention` теперь фиксирует, что coverage-маркер пишется внутри `stage_product_image_addition_in_tx` до `Ok((txn, result))`, а admitting-флоу вызывает stage и коммитит — интент «маркер коммитится вместе с картинкой» сохранён и усилен.
5. **Fail-closed парсинг — ОСТАВЛЕН, маркер переименован**: хелпер `parse_product_admin_inventory_quantity_input` → `parse_product_admin_quantity_input` (парсит и сток, и позицию); обновлены `verify-product-admin-boundary` и фикстура его `.test.mjs`.

**Также в перепроверке:** исправлен regex в `secondary-write-idempotency.test.mjs` (не учитывал комментарии внутри вызова `onAddImage` — упал бы при прогоне); dry-run хендлера relation-диалога на 12 поведенческих сценариях — все проходят; tsc next-admin = 0.

**Окружение, не код:** `verify-fly-grapesjs-roundtrip` требует Playwright-браузер — в песочнице падает на запуске Chromium (на базе «проходит» только потому, что там нет node_modules и скрипт честно скипается); файлы скрипта мы не трогали.

---

## 7. Раунд 7: разбор 22 предсуществующих красных гейтов (долг main в зоне продукта)

Метод: те же 49 скриптов, дифф сообщений ours-vs-base. Каждый из 22 падений классифицирован по фактическому коду: stale-маркер (гейт отстал от санкционированной эволюции кода) vs реальный пробел (недописанная фича/WIP).

**Закрыто 8 гейтов (stale-маркеры приведены к текущей санкционированной форме; интент каждого сохранён):**

1. `verify-product-admin-category-sort` — статус-фильтр: `Status.eq(status)` → `Column::Status.eq(` (текущая форма; план-sync гейт уже принимает её).
2. `verify-index-product-storefront-deep-page-policy` — offset: saturating-формула → overflow-checked форма (`checked_sub/checked_mul` + fail-closed Validation). Сиблинг-маркеры того же гейта (shadow executor) уже пинировали checked-форму — владелец отстал.
3. `verify-index-product-storefront-parity-gate` — нативный транспорт: прямое `CatalogService::new(...)` → seam `native_server_adapter::product_read_port(&scope).list_filtered_published_products(`; плюс тот же checked-offset маркер.
4. `verify-index-product-storefront-localized-query-architecture` + `verify-index-text-like-filter` — `StorefrontProductListQuery` переехал в `ports/src/types.rs` (пакет публикуемых контрактов); маркеры структуры/полей перепинены туда, нормализация — на текущую форму `normalize_storefront_product_search`.
5. `verify-product-storefront-category-sort` — поле `category_id` в ports-структуре; graphql-резолвер ходит через read-port `.list_filtered_published_products(`.
6. `verify-index-product-storefront-tag-hydration` — таксономия:注入-порт `.taxonomy()` вместо прямого `TaxonomyService::new`; сигнатура `resolve_term_names` (теперь принимает connection); миграция: типизированные константы `TaxonomyScopeType` восстановлены (Display-эквивалент; raw-SQL доступ к кросс-онер таблицам таксономии закреплён как конвенция — 10+ миграций так делают).
7. `verify-product-next-data-table-host` — закрыт ранее (конверсия brands-table).

Фикстуры `.test.mjs` двух category-sort гейтов обновлены зеркально.

**Осталось 16 предсуществующих (не наши, решение за мейнтейнером):**
- **Недописанные фичи/контракты:** admin-list read contract (`AdminProductsRequest`), storefront GraphQL read (`Filtered.../Projection.../Legacy...` типы), `ProductCatalogReadPort` REST-cutover, фасет-порты (×2 гейта), media/price-сводки каталога (10 маркеров), lifecycle `archive_product` порт, Leptos `ProductEditorPage` + routed recovery editor (×2).
- **Дивергенция политики:** `storefront_http_read_port` зовёт product-копию `is_metadata_visible_for_public_channel` вместо inventory-владельца (две идентичные копии могут разойтись; product не зависит от rustok-inventory — вопрос направления зависимости за мейнтейнером).
- **WIP:** wave18 — `validate_new_product_publish_requirements_in` существует, но никем не вызывается, и тело выглядит всегда-ошибочным (не подключать без переработки).
- **Инвентарная координация** (×3): BootstrapService-порядок, blocker-классификации, digest-локи `event-contract-digests.json`.

**Итог свипа 49 скриптов:** 25 PASS/PASS · 8 закрыто нами · 16 предсуществующих (идентично базе) · 0 регрессий.

## 8. Раунд 8: закрытие остальных 12 предсуществующих гейтов (stale-маркеры)

Метод прежний: ours-vs-base msg-дифф на worktree базы `58ccfb8`; перед каждой правкой — батч-проверка всех маркеров гейта против файлов (fail-fast скрывает слои). Маркеры перепинены на текущую санкционированную форму кода, интент каждого сохранён.

**1. Порты и контракты каталога (6 гейтов).**
- `verify-product-lifecycle-events` — трейт командного порта опубликован в `rustok-product/ports/src/ports.rs`; маркер input_status + трейт перепинен туда.
- `verify-product-facet-chain` + `verify-product-admin-facets` — фасет-реквесты/DTO живут в `ports/src/types.rs` (добавлены док-комментарии «Accepted by ... adapter»), `facets.rs` — re-export + `MAX_*`; вызов `.load_storefront_catalog_facets(`.
- `verify-commerce-rest-storefront-product-detail-owner-read-cutover` — трейт `ProductCatalogReadPort` → `ports/src/ports.rs`.
- `verify-commerce-product-storefront-graphql-read` — маркеры разбиты по контракт-файлам (ports / contractsTypes / contractsPorts); retired-код `product.storefront_detail_unavailable` → актуальный `product.storefront_projection_unavailable`.
- `verify-commerce-product-admin-list-read` — 4 блока: ports / contractsTypes / `dto/product.rs` / contractsPorts; unavailable-текст → `admin product listing is unavailable`.

**2. Leptos-редактор (2 гейта).** `verify-product-variant-axes-ui` + `verify-product-admin-operation-results` — routed-recovery форма: `use super::product_editor::ProductEditorPage;` + `<ProductEditorPage is_new=false product_id=product_id />`.

**3. HTTP error-safety admin-чтения (1 гейт, полная переработка).** `verify-commerce-admin-product-read-http-error-safety` — 56 слоёв фейлов сняты батч-инспекцией: якорь на `map_admin_product_port_error(` (controllers/products.rs:382, match по `PortErrorKind` + коды владельца), 12 маркеров диспетча (вкл. `product.revision_required/duplicate_handle/duplicate_sku/revision_conflict`, делегация `inventory_deletion/quantity_public_failure` → `product_port_http_policy`), счётчик вызовов маппера ===7, `show_product` использует `map_admin_product_error(` (второй маппер, :230 — не взаимозаменяемы), операции владельца → `catalogCommands/catalogProjection`.

**4. Инвентарная координация (3 гейта).**
- `verify-inventory-source-delivery` — call-сайты на порт-форму: `.bootstrap_variants_in_tx(&txn, tenant_id, actor_id, ...)` / `.delete_records_for_variants_in_tx(...)`; в evidence-пакете обновлены 2 sha256 (ковариантная пара нового контракта).
- `verify-inventory-quantity-writes` — маркеры разнесены по фактическим владельцам: call-site `self.inventory().update_initial_quantity_in_tx(&txn, ...)` (commands.rs), чтение `port.load_available_quantities(conn.as_inventory_connection(), ...)` (projection.rs), сохранение типизированного блокера `crate::error::inventory_quantity_port_error(&blocker)` (commandPort), классификатор `inventory_quantity_blocker(error).as_ref().map(inventory_quantity_public_policy)` (error.rs), nonterminal kind-набор (write_transaction.rs), `QuantityWriteBlocker::PublicationUnverified.port_error()` (quantity.rs:161). Sha256 `event-contract-digests.json` в пакете обновлён; **счётчик preserved_hashes === 14 сохранён**.
- `verify-inventory-catalog-deletion` — call-сайты `.delete_records_for_variants_in_tx(&txn, tenant_id, actor_id, ...)` с порядком «inventory → pricing cleanup»; точный отказ `ProductInventoryDeletionBlocker::IdentityUnavailable` + `CommerceError::InventoryDeletionBlocked(` в `delete_product`; лимит `crate::ports::MAX_CATALOG_DELETE_VARIANTS` (re-export из портов); классификатор `product_catalog_command_error_is_terminal` опубликован в `ports/src/ports.rs` (коды вкл. `product.inventory_deletion_changed` = ConcurrentChange-терминальность).

**Протокол evidence-пакетов:** sha256 в `preserved_hashes` обновляются только после подтверждения, что текущий файл проходит собственный реестровый гейт (digest-admission PASS); количество записей не меняется.

**Остаток — 4 SAME-FAIL, все предсуществующие (решение за мейнтейнером):**
- `verify-product-storefront-media-price` — гап: `StorefrontProductMediaSummary/PriceSummary`, `primary_image`, `price_from`, display-currency в list-query — типы не существуют (недописанная фича сторфронт-листинга).
- `verify-product-audit-wave18-findings` — гап: `validate_new_product_publish_requirements_in` (values.rs:302) не вызывается никем, тело всегда-ошибочно (WIP; не подключать без переработки).
- `verify-commerce-storefront-product-http-error-safety` — дивергенция: дубль `is_metadata_visible_for_public_channel` (helpers.rs:61 vs inventory public_channel.rs:59); product не зависит от rustok-inventory — направление зависимости за мейнтейнером.
- `verify-frontend-host-ffa-contract` — ENOENT `apps/admin/src/features/workflow/mod.rs` и на базе (зона dioxus-admin, исключена из правок по ограничению).

Вне скоупа продукта: `verify-forum-search-versioned-invalidation-{causation-api,publisher,wire}` — SAME-FAIL на базе (отсутствует docs-контракт FORUM-23B2G2B3A), к нашим правкам отношения не имеют; `verify-index-contract-ci` — PASS на ours и базе.

**Итог свипа 49 скриптов (раунд 8, финал): 25 PASS/PASS · 20 FIXED (8 раунда 7 + 12 раунда 8) · 4 предсуществующих · 0 регрессий.**

## 9. Раунд 9: QA компаньонов (.test.mjs) — соответствие фикстур закрытым гейтам

Правки гейтов раундов 7–8 перепиняли маркеры на текущую архитектуру; компаньоны (`scripts/verify/*.test.mjs`) при этом могли остаться на старых формах: мутационные фикстуры мутируют уже не существующие строки (`change()` падает на `assert.ok(source.includes(before))`), а синтетические workspace не предоставляют файлы, которые гейт теперь читает.

**Метод.** Компаньоны не запускались под `node --test` (за пользователем); вместо этого — статический replay-harness: копия компаньона с подменённым импортом `node:test` на коллектор, все остальные импорты/логика исполняются как есть (фикстуры пишутся в temp, гейты запускаются как подпроцессы). Полный свип 154 компаньонов ours-vs-base (`58ccfb8`) с msg-диффом.

**Найдено и исправлено (4 компаньона):**
1. `verify-inventory-quantity-writes.test.mjs` — 3 stale-мутации перепинены на фактических владельцев: projection `port.load_available_quantities(conn.as_inventory_connection(), …)`, commands `.update_initial_quantity_in_tx(`, commandPort `crate::error::inventory_quantity_port_error(&blocker)`; добавлена мутация классификатора в productError (`inventory_quantity_blocker(error)`). Каунты гейта (rust 33 / node 10) не дрейфовали.
2. `verify-product-lifecycle-events.test.mjs` — фикстура переведена на пакет контрактов: `ports/src/ports.rs` (трейт) + `src/catalog_command_port.rs` (impl с `let operation = "archive_product";`); `input.status` → `input_status`; негативная мутация трейта ретаргетирована в ports.rs. В гейтеread `ports.rs` добавлен в pre-check существования (ENOENT → graceful fail).
3. `verify-product-facet-chain.test.mjs` — фикстура: реквесты/DTO → `ports/src/types.rs`, новый `ports/src/ports.rs` (default-Err `product.{,admin_}catalog_facets_unavailable`), `facets.rs` → `pub use rustok_product_ports::{…}` re-export вместо локальных DTO, storefront-native → `product_read_port(&scope).load_storefront_catalog_facets(`. Из гейта удалён мёртвый read `src/ports/types.rs`.
4. `verify-product-catalog-controls-plan-sync.test.mjs` — единственная регрессия раунда (base PASS → ours FAIL): фикстура `PrimaryCategoryId.eq(category_id)` ×3 → `PrimaryCategoryId.is_in(subtree)` (догоняет перепин раунда 7).

**Итог свипа компаньонов:** 154 = **121 PASS · 33 предсуществующих FAIL** (режимы падения побайтово идентичны базе с точностью до абсолютных путей; почти все — ENOENT на отсутствующие docs/файлы в зонах blog/pricing/notifications/search/forum/fba — чужой долг, вне скоупа продукта) · 0 регрессий после фикса.

Перепроверено после правок: все изменённые гейты PASS на реальном репо; `node --check` по всем изменённым `.mjs` чистый; evidence-пакеты валидны (quantity-writes preserved_hashes === 14, source-delivery === 21); временные артефакты harness удалены, worktree базы снят.

**Оговорка:** replay-harness — не `node --test`; авторитетный прогон компаньонов остаётся за мейнтейнером (`node --test scripts/verify/`).

## 10. Раунд 10: охота за косяками в коде модуля (сверх гейтов)

Метод: шесть статических свипов по классам дефектов + ручная трассировка проводки рантаймов. Свипы: unwrap/expect/panic с трекингом тест-блоков; find_by_id/delete_by_id без tenant-окна; raw SQL DELETE/UPDATE без tenant_id; события вне транзакции; порядок schema-lock → product-lock; сканер мёртвых pub-fn (423 определения, ручная верификация кандидатов — автомат даёт ложные срабатывания на same-file вызовах).

**Чисто (класс проверен, дефектов нет):**
- unwrap/expect/panic — 36 production-хитов, все легитимные инварианты: `unreachable!()` на `Clear` прикрыт ранним guard-ветвлением с DELETE; «ranked map is never empty» доказуемо (entry создаётся только через insert); «normalized to an object» — `*base = Object` строкой выше; «binding checked above» — первый проход миграции возвращает Migration-ошибку на missing binding.
- Изоляция тенантов — все find_by_id/delete_by_id либо фильтруются по TenantId, либо по ProductId от tenant-верифицированного продукта (композитная безопасность); raw-SQL DELETE по value_id, только что возвращённому tenant-скопированным upsert; full-table DELETE оказался тестовым фикстурным кодом.
- События — публикуются строго `publish_in_tx_with_envelope_id` внутри ProductWriteTransaction (outbox-паттерн); прямых публикаций в event_bus вне транзакции нет.
- Lock ordering — commands.rs 13/13 schema-lock → product-lock; бинари (index_refresh, channel_convergence) дисциплинированы: bounded tenants, MissedTickBehavior::Delay, JoinSet, таймауты, lease/retry с lease-lost как benign.

**F1 — мёртвые дубликаты read-хендлеров (главная находка).** `controllers/products.rs` (shared): `list_products` (:482, с tag-map на голом CatalogService :615) и `show_product` (:693, полный detail через `get_product_with_locale_fallback` на `CatalogService::new`) — **не смонтированы и не вызываются**: оба роутера разрешают `products::show_product` в собственные модули (`admin/products.rs:230` — через read-port; `store/products.rs:586` — через read-port). Опасность латентная: голый CatalogService без коллабораторов → projection молча возвращает пустые цены (`if let Some(port) = &self.pricing_port … else Ok(Vec::new())`) и пустой инвентарь (Noop → `Ok(HashMap::new())`); плюс другой error-маппер и отсутствие channel-обработки. Если мейнтейнер смонтирует эти хендлеры — admin/storefront detail молча потеряет цены и сток. Рекомендация: удалить оба + `list_product_tags` (входят только в них) или перевести на read-port.

**F2 — 17 мёртвых pub API** (production-вызовов нет, только определения/тесты): builders `with_pricing_port`/`with_inventory_bootstrap_port`/`with_media_maintenance_port` (superseded прямыми присваиваниями полей в `in_process_with_flex_schema`); projection `get_variant`/`get_product_variant_axes` (superseded порт-формами `*_in`); `validate_product_publish_requirements` (обёртка; подключённый `_in`-вариант вызывается из create/update/publish ×4); helpers legacy-metadata ×6 (`split_product_metadata_payload`, `merge_*`, `prepare_product_custom_fields_*`, `resolve_product_metadata`, `map_flex_cleanup_error`); `list_changes` (index relation, superseded convergence-store); `declares_value_rules`; `normalize_storefront_currency_code` (примыкает к гапу storefront-media-price — валидация валюты существует, но не подключена). Maintenance-debt, не баги; wave18-пара (`validate_new_product_publish_requirements*`) остаётся задокументированным гапом §8.

**F3 — наблюдение (низкий риск):** attached-values пишет product.metadata, читая flex-схему без общего advisory-lock (в отличие от каталог-атрибутов, 13/13 под `lock_schema_for_product_write`). Гонка с мутацией field definitions может записать ключ снятого поля; ключ фильтруется при чтении и перезаписывается следующим write (self-healing), FK нет (JSON). За мейнтейнером.

**F4 — наблюдение (UX):** `fetchProductAttributeValues` при отсутствии auth возвращает `[]`, все сиблинги в api/products.ts бросают «Sign in again». Опасности нет — editor собирает только dirty-патчи (`dirtyAttributeIds` стартует пустым), пустой initial не стирает значения.

**F5 — подтверждение архитектуры (не баг):** standalone gRPC-сервис (`bin/service.rs:293`) компонирует CatalogService без коллабораторов — но Noop-порты fail-closed: delete → `InventoryDeletionBlocked(StateUnverified)`, ненулевой начальный инвентарь → `LocationUnavailable`, чтения → «нет данных» вместо фиктивных. Монолит (apps/server) проводит полный набор: `in_process_with_collaborators(pricing, inventory, taxonomy)` — единственный production-insert в shared; commerce-fallback без inventory перекрывается host-композицией; контракт separate-process (`mod-commerce,mod-ai`) — read-scale-out, probe проверяет 6 read-операций. Co-requisites Продукта (product-lifecycle-collaboration.md) не пускают монолит без Inventory/Pricing.

**Итог раунда 10:** 1 латентный косяк (F1, мёртвые дубликаты read-хендлеров с риском молчаливой потери цен при монтаже), 17 мёртвых API, 2 наблюдения, 1 архитектурное подтверждение; критических багов в живых путях не найдено — проводка монолита/GraphQL/storefront/admin корректна, fail-closed инварианты держатся.

### 10.1. Фикс F1 (исполнено по команде «чиним»)

**Код.** Из `crates/modules/rustok-commerce/src/controllers/products.rs` удалены непримонтированные read-хендлеры `list_products` (:482) и `show_product` (:693) вместе с приватным хелпером `pick_product_translation` (потреблялся только list_products; −264 строки) и осиротевшим после этого error-маппером `map_admin_product_error` + `adopt_product_error_identity` (−39 строк; маппер использовался только удалёнными read-хендлерами, lifecycle-пути идут через `map_admin_product_port_error`). Итог: 959 строк (было 1273; суммарно −318/+4), скобки/парены сбалансированы (95/95, 405/405). Ингредиенты сохранены намеренно: `ProductListItem`/`ListProductsParams` (потребители — admin/products.rs, store owner_list, openapi.rs), `product_error_policy` (тесты паритета domain↔port), `AdminProductDiagnosticContext/Error` и `ADMIN_PRODUCT_OWNER/BOUNDARY` (живой port-маппер), `product_core_http_policy` (store/products.rs). Семантика inventory-отказов на живом пути не изменилась: `map_admin_product_port_error` форсит `retryable=false` для `inventory_deletion/quantity_public_failure` (`(status, code, message, kind, false)` → `HttpError::new(...).with_retryable(retryable)`).

**Синхронизация гейтов (7 файлов).**
- `verify-commerce-product-admin-list-read.mjs` — pin «unmounted compatibility list source must remain explicit» заменён на forbid-блок: `list_products(`/`show_product(`/`map_admin_product_error(` не должны возвращаться без read-port cutover.
- `verify-commerce-admin-product-read-http-error-safety.mjs` — end-marker маппера `'/// Shared admin product list handler.'` → `'… delete handler.'`; удалены listHandler/showHandler-блоки и их чек-циклы (delete/publish/unpublish-тройки сохранены); `portMapperUses === 7` валиден.
- `verify-commerce-admin-product-diagnostic-safety.mjs` — policy-блок перезаякорен на `admin_product_command_idempotency_key`; stale-маркер `core_port_error(error).kind` (не существовал в коде ни в базе, ни до фикса — гейт был красным на pristine-базе) заменён на фактическую форму `&rustok_product::error::core_port_error(error),`; read-контракт перенесён на смонтированный admin/products.rs; добавлен forbid на возвращение удалённых хендлеров.
- `verify-commerce-admin-product-route-error-context.mjs` — listRoute/showRoute перезаякорены на admin/products.rs (port-формы: `.list_admin_products(`, `.read_product_projection(`, correlation/channel-пины); массив shared-маркеров маппера удалён; scope-loop сужен до выживших shared lifecycle-роутов; вместо `sharedMapperUses === 5` — forbid на возврат `map_admin_product_error(`.
- `verify-commerce-product-rest-lifecycle-command-cutover.mjs` — без правок, PASS.
- `verify-inventory-catalog-deletion.mjs` — pin `response.with_retryable(false)` (литерал удалённого маппера) репинят на живые формы: `(status, code, message, kind, false)` + `HttpError::new(status, code, message).with_retryable(retryable)`.
- `verify-commerce-admin-product-write-http-error-safety.mjs` — энд-маркер port-маппера синхронизирован (list → delete handler); `verify-commerce-product-command-port.mjs` — pin «remaining direct construction must stay visible as follow-up source debt» перевёрнут в forbid (долг F1 закрыт).

**Верификация.** Все 14 гейтов, читающих controllers/products.rs: **12 PASS, 2 FAIL** — и оба FAIL побайтово идентичны pristine-базе 58ccfb8 (подтверждено git-worktree-прогоном): `write-http-error-safety` (2 пина: `headers.get("Idempotency-Key")` в однострочной форме — в коде всегда multiline; `require_policy(PortCallPolicy::write())`) и `command-port` (4 пина: ProductCatalogCommandPort-публикация, deadline/idempotency-семантика, Idempotency-Key-литерал) — stale-гейты со дня коммита, не связаны с фиксом. Прочее: detail/graphql cutover PASS; компаньон `verify-inventory-catalog-deletion.test.mjs` — `node --test` 0 fail; маркеры удалённых хендлеров в компаньон-фикстурах отсутствуют (grep по всем `.test.mjs`).

**Новые pre-existing остатки (обнаружены расширенным свипом, не от фикса; FAIL на pristine-базе):** `verify-commerce-rest-storefront-product-list-owner-read-cutover` (та же семья channel-visibility, что известный остаток storefront-product-http-error-safety: `rustok_inventory::is_metadata_visible_for_public_channel` ожидается в storefront_http_read_port.rs), `verify-commerce-storefront-line-item-http-error-safety`, `verify-commerce-storefront-line-item-owner-context` (line_item_resolution-блоки). Передаются мейнтейнеру вместе с прежними четырьмя.

F2 (17 мёртвых pub API) — вне скоупа до явного запроса; F3/F4 — наблюдения, не чинились.

### 10.2. Фикс F2 (исполнено по команде «продолжим»)

Из 17 мёртвых pub API раунда 10 **удалено 14**, 3 оставлены по явным причинам (ниже). Перед удалением каждый кандидат перепроверен свежим сканером ссылок по ВСЕМ rust-файлам репо (без исключений путей): 0 вызывателей вне определений; для тонких обёрток целевые функции подтверждены живыми через других потребителей.

**Удалено (14):**
- `runtime.rs` — `with_media_maintenance_port` (билдер; геттер `media_maintenance_port()` жив: admin/product_media.rs:75 + тесты apps/server).
- `services/catalog.rs` — `with_inventory_bootstrap_port` (синоним; `with_inventory_port` жив через runtime.rs:296).
- `services/catalog/helpers.rs` — 7 шт.: `map_flex_cleanup_error`, `split_product_metadata_payload`, `merge_product_metadata_patch`, `merge_reserved_product_metadata`, `prepare_product_custom_fields_for_create`, `prepare_product_custom_fields_for_update`, `resolve_product_metadata`. Тонкие обёртки над `field_values::*` (rustok-core), которые живы через `catalog/attached_values.rs`; `pub(super)`-варианты `prepare_*_and_schema_for_*` живы (commands); `load_product_custom_fields_schema` жив через `flex_schema_projection.rs:111`.
- `services/catalog/projection.rs` — `get_product_variant_axes`, `get_variant` (селф-дб-обёртки; `_in`-варианты живы, свободная `load_product_variant_axes` жива через :748/тесты).
- `services/catalog/types.rs` — `normalize_storefront_currency_code` (`#[allow(dead_code)]`, не подключена — примыкает к residual-гепу storefront-media-price, гейт её не пиннит).
- `services/catalog_schema_service/attribute_validation.rs` — `declares_value_rules` (`#[allow(dead_code)]`, только 2 тест-ассерта).
- `services/catalog_schema_service/values.rs` — `validate_product_publish_requirements` (pub-обёртка; подключённый `_in` жив на :527 в txn).

**Оставлено (3) с обоснованием:**
- `with_pricing_port` (catalog.rs) — живые тест-потребители: rustok-commerce store/tests/mod.rs:186 и `rustok-pricing/admin` (запретная зона dioxus-admin) tests/support.rs:38; без билдера внешние тесты не соберут CatalogService с pricing.
- `list_changes` (index_channel_relation.rs) — зона Ledger (не трогать) + pin гейта verify-index-product-channel-relation-ledger.mjs:62.
- `validate_new_product_publish_requirements` (+ его `_in`-цель) — wave18-гап §8: цепочка пиннится гейтами verify-product-attribute-validation-rules.mjs (:357/:361) и verify-product-audit-wave18-findings.mjs (:79/:80/:83); удаление = тяжёлая синхронизация двух гейтов и компаньонов против документированного гепа — решение за мейнтейнером.

**Тесты:** удалены 3 теста мёртвых обёрток в `helpers::product_metadata_tests` (split/merge_patch/merge_reserved; тесты живых `reject_reserved_tag_metadata`, `resolve_product_metadata_with_schema`, `apply_shipping_profile_to_metadata` сохранены, use-super сужен); из 2 тестов attribute_validation удалены только ассерты `declares_value_rules()` (остальные утверждения на месте).

**Синхронизация гейтов:** единственный задетый — `verify-product-catalog-receipts.mjs`: энд-граница productProjection-блока `'pub async fn get_variant('` → `'pub(super) async fn get_variant_in'` (компаньон .test.mjs маркер не содержит). Греп по scripts/verify подтверждает: больше ни один гейт не пиннит удалённые имена (остальные совпадения — `_in`-формы живых функций и Ledger-pin).

**Верификация.** Все 48 гейтов, читающих 7 отредактированных файлов: **39 PASS, 9 FAIL — все 9 побайтово идентичны pristine-базе 58ccfb8** (worktree-диффы отказов): 4 известных остатка (wave18-findings, storefront-media-price, commerce-product-command-port, rest-storefront-list-owner-read-cutover) + 5 ранее не учтённых pre-existing: `verify-index-product-storefront-serving-budget-policy` (distribution hydrate-пин), `verify-pricing-product-identity-context` (ENOENT — читает несуществующий rustok-pricing/src/services/pricing.rs), `verify-product-catalog-schema-read-port` (2 пина schema-read-проводки), `verify-product-catalog-schema-write-port` (пин schema-write-композиции), `verify-product-schema-attribute-create-receipts` (пин rustok_outbox::idempotency). F1-сетка (14 гейтов по controllers/products.rs) без изменений: 12 PASS / 2 pre-existing. Баланс скобок/паренов всех 7 файлов соответствует базовым дельтам; упоминаний удалённых имён в rust-коде 0.

**Итог F2:** −14 мёртвых API (−177 строк прод-кода и −74 строки тестов, суммарно −251), 1 гейт репинят, 0 регрессий; 3 API оставлены (2 запретные зоны, 1 wave18-геп). Новых pre-existing остатков передано мейнтейнеру 5 (см. выше).

## 11. Раунд 11: корневые причины pre-existing остатков + новая находка F6

Метод: каждый красный гейт — трассировка пина до фактической формы в коде; атрибуция «мой регресс vs база» через git-worktree прогоны на pristine-базе 58ccfb8 с побайтовым диффом наборов отказов; компаньоны .test.mjs grepped по каждому изменённому маркеру.

### 11.1. Восстановлено 9 гейтов (код был корректен, пины устарели)

| Гейт | Корневая причина stale-пина |
|---|---|
| verify-commerce-storefront-product-http-error-safety | пин `rustok_inventory::is_metadata_visible_for_public_channel(` — крейт product не зависит от rustok-inventory; функция продублирована локально в helpers.rs:61 (та же семантика), вызов `helpers::`-формой; visibility применяется ДО пагинации (port :158→:171) |
| verify-commerce-rest-storefront-product-list-owner-read-cutover | то же семейство, 2 пина |
| verify-index-product-storefront-serving-budget-policy | вызов `self.phases.hydrate_projected_tags(tag_context, params.fallback_locale, projected)` — поле квалифицировано через `params.` |
| verify-product-catalog-schema-read-port | лог уже в privacy-форме `correlation_id_length = context_facts.correlation_id_length` (пин требовал сырую); проводка схемы вынесена в переменную `schema_service` (пин требовал инлайн-литерал конструктора) |
| verify-product-catalog-schema-write-port | вынесенный конструктор (проводка существует, runtime.rs:277) |
| verify-product-schema-attribute-create-receipts | крейт переименован: `rustok_outbox_storage::idempotency` (пин держал старый `rustok_outbox::`) |
| verify-commerce-product-command-port | 4 формы: multiline re-export в lib.rs; write-адмиссия через `catalog_scope(context, operation, PortCallPolicy::write())`; `.get("Idempotency-Key")` multiline; functionSlice не захватывал собственный `#[utoipa::path(` хендлера (OpenAPI-контракт) |
| verify-commerce-admin-product-write-http-error-safety | 2 формы: `.get(` multiline; catalog_scope-адмиссия |
| verify-commerce-product-admin-detail-read | пин «admin list must remain explicit follow-up debt» требовал делегирование `super::super::products::list_products` — долг снят ещё ДО базы (admin list на read-port); флип в forbid + позитивные пины `.product_catalog_read_port()`/`.list_admin_products(` — консистентно с F1 |

Все правки: точные формы с count-assert, `node --check`, прогоны PASS, компаньоны чисты (изменённых гейтов компаньонов нет; смежные 3 чужих компаньона — 83 pass / 0 fail).

### 11.2. F6 — новая находка: line_item_resolution — superseded граница без прод-вызывателей

`crates/modules/rustok-commerce/src/controllers/store/line_item_resolution.rs` (граница резолва line-item, pub(crate)): **0 вызывателей в прод-коде**. Живой путь — `carts.rs:399 .add_line_item(` на cart-порту + `map_cart_port_error`; модуль импортируется только `store/tests/products.rs` (тесты мёртвой границы). Красные гейты семейства (4 шт.: storefront line-item http-error-safety/owner-context, graphql line-item compatibility-cutover/typed-diagnostic-safety) пиннят удалённый `validate_store_line_item_quantity(` и структуру, которой нет — ожидаемо красные с базы. Варианты за мейнтейнером: удалить модуль + тесты + 4 гейта (аналог F1 в масштабе модуля) либо перемонтировать. Гейты сознательно НЕ репинялись: полировать гарды над мёртвым кодом вводит в заблуждение.

### 11.3. Классифицированные остатки (за мейнтейнером, все красные на pristine-базе)

- **verify-product-storefront-media-price** — фича-контракт: нереализованные media/price-сводки (primary_image, price_from, currency builder, build_storefront_list_price_from_map). Продуктовое решение, не дефект.
- **verify-product-audit-wave18-findings** — требование «превалидация до вставки». Инвариант ДЕРЖИТСЯ: create валидирует publish-требования в той же транзакции до commit (`if input.publish { validate_..._in(&txn,...)? }`, commands.rs:497–501, `?`→rollback); расхождение только в порядке/варианте. Стилистическое требование.
- **verify-pricing-product-identity-context** — жертва реструктуризации pricing: 6 путей переехали (services/pricing.rs → services/pricing/{mod,projections}.rs; ports.rs → ports/; read_context → integrations/; public_error/write_context/admin-transport перемещены). Гейту нужен re-pointing + реверификация маркеров по split-файлам.
- **verify-frontend-host-ffa-contract** — ждёт удалённую фичу apps/admin workflow (файлов нет нигде); запретная зона dioxus-admin.
- **verify-product-admin-fallback-mutation-error-safety** — ждёт `#[path]`-модули в rustok-product/admin (dioxus-admin, запретная зона); UI-фича.
- **verify-product-ui-grid-facets** — UI-фича (a11y «Load more») в admin-зонах.
- **verify-commerce-product-admin-graphql-read** — легаси-контракт (`LegacyAdminProductsRequest`, `product.legacy_admin_list_unavailable`) удалён.
- **verify-commerce-product-graphql-lifecycle-command-cutover** — `in_process_with_pricing(` superseded `in_process_with_collaborators` (см. вердикт проводки §10), форма `pub trait` переехала.
- **verify-commerce-graphql-product-retryability** — старые формы маппера публичного конверта.
- **verify-product-catalog-port-error-safety / verify-product-public-error-diagnostic-safety** — дрейф стабильных кодов/ARMок (`product.context_invalid` и др.).
- **product-category ×5 / product-taxonomy ×5 / verify-product-catalog-grpc-deployment** — refactor-drift (retirement-маркеры, TaxonomyTermKind-гарды, variant-identity проводка); детально не разбирались — кандидаты на следующий раунд.
- **line-item ×4** — следствие F6 (см. 11.2).

### 11.4. Общая перепись гейтов репо

Полный статический прогон всех 1217 verify-гейтов (без .test.mjs) на текущем дереве: **703 PASS / 514 FAIL**. Красная масса сконцентрирована вне Product-зоны (blog, forum, iggy, index-replay, social-graph, moderation, checkout-компенсации…) и включает evidence/harness-гейты, требующие инфраструктуры (postgres-раннеры, артефакты) — «красный» там не равен дефекту. Product-зона после раундов 8–11: остатки исчерпывающе перечислены в 11.3; критических дефектов в живых путях по-прежнему 0.

**Итог раунда 11:** 9 гейтов восстановлено (все — устаревшие пины против корректного кода), F6 (мёртвая граница line_item_resolution, ~500 строк + 4 гейта + тесты — решение за мейнтейнером), 20+ остатков классифицированы с корневыми причинами, инвариант wave18 подтверждён держащимся.

## 12. Раунд 12: глубокое код-ревью всех правок сессии (по команде «перепроверь»)

Метод: импорт-аудит каждого задетого файла (use → использования вне use-строк, с учётом trait-resolution для Digest/UnicodeNormalization); скан приватных элементов на осиротение (file-local + внешние потребители); контроль стыков (тройные пустые, осиротевшие атрибуты, trailing-ws); вэкуозность репинутых гейтов; побайтовая сверка компаньонов с pristine-базой.

### 12.1. Найдено и исправлено (2 дефекта моего F1-фикса)

1. **`product_error_policy` стал test-only.** После удаления `map_admin_product_error` (единственного прод-вызывателя) функция осталась только для parity-тестов domain↔port (:887/:927) → dead_code warning в прод-сборке. Фикс: `#[cfg(test)]` (функция сохранена как parity-оракул; якоря/маркеры гейтов не задеты).
2. **`AdminProductDiagnosticError` осиротел полностью.** Структура + Debug-impl («redacted») конструировались только удалённым маппером; port-маппер логирует bounded-facts (owner_code_length) без error-value. Фикс: удалены структура и impl (−7 строк, файл 953 строки) + синхронизирован verify-commerce-admin-product-diagnostic-safety (энд-якорь conversion-блока → `fn uuid_shape(`; блок diagnosticError с его маркерами/форбидами удалён).

### 12.2. Проверено чистым (действий не требует)

- **Импорты**: products.rs — все 23 use-цели живы (Digest — трейт для `Sha256::new().update()`; PaginationParams — поле ListProductsParams :704); 7 файлов F2 — ноль неиспользуемых (флаги Value/Uuid оказались ложными — таких импортов нет; UnicodeNormalization — `.nfc()`).
- **Сироты**: скан всех приватных элементов products.rs (18) и 7 файлов F2 — true-сирот 0; `prepare_*_and_schema_for_*` живы (commands.rs:218/:649), `resolve_product_metadata_with_schema` жив (projection.rs:805), `pick_response_translation` жив (:478), геттер `media_maintenance_port()` жив (admin/product_media.rs).
- **Сборочная целостность F1**: OpenAPI-реестр ссылается только на живые хендлеры (`store::list_products` / `admin::list_products` через glob re-export `pub use products::*`), не на удалённые shared; роуты admin/store резолвятся в собственные модули.
- **OpenAPI-контракт**: `Idempotency-Key` опубликован в utoipa и create (:160), и update (:304) — расширение functionSlice в command-port захватывает собственный атрибут каждого хендлера корректно.
- **Стыки**: 0 тройных пустых, 0 осиротевших атрибутов, 0 trailing-ws во всех 8 файлах.
- **Тест-модуль helpers**: product_metadata_tests — все имена резолвятся (2 use-super + super::-путь + собственные импорты); `entities::` живёт в другом тест-модуле (variant_title_tests), не задетом.
- **Вэкуозность гейтов**: репины содержательны — forbid-блоки на конкретные маркеры, комбинированные массивы с реальными значениями, adminSource-пины обоих списков на месте.
- **Компаньоны (154 файла, node --test поштучно)**: 16 красных файлов / 44 отказа — все с идентичными счётчиками на pristine-базе 58ccfb8; для 4 файлов, потенциально читающих мои правки (checkout-plan-product-projection-context, module-reference-contract, inventory-source-history, return-restock-identity), — идентичны и имена падающих тестов. **0 регрессий компаньонов за всю сессию.**

### 12.3. Финальное состояние

Объединённый прогон 57 гейтов (читатели всех задетых rust-файлов + все отредактированные гейты): **54 PASS / 3 FAIL** — все три документированные остатка мейнтейнера (pricing-identity-context — жертва реструктуризации; wave18 — стилистическое требование при держащемся инварианте; media-price — нереализованная фича). products.rs — 953 строки, скобки/парены сбалансированы.

## §13. Раунд 13: product-зонные красные — category ×5, taxonomy ×5, grpc-deployment, port-error пары

### 13.1. Category-пятёрка — born-red (unimplemented-slice, решение за мейнтейнером)

`verify-product-category-{closure-storage-retirement, closure-write-retirement, legacy-storage-retirement, legacy-write-retirement, seo-seam}` — все 5 красные, и красные **по рождению**:

- Гейты требуют donor-машинерию CAT-26…29: `list_categories_from_product_donor`, `should_write_legacy_category_translation`, `TaxonomyOwnerCategoryReader` в product и т.п. — маркеры отсутствуют **и в рабочем дереве, и на базе 58ccfb8** (worktree-проверка: 0 вхождений).
- `TaxonomyOwnerCategoryReader` существует только в тестах blog/forum — в rustok-product его нет → репин невозможен без имплементации целой фичи.
- Классификация: **unimplemented-slice** — гейты описывают незакрытый срез миграции категорий product→taxonomy, которого нет в коде. Не дефект правок аудита; вынесено мейнтейнеру (либо имплементировать срез, либо ретайровать гейты).

### 13.2. Taxonomy-пятёрка — репины по фактическим формам, 5/5 PASS

Прод-интеграция product↔taxonomy идёт **через порт** (`ports/taxonomy.rs`: трейт `ProductTaxonomyPort` — 4 метода; `Noop` fail-closed), реальная имплементация — `rustok-taxonomy/src/product_taxonomy_port.rs`.

| Гейт | Причина красноты | Репин (фактическая форма) |
|---|---|---|
| create-sync | Вызов и маппинг ошибок переехали в taxonomy-крейт | `sync_module_category_in_tx(` → `.sync_category_translation_in_tx(`; пины `TaxonomyError::Database(error) => CommerceError::Database(error)` и `Conflict("Taxonomy category conflicts with current state")` перенесены policy → `product_taxonomy_port.rs` (+ чтение файла в гейт) |
| read-cutover | Чтение категорий через порт | Порт-вызов как 2 маркера (`let owners = service` + `.load_scoped_categories(`); `TaxonomyScopeType` → строковые формы ×2; + `fn map_taxonomy_read_error(` |
| hierarchy-consumers | Parent-map стал path-based | Убраны owner-формы/Reader/`Some(&category_ids)`/map_err; добавлены `ProductCategoryPathRow::find_by_statement(` и `path_to_id.get(&row.path[..pos]).copied()` (effective_forms.rs :424/:463) |
| backfill | Миграция на raw-SQL, не SeaORM-ActiveModel | `TaxonomyTermKind::Category`/`TaxonomyScopeType::Module` → фактические raw-гварды `existing.kind != "category"` / `existing.scope_type != "module"`; `taxonomy_term_translation::Entity::find_by_id(translation.id)` → `SELECT term_id FROM taxonomy_term_translations WHERE id = $1` + параметр; `slug: Set(route_key.clone())` → `route_key.clone().into(),`; `normalize_term_locale`/`normalize_term_route_key` → `exact_taxonomy_locale(`/`exact_taxonomy_route_key(`; `taxonomy_term_route_key::Entity::find()` → `ensure_route_owner(...).await?`; `translation_change::ActiveModel` → `ensure_translation_change(...).await?`; `taxonomy_category_hierarchy::Entity::find_by_id` → `ensure_category_hierarchy(txn, category).await?` |
| directory-order | Donor-функции нет — конец среза теперь sync-функция | Энд-якорь `async fn list_categories_from_product_donor(` → `async fn sync_created_category_to_taxonomy_in_tx(` (проверено: compose → sync, между ними hierarchy_order/append_subtree — все 9 маркеров среза живы) |

Дополнительно: string-гард compose (:431) `owner.scope_type != "module"` + `owner.scope_value.as_deref() != Some(PRODUCT_TAXONOMY_SCOPE)` — **строже** старого пина гейта (инвариант держится).

### 13.3. grpc-deployment — дрейф клона, PASS

Единственный отказ: `.with_variant_identity_read_port(provider)` — фактическая форма в `apps/server/src/services/product_catalog_deployment.rs:142` `.with_variant_identity_read_port(provider.clone())` (runtime затем мутируется `.with_attached_values_read_port(provider)` — move последнего клона). Репин маркера; компаньон `.test.mjs` синхронизирован (canonical-фикстура) → **9/9 PASS**.

### 13.4. Port-error пары (сверх плана раунда) — 2/2 PASS

- **catalog-port-error-safety**: гейт читает `src/ports/` (монолит `ports.rs` расщеплён), но 2 маркера (`code = "product.context_invalid"`, `"product operation could not be completed safely"`) живут в командном порту `catalog_command_port.rs` (:590/:516) — и на базе тоже; read-set гейта файл никогда не покрывал. Репин по прецеденту create-sync: в гейт добавлено чтение `catalog_command_port.rs`, оба маркера скоуплены на него. Остальные маркеры ports/ подтверждены: per-arm стабильные коды (`product.database_unavailable`, `product.duplicate_handle`, …) + correlation-aware логирование в types.rs/diagnostics.rs.
- **public-error-diagnostic-safety**: `CommerceError::Conflict(_) =>` → фактическая форма `CommerceError::Conflict(_) | CommerceError::VariantAxisPolicyConflict(_) =>` (public_error.rs:114/:233 — покрывает больше, строже пина).

### 13.5. Дифф остаточных красных с базой (закрытие вопроса §12)

Побайтовое сравнение отказов (рабочее дерево vs worktree 58ccfb8) для 7 гейтов: admin-fallback-mutation, audit-wave18, storefront-media-price, ui-grid-facets, graphql-product-retryability, commerce-product-admin-graphql-read, commerce-product-graphql-lifecycle-command-cutover — **все 7 IDENTICAL**. Ноль дрейфа отказов от правок раундов 4–13.

### 13.6. Итог раунда 13

- Taxonomy ×5 → **PASS** (5 репинов, все на живые фактические формы).
- grpc-deployment → **PASS** (+ компаньон 9/9).
- catalog-port-error-safety, public-error-diagnostic-safety → **PASS** (сверх плана).
- Category ×5 → born-red CAT-26…29 (unimplemented-slice, мейнтейнер).
- Product-зона (verify-product-* + verify-commerce-product-* + graphql-product-retryability = 78 гейтов): **66 PASS / 12 RED** = 5 born-red (category) + 7 баз-идентичных остатков мейнтейнера (media-price, wave18, ffa/admin-fallback, ui-grid, admin-graphql-read, graphql-lifecycle-cutover, retryability — все без дрейфа с базы; pricing-identity-context — pricing-зона, вне этих 78, статус прежний: баз-идентичный остаток).
- Компаньоны изменённых гейтов: единственный затронутый (grpc-deployment.test.mjs) синхронизирован, 9/9; у taxonomy-пятёрки и port-error пар компаньонов нет.

## §14. Раунд 14: разбор «остатков мейнтейнера» — 8 гейтов репинено, классификация §13.6 пересмотрена

Раунд 13 закрыл «остатки за мейнтейнером» как баз-идентичные. Раунд 14 показал: баз-идентичность ≠ нереализованность — grpc-deployment и port-error пары раунда 13 уже доказали этот паттерн. Протокол тот же: корневая причина → живая форма → репин либо обоснованная классификация.

### 14.1. graphql-product-retryability — конверт переехал к владельцу, 29 маркеров

Дизайн сменился: мутации идут через **командный порт** (`product_command_port_error(&port_context, error, op)` в mutations/catalog.rs, 24 вызова; терминальность — `product_catalog_command_error_is_terminal`), запросы/фасеты — через `map_product_service_error` → **`rustok_product::map_product_public_error`** (public_error.rs:170, полный типизированный конверт `(message, code, retryable)` + bounded-логирование + boundary). Репин: конверт-маркеры → public_error.rs (Database/PRODUCT_TEMPORARILY_UNAVAILABLE/true; NotFound/DuplicateHandle/DuplicateSku/NoVariants/CannotDeletePublished/Core-fallback PRODUCT_OPERATION_FAILED; живые инварианты Conflict|VariantAxisPolicyConflict и InventoryQuantityBlocked); extensions → `public.code`/`public.retryable` в mod.rs-делегации; запросы переехали query.rs → product_catalog.rs (`storefront_/admin_product_catalog_input`, 6 вызовов). Устаревшие маркеры (InvalidPrice, InsufficientInventory, InvalidOptionCombination, ShippingProfile ×2) сняты — срезы перераспределены (pricing-коллаборатор, inventory-политики, axes-policy).

### 14.2. storefront-media-price — фича РЕАЛИЗОВАНА, классификация §12/§13.6 снята

10 маркеров разошлись по трём слоям, все живы:
- **Типы** → крейт портов (`ports/src/types.rs:162-169-177-279`: StorefrontProductListImage/Price, primary_image, price_from, currency_code — дословно).
- **From-цена** → pricing-порт (`product_pricing_port.rs`): `port.load_storefront_list_prices(` в queries.rs:202; комбинированный гвард `price_list_id.is_some() || price.min_quantity.is_some()`; валюта `eq_ignore_ascii_case`; `.min_by(|l, r| l.amount.cmp(&r.amount))` — минимальная цена страницы.
- **Нативный маппинг** → `catalog_list_native.rs:86-87/:173/:183` (`map_owner_product_image`, `map_owner_product_list_price`) — гейт читал только native_server_adapter.rs.
Компаньон синхронизирован: +3 фикстурных файла (ports-типы с тумблером includePriceField, pricing-порт, list-native), queries-фикстура → `port.load_storefront_list_prices()` → **10/10**.

### 14.3. audit-wave18 — пре-инсертный вариант мёртв, живой инвариант строже

Единственный отказ — позиционный: гейт требовал `validate_new_product_publish_requirements_in(` ДО вставки. Факт: эта функция (values.rs:293) и её обёртка (:282) — **0 вызывателей** (мёртвый код); живая проверка — пост-инсертная `validate_product_publish_requirements_in(&txn, tenant_id, product_id)` (commands.rs:500, внутри txn, под `if input.publish`, до `txn.publish(`), валидирует **персистед-состояние** (сильнее пре-инсертной проверки инпута). Репин всех трёх ассертов publish-секции на живые формы; absent-маркер → запрет pool-вызова `(&self.db`. Наблюдение мейнтейнеру: `validate_new_product_publish_requirements*` — кандидат на удаление.

### 14.4. admin-graphql-read + graphql-lifecycle-command-cutover — расщепление крейта портов

- **admin-graphql-read**: гейт читает `src/ports.rs` → fallback `src/ports/*`; 2 маркера (`"product.legacy_admin_list_unavailable"`, `pub struct LegacyAdminProductsRequest`) живут в крейте `rustok-product/ports/src/` (ports.rs:123, types.rs:102) — добавлены чтения обоих файлов, маркеры скоуплены.
- **lifecycle-command-cutover**: `pub trait ProductCatalogCommandPort` → ports/src/ports.rs:147 (в catalog_command_port.rs — реэкспорт + impl); хост-композиция — `ProductCatalogCommandRuntime::in_process_with_collaborators(` (commerce_provider_runtime.rs:447, 4 коллаборатора: pricing+inventory+taxonomy — богаче пина `in_process_with_pricing`;后者 жив в graphql_runtime.rs:337 и закреплён там отдельным маркером).

### 14.5. admin-fallback — модульная структура иная с базы (born-red маркеры)

7 маркеров не соответствовали даже базе 58ccfb8: `#[path]`/`mod`-декларации живут в transport.rs:10-11 (`pub mod graphql_fallback_mutations;`), фасад реэкспортирует через `crate::transport::` (не голое `use graphql_fallback_mutations::`); `TenantUserScopedVariables` — тип ORDER-крейта (product-аналог: `TenantScopedVariables<T>`, graphql_adapter.rs:84). Репин по живым формам. `save_product_attribute_values`: санитизированный враппер жив (graphql_fallback_mutations.rs:201) и пинится гейтом в блоке wrappers, но фасадом не смонтирован (0 вызывателей из UI — grep apps/admin + admin/src чист) — наблюдение мейнтейнеру.

### 14.6. ui-grid-facets — i18n-изация кнопки

`"Load more"`/`aria-label="Load more rows"` → `messages.get_value().text("grid.loadMore")` (leptos :163/:171) / `messages.text("grid.loadMore")` (dioxus :96/:99). Репин обоих наборов; компаньоны чисты (ui-grid-facets 16/0, ui-grid-parity 10/0).

### 14.7. pricing-product-identity-context — реструктуризация крейта, read-set + 7 контентных слоёв

6 из 27 sourcePaths указывали на расщеплённые файлы (services/pricing.rs → dir; ports.rs → dir; read/write_context.rs → integrations/; public_error.rs → error.rs; admin native_server_adapter.rs → dir). Лоадер расширен **разделяемым экспортом `readSource(root, relative)`** (файл или конкатенация .rs директории) — используется и гейтом, и компаньоном. Контентные репины: `.with_variant_identity_read_port(catalog.clone())` в `in_process_with_flex_schema` (не `in_process`); `pub tenant_id: Uuid` → ports/src/types.rs:138 (новый ключ identityTypes); `.map_err(projections::pricing_identity_port_error)`; before-маркеры `entities::price::Entity::find` → `price::Entity::find()` (стиль импорта projections.rs); фабричный require → `pub fn in_process_pricing_${kind}_port(` в integrations-обёртке (старый module-path фасад упразднён). Счётчик `read_with_deadline` — ровно 8 ✓. Компаньон: 4 мутации пересинхронизированы (identityTypes-ключ; bypass-мутация → срыв `.with_product_catalog_read_runtime(product_runtime)` в PricingService::new; retryable-мутация → replace-all по двум вхождениям; канал-маркер гейта усилен `;` — мутация `.or(context.channel)` стала детектируемой) → **13/13**.

### 14.8. Итог раунда 14

- 8 гейтов репинено → **PASS**: wave18, admin-graphql-read, graphql-lifecycle-command-cutover, graphql-product-retryability, admin-fallback, storefront-media-price, ui-grid-facets, pricing-identity-context.
- Компаньоны: pricing-identity 13/13, media-price 10/10 (синхронизированы); ui-grid 16/0 + 10/0; у остальных изменённых гейтов компаньонов нет.
- **Product-зона (79 гейтов): 74 PASS / 5 RED** — исключительно born-red category-пятёрка (CAT-26…29, unimplemented-slice, мейнтейнер).
- Классификация §13.6 «7 баз-идентичных остатков мейнтейнера» пересмотрена: все 7 оказались дрейфнувшими пинами живых инвариантов (+pricing-identity). Урок раунда: баз-идентичность отказа — необходимое условие отсутствия регрессий, но не классификация причины; репин требует grep живых форм.
- Наблюдения мейнтейнеру (не чинилось): мёртвый `validate_new_product_publish_requirements*` (values.rs:282/:293); несмонтированный `save_product_attribute_values` в фасаде admin.

## §15. Раунд 15: ревью раундов 13–14, полная сертификация компаньонов, gap-спек born-red пятёрки

### 15.1. Дифф-ревью правок гейтов раундов 13–14 (ослаблений нет)

Поштучный аудит диффов 11 гейтов + 3 компаньонов против 58ccfb8:
- **taxonomy ×5**: все замены — эквивалентные или более сильные формы (Entity-маркеры → полные вызовы `ensure_route_owner/ensure_translation_change/ensure_category_hierarchy(...).await?`; коллизионный гвард усилен вторым маркером-параметром `translation.id.into(),`); счётчики и retired-форбиды не тронуты; read-cutover-гард стал строже (строковые проверки `!= "module"` + `scope_value` вместо одного enum-сравнения).
- **grpc-deployment**: единственный маркер `provider` → `provider.clone()` (дрейф клона), компаньон синхронизирован зеркально.
- **retryability** (полная перезапись): все 3 счётчика сохранены (databaseBlock-regex, retryableTrue === 1, retryableExtension === 1 — перенацелены на фактические источники); legacy-forbid сохранён; снятые 10 маркеров (InvalidPrice, InsufficientInventory, InvalidOptionCombination, ShippingProfile ×2, Rich, VARIANT_NOT_FOUND-без-префикса) ссылались на **несуществующие варианты `CommerceError`** (enum = 15 вариантов, матч исчерпывающий) — потерь инвариантов нет; `PRODUCT_VARIANT_NOT_FOUND` точнее старого подстрочного `VARIANT_NOT_FOUND`.
- **wave18/admin-graphql-read/lifecycle/admin-fallback/media-price/ui-grid/pricing-identity**: репины по живым формам, добавления read-set'ов без удаления проверок; в admin-fallback wrappers-цикл сохраняет ВСЕ operationNames (включая несмонтированный save_product_attribute_values).
- **Усиление в этом раунде**: wave18 `gateAt` ужесточён до `if input.publish {\n            schema` — раньше ловился `status: Set(if input.publish {` (:247) вместо настоящего гейта (:497). Гейт остался зелёным.

### 15.2. Полная сертификация компаньонов: 0 новых красных за всю сессию

Побайтово-посчетный прогон всех 154 .test.mjs в рабочем дереве и в worktree 58ccfb8:

| | база 58ccfb8 | рабочее дерево |
|---|---|---|
| Красные файлы | 20 | 15 |
| Сумма отказов | 51 | 44 |

- **Новых красных: 0** (комм по именам файлов).
- Исцелено за сессию: 5 файлов — inventory-catalog-deletion / inventory-quantity-writes / inventory-source-delivery (раунд 9), product-next-data-table-host (раунд 8), pricing-product-identity-context (раунд 14).
- Все 15 общих красных — **счётчики отказов идентичны базе** (пофайловый join).
- Консистентность с §12: «16 красных / 44 отказа» раунда 12 = база 20/51 минус 4 файла/7 отказов, исцелённых к тому моменту; раунды 13–14 сняли ещё 1 файл (pricing-identity). Цепочка 20→16→15 сходится без противоречий.

### 15.3. Gap-спек born-red пятёрки для решения мейнтейнера

Уточнение классификации §13.1: план-документ `rustok-taxonomy/docs/implementation-plan.md:384-393` **содержит историю слайсов** (PR #3738/CAT-26 — перенос name/localized, PR #3739/CAT-27 — изоляция meta_title/meta_description, PR #3740/CAT-28 — остановка legacy-записей, CAT-29 — физическое retirment catalog_category_translations), но смёрженный код принял **другую архитектуру**: Postgres-only портовую интеграцию (порт + Noop fail-closed, миграция m20260829_000016 отказывает на не-Postgres) вместо мульти-бэкенд дизайна гейтов. Гейты пиннят непринятый дизайн → born-red. Конкретный gap по слайсам:

- **CAT-26** — `TaxonomyOwnerCategoryReader` в product: не имплементирован (ридер существует только в тестах blog/forum); чтение идёт через `ProductTaxonomyPort.load_scoped_categories`.
- **CAT-27** — SEO-шов: частично реализован иначе — product-owned таблица `catalog_category_seo_translations` + `write_category_seo_translation_in_tx` (categories.rs:525). **Наблюдение**: на не-Postgres и без SEO-полей запись тихо no-op'ится (`return Ok(())`, :531-534) — потенциальная потеря данных на не-Postgres деплоях; гейт требовал fail-closed отдельного донор-retirement-слайса.
- **CAT-28** — `should_write_legacy_category_translation(backend)`: guard не существует; портовый путь безусловен (мульти-бэкенд ветки нет).
- **CAT-29** — физическое retirment донора: create-путь уже не пишет `catalog_category_translations` (подтверждено retired-форбидами create-sync PASS), но «retained non-PostgreSQL donor read/write» (`list_categories_from_product_donor`, `FROM catalog_category_translations`, closure-записи для SQLite/MySQL) не имплементирован — не-Postgres не поддерживается вовсе.
- Гейты также пиннят план-курсоры точными фразами («TAXONOMY-CAT-29 is the bounded PostgreSQL physical donor-retirement slice», canonical-key ownership guard), которых в план-документе нет даже при наличии истории слайсов.

Решение за мейнтейнером: (a) имплементировать мульти-бэкенд совместимость (донор-путь для SQLite/MySQL + fail-closed SEO-шов) под существующие гейты, либо (b) ретайровать пятёрку как описывающую отменённый дизайн, либо (c) зафиксировать Postgres-only контрактом и переписать гейты под него.

### 15.4. Финальное состояние аудита

- **Product-зона: 79 гейтов = 74 PASS / 5 RED** (исключительно born-red category-пятёрка, §15.3).
- **Компаньоны: 154 файла, 0 новых красных за сессию, 5 исцелено, дрейф счётчиков 0** (§15.2).
- Rust-источники: правки только раундов 4–12 (19 файлов, задокументированы §4–12); раунды 13–15 — только гейты/компаньоны/отчёт.
- Открытые наблюдения мейнтейнеру: мёртвый `validate_new_product_publish_requirements*` (values.rs:282/:293); несмонтированный `save_product_attribute_values` в фасаде admin; SEO silent no-op на не-Postgres (§15.3); born-red пятёрка (три опции §15.3); F3/F4/F6 — без изменений.

## §16. Раунд 16: источнико-уровневая сертификация + диагностика ffa-contract

### 16.1. Сертификация «читателей изменённых исходников» (дополнение §15.2)

Все 64 гейта, чей read-set пересекается с 19 изменёнными .rs-файлами (правки раундов 4–12; ports/src/types.rs — только док-комментарии), прогнаны в рабочем дереве и в worktree 58ccfb8 с побайтовым сравнением выходов:

- **31 расхождение**: 30 = отказ→успех (задокументированные фиксы раундов 4–14: идемпотентность/receipts, RU→EN, richtext, tag-hydration миграция, category-sort, inventory ×3, admin-detail-read, channel-visibility, diagnostic-safety синк, wave18/admin-graphql-read/media-price/pricing-identity репины), 1 = ffa-contract (§16.2).
- **0 регрессий**: ни один зелёный на базе гейт не стал красным.
- Итог по уровням: компаньоны (§15.2) + читатели исходников (§16.1) — двойная сертификация «0 регрессий за сессию».

### 16.2. verify-frontend-host-ffa-contract — born-red: неимплементированный frontend-фронт

Единственное «подозрительное» расхождение сертификации оказалось ложным (ENOENT-краш идентичен базе, дифф — только префикс пути). Диагноз:

- Гейт читает 87 путей; **отсутствуют 19** (и на базе тоже): **admin workflow-UI** — 11 файлов (`features/workflow/{mod,model,api}.rs`, `transport/{mod,graphql_adapter,native_server_adapter}.rs`, `components/{workflow_step_editor,version_history,template_gallery}.rs`, `pages/{workflows,workflow_detail}.rs`); **admin cache-UI** — 5 файлов (`features/cache/{mod,model}.rs`, `transport/{mod,native_server_adapter}.rs`, `pages/cache.rs`); плюс `features/installer/api.rs`, `features/oauth_apps/api.rs`, `storefront/shared/context/native_server_adapter.rs`.
- Backend-модули при этом **живы** (`rustok-cache/admin` с contract_tests/core/i18n; workflow-модуль с verify-workflow-* гейтами) — отсутствует именно админ-frontend.
- Гейт fail-fast падает на первом ENOENT (:149), не оценивая остальные 68 существующих путей.
- Классификация: **born-red / unimplemented-slice** (одно семейство с category-пятёркой §15.3). Решение за мейнтейнером: имплементировать workflow/cache админ-UI под контракт гейта, либо ретайровать/сузить гейт до реализованной поверхности.

### 16.3. Кампания правок — сводка за всю сессию (раунды 1–16)

**Аудит (§1–3):** 1×P1 (заголовок варианта), 5×P2, 3×P3 + наблюдения F1–F6. **Фиксы (§4–10):** идемпотентность write-journal (+ .test.mjs), RU→EN строки, fail-closed richtext, apps/admin источники, data-table-host, свепы 1175×2+49, tag-hydration миграция, category-sort фикстуры, F1 (−6 строк products.rs), F2 (−14 API, −251 строка). **Восстановление гейтов (§9–14):** 11 (р.9) + 4 (р.10) + 9 (р.11) + 8 (р.13) + 8 (р.14) = **40 гейтов репинено/закрыто**, все — на живые фактические формы при держащихся инвариантах. **Ревью (§12, §15, §16):** импорты/сироты/скобки чисты, компаньоны 154 = 0 новых красных (5 исцелено), читатели исходников 64 = 0 регрессий, цепочка красных компаньонов 20→16→15 сходится.

**Финальное состояние:** product-зона **79 гейтов = 74 PASS / 5 born-red** (category-пятёрка, §15.3); ffa-contract — born-red вне product-зоны (§16.2); компаньоны — 15 красных, все баз-идентичны. Rust-правки — 19 файлов (раунды 4–12), задокументированы построчно; раунды 13–16 — только гейты/компаньоны/отчёт. Открытые пункты мейнтейнера: category ×5 + ffa-contract (имплементация vs ретайр), мёртвый `validate_new_product_publish_requirements*`, несмонтированный `save_product_attribute_values`, SEO silent no-op на не-Postgres, F3/F4/F6.

## §17. Раунд 17: глобальная финальная сертификация — весь репозиторий, все зоны

Полный прогон всех 1186 верификационных гейтов (verify-*.mjs без .test.mjs) в рабочем дереве и в worktree 58ccfb8, сравнение по exit-коду:

| | база 58ccfb8 | рабочее дерево |
|---|---|---|
| PASS | 661 | **707** |
| RED | 525 | 479 |

- **Регрессии: 0.** Ни один гейт, зелёный на базе, не стал красным — по ВСЕМ зонам репозитория (product, commerce, cart, order, payment, fulfillment, blog, forum, iggy, index, social-graph и пр.), а не только в product-зоне. Тройная сертификация замкнута: компаньоны (§15.2) → читатели исходников (§16.1) → полный корпус гейтов (§17).
- **Улучшения: +46, все задокументированы.** Полный список красный→зелёный совпадает с фиксовой кампанией §4–§16 без посторонних имён: раунды 4–10 (идемпотентность/receipts, RU→EN, richtext, tag-hydration, category-sort, inventory ×3, data-table-host, operation-results, variant-axes-ui, catalog-schema-порты, facet-chain, lifecycle-events, http-error-safety ×3, rest-cutover ×2, index-product ×5, text-like-filter, command-port, admin-detail-read) + раунды 11–12 (diagnostic-safety, channel-visibility) + раунд 13 (taxonomy ×5, grpc-deployment, catalog-port-error, public-error) + раунд 14 (wave18, admin-graphql-read, lifecycle-cutover, retryability, admin-fallback, media-price, ui-grid-facets, pricing-identity).
- Оставшиеся 479 красных — вне product-зоны (другие модули, инфраструктурные/evidence-гейты) либо классифицированные born-red (category ×5 §15.3, ffa-contract §16.2); их статус не изменён сессией.

### Итоговая формула кампании

**661 → 707 PASS / 0 регрессий / +46 документированных закрытий; product-зона 74 из 79 (5 born-red с gap-спеками); компаньоны 20→15 красных файлов, 0 новых; 19 .rs-файлов точечных фиксов (§4–12); 40+ гейт-репинов на живые формы (§9–14).** Аудит product-модуля и сопутствующая фиксовая кампания завершены; всё оставшееся — решения мейнтейнера с подготовленными опциями.

## §18. Раунд 18: код-ревью собственных правок (по требованию) — вердикт: дефектов нет

Поштучное ханк-за-ханком ревью всех правок сессии: 19 .rs-файлов (1 532+/823−), ~61 гейт/компаньон .mjs, untracked-тест. Метод: дифф против 58ccfb8 + чтение финального кода в контексте вызывателей.

### 18.1. Rust-источники — проверено и подтверждено

- **P1 (заголовки вариантов)**: `sync_variant_auto_titles_in_tx` — корректная и консервативная механика: строка считается авто-только если пуста или равна предыдущему авто-значению (плюс консистентный "Default"-фолбэк в `variant_auto_titles_by_locale_in_tx`), авторские переводы не трогаются; `current != next_title` отсекает no-op update; create/create_variant/добавление локали сеют читаемые заголовки батчами (`load_variant_display_titles` — N+1 устранён; порядок биндов проверен для обеих веток: SQLite — текстовый порядок `?`, Postgres — нумерованные `$n` при `format!`-маппинге `$1..$4+`; `detached_at IS NULL` учитывается; ORDER BY детерминирован).
- **Subtree-фильтр категорий**: `WITH RECURSIVE` + `LIMIT MAX+1` + отказ при превышении; пустой subtree → `1 = 0` (fail-closed); бинды корректны для SQLite/Postgres.
- **Media-компенсация**: `compensate_retained_media_hold` ставится в очередь вне упавшей транзакции только при **definite rollback** (Err из `stage_product_image_addition_in_tx`); неоднозначный сбой commit намеренно не компенсируется (задокументировано инлайн); ошибки компенсации логируются, не всплывают.
- **Валидация количества**: `parse_product_admin_quantity_input` → `Result` (только ASCII-цифры, без негатива/переполнения) + `set_error`-баннер + красная рамка инпута + строковое состояние поля (частичный ввод не уничтожается). Тест покрывает 6 кейсов.
- **Анти-фабрикация данных** (storefront entities): «Hardware»/4.9/36 отзывов → рендер только при фактическом наличии.
- **Сатурация ревизий**: `unwrap_or(0)` → `unwrap_or(i64::MAX)` (ревизии ≤ 0 невалидны — переполнение не должно маскироваться под 0).
- **DuplicateSku**: пустой SKU → «unknown» вместо «Duplicate SKU: ».
- **Удалённое мёртвое**: 5 metadata-хелперов (`split/merge/prepare_*` без `_and_schema`) — 0 ссылок, преемники `prepare_*_and_schema_for_*` используются (commands.rs ×2); `with_media_maintenance_port` — поле инициализируется в 5 внутренних билдерах runtime, геттер жив; `normalize_storefront_currency_code` (`#[allow(dead_code)]`) — нормализация ушла в pricing-порт.
- **Поиск (P2 №3)**: остаётся `LIKE` без ILIKE и без экранирования — **подтверждаю корректность отложенного статуса**: экранирование расходится с shadow-адаптером (`FilterExpr::TextLike`, осознанная wildcard-семантика, 5+ гейтов + reference-тест), синхронный скоуп owner+shadow+index обязателен; статус «регистронезависимость не реализована» в отчёте честен.

### 18.2. Гейты/компаньоны/untracked

- `node --check` — все изменённые .mjs синтаксически валидны; скан на мусор (`console.log`/`TODO`/`dbg!`/`println!`) — чисто.
- Баланс скобок/паренов (string/comment/lifetime-aware сканер) — 19/19 файлов нулевой дисбаланс.
- **secondary-write-idempotency.test.mjs** (untracked): синтаксис OK, встроен в `"test": "node --test src/*.test.mjs"` пакета next-admin/rustok-product, импортирует `write-attempt.ts` (UNKNOWN_PRODUCT_WRITE жив). В песочнице не запускается: транзитивный импорт `@rustok/next-fluent` (github-pinned зависимость, нет node_modules) — **как и 10 из 13 соседних .test.mjs того же пакета** (PASS только у 3, не импортирующих TS-модули с зависимостями). CI-side тест, не дефект.

### 18.3. Итог ревью

**Дефектов в правках сессии не найдено; чинить нечего.** Два «подозрения» при проверке оказались задокументированными корректными решениями (отложенный поиск-фикс — shadow-parity; нераннability теста — env). Поведенческие сертификаты раундов 15–17 (компаньоны 0 новых красных, читатели исходников 0 регрессий, полный корпус 661→707/0 регрессий) согласуются с этим вердиктом на трёх независимых уровнях.

## §19. Полная функциональная верификация: что нужно сделать (ранбук)

Статическая сертификация замкнута (§15–18: корпус 1186 гейтов, компаньоны, читатели, код-ревью). **Живой** прогон всего функционала требует окружения, которого нет в этой песочнице (нет `cargo`, `docker`, `npm ci` с github-pinned зависимостями) — ранбук ниже для машины мейнтейнера или CI. Инфраструктура в репо уже готова, ничего строить не надо.

### 19.0. Предусловия (один раз)

- Rust stable + `cargo nextest` (`cargo install cargo-nextest`), Node 22+, Docker.
- `cd apps/next-admin && npm ci` — подтянет github-pinned `@rustok/next-fluent` (нужен доступ к GitHub; именно его отсутствие валит 11/14 .test.mjs в песочнице).
- `cp .env.dev.example .env.dev` (Postgres `db:5432/rustok_dev`, сервер `:5150`, JWT-секреты).

### 19.1. Уровень 1 — юнит-тесты Rust (13 244 теста, ~первые часы)

```
make rust-test        # cargo nextest run --workspace --all-targets --all-features
make rust-doc-test    # cargo test --workspace --doc --all-features
```

БД **не нужна** — интеграционные тесты идут на `sqlite::memory:`. Это закрывает большую часть правок сессии на рантайм-уровне: P1-заголовки вариантов, subtree-фильтр категорий, Media-компенсацию, quantity-валидацию, сатурацию ревизий, фасеты, миграцию 13_000014.

### 19.2. Уровень 2 — пакеты next-admin

```
cd apps/next-admin
npm run test:product   # 14 .test.mjs packages/rustok-product (включая secondary-write-idempotency)
npm run typecheck && npm run lint:strict
```

### 19.3. Уровень 3 — живой стек

```
./scripts/dev-start.sh          # = docker compose --profile full-dev up
curl http://localhost:5150/health
```

Поднимает: PostgreSQL (+bootstrap, миграции применяются автоматически), Axum-сервер `:5150` (GraphQL `/api/graphql`), Next-admin `:3000`, Leptos-admin `:3001`, Next-storefront `:3100`, Leptos-storefront встроен в `:5150`.

### 19.4. Уровень 4 — функциональная smoke-матрица (через UI :3000 или GraphQL)

| # | Домен | Сценарий | Что проверяет (правка сессии) |
|---|-------|----------|-------------------------------|
| 1 | Категории | Дерево >2 уровней, primary-binding товара, фасетный фильтр по вложенной категории; тест-категория с >500 потомков → валидационный отказ | subtree-LIMIT, fail-closed |
| 2 | Товары | Создание с переводами (2+ локали), publish-гейты, редактирование custom fields, reserved metadata | prepare_*_and_schema_* |
| 3 | **Варианты (P1)** | Создать вариант → заголовок = проекция опций осей; переопределить заголовок вручную; переименовать опцию → **авторский** заголовок не тронут, **пустой/авто** — обновлён; открепить опцию → пропала из заголовка | sync_variant_auto_titles |
| 4 | Медиа | Добавить изображения, позиции, alt по локалям; оборвать добавление (например, не-валидный ввод) → hold не «висит» | compensate_retained_media_hold |
| 5 | Инвентарь/цены | Ввести `12a`/`-3` в quantity → красная рамка + баннер, отправка заблокирована; дубликат SKU с пустым значением → «unknown», не «Duplicate SKU: » | parse_product_admin_quantity_input |
| 6 | Поиск | Поиск по названию на **Postgres**: известное ограничение — кейс-чувствительность и wildcard-семантика `%`/`_` (осознанно отложено, §18.1) | — |
| 7 | Сторфронт | Карточка товара без категории/рейтинга → бейджа и звёзд нет (никаких «Hardware»/4.9/36) | анти-фабрикация |
| 8 | Shadow-индекс | Наблюдать логи storefront-shadow при операциях 1–7 — расхождений с owner-запросами быть не должно | паритет TextLike |

### 19.5. Уровень 5 — браузерные E2E

```
cd apps/next-admin && npm run test:e2e:install && npm run test:e2e
```

25 готовых спеков (fly-editor, smoke, forum, page-builder). Продуктовых e2e-спеков на момент написания не было — **добавлены в §20.2**: каталог `tests/product-admin/` + скрипт `npm run test:e2e:product` (против живого стека, §20.3).

### 19.6. Порядок и стоимость

1. `make rust-test` — максимальное покрытие за минимальные усилия, старт здесь.
2. `npm ci` + `test:product` + `typecheck`.
3. Живой стек + smoke-матрица 19.4 (Postgres-специфика: поиск, миграции, SEO-шов).
4. Playwright E2E (опционально, после стабилизации).

## §20. Статический обход цепочек функционала + продуктовые E2E-спеки

### 20.1. Статическая карта цепочек (каждый хоп проверен grep-ом с координатами)

Метод: для каждого домена прохожу всю цепочку «UI → транспорт → сервер-роут → runtime → порт → сервис-владелец → таблицы» и подтверждаю, что каждое звено существует и вызывается. Мёртвых хопов **не обнаружено**.

**A. Создание/сохранение товара** (next-admin, RSC + server actions):
```
packages/rustok-product (ProductEditorPage, actions)
  → @/shared/api/graphql.ts:13  fetch(NEXT_PUBLIC_GRAPHQL_ENDPOINT ?? '/api/rustok/graphql')
  → src/app/api/rustok/graphql/route.ts:3  прокси на RUSTOK_API_URL ?? http://127.0.0.1:5150
  → Axum :5150 /api/graphql
  → graphql/mutations/catalog.rs:433  create_product (RBAC: PRODUCTS_CREATE, module-gate)
  → catalog.rs:43 product_command_runtime → graphql_runtime.rs:337
      ProductCatalogCommandRuntime::in_process_with_pricing
  → catalog_command_port.rs:23 (порт) → services/catalog/commands.rs:158 create_product (владелец)
  → products + product_translations + metadata (миграция 13_000014) + outbox-события
```
Особенность архитектуры: страницы — React server components, GraphQL-вызовы происходят **на Next-сервере**, мутации — через server actions (`app/dashboard/product/actions.ts`). Поэтому продуктовые E2E обязаны идти против живого стека — mock на уровне браузера цепочку не покрывает.

**B. Варианты**: `catalog.rs:678 create_product_variant` → `command_port().create_variant` → `commands.rs:1342 create_variant` (+ `set_variant_axes` порт `catalog_command_port.rs:59`). Здесь живёт P1-механика: `sync_variant_auto_titles_in_tx` (commands.rs:2365) + `load_variant_display_titles` (projection.rs:292). UI: `product-variants-card.tsx` («Pricing & Variants Matrix»: SKU, штрихкоды, мультивалютная сетка, сток).

**C. Атрибуты**: `catalog.rs:1422 create_product_attribute` (RBAC: PRODUCTS_MANAGE) → `product_schema_write_port(...).create_attribute` → `catalog_schema_service/attributes.rs:41`. UI: `components/attributes/*` (7 карточек/диалогов).

**D. Категории**: UI `api/categories.ts` (мутации :37 CreateCatalogCategory, :50 CreateCategoryGroup, :63 SetCategorySchemaMode, :74 BindCategoryAttribute) → `catalog.rs:1522/:1643` → сервис категорий; primary-binding товара → `validate_primary_category` (free fn, commands.rs); фасеты → `facets.rs` + `helpers.rs load_category_filter_subtree` (WITH RECURSIVE, LIMIT 500).

**E. Медиа**: `products.ts:213 addProductImage` → `catalog.rs:778 add_product_image` → `commands.rs:1862` → retain hold → `stage_product_image_addition_in_tx` (:1942) → компенсация при rollback (`:1917` → `media_reference.rs compensate_retained_media_hold`). UI: `product-media-card.tsx` («Media Gallery»).

**F. Чтение/список/поиск**: `query.rs:1876 products(` (module-gate + channel-scope) → `product_read_runtime` → `queries.rs` (листинг) + `admin_product_title_search_condition` (:210). UI-лист: `product-table/columns.tsx` (колонка «Product», фильтры статусов Published/Active/Draft/Archived, пагинация через ?page/?perPage).

**G. Storefront**: главная `apps/storefront/src/app/mod.rs` → `shared::local::featured_products` (лок. набор) при включённом модуле product; реальный каталог — через storefront read-runtime (`graphql_runtime.rs:325 in_process_with_pricing`), Leptos-сторфронт встроен в :5150; карточка `entities/product/mod.rs` — здесь анти-фабрикация (§18.1).

**H. Leptos-admin**: `apps/admin/features/modules/transport/native_server_adapter.rs` — reqwest HTTP-запросы к серверу (registry governance), с явной ошибкой на не-2xx.

**Вывод**: все 8 цепочек непрерывны; правки сессии сидят на живых путях (create_product — переводы/метадата; вариантная ветка — P1; медиа — компенсация; поиск — известное PG-ограничение §18.1).

### 20.2. Продуктовые E2E-спеки (закрыт пробел §19.5)

Новые файлы (по конвенции forum-category-taxonomy: отдельный конфиг, без webServer, auth через storageState, все входы — валидируемые env):

- `apps/next-admin/tests/product-admin/product-admin.spec.ts` — 4 теста:
  1. неаутентифицированный `/dashboard/product` → редирект `/auth/sign-in` (middleware.ts:29);
  2. список каталога: колонка «Product» + сид-строка видима;
  3. поиск по `?title=`: фрагмент сид-названия находит, промах-терм — нет (сквозная проверка LIKE-цепочки через Next → GraphQL → Postgres);
  4. редактор: карточки «Pricing & Variants Matrix» и «Media Gallery» + сид-SKU (если задан).
- `apps/next-admin/playwright.product-admin.config.ts` — single worker, chromium, `test-results-product-admin/`.
- `package.json`: скрипт `"test:e2e:product"`.

Синтаксис валидирован stripTypeScriptTypes + `node --check` (node_modules в песочнице нет — полноценно спеки исполняются только в окружении §19.3).

### 20.3. Как прогнать (дополнение к ранбуку §19)

```
# 1) живой стек (§19.3): ./scripts/dev-start.sh  (Postgres + сервер :5150)
# 2) next-admin на отдельном порту и вход вручную один раз:
cd apps/next-admin && npm run dev -- --hostname 127.0.0.1 --port 3200
npx playwright open --save-storage auth/product-admin.json http://127.0.0.1:3200/auth/sign-in
# 3) прогон:
RUSTOK_PRODUCT_ADMIN_BASE_URL=http://127.0.0.1:3200 \
RUSTOK_PRODUCT_ADMIN_STORAGE_STATE=auth/product-admin.json \
RUSTOK_PRODUCT_ADMIN_SEEDED_PRODUCT_ID=<uuid сид-товара> \
RUSTOK_PRODUCT_ADMIN_SEEDED_TITLE='<фрагмент названия>' \
RUSTOK_PRODUCT_ADMIN_SEEDED_SKU='<sku варианта>' \
npm run test:e2e:product
```

Сид-товар: создать через UI (или GraphQL `createProduct`) один товар с переводом, вариантом и SKU — дальше спеки самовоспроизводимы.

## §21. Домен категорий: глубокий обход цепочек + searchable-пикеры + e2e

### 21.1. Статический обход (продолжение §20.1, цепочка D вскрыта до таблиц)

**Создание категории/подкатегории — работает, механика целостна:**
- UI: `categories-page.tsx` (server component, полный список через `catalogCategories(tenantId, locale)`) → `CategoryCreateDialog` (валидация name/code/slug, автогенерация slug/code из name) → server action → `createCatalogCategory(idempotencyKey, locale, input{parentId, code, slug, kind, name, description})`.
- Сервер: `catalog.rs:1522` (RBAC PRODUCTS_MANAGE + module-gate + идемпотентность) → `catalog_schema_service/categories.rs:32 create_category`: `input.validate()` (code/slug/metadata/rule/переводы) → `load_category_parent` (schema_service.rs:893: родитель существует + тенант + не soft-deleted) → **level = parent.level + 1, path = parent.path + "/" + slug** → dual-write (`sync_created_category_to_taxonomy_in_tx` + INSERT `catalog_categories` + SEO-переводы) → событие `CatalogCategoryCreated`.
- DB-инварианты: `UNIQUE (tenant_id, code)`, `UNIQUE (tenant_id, parent_id, slug)` (миграция 000001:264-265); циклы и каноничность дерева — deferred constraint triggers на Postgres (миграция 000025: `rustok_product_assert_category_tree`, WITH RECURSIVE + visited_ids); closure-хранилище позже retiring (000019/000020) → subtree-чтения переведены на WITH RECURSIVE с LIMIT 500 (§18.1).
- Таблица категорий: клиентский поиск по name/code/slug/path + иерархический отступ по path.
- Primary-категория товара: structural-only фильтр + legacy-выбор виден с флагом (product-category-card).
- Storefront: фасеты по subtree категорий (fail-closed, §18.1).

**Найденные пробелы UX (ожидаемая пользователем функциональность):**
1. **Пикер родителя в диалоге создания — Select без поиска** (плоский список всего каталога; при сотнях категорий неработоспособен). → **исправлено (§21.2)**.
2. **Пикер primary-категории в редакторе товара — тот же Select без поиска.** → **исправлено (§21.2)**.
3. Наблюдения мейнтейнеру (не чинилось, вне скоупа UI-фикса):
   - серверный `catalogCategories` не имеет аргументов поиска/пагинации — тянет весь список (клиентский фильтр консистентен сейчас, но на масштабе нужен серверный search);
   - в commerce GraphQL **нет update/move/delete** категорий каталога (только create + схема/атрибуты) — переименование/перемещение/удаление недоступны оператору;
   - `load_category_parent` не проверяет kind родителя — виртуальная (rule-based) категория может стать родителем структурной;
   - глубина дерева не ограничена при создании (лимит 500 — только на чтение subtree).

### 21.2. Фикс: SearchableCategorySelect (поиск категорий при выборе)

Новый компонент `components/categories/searchable-category-select.tsx` — комбобокс по конвенции репо (Popover + Command из shadcn, паттерн `data-table-faceted-filter`): поиск по метке (path/name/code), пустой выбор «None (Root category)» / «No category assigned», чек-марка текущего значения, disabled-пропагация. Фильтрация клиентская по уже загруженному списку — консистентна с текущей архитектурой (серверного поиска нет, §21.1-3).

Вживлён в два места:
- `category-create-dialog.tsx` — пикер родителя (`#cat-parent`, Label привязан по htmlFor; Kind-селектор не тронут);
- `product-category-card.tsx` — primary-категория (`#category-select`, structural-фильтр и legacy-флаг сохранены, мёртвый Select-импорт убран).

Синтаксис всех трёх .tsx провалидирован esbuild (tsx-loader, parse-only; установлен вне воркспейса в /tmp). Гейты не задеты: единственный читатель области — `verify-product-next-data-table-host.mjs` (читает нетронутый categories-table) — PASS; пакетные .test.mjs на изменённые компоненты не ссылаются.

### 21.3. E2E: category-admin.spec.ts

`tests/product-admin/category-admin.spec.ts` — 3 теста против живого стека (конфиг и скрипт те же, `npm run test:e2e:product`):
1. фильтр таблицы категорий находит/отсеивает сид-категорию;
2. **пикер родителя ищет**: фрагмент названия → опция видима; промах → «No categories found.»;
3. **создание подкатегории под сид-родителем**: имя `E2E Subcategory <ts>` (уникальность), выбор родителя через поиск, сабмит → тост «Category created successfully» → reload → строка в таблице (сквозная проверка server action → GraphQL → сервис → dual-write → БД).

Env: `RUSTOK_CATEGORY_ADMIN_BASE_URL`, `RUSTOK_CATEGORY_ADMIN_STORAGE_STATE`, `RUSTOK_CATEGORY_ADMIN_SEEDED_PARENT_TITLE` (storage state может указывать на тот же файл, что и product-сьюта).

## §22. Домен атрибутов: глубокий обход цепочек + searchable-пикеры + e2e

### 22.1. Статический обход (продолжение §20.1/§21.1)

**Цепочка целостна и полнее, чем у категорий:**
- UI: `attributes-page.tsx` (server component) монтирует таблицу, диалог создания, schema-authoring-card (5 секций: режим схемы категории, группа схемы, привязка атрибута к схеме, группа категории, привязка атрибута к категории) и lifecycle-карточку → server actions → `api/attributes.ts` + `api/attribute-lifecycle.ts` → GraphQL.
- Сервер: `catalog.rs` — полный набор мутаций: `update_product_attribute_definition` (:1391), `create_product_attribute` (:1422), `create_product_attribute_option` (:1482), `create_product_attribute_schema` (:1566), `create_product_attribute_schema_group` (:1603), `create_catalog_category_attribute_group` (:1643), `bind_product_attribute_schema_attribute` (:1718), `bind_catalog_category_attribute` (:1769), `save_product_attribute_values` (:1821), `clear_detached_product_attribute_values` (:1861). Все с RBAC + module-gate + идемпотентностью.
- Сервис: `attributes.rs` (create :41, option :143, списки) + **`attribute_lifecycle.rs:540 update_attribute_definition`** — образцовая машина состояний: validate → txn → policy-lock → row-lock (`lock_definition`) → optimistic concurrency по `state_token` (отказ `Stale` вместо слепой перезаписи), отказы `NotFound`/`ParentArchived`/`in_use`/`inspection_limit` вместо жёстких ошибок, архивация/восстановление атрибута и опции.
- Клиентская модель `attribute-lifecycle.ts` (543 строки) зеркалит серверный контракт (action update/archive/restore + expectedState + классификация отказов) и покрыта `attribute-lifecycle.test.mjs`.
- `saveProductAttributeValues`: next-admin вызывает (api/products.ts:632 → мутация :1821) — значение атрибутов товара сохраняется через владельца. Ранее зафиксированное наблюдение касалось только несмонтированного враппера в dioxus-фасаде (`graphql_fallback_mutations.rs:201`) — остаётся в силе, это не про next-admin.
- Поиск по таблице атрибутов — клиентский (label/code) ✓.

**Найденный пробел (тот же класс, что §21):** schema-authoring-card — **все пикеры нативные `<select>` без поиска**: 4 категорийных (режим схемы, clone-from, группа категории, привязка категории), 3 схемных, 2 атрибутных. При росте справочников невыполнимы.

### 22.2. Фикс: generic SearchableSelect + 9 замен

- Новый `components/shared/searchable-select.tsx` — generic-комбобокс (Popover+Command, фильтр по метке, пустой выбор, чек марка, disabled; `cn` = twMerge — перекрытие классов корректно).
- `searchable-category-select.tsx` отрефакторен в тонкого делегата generic-версии (публичный API прежний — оба вызывателя из §21.2 не изменились ни на строку).
- schema-authoring-card: 9 нативных селектов заменены — категории ×4 → `SearchableCategorySelect`, схемы ×3 и атрибуты ×2 → `SearchableSelect`. Ограниченные enum (schema mode: inherit/use_schema/clone_from_category/custom; binding kind) **намеренно оставлены** нативными — поиск по 4 фиксированным опциям бессмыслен.
- Не тронуто (наблюдения мейнтейнеру): preview-пикер структурных категорий в schema-authoring-card (radix Select с aria-label — кандидату на тот же комбобокс); directory-селект в attribute-lifecycle-card — карточка полностью i18n-изирована (типизированный `t()` + ftl), поиск требует нового ftl-ключа + typegen — за мейнтейнером.

### 22.3. E2E: attribute-admin.spec.ts

`tests/product-admin/attribute-admin.spec.ts` — 3 теста (тот же ран `npm run test:e2e:product`):
1. фильтр таблицы атрибутов находит/отсеивает сид-атрибут;
2. **пикер атрибутов в schema-authoring ищет**: фрагмент → опция видима; промах → «No results found.»;
3. **создание атрибута через диалог** (`E2E Attribute <ts>`, дефолты text/product) → тост → reload → строка в таблице (server action → GraphQL → сервис → БД).

Env: `RUSTOK_ATTRIBUTE_ADMIN_BASE_URL`, `RUSTOK_ATTRIBUTE_ADMIN_STORAGE_STATE`, `RUSTOK_ATTRIBUTE_ADMIN_SEEDED_LABEL`.

### 22.4. Верификация правок

- esbuild (tsx, parse-only): все 5 изменённых/новых .tsx — OK.
- Гейты: `verify-product-admin-operation-results` (экспорт/монтирование schema-authoring-card) — PASS; `verify-product-next-data-table-host` — PASS; вся admin-зона product-гейтов: **19 PASS / 0 RED**.
- Инвариант `secondary-write-idempotency.test.mjs` сохранён: `parsePosition(` ровно 4, паттерн `Number.isNaN(parsed) ? 0 : parsed` отсутствует (0 вхождений); ассерты на attribute-options-dialog не задеты (файл не менялся).

## §23. Полный обход: все цепочки продуктового функционала

Закрывающий раунд: оставшиеся домены (товар, варианты/оси, медиа, цены, бренды, бандлы, связи, storefront-чтение, переводы) — тем же методом (UI → transport → server → runtime → порт → сервис → БД, каждый хоп с координатами).

### 23.1. Карта цепочек (все живые)

**A. Товар — CRUD и lifecycle:**
- **Создание**: список `/dashboard/product` (Plus) → `/dashboard/product/new` → роут `[productId]` с `productId === 'new'` → `isNew`-режим `ProductEditorPage` (первичное сеяние осей из effective form) → `createProductAction` → `createProduct` (index.ts:390) → `catalog.rs:433` → `commands.rs:158` (переводы, метадата, primary-category, бренды, shipping).
- **Обновление**: `saveProductAction` → `updateProductDetail` → `update_product` (:464 → :553): валидация брендов/shipping/primary-category/текстов + **in-tx publish-гейтинг** `remains_active` (:748/:929) — переход draft→active через generic-update валидируется так же, как выделенным `publish_product`.
- **Статус**: `changeProductStatusAction` → тот же update (эквивалентно; деактивация обнуляет `published_at`).
- **Удаление**: `deleteProductAction` → `delete_product` (:648).

**B. Варианты и оси**: `setVariantAxes`/`previewVariantAxes` (axes card; пикер кандидатов ограничен variant-scope атрибутами схемы, ≤32 строк) → `catalog.rs:522`; create/update/delete варианта → :678/:712/:746 → `commands.rs:1342/1533`. `UpdateVariantInput` несёт цены/инвентарь/shipping; сервис принимает **только unscoped base-цены** — channel-цены отклоняются с явной ошибкой «use Pricing for contextual prices» (commands.rs:1561 area) ✓. P1-заголовки живут в этой ветке.

**C. Медиа**: `addProductImage(mediaId, altText, locale)` / delete / reorder → `catalog.rs:778/:847/:880` → `commands.rs:1862` (Media hold + компенсация §18.1); alt-текст локализуется при добавлении; reorder — позиционная перестановка.

**D. Цены (Pricing-поверхность)**: серверные `update_admin_pricing_variant_price` / `preview_admin_pricing_variant_discount` / `apply_admin_pricing_variant_discount` (mutations/pricing.rs:215/:274/:328) + клиент `api/pricing.ts` + `actions.ts:439-470` — **определены, но не смонтированы ни в одной странице** (0 вызывателей). Базовые цены редактируются через вариантную сетку (B); channel-цены/прайс-листы/скидки UI не имеют.

**E. Бренды**: страница `/dashboard/product/brands` — полный CRUD (`listBrands/createBrand/updateBrand/deleteBrand` → `catalog.rs:1036/:1073/:1115`). **Привязка бренда к товару в UI отсутствует**: `assign_product_brand`/`unassign_product_brand` (:1133/:1159) не имеют ни одного вызывателя, в редакторе товара 0 упоминаний бренда.

**F. Бандлы**: страница + `product-bundle-card` в редакторе: полный CRUD + items (`catalog.rs:1184-1356`); диалог добавления item — **дебаунс-поиск товаров** ✓.

**G. Связи**: `product-relations-card` + диалог с дебаунс-поиском ✓; add/remove/reorder → `catalog.rs:913/:957/:990`.

**H. Storefront-чтение**: next-frontend `StorefrontCommerceProduct(handle, locale)` / `StorefrontProductCatalog(filter)` → `query.rs:2397/:2474` (channel-scope) → read-runtime `in_process_with_pricing` → `queries.rs` (в `variants { title }` — P1-проекция заголовков).

**I. Переводы товара**: редактор — табы **en/ru захардкожены** (general-card Tabs + сид editor-page:277-295); бэкенд поддерживает произвольные локали (категории/варианты нормализуют любой тег).

### 23.2. Фикс раунда

`brands-table.tsx` — таблица брендов **не имела поиска** (единственная из справочных таблиц): добавлен фильтр по name/slug/description + счётчик, зеркально categories-table (паттерн репо). Читателей файла в гейтах/тестах нет; esbuild OK.

### 23.3. Наблюдения мейнтейнеру (сводка несмонтированного/незавершённого)

1. **Pricing-UI отсутствует**: мутации pricing.rs + api/pricing.ts + 4 action'а определены и не смонтированы — channel-цены, прайс-листы и скидки недоступны оператору (базовые цены доступны через варианты).
2. **Привязка бренда к товару** — серверные мутации есть, UI нет.
3. **`update_product_image`** (:812): сервер жив, клиентский GraphQL-документ `updateProductImage` (api/products.ts:224) мёртв (нет функции-обёртки и вызывателей) — alt-текст нельзя отредактировать после добавления изображения.
4. **Медиатека**: mediaId вводится вручную как UUID-строка (валидация паттерном есть, пикера медиа-библиотеки нет).
5. **Выделенные lifecycle-мутации** `publish_product`/`unpublish_product`/`archive_product` next-admin'ом не используются (статус идёт через update с идентичным in-tx-гейтингом) — избыточная поверхность или задел под другие клиенты.
6. **Локали товара захардкожены en/ru** в UI редактора.
7. (из §21/§22) preview-пикер категорий в schema-authoring; directory-селект lifecycle-карточки (нужен ftl-ключ); серверный поиск `catalogCategories`/`productAttributes` без аргументов.

### 23.4. Верификация

- product-зона гейтов после правок: **83 PASS / 5 RED** — красные ровно born-red пятёрка §15.3 (closure/legacy retirement + seo-seam), регрессий 0.
- esbuild: brands-table OK; инварианты secondary-write-теста не задеты (читателей brands-table нет).

**Итог обхода**: все 9 групп цепочек непрерывны от UI до БД; мёртвых звеньев в проводке нет. Незакрытое — это несмонтированные поверхности (23.3), а не разрывы: серверная часть везде готова, не хватает UI-монтажа.

## §24. Монтирование привязки брендов (канонический путь) + цепочки commerce

### 24.1. Археология перед фиксом

- Владелец привязки товар↔бренд — `rustok-product::product_brands` (нота в deprecated-порте `rustok-brand/ports/brand.rs:83`): канонический путь — `create_product`/`update_product` с полем `brands` (валидация `validate_product_brands`: без nil, без дублей, ровно один primary при нескольких; синхронизация `sync_product_brands_in_tx`).
- Чтение домена уже несёт бренды: `ProductResponse.brands` (projection.rs:1006, primary-first сортировка).
- **Обрыв был в тонком слое commerce GraphQL**: GQL `UpdateProductInput` не имел `brands` (хардкод `brands: None`), create — `brands: Vec::new()`, `GqlProduct` — без поля; клиентский `ProductBrandItem` (types.ts:570) существовал, но не подключён. Единственный GQL-путь шёл через deprecated-мутации `assign/unassign_product_brand` (catalog.rs:1133/:1159, deprecated BrandPort, без идемпотентности).

### 24.2. Фикс: brands сквозь канонический путь (5 звеньев)

**Rust (rustok-commerce):**
- `graphql/types.rs`: `GqlProductBrand` (SimpleObject: brandId/isPrimary/role/position) + `From<ProductBrandResponse>`; поле `brands` в `GqlProduct` (+ маппинг в `From<ProductResponse>` :2066); `ProductBrandInputObject` (InputObject) + поле `brands` в GQL `CreateProductInput` и `UpdateProductInput` (None=keep, Some=sync — зеркалит доменный Patch-контракт; `brands.is_some()` уже требует revision, ports dto :216).
- `mutations/helpers.rs`: `convert_product_brand_inputs` (defaults: role=manufacturer, position=0 — зеркалит serde-defaults домена); create-конверсия теперь пробрасывает `brands`.
- `mutations/catalog.rs`: update-мутация — `brands: input.brands.map(convert...)` вместо `None`.

**Клиент (next-admin):**
- `api/products.ts`: PRODUCT_DETAIL_QUERY + `brands { brandId isPrimary role position }`; `UpdateProductInput.brands`.
- `write-attempt.ts`: `ProductSavePayload.brands?: ProductBrandItem[]`.
- `actions.ts`: update-путь маппит payload.brands → input.
- `product-editor-page.tsx`: состояние `productBrands` (из initialProduct), в payload (update-only; isNew не шлёт), в карточку.
- `product-general-card.tsx`: секция «Brands» (только для существующих товаров): бейджи текущих брендов (primary ★, удаление ✕), добавление через `SearchableSelect` по справочнику; **инвариант «ровно один primary» поддерживается локально** (первый добавленный — primary; удаление primary промоутит первого оставшегося) — серверу не придётся отказывать.
- `[productId]/page.tsx`: серверный фетч `listBrands` + прокидывание в обе инстанции редактора.

### 24.3. Цепочки commerce (обход)

- **Корзина (storefront)**: GraphQL `create_storefront_cart` / add/update/remove line item / update context (mutations/cart.rs:21-404, channel-gate) → `in_process_cart_storefront_port(db)` → крейт `rustok-cart`. Браузерная корзина Leptos-сторфронта: `cart_buyer_client.rs` (WASM: «Cart owns the controller and UI, Product owns selection and quotes», owner-barrier против двойного монтирования) + `cart_buyer_route.rs` (query-параметры cart_id/quantity).
- **Чекаут**: `create_storefront_payment_collection` (safe_checkout.rs:329) → cart-порт; `complete_storefront_checkout` (:468) → `storefront_staged_checkout_runtime` (стейджинг заказа/оплаты).
- **Fulfilment**: `CommerceFulfillmentCommandRuntime::in_process_with_shipment_stock` (graphql_runtime.rs:559/:570); shipping options/profiles — мутации safe_checkout.rs:527+.
- **Admin-commerce**: next-admin `packages/commerce` (orders UI, commerce-admin.test.mjs).
- **Born-red срез**: 31 гейт `verify-commerce-graphql-*` пинит целевую архитектуру (error-boundary модули, include-файлы, typed-cutover), которой нет ни в дереве, ни в базе — **сертифицировано прогоном на worktree 58ccfb8: 0/31 PASS на базе = 0 регрессий от моих правок** (маркеры вроде `include!("cart.rs")` отсутствуют в дереве вовсе).

### 24.4. Верификация раунда

- esbuild: 6 клиентских файлов OK; баланс-сканер: 3 Rust-файла 0/0/0.
- Гейты: product-зона 83 PASS / 5 born-red (без изменений); расширенный своп с commerce-graphql: 98 PASS / 31 born-red (все 31 красные и на базе — worktree-сертификация).
- Инварианты secondary-write-теста: `idempotencyKey?: string` ×4 в actions.ts ✓, `addImageAction(pid, img, idempotencyKey)` ✓, срез addProductImage не задет ✓.
- §23.3-2 (привязка бренда без UI) — **закрыто**; deprecated-мутации остаются как параллельная поверхность (наблюдение сохраняется).

## §25. Монтирование двух крупнейших функциональных пробелов: alt-редактирование и Pricing-UI

### 25.1. Alt-редактирование изображений (мёртвый документ → живая функция)

Серверная мутация `update_product_image` (catalog.rs:814, RBAC + идемпотентность) и сервис (commands.rs:2042: alt требует канонический locale, upsert `product_image_translation`; position — слот-переупорядочивание) были полностью готовы; клиентский GraphQL-документ существовал, но без функции-обёртки и вызывателей (§23.3-3).

Смонтировано сквозное:
- `api/products.ts`: `updateProductImage(opts, productId, id, input, idempotencyKey?)` — документ наконец используется; размещён вне среза addProductImage…deleteProductImage, который пинится secondary-write-тестом.
- `actions.ts`: `updateImageAction(pid, id, input, idempotencyKey: string)` — ключ **обязательный** (не `?`), поэтому инвариант теста «ровно 4 опциональных ключа у ADD-действий» не тронут (проверено: count=4).
- `product-media-card.tsx`: кнопка ✎ на каждой карточке изображения → диалог (locale en/ru + alt) с **тем же one-key-per-intent контрактом**, что у add-диалога (`??= randomUUID`, ротация при подтверждении/definitive refusal — код и комментарии зеркальны).
- Проводка: editor page (assertEditorialWriteResolved + router.refresh) → host (`updateImageAction`).

### 25.2. Pricing-UI (был: мутации+api+actions без единого вызывателя)

Новая карточка `components/products/product-channel-pricing-card.tsx` — «Channel Pricing & Discounts» (после матрицы вариантов, только для существующих товаров):
- **пикер варианта** (SearchableSelect по SKU) + **пикер прайс-листа** (SearchableSelect по активным спискам с channelSlug; «Direct price (no list)» — пустой выбор);
- **upsert channel-цены**: currency (дефолт из базовой цены варианта), amount, compare-at, min/max qty → `update_admin_pricing_variant_price` (pricing.rs:215 → pricing write port; RBAC PRODUCTS_UPDATE);
- **скидка**: discount % → **Preview** (строка `current → adjusted (−%)` из `PricingAdjustmentPreview`) → Apply (`preview/apply_admin_pricing_variant_discount` :274/:328);
- клиентская валидация: amount — decimal, процент ∈ (0,100) эксклюзивно.
- Разделение ответственности сохранено и задокументировано в UI: базовые цены — в редакторе варианта (каталог), контекстные — здесь (Pricing owner); это зеркало серверного правила «Catalog edits accept only unscoped base prices» (§23.1-B).

Проводка: editor props (4 колбэка, обёртки с assert+refresh для мутаций) → host (`fetchActivePriceListsAction`/`upsertVariantPriceAction`/`previewVariantDiscountAction`/`applyVariantDiscountAction` — все четыре action'а из §23.3-1 наконец смонтированы).

### 25.3. Верификация

- esbuild: 6 изменённых/новых файлов OK.
- Гейты: зона product+pricing — **87 PASS / 11 RED**; все 11 красных сертифицированы на базовом worktree 58ccfb8 как born-red (5 category-retirement/seo-seam §15.3 + 6 pricing: `rustok-pricing/admin/src/transport.rs` отсутствует и в базе — несмонтированный admin-транспорт Pricing). Регрессий 0.
- Инвариант secondary-write: `idempotencyKey?: string` ×4 ✓ (updateImageAction — обязательный ключ).

### 25.4. Что осталось из §23.3

Медиатека (выбор mediaId из библиотеки вместо ручного UUID — требует Media-модуль UI), локали товара кроме en/ru (ftl+typegen), preview-пикер категорий в schema-authoring, directory-селект lifecycle-карточки (ftl-ключ). Все крупные функциональные пробелы product-домена закрыты.

## §26. Монтирование: медиатека-пикер и произвольные локали товара

### 26.1. Медиатека вместо ручного UUID (§23.3-4)

Библиотека медиа уже существовала (`@rustok/media-admin`: `listMedia` с пагинацией, страница `/dashboard/media`), но карточка медиа товара требовала ввести Media UUID вручную.

Смонтировано:
- `actions.ts`: `listProductMediaAction()` — первая страница библиотеки (perPage 100, кап клиента) в slim-форме `{id, name, url}` через `@rustok/media-admin` (первый кросс-модульный монтаж product↔media в next-admin).
- `product-media-card.tsx`: в add-диалоге — **пикер библиотеки** (SearchableSelect, клиентский фильтр по имени; ленивая загрузка при первом открытии) + **превью-миниатюра** выбранного ассета; выбор заполняет UUID. Ручной ввод UUID сохранён как fallback (лейбл динамически помечает «or pick from the library above»); валидация MEDIA_UUID_PATTERN не тронута. Смена выбора сбрасывает intent-ключ (семантика field-edit контракта secondary-write).
- Проводка: editor `onListMedia` → host `listProductMediaAction`.

Ограничение (наблюдение): media-запрос не имеет поиска/фильтра — пикер фильтрует локально по первой сотне; на больших библиотеках нужна серверная search-аргументация (зеркалит §21.1-3 для категорий).

### 26.2. Произвольные локали товара (§23.3-6)

Табы переводов были захардкожены `en`/`ru`, хотя бэкенд принимает любые канонические теги (TenantLocale), а `handleTranslationChange` уже умел upsert любую локаль, и save-фильтр (`title.trim().length > 0`) безопасно отбрасывает пустые.

Смонтировано:
- `product-general-card.tsx`: табы теперь **динамические** — из map'а переводов (en, ru первыми, далее по алфавиту); известные теги получают дружелюбные подписи (LOCALE_LABELS: en/ru/fr/de/es/it/pt/pt-BR/uk/pl), неизвестные — upper case. Кнопка «+» → Popover с вводом BCP-47-тега (валидация `/^[a-z]{2}(-[a-z0-9]{2,8})?$/i`, отказ при дубле) и подсказкой, что пустая локала не сохранится.
- `product-editor-page.tsx`: `handleAddLocale` — пустая запись перевода + активация таба.

### 26.3. Верификация

- esbuild: все изменённые файлы OK (карточки, editor, actions, host).
- Гейты: зона product+pricing+graphql-product — **88 PASS / 11 RED**, красные — те же сертифицированные born-red (5 category + 6 pricing), регрессий 0.
- Инварианты secondary-write (контракт «media card add-image intent»): mint/rotate/ownerCall — нетронуты; `resetAddAttempt()` вызовов стало 6 при требовании `>= 4` ✓ — добавленный вызов в пикере соответствует семантике «field edits» контракта.

### 26.4. Остаток §23.3

Preview-пикер категорий в schema-authoring (косметика), directory-селект lifecycle-карточки (нужен ftl-ключ + typegen), серверный поиск для media/категорий на масштабе. Все функциональные пробелы product-домена из списка §23.3 закрыты или переведены в infra-задачи мейнтейнеру.

## §27. Commerce-админка: страница деталей заказа + навигационные i18n-ключи

Направление выбрано по директиве «самые косяки, а не код вылизивать»: 31 born-red commerce-graphql гейт — Rust-архитектура (include-файлы/error-boundary, §24), а вот UI-слой next-admin содержал два жёстких пользовательских пробела.

### 27.1. 404 на «View Details» заказа

Таблица заказов ссылалась на маршрут, которого не существовало: `cell-action.tsx` («View Details», «Open in new tab») и `columns.tsx:69` (клик по `#shortId`) вели на `/dashboard/commerce/orders/[id]` — в `app/dashboard/commerce/orders/` была только `page.tsx`. Бэкенд при этом полностью готов: admin-GraphQL `order(tenantId, id)` возвращает `GqlAdminOrderDetail { order (полная, с adjustments/taxLines), paymentCollection (payments+refunds), fulfillment (items с shipped/delivered) }` (query.rs:1100).

Смонтировано:
- `packages/commerce/src/api.ts`: `ORDER_QUERY` (полная проекция деталей: line items с fulfillmentRequirement/shippingProfileSlug/sellerId, adjustments, tax lines, payment collection с payments и refunds, fulfillment с построчными количествами) + `getOrderDetail(opts, id)` — fail-closed проверка креденшелов, `null`-passthrough для отсутствующего заказа.
- `packages/commerce/src/types.ts`: 10 типов домена деталей (AdminOrder/AdminOrderLineItem/AdminOrderAdjustment/AdminOrderTaxLine/AdminPayment/AdminRefund/AdminPaymentCollection/AdminFulfillment/AdminFulfillmentItem/AdminOrderDetail) — имена без коллизий с существующими OrderListItem/OrderChange*.
- `packages/commerce/src/components/order-detail/order-detail-view.tsx` (+ barrel + экспорт из index.ts): презентационный вью — header с back-link и статус-бейджем (та же палитра, что в columns.tsx), Summary (клиент/канал/метод оплаты/перевозчик/трекинг + разбивка сумм: subtotal/shipping/adjustments/tax(included)/total + причина отмены), Timeline (created→confirmed→paid→shipped→delivered/cancelled, детерминированный UTC-формат против hydration-mismatch), Line items / Payments / Refunds / Fulfillment lines — через **DataTableStatic** (хост-таблица; сырой `<Table>` запрещён ratchet-гейтом), пустые состояния для отсутствующих paymentCollection/fulfillment.
- `src/app/dashboard/commerce/orders/[id]/page.tsx`: серверный компонент — auth → `getOrderDetail` → вью; error-card при отказе, not-found-card с back-link при `null` (зеркалит паттерн orders/page.tsx).
- `commerce-admin.test.mjs`: +1 тест (7 всего): getOrderDetail диспатчит типизированный запрос с `{tenantId, id}`, null-passthrough, fail-closed.

### 27.2. Сырые i18n-ключи в сайдбаре (12 шт., все модули)

Фолбэк next-fluent на отсутствующий ключ — буквальная строка вида `app.nav.commerce` (errors.ts:98 `defaultMessageFallback`). Проверка всех `i18nKey` из nav-файлов пакетов против словарей дала **12 отсутствующих** ключей в en.ftl И ru.ftl: `brands` (остаток §24!), `commerce`, `orders`, `shippingProfiles`, `cartPromotions`, `orderChanges`, `returnDecisions` (commerce), `media`, `forms`, `pages`, `topicComposer`, `translation.title` — т.е. сайдбар показывал сырые ключи для целых разделов Media/Forms/Pages/Commerce в обеих локалях.

Смонтировано:
- `messages/en.ftl` + `messages/ru.ftl`: все 12 ключей (ru: Коммерция, Заказы, Профили доставки, Промо корзины, Изменения заказов, Решения по возвратам, Бренды, Медиа, Формы, Страницы, Редактор тем, Перевод).
- `src/types/i18n.d.ts`: те же 12 в обеих формах (dash + dot) в отсортированные позиции — артефакт typegen держится в синхронном формате до следующей регенерации.
- Валидация: полный парс обоих словарей `@fluent/syntax` — junk=0, en=ru=1224 сообщения, полная парность ключей, все 12 на месте.

### 27.3. Верификация

- esbuild: 6/6 затронутых файлов OK (api.ts, types.ts, order-detail-view.tsx, barrel, index.ts, [id]/page.tsx); i18n.d.ts OK.
- Пакетный тест commerce: **7/7** (было 6). Fixture `verify-commerce-admin-boundary.test.mjs`: 5/5.
- Зона product+pricing+graphql-product: **11 RED — ровно born-red реестр** (5 category §15.3 + 6 pricing transport), 0 регрессий.
- Зона commerce-admin (40 гейтов, все читают только crates/): **23 PASS / 17 RED**. Все 17 сертифицированы на базовом worktree 58ccfb8: на базе 20 RED ⊇ текущих 17 (3 базовых красных — product-diagnostic/read/write-http — закрыты моими Rust-фиксами ранних раундов), т.е. все 17 текущих — born-red, регрессий от §27 — 0 (правки раунда не выходят за apps/next-admin).
- Структурные: `verify-product-next-data-table-host` PASS (новый вью через DataTableStatic, ratchet не расширен); `verify-commerce-admin-boundary` PASS; `verify-frontend-host-ffa-contract` — прежний born-red (ENOENT apps/admin workflow, зона dioxus-admin), режим падения не изменился.
- lucide-react@1.69.0 (lock): все 4 новые иконки (ArrowLeft/Landmark/ReceiptText/RotateCcw) подтверждены в d.ts и в рантайм-бандле.

### 27.4. Наблюдения мейнтейнеру (прирост)

- **17 born-red `verify-commerce-admin-*`** — Rust-архитектура admin-контроллеров (owner-port cutover для payments/refunds/fulfillment/shipping-options, статические HTTP-конверты, bounded-логирование): пинят целевую архитектуру, которой нет ни в дереве, ни в базе. Крупный рефакторингный трек без cargo-валидации не выполнялся.
- **Отсутствуют UI-страницы** для payment collections / refunds / fulfillments / shipping options — GraphQL-запросы в бэкенде есть (`payment_collections`, `refunds`, `fulfillments`, `shipping_options` с фильтрами), навигация не создана. Детали заказа теперь показывают payment+fulfillment по конкретному заказу, но списков уровня раздела нет.
- **CommerceMetricsWidget** — заглушки «-» на дашборде (нет метрик-запроса; AdminRenderContext не несёт данных).
- i18n.d.ts ручная досинхронизация — прогнать `npm run typegen:i18n` при ближайшей возможности.

## §28. Commerce-админка: списки Payments / Refunds / Fulfillments

Продолжение §27 (вариант A). GraphQL-бэкенд давно отдавал три списковых admin-запроса с фильтрами и пагинацией, но в next-admin не было ни страниц, ни пунктов навигации — оператор видел платежи и отправления только внутри карточки конкретного заказа.

### 28.1. Смонтированные поверхности

- **api.ts**: `PAYMENT_COLLECTIONS_QUERY` / `REFUNDS_QUERY` / `FULFILLMENTS_QUERY` + `listPaymentCollections` (фильтры status/orderId/cartId/customerId) / `listRefunds` (paymentCollectionId/orderId/status) / `listFulfillments` (status/orderId/customerId) — fail-closed проверка креденшелов, как у остальных методов пакета.
- **types.ts**: `PaymentCollectionsFilter`/`RefundsFilter`/`FulfillmentsFilter` + list-обёртки (`PaymentCollectionList`/`RefundList`/`FulfillmentList`) поверх типов §27.
- **Три таблицы** (зеркало orders-table: select → id → status → … → actions, все через хост-`DataTableShell`, без сырого `<Table>`):
  - `payments-table`: статус-палитра по словарю бэкенда (pending/authorized/captured/cancelled — PaymentCollectionStatusKind), amount с подстрокой captured/refunded, provider, **линк на заказ** (orderId), счётчик payments/refunds, cell-action (Copy ID + View Order, при наличии orderId).
  - `refunds-table`: статусы refunded/pending/cancelled (RefundStatusKind), amount, reason, collection short-id; actions-колонка без линков (у GqlRefund нет order_id — возврат инспектируется через заказ/коллекцию).
  - `fulfillments-table`: статусы delivered/shipped/pending/cancelled (FulfillmentStatusKind), carrier+tracking, счётчик строк и штук, shipped/delivered даты, cell-action (Copy ID + View Order — orderId обязателен).
- **Страницы**: `/dashboard/commerce/payments`, `/dashboard/commerce/refunds`, `/dashboard/commerce/fulfillments` — серверные компоненты (auth → list-call → таблица), searchParams `page/perPage/status/orderId`, error-card при отказе — точное зеркало orders/page.tsx. Все три под ModuleGuard `slug='commerce'` (layout уровня папки).
- **Навигация**: `commerceNavItems` 5 → **8** пунктов (Orders, Payments, Refunds, Fulfillments, Shipping Profiles, Cart Promotions, Order Changes, Return Decisions).
- **i18n**: `app-nav-payments`/`app-nav-refunds`/`app-nav-fulfillments` в en+ru (Платежи, Возвраты средств, Отправления) + синхрон i18n.d.ts (обе формы, отсортированные позиции). **Чек всех nav i18nKey всех пакетов против словарей: пропусков больше нет** (было 12 в §27 + 3 новых).

### 28.2. Верификация

- Пакетный тест commerce: **8/8** (7→8: расширен нав-тест 5→8, добавлены fail-closed для трёх методов + happy-path с проверкой диспатча всех трёх запросов и фильтров).
- esbuild: 19/19 затронутых файлов OK.
- FTL: полный парс обоих словарей — junk=0, en=ru=1227, парность 0/0.
- lucide-react@1.69.0 (lock): ShieldCheck подтверждён в d.ts (остальные иконки уже использовались).
- Гейты: `verify-product-next-data-table-host` PASS (ratchet не расширен — три новые таблицы через DataTableShell); `verify-commerce-admin-boundary` PASS + fixture 5/5; commerce-admin зона **23 PASS / 17 born-red** (без изменений, сертифицированы в §27); зона product+pricing+graphql-product — **11 RED = born-red реестр**, 0 регрессий; ffa-contract — прежний born-red (ENOENT apps/admin workflow), режим падения не изменился.

### 28.3. Остаток

- Страниц-деталей для payment collection / fulfillment нет — по клику оператор уходит на заказ, где обе панели уже есть (§27); отдельные детали потребуют либо новых страниц, либо expandable-строк.
- Refund-строка не линкуется на заказ (нет order_id в GqlRefund) — наблюдение мейнтейнеру: добавить order_id в list-проецию refunds упростило бы навигацию.
- CommerceMetricsWidget — заглушки «-» остаются (нет метрик-запроса).

## §29. Форум-лендинг (404 корня навигации) + живые метрики дашборда

### 29.1. Систематический скан пробелов next-admin

Вместо выбора «на глаз» выполнен машинный скан всего приложения: все `href`/`push`/`replace`/`redirect`/nav-`url` на `/dashboard/*` (статические и с динамической маршрутизацией) против реальных маршрутов + grep заглушек. Итоги:
- `/dashboard/product/new` — **ложная тревога**: матчится `[productId]` с полным isNew-режимом редактора (§24–26).
- `/dashboard/profile` — ложная тревога (optional catch-all `[[...profile]]`).
- **`/dashboard/forum` — мёртвый корень навигации**: пункт «Forum» в сайдбаре вёл на 404 (страниц не было, только вложенные инструменты). ЕДИНЫЙ реальный dead-link приложения.
- Заглушки: `value='-'` в двух файл виджетов дашборда (commerce ×2, blog ×1) — больше «Coming soon»/«Not implemented» в дереве нет.

### 29.2. Форум-лендинг `/dashboard/forum`

Серверная страница (зеркало паттерна forum/categories): `listForumCategories` + `listForumTopics` параллельно; no-tenant → dashed-подсказка, ошибка → destructive-card. Контент:
- 4 StatCard: Categories / Topics / Replies (сумма replyCount) / Solved (solutionReplyId != null) — иконки проверены в lock-версии @tabler/icons-react@3.50.0 (IconCategory/IconMessages/IconMessage2/IconCheck; IconFolderTree/IconMessageCircle2 в 3.50 отсутствуют — заменены).
- «Topics» — первые 10 потоков: title, категория (мап id→name), replies, locale-бейдж, Solved-бейдж.
- «Moderation tools» — 8 ссылок на существующие поверхности (Categories, Topic/Reply Composer, Merge, Split, Fork, Reply range, Rename slug).
- Под ModuleGuard `slug='forum'` (layout папки), метаданные, server-only рендер.

### 29.3. Живые данные commerce-виджетов дашборда

`AdminRenderContext` **расширен полем `tenantId`** (host: `src/modules/types.ts` + заполнение в `dashboard/page.tsx` из сессии) — без него fail-closed методы пакетов (`!opts.tenantId → throw`) не могли работать из виджетов. Единственный конструктор контекста в дереве — дашборд.

`commerce-dashboard-widgets.tsx` переписан:
- **CommerceMetricsWidget**: `listOrders({perPage:1}).total` + `listFulfillments({perPage:1,status:'pending'}).total` в одном `Promise.all` — карточки «Orders» (было «-») и «Pending fulfillments» (вместо фиктивной «Revenue snapshot» — агрегата выручки в GraphQL нет). Состояния: загрузка «…», ошибка «—» + helper «unavailable — check sign-in»; отмена in-flight через `cancelled`-флаг.
- **CommerceRecentOrdersWidget**: 5 последних заказов — строки-линки на страницу деталей §27 (#shortId, items, total+валюта, статус-бейдж), loading/empty/error состояния, футер «Open orders». Без сырого `<Table>` (div-строки, ratchet не тронут).
- Клиентский фетч валиден: `graphqlRequest` (= `@/lib/graphql` → shared/api/graphql) работает в обоих режимах через Apollo c URL-резолвом.

Не смонтировано (архитектурное): **BlogMetricsWidget «-»** — блоговые списки keyset-пагинированы и «never count rows» (PostListResponse без total, комментарий в api/posts.ts:107). Каунт потребовал бы выкачки всех страниц — наблюдение мейнтейнеру: добавить метрик-агрегат или totalCount в GraphQL блога.

### 29.4. Верификация

- Повторный скан ссылок: `/dashboard/forum` закрыт; единственные остатки — ложные срабатывания (profile catch-all).
- Тесты: commerce-пакет 8/8, forum-contract 7/7.
- esbuild 4/4 (после переустановки /tmp/synchk — песочница потерала esbuild, урок R22).
- Гейты: `verify-product-next-data-table-host` PASS; `verify-commerce-admin-boundary` PASS; `verify-forum-admin-boundary` PASS; `verify-blog-forum-ui-ownership` PASS (пинит пакетные файлы форума — не тронуты); `verify-ffa-ui-migration-contract` PASS; commerce-admin зона **23/40** (17 born-red без изменений); зона product+pricing — **11 RED = born-red реестр**; ffa-contract — прежний ENOENT born-red. Регрессий 0.

## §30. Блог-метрика: Rust-агрегат `blogStats` + живой виджет

По порядку опций §29: (A). Археология показала, что `dashboardStats` (apps/server) уже отдаёт `totalPosts` — но это **content-модуль** (COUNT `nodes` WHERE kind='post'), не блог (`blog_posts`); домены разные, переиспользовать нельзя. Блоговые списки keyset-пагинированы и принципиально «never count rows» (PostListResponse без total) — счётчика в модуле не было вовсе.

### 30.1. Rust (5 файлов, ~124 строк, зеркалит паттерны модуля)

- **dto/post.rs**: `PostStatusCounts { total, draft, published, archived }` (u64, derives как у PostListResponse).
- **services/post/queries.rs**: `PostService::count_posts_by_status(tenant_id, security)` — nil-guard + `enforce_scope(List)` (как у list_posts); видимость зеркалит листинг: All-scope читатели видят счётчики тенанта, Own-scope авторы — только свои (AuthorId-фильтр), остальные — `BlogError::forbidden`. Три COUNT-запроса по каноническим storage-статусам через `status_to_storage` (единственный владелец словаря), `saturating_add` для total.
- **graphql/types.rs**: `GqlBlogStats` (SimpleObject, u64 — как GqlOrderList commerce).
- **graphql/query.rs**: резолвер `blogStats(tenantId: UUID!)` — авторизация зеркалит widgets::authorize (unauthenticated → AuthContext; BLOG_POSTS_READ через `has_any_effective_permission`; `query_tenant_id` с tenant-match), module-enabled, сервис через `BlogGraphqlRuntimeData::attached(ctx).post_service(...)` (taxonomy-порт доезжает до счётчика тем же путём, что и до листинга), телеметрия `metrics::record_read_path_query("graphql","blog.blog_stats","service_count",...)`.
- **services/post/mod.rs**: импорт PostStatusCounts в dto-блок.

### 30.2. next-admin (3 файла)

- **packages/blog/src/api/stats.ts** (новый): `BLOG_STATS_QUERY` + `fetchBlogStats(opts)` — fail-closed креденшелы, как у всего пакета.
- **index.ts**: экспорт api/stats.
- **widgets/blog-dashboard-widgets.tsx**: `BlogMetricsWidget` — живой счётчик (loading «…» / error «—» + helper), подпись `published N · draft M` (было навсегда «-»). Паттерн фетча — зеркало §29 (useEffect + cancelled-флаг; tenantId приходит из AdminRenderContext §29.3).

### 30.3. Расширение гейта (репин на живую форму)

`verify-blog-nextjs-graphql-contract.mjs`: CLIENT_DOCUMENTS + `apps/next-admin/packages/blog/src/api/stats.ts` — гейт парсит Rust-схему (types.rs + query.rs резолверы) и валидирует клиентские GraphQL-документы; теперь он **статически сертифицирует контракт `blogStats`**: корневое поле, аргумент и поля total/draft/published/archived согласованы между Rust и клиентом. Прогон: PASS, «69 Blog schema types checked against 8 client documents».

### 30.4. Верификация

- Blog-гейты: **25 PASS / 40 RED**; worktree-сертификация @58ccfb8: дифф rustok-blog против базы = ровно 5 файлов §30, **40 красных побайтово идентичны базе** (comments-audit/tcp-delegation/taxonomy/tag — infra-evidence гейты, born-red) — 0 регрессий. Процедурный урок: /tmp между вызовами вычищается — worktree-сертификацию делать одним bash-вызовом.
- Контракт-гейт с расширенным клиентским списком: PASS.
- esbuild: stats.ts / widget / index.ts — 3/3.
- Пакетные тесты: commerce 8/8, forum 7/7; blog model-тесты 0/2 — окружение (`Cannot find package 'zod'`, node_modules не установлены, CI-side; правки §30 модель не трогали).
- Смежные зоны без дрейфа: data-table-host PASS; ffa-contract прежний ENOENT born-red; commerce-admin 23/40; product+pricing 77/11 (born-red реестр).
- Rust без cargo: код зеркалирует живые паттерны модуля (`.count(&self.db)` из scheduled.rs:157; enforce_scope/can_read_non_public_posts/PermissionScope::Own — из list_posts; u64 в SimpleObject — прецедент GqlOrderList), самопроверка ханков построчно.

### 30.5. Остаток

- `blog_stats` не попал в REST-админку блога (controllers/) — GraphQL-поверхность самодостаточна для дашборда; REST-зеркало при потребности.
- 40 born-red blog-гейтов — инфраструктурные evidence/audit-гейты (postgres-раннеры, tcp-delegation) — вне статического аудита.
