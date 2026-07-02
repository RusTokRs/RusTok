# Документация `rustok-search`

`rustok-search` — dedicated core search module платформы. Локальная документация
модуля должна описывать сам search runtime, а не смешивать его с `rustok-index`
или host-specific UI wiring.

## Назначение

- публиковать канонический search API и runtime contracts;
- держать search document materialization, ranking и query normalization внутри модуля;
- развивать admin/storefront search surfaces поверх общего backend contract.

## Зона ответственности

- `search_documents` и связанные search-owned словари/analytics storage;
- search query parsing, ranking, filter presets, typo tolerance и merchandising rules;
- admin/storefront query surfaces и module-owned UI packages;
- observability, rebuild и diagnostics для search state;
- optional connector model для внешних search engines.

## Интеграция

- остаётся отдельным модулем по отношению к `rustok-index`: `search` отвечает за UX, ranking и engine semantics, а не за shared indexed read-model substrate;
- использует PostgreSQL как baseline engine и может расширяться отдельными connector crates;
- публикует module-owned миграции `search_settings`, `search_documents`, query analytics, dictionaries и typo-tolerance indexes; server migrator обязан подключать их как часть backend schema wiring, иначе admin/storefront search bootstrap не считается рабочим;
- должен держать Leptos и Next UI surfaces на одном backend contract;
- GraphQL query/mutation/types живут в `rustok-search`; `apps/server` только композирует roots и передаёт host runtime context, включая rate-limit adapter;
- event-driven ingestion публикуется модулем через `SearchModule::register_event_listeners(...)` и подключается сервером через `ModuleRegistry`, без отдельного host-owned search dispatcher;
- доменные модули поставляют изменения через ingestion path, не зная об активном engine.

## Projection correctness

- Search projector операции tenant-scoped: ingestion всегда берёт `tenant_id` из `EventEnvelope`, а `PgSearchEngine` требует `SearchQuery.tenant_id`.
- Повторная доставка событий не должна портить read model: projector выполняет scoped delete + rebuild/upsert в транзакции, а materialized rows пишутся через stable `document_key`.
- `search_documents.document_key` является primary key; content/product materialization использует `ON CONFLICT (document_key) DO UPDATE`, поэтому повторный upsert обновляет существующую строку, а не создаёт дубль.
- Product catalog search читает нормализованные highload-проекции, которые строит `rustok-index`: `index_product_categories` для primary/additional/materialized virtual category assignments и `index_product_attribute_values` для effective attribute facet/search/sort rows.
- GraphQL search input поддерживает optional `channelId`, `categoryIds`, `attributeFilters`, `sortAttributeCode` и `sortDesc`. Если `channelId` не задан, PostgreSQL engine читает только global rows (`channel_id IS NULL`); если задан, читает только строки этого канала без fallback-цепочки.
- Leptos admin/storefront DTO поддерживают те же catalog filters/sort поля: admin native `#[server]` является основным внутренним transport, GraphQL остаётся параллельным. Локализованные facet labels приходят из projection для effective locale, заданной host, без package-local query/header/cookie fallback.
- Leptos и Next admin/storefront surfaces показывают catalog filter/sort controls поверх того же контракта. Leptos storefront хранит selection state в URL через typed `snake_case` query keys (`channel_id`, `category_ids`, `attribute_code`, `attribute_values`, `sort_attribute_code`, `sort_desc`), а Next packages получают locale только от host/runtime props.
- Search UI surfaces поддерживают picker-ready host metadata: host может передать category/attribute option lists, а Leptos/Next packages покажут datalist-подсказки без прямого импорта `rustok-product`. Реальный источник этих options должен оставаться product-owned public/admin transport.
- Next admin host уже передаёт реальные category/attribute options в search playground через product-owned GraphQL helpers из `packages/rustok-product`; labels запрашиваются с host effective locale, а при ошибке metadata search surface остаётся доступным без подсказок.
- Фильтр по категории работает через `index_product_categories`, поэтому materialized virtual categories участвуют в listing/search так же, как structural/collection assignments.
- Атрибутные facets строятся только из effective `is_filterable = TRUE` и `is_detached = FALSE` rows. Bucket `value` остаётся стабильным `facet_bucket_key`, а optional `label` отдаёт локализованную подпись из projection для клиентов, которым нужен display text.
- Exact-query pinned rules не догружают raw `search_documents` items при catalog filters; pinned result может подняться только если он уже попал в отфильтрованный result set.
- Restart recovery выполняется через `SearchProjector::ensure_bootstrap`: если для tenant нет `search_documents`, запускается tenant-wide rebuild.
- Миграция `m20260324_000002_create_search_documents` создаёт `search_vector`, trigger обновления `tsvector`, GIN index `idx_search_documents_fts` и tenant-aware btree indexes `idx_search_documents_lookup` / `idx_search_documents_entity`.
- Миграция `m20260325_000006_add_search_typo_tolerance_indexes` включает `pg_trgm` и создаёт GIN trigram indexes для `title`, `slug`, `handle` и `keywords_text`.
- GiST index для текущего PostgreSQL baseline не используется: FTS и typo-tolerant path рассчитаны на GIN indexes. Если появится GiST-specific search strategy, её нужно оформить отдельной миграцией и query-plan evidence.
- Live PostgreSQL gate `tests/postgres_query_plan.rs` создаёт 100 000 временных
  документов, выполняет `EXPLAIN (ANALYZE, BUFFERS)` и проверяет GIN FTS/trigram
  indexes. Baseline от 2026-06-27: FTS `6.627 ms`, typo fallback `327.516 ms`.
- Typo fallback строит кандидатов через `UNION` четырёх индексируемых веток
  (`title`, `slug`, `handle`, `keywords_text`), чтобы общий `OR` не деградировал
  в parallel sequential scan.
- Product-owned Leptos admin transport публикует нейтральный `fetch_catalog_search_options`: current-tenant native `#[server]` endpoint является основным путём, GraphQL остаётся параллельным fallback. `apps/admin::SearchAdminComposition` уже передаёт host effective locale/auth/tenant, проверяет включённость `product` и маппит options в публичный search DTO без импорта product internals внутри search UI.
- Product-owned Leptos storefront transport отдельно публикует public-safe category/attribute options через native `#[server]` first и GraphQL `storefrontCatalogSearchOptions(locale: String!)`. `apps/storefront::SearchStorefrontComposition` передаёт host locale, проверяет включённость `product` и маппит owner DTO в `SearchCatalogFilterOption`; search storefront package не импортирует product internals.
- Next storefront повторяет тот же boundary через `apps/next-frontend/src/features/search`: host передаёт route locale/tenant slug, product-owned `apps/next-frontend/packages/rustok-product` читает `storefrontCatalogSearchOptions(locale: String!)`, а search package получает только category/attribute option props.

## Проверка

- `cargo xtask module validate search`
- `cargo xtask module test search`
- `cargo test -p rustok-search -- --include-ignored --nocapture` с live PostgreSQL `DATABASE_URL`
- targeted tests для query normalization, ranking profiles, rebuild flows и diagnostics surfaces

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Observability runbook](./observability-runbook.md)
- [ADR: boundary `index != search`](../../../DECISIONS/2026-03-29-index-search-boundary.md)
