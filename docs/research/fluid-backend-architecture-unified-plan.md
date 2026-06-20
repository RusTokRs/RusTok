# Единый план реализации Fluid Backend Architecture (FBA) для RusTok

Этот документ — **единственный актуальный план внедрения FBA** в RusTok.
Он заменяет разрозненные плановые материалы и задаёт обязательную последовательность этапов.

Связанный концептуальный документ: [Fluid Backend Architecture для RusTok](./fluid-backend-architecture.md).


Связка [Fluid Frontend Architecture (FFA)](./fluid-frontend-architecture.md) и [Fluid Backend Architecture (FBA)](./fluid-backend-architecture.md) даёт RusTok переносимость модулей между embedded и headless/remote профилями без переписывания core-логики.


## 0) Визуальный контекст админок (UI parity)

Ниже — иллюстрации двух runtime-вариантов админки, которые должны сохранять одинаковую
бизнес-семантику и навигационный контракт в рамках FFA+FBA.

### Leptos-вариант (SSR-first)

![Leptos Admin Dashboard](https://github.com/user-attachments/assets/leptos-admin-dashboard)

*Описание:* тёмная SSR-first админка с блоками `Total users / Content nodes / Orders / Revenue snapshot`,
блоком `Recent activity` и виджетом `Enabled modules`. Это референс для Leptos-host профиля
(`apps/admin`) и module-owned UI route contract.

### Next-вариант (headless/runtime parity)

![Next Admin Dashboard](https://github.com/user-attachments/assets/next-admin-dashboard)

*Описание:* Next-host вариант с тем же смысловым набором: дашборд метрик, активность,
операторские действия и модульная навигация. Это референс для `apps/next-admin`, где должен
сохраняться parity с Leptos-вариантом по данным, ролям и сценариям.

> Примечание: визуальный стиль может отличаться, но контракт FFA+FBA требует стабильности
> доменных сценариев, прав доступа, route/query semantics и backend orchestration behavior.

---

## 1) Цель и границы

## 1.1 Цель

Переводить отдельные module boundaries в remote execution profile (например, gRPC/async worker) **без переписывания domain/application-логики**.

## 1.2 Архитектурный инвариант FBA

Во всех этапах сохраняется:

- `module identity` (slug/ownership/область ответственности);
- `service contract` (команды, query, события);
- `domain rules` и policy semantics.

Меняется только `runtime topology`: embedded / remote / hybrid.

## 1.3 Что запрещено

- «Каждый crate = микросервис».
- Дублирование бизнес-логики по transport handlers.
- Прямой доступ к чужим таблицам после формализации портов.
- Ранний переход на service-owned DB до зрелости портов/событий/наблюдаемости.

---

## 2) Структура этапов (обязательный порядок)

1. **Этап A — Аудит и готовность модулей**
2. **Этап B — Базовые контракты FBA (до транспорта)**
3. **Этап C — Событийная дисциплина и contract testing**
4. **Этап D — Пилот 1 (async/read-boundary)**
5. **Этап E — Пилот 2 (Inventory Reservation)**
6. **Этап F — Пилот 3 (Payment/Fulfillment/Product read/Pricing)**
7. **Этап G — Выборочная storage-decomposition и write extraction**

Переход к следующему этапу допускается только после выполнения Exit Criteria текущего.

## 2.1 Текущие FBA-треки и единый шаблон

На 2026-06-14 в репозитории уже есть несколько FBA-треков. Они не должны переводиться
разными способами: новые и существующие инкременты обязаны сходиться к одному шаблону
`provider/consumer metadata + нейтральные ports + typed errors + locked contract-test/fallback-smoke metadata + fallback/rollout evidence`.

| Модуль | Текущая роль | Статус | Единообразный следующий шаг | Источник evidence |
|---|---|---|---|---|
| `page_builder` | reference provider для `preview/tree/properties/publish` | `in_progress` | Продолжить после первого migration slice: `PageBuilderCapabilityService` уже принимает `PortContext`, следующий шаг — capability handlers и contract tests до `boundary_ready` без смены provider/consumer metadata format | `crates/rustok-page-builder/contracts/page-builder-fba-registry.json`, `crates/rustok-page-builder/docs/implementation-plan.md` |
| `pages` | первый consumer reference provider-а `page_builder` | `in_progress` | Заменить synthetic Wave 0 evidence фактическими tenant before/after snapshots и smoke/trace packet | `crates/rustok-pages/docs/implementation-plan.md`, registry page-builder |
| `commerce` | umbrella orchestration/readiness-hardening для ecommerce slices | `in_progress` | Выравнивать checkout/post-order boundaries под тот же шаблон: owner-module ports, typed errors/context, events и отсутствие rules в transport/UI; consumer registry теперь фиксирует зависимости checkout orchestration от pricing/inventory/order/payment/fulfillment/cart provider contracts и mirrors payment/fulfillment provider SPI default-provider/lifecycle metadata, а payment/fulfillment provider SPI source markers/source paths/explicit lifecycle owner service names/default provider ids/capability fields проверяются тем же fast gate | `crates/rustok-commerce/contracts/commerce-fba-registry.json`, `crates/rustok-commerce/docs/implementation-plan.md` |
| `product` | provider catalog read-projection для commerce checkout/storefront и pricing enrichment consumers | `in_progress` | Закрыть runtime contract tests/fallback smoke для `ProductCatalogReadPort`, подтвердить embedded/GraphQL catalog parity snapshots перед повышением статуса | `crates/rustok-product/src/ports.rs`, `crates/rustok-product/contracts/product-fba-registry.json`, `crates/rustok-product/docs/implementation-plan.md` |
| `pricing` | provider read-projection для checkout/product price consumers | `in_progress` | Закрыть contract tests для `PricingReadPort` и заменить embedded/GraphQL compatibility evidence фактическими parity snapshots перед повышением статуса | `crates/rustok-pricing/src/ports.rs`, `crates/rustok-pricing/contracts/pricing-fba-registry.json`, `crates/rustok-pricing/docs/implementation-plan.md` |
| `inventory` | provider availability/reservation для checkout/product inventory consumers | `in_progress` | Закрыть contract tests/fallback smoke для `InventoryReservationPort`, подтвердить idempotency/write semantics и storefront projection parity перед повышением статуса | `crates/rustok-inventory/src/ports.rs`, `crates/rustok-inventory/contracts/inventory-fba-registry.json`, `crates/rustok-inventory/docs/implementation-plan.md` |
| `order` | provider checkout completion/result для commerce orchestration | `in_progress` | Закрыть contract tests для `CheckoutCompletionPort`, зафиксировать идемпотентность completion и remote-fallback smoke до `boundary_ready` | `crates/rustok-order/src/ports.rs`, `crates/rustok-order/contracts/order-fba-registry.json`, `crates/rustok-order/docs/implementation-plan.md` |
| `payment` | provider payment collection create/reuse для commerce checkout | `in_progress` | Закрыть contract tests для `PaymentCollectionPort`; provider SPI metadata/source markers/source path/explicit lifecycle owner/default provider id/capability fields уже фиксируются fast-verifier-ом без переноса gateway rules в commerce transport | `crates/rustok-payment/src/ports.rs`, `crates/rustok-payment/contracts/payment-fba-registry.json`, `crates/rustok-payment/docs/implementation-plan.md` |
| `customer` | provider read-projection для commerce checkout/order customer consumers | `in_progress` | Закрыть contract tests/fallback smoke для `CustomerReadPort`, подтвердить embedded/GraphQL checkout compatibility snapshots перед повышением статуса | `crates/rustok-customer/src/ports.rs`, `crates/rustok-customer/contracts/customer-fba-registry.json`, `crates/rustok-customer/docs/implementation-plan.md` |
| `cart` | provider checkout snapshot для commerce checkout consumers | `in_progress` | Закрыть contract tests/fallback smoke для `CartSnapshotReadPort`, подтвердить embedded/GraphQL checkout/storefront compatibility snapshots перед повышением статуса | `crates/rustok-cart/src/ports.rs`, `crates/rustok-cart/contracts/cart-fba-registry.json`, `crates/rustok-cart/docs/implementation-plan.md` |
| `tax` | provider tax calculation для cart tax-line consumers | `in_progress` | Закрыть runtime contract tests/fallback smoke для `TaxCalculationPort`, подтвердить cart/order tax snapshot parity перед повышением статуса | `crates/rustok-tax/src/ports.rs`, `crates/rustok-tax/contracts/tax-fba-registry.json`, `crates/rustok-tax/docs/implementation-plan.md` |
| `comments` | provider generic comment thread boundary для blog/commentable-surface consumers | `in_progress` | Закрыть runtime contract tests/fallback smoke для `CommentsThreadPort`, подтвердить blog embedded/native compatibility snapshots перед повышением статуса | `crates/rustok-comments/src/ports.rs`, `crates/rustok-comments/contracts/comments-fba-registry.json`, `crates/rustok-comments/docs/implementation-plan.md` |
| `blog` | consumer generic comment thread boundary от `comments` | `in_progress` | Закрыть real runtime consumer contract execution для `blog_post_comments`; no-compile source-smoke уже подтверждает source markers для degraded modes `hide_comment_form`/`show_cached_thread_snapshot` без повышения статуса | `crates/rustok-blog/contracts/blog-fba-registry.json`, `crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json`, `crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json`, `crates/rustok-blog/docs/implementation-plan.md` |
| `media` | provider asset read/image descriptor boundary для SEO/AI media consumers | `in_progress` | Запустить executable runtime contract tests/fallback smoke для `MediaAssetReadPort`; source-locked degraded modes зафиксированы в `media-runtime-fallback-smoke.json`, но статус не повышается без runtime execution | `crates/rustok-media/src/ports.rs`, `crates/rustok-media/contracts/media-fba-registry.json`, `crates/rustok-media/docs/implementation-plan.md` |
| `seo` | consumer image descriptor boundary от `media` | `in_progress` | Закрыть runtime consumer contract execution/fallback smoke для `seo_image_descriptor`; static matrix уже `source_locked_pending_consumer_runtime` и подтверждает provider fallback-smoke source/degraded modes `omit_image_metadata`/`keep_existing_seo_image`, но не повышает статус без runtime evidence | `crates/rustok-seo/contracts/seo-fba-registry.json`, `crates/rustok-seo/contracts/evidence/seo-media-consumer-static-matrix.json`, `crates/rustok-seo/docs/implementation-plan.md` |
| `ai-media` | support consumer image asset descriptor boundary от `media` | `in_progress` | Runtime fallback source-smoke для `ai_asset_descriptor` закрыт; следующий шаг — executable contract tests перед повышением статуса | `crates/rustok-ai-media/contracts/ai-media-fba-registry.json`, `crates/rustok-ai-media/contracts/evidence/ai-media-consumer-static-matrix.json`, `crates/rustok-ai-media/contracts/evidence/ai-media-runtime-fallback-smoke.json`, `crates/rustok-ai-media/docs/implementation-plan.md` |
| `ai-alloy` | support adapter script execution policy boundary для Alloy vertical | `in_progress` | Source-level policy registry фиксирует `alloy_script_execution_policy`; следующий шаг — executable targeted tests/evidence | `crates/rustok-ai-alloy/contracts/ai-alloy-policy-registry.json`, `crates/rustok-ai-alloy/contracts/evidence/ai-alloy-policy-static-matrix.json`, `crates/rustok-ai-alloy/docs/implementation-plan.md` |
| `search` | provider search query/suggestions boundary для storefront/admin consumers | `in_progress` | Закрыть runtime contract tests/fallback smoke для `SearchQueryPort`/`SearchSuggestionPort`, подтвердить embedded PostgreSQL/native+GraphQL search parity snapshots перед повышением статуса | `crates/rustok-search/src/ports.rs`, `crates/rustok-search/contracts/search-fba-registry.json`, `crates/rustok-search/docs/implementation-plan.md` |
| `fulfillment` | provider seller-aware shipping selection для commerce checkout | `in_progress` | Закрыть contract tests для `ShippingSelectionPort`; provider SPI metadata/source markers/source path/explicit lifecycle owner/default provider id/capability fields уже фиксируются fast-verifier-ом, далее сохранить fallback/degraded profiles и перенести оставшийся selection transport из commerce compatibility path | `crates/rustok-fulfillment/src/ports.rs`, `crates/rustok-fulfillment/contracts/fulfillment-fba-registry.json`, `crates/rustok-fulfillment/docs/implementation-plan.md` |
| `rbac` | provider permission-decision boundary для admin consumers | `in_progress` | Закрыть runtime fallback smoke для `RbacPermissionDecisionPort`, подтвердить claims-scope/degraded action hiding semantics перед повышением статуса | `crates/rustok-rbac/src/ports.rs`, `crates/rustok-rbac/contracts/rbac-fba-registry.json`, `crates/rustok-rbac/docs/implementation-plan.md` |
| `tenant` | provider read-projection boundary для server-host tenant resolution consumers | `in_progress` | Закрыть runtime contract tests/fallback smoke для `TenantReadPort`, подтвердить inactive-tenant degraded modes и resolver/cache parity evidence перед повышением статуса | `crates/rustok-tenant/src/ports.rs`, `crates/rustok-tenant/contracts/tenant-fba-registry.json`, `crates/rustok-tenant/docs/implementation-plan.md` |
| `workflow` | provider read-projection boundary для workflow admin consumers | `in_progress` | Закрыть runtime contract tests/fallback smoke для `WorkflowReadPort`, подтвердить native/GraphQL admin read projection parity snapshots перед повышением статуса | `crates/rustok-workflow/src/ports.rs`, `crates/rustok-workflow/contracts/workflow-fba-registry.json`, `crates/rustok-workflow/docs/implementation-plan.md` |
| `forum` | deferred consumer candidate для `page_builder` | `not_started` | Не повышать статус до появления local consumer evidence; держать запись как deferred в provider registry | `crates/rustok-page-builder/contracts/page-builder-fba-registry.json` |

Правила единообразия:

1. **FBA остаётся названием rollout-а, а не обязательным префиксом типов.** Code-facing контракты используют нейтральные имена (`PortContext`, `PortError`, `*Port`, `provider`, `consumer`).
2. **Источник статуса — local `docs/implementation-plan.md`, центральный board синхронизируется в том же изменении.** Нельзя оставлять `not_started`, если есть активный FBA provider/consumer evidence.
3. **Machine-readable metadata обязательна для provider/consumer tracks.** Для `page_builder -> pages` источником является `page-builder-fba-registry.json`; следующие tracks должны переиспользовать тот же формат или явно расширять его в этом плане, а не создавать параллельный формат.
4. **Нейтральные port primitives применяются только к новым/обновляемым портам.** Уже существующие FBA slices не переписываются механически без feature work; при следующем изменении они приводятся к тем же `context/error/idempotency/deadline` требованиям.
5. **Повышение до `boundary_ready` или `transport_verified` требует evidence.** Наличие metadata или FFA split само по себе не считается remote/runtime verification.

## 2.2 Структурный стандарт перевода модуля

Да, единый стандарт есть. Для каждого нового FBA-инкремента обязательна одинаковая
структура артефактов; отсутствие одного из пунктов ниже считается gap и не даёт повышать
статус выше `in_progress`:

1. **Local source of truth:** `crates/<module>/docs/implementation-plan.md` содержит
   `## FFA/FBA status`, текущую роль (`provider`, `consumer`, `orchestrator`, `support`)
   и evidence по boundary/metadata/verification.
2. **Central status:** `docs/modules/registry.md` содержит синхронную строку readiness board
   с тем же FBA-статусом и ссылкой на local plan.
3. **Runtime metadata:** `rustok-module.toml` или module-owned machine-readable registry
   фиксирует provider/consumer dependency profile, contract versions, degraded modes и
   toggle/fallback profiles, если модуль участвует в provider/consumer track.
4. **Contract location:** transport-neutral DTO/port/error contracts живут в owner module
   или shared foundation crate только если они действительно cross-module; host apps не
   становятся владельцами domain/application contract.
5. **Verification location:** рядом с machine-readable metadata есть anti-drift/fallback gate
   (`scripts/verify/*` или module-local verifier), а local plan перечисляет command/evidence.
6. **Evidence packet:** для Wave/pilot rollout есть фактические или явно помеченные
   synthetic before/after snapshots, smoke outcomes, metrics/traces и keep/rollback decision.
7. **Docs sync:** если меняется FBA status, provider/consumer metadata, ports/events, routing,
   tenancy, UI contract или observability, одновременно обновляются local docs, central board
   и этот unified plan, если меняется сам стандарт.


На 2026-06-16 ecommerce provider track дополнительно получил единый static evidence layer для будущих contract tests: `pricing`, `inventory`, `order`, `payment`, `fulfillment`, `customer` и `cart` имеют `contracts/evidence/*-contract-test-static-matrix.json`, команда `npm run verify:ecommerce:fba` запускает registry + evidence gates, включая проверку соответствия evidence-пакетов provider registry cases/fallback profiles через `npm run verify:ecommerce:fba-contract-evidence`. Это всё ещё не повышает статус до `boundary_ready`: runtime execution и fallback smoke остаются отдельным gate.

На 2026-06-18 fast gate `npm run verify:ecommerce:fba-registries` дополнительно проверяет in-process provider implementations на уровне каждой операции, если registry объявляет `in_process_provider_impl`: read operations должны вызывать `require_deadline_semantics()?`, write operations с `idempotency_required = true` должны вызывать `require_write_semantics()?`, а read operations не должны случайно требовать write-idempotency. Это закрывает anti-drift для typed context/deadline/idempotency semantics без запуска долгой компиляции, но также не повышает статус без runtime contract execution. Для `order.checkout_completion.v1` тот же fast gate теперь дополнительно блокирует преждевременный `OrderService` in-process implementation (и registry metadata, и source-level impl), пока registry не содержит `runtime_evidence.checkout_completion_owner_path.status = "runtime_verified"`; это защищает checkout completion boundary от фиктивного embedded provider без cart/result projection evidence.

На 2026-06-18 `product` добавлен в ecommerce provider track как catalog read provider: `ProductCatalogReadPort`/`product.catalog_read.v1`, registry `crates/rustok-product/contracts/product-fba-registry.json` и static evidence `crates/rustok-product/contracts/evidence/product-contract-test-static-matrix.json` включены в `npm run verify:ecommerce:fba` вместе с product dependency в commerce consumer registry.


На 2026-06-18 `customer` добавлен в ecommerce provider track как customer read-projection provider: `CustomerReadPort`/`customer.read_projection.v1`, registry `crates/rustok-customer/contracts/customer-fba-registry.json` и static evidence `crates/rustok-customer/contracts/evidence/customer-contract-test-static-matrix.json` включены в `npm run verify:ecommerce:fba` вместе с customer dependency в commerce consumer registry.


На 2026-06-18 `cart` добавлен в ecommerce provider track как checkout snapshot provider: `CartSnapshotReadPort`/`cart.checkout_snapshot.v1`, registry `crates/rustok-cart/contracts/cart-fba-registry.json` и static evidence `crates/rustok-cart/contracts/evidence/cart-contract-test-static-matrix.json` включены в `npm run verify:ecommerce:fba` вместе с cart dependency в commerce consumer registry.

На 2026-06-18 `tax` добавлен как support provider track для cart tax calculation: `TaxCalculationPort`/`tax.calculation.v1`, registry `crates/rustok-tax/contracts/tax-fba-registry.json` и static evidence `crates/rustok-tax/contracts/evidence/tax-contract-test-static-matrix.json` проверяются быстрым gate `npm run verify:tax:fba` без повышения до `boundary_ready` до runtime execution/fallback smoke.

На 2026-06-18 `comments` добавлен как provider track для generic comment threads: `CommentsThreadPort`/`comments.thread.v1`, registry `crates/rustok-comments/contracts/comments-fba-registry.json` и static evidence `crates/rustok-comments/contracts/evidence/comments-contract-test-static-matrix.json` проверяются быстрым gate `npm run verify:comments:fba` без повышения до `boundary_ready` до runtime execution/fallback smoke.

На 2026-06-19 `search` добавлен как provider track для search query/suggestions boundary: `SearchQueryPort`/`SearchSuggestionPort`/`search.query.v1`, registry `crates/rustok-search/contracts/search-fba-registry.json` и static evidence `crates/rustok-search/contracts/evidence/search-contract-test-static-matrix.json` проверяются быстрым gate `npm run verify:search:fba` без повышения до `boundary_ready` до runtime execution/fallback smoke.

На 2026-06-19 `blog` добавлен как consumer track для `comments.thread.v1`: registry `crates/rustok-blog/contracts/blog-fba-registry.json`, static evidence `crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json`, no-compile source-smoke `crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json`, manifest `[fba.consumer]` и быстрый gate `npm run verify:blog:fba` фиксируют `CommentsThreadPort` dependency, fallback profiles и degraded modes без повышения до `boundary_ready` до real runtime execution.

На 2026-06-19 `media` добавлен как provider track для asset read/image descriptor boundary: `MediaAssetReadPort`/`media.asset_read.v1`, registry `crates/rustok-media/contracts/media-fba-registry.json` и static evidence `crates/rustok-media/contracts/evidence/media-contract-test-static-matrix.json` и source-locked fallback smoke `crates/rustok-media/contracts/evidence/media-runtime-fallback-smoke.json` проверяются быстрым gate `npm run verify:media:fba` без повышения до `boundary_ready` до executable runtime execution/fallback smoke.

На 2026-06-19 `seo` добавлен как consumer track для `media.asset_read.v1`: registry `crates/rustok-seo/contracts/seo-fba-registry.json`, static evidence `crates/rustok-seo/contracts/evidence/seo-media-consumer-static-matrix.json`, manifest `[fba.consumer]` и быстрый gate `npm run verify:seo:fba` фиксируют `MediaAssetReadPort` dependency, provider fallback-smoke source, fallback profiles и degraded modes в состоянии `source_locked_pending_consumer_runtime` без повышения до `boundary_ready` до consumer runtime execution/fallback smoke.

На 2026-06-20 `ai-media` имеет support-consumer track для `media.asset_read.v1`: registry `crates/rustok-ai-media/contracts/ai-media-fba-registry.json`, static evidence `crates/rustok-ai-media/contracts/evidence/ai-media-consumer-static-matrix.json`, runtime fallback source-smoke `crates/rustok-ai-media/contracts/evidence/ai-media-runtime-fallback-smoke.json` и быстрый gate `npm run verify:ai-media:fba` фиксируют `MediaAssetReadPort` dependency, adapter source markers, fallback profile и degraded mode без повышения до `boundary_ready` до executable runtime tests. `ai-alloy` добавлен как source-level policy track: `crates/rustok-ai-alloy/contracts/ai-alloy-policy-registry.json` и `alloy_script_execution_policy` фиксируют ownership of script runtime payload policy while runtime composition stays in `rustok-ai`.

На 2026-06-19 `workflow` добавлен как provider track для admin read-projection boundary: `WorkflowReadPort`/`workflow.read_projection.v1`, registry `crates/rustok-workflow/contracts/workflow-fba-registry.json` и static evidence `crates/rustok-workflow/contracts/evidence/workflow-contract-test-static-matrix.json` проверяются быстрым gate `npm run verify:workflow:fba` без повышения до `boundary_ready` до runtime execution/fallback smoke.

На 2026-06-19 `rbac` добавлен как provider track для admin permission-decision boundary: `RbacPermissionDecisionPort`/`rbac.permission_decision.v1`, registry `crates/rustok-rbac/contracts/rbac-fba-registry.json` и static evidence `crates/rustok-rbac/contracts/evidence/rbac-contract-test-static-matrix.json` проверяются быстрым gate `npm run verify:rbac:fba` без повышения до `boundary_ready` до runtime fallback smoke.

На 2026-06-20 `tenant` добавлен как provider track для tenant read-projection boundary: `TenantReadPort`/`tenant.read_projection.v1`, registry `crates/rustok-tenant/contracts/tenant-fba-registry.json` и static evidence `crates/rustok-tenant/contracts/evidence/tenant-contract-test-static-matrix.json` проверяются быстрым gate `npm run verify:tenant:fba` без повышения до `boundary_ready` до runtime contract/fallback smoke.

На 2026-06-20 `region` добавлен как provider track для region/country read-projection boundary: `RegionReadPort`/`region.read_projection.v1`, registry `crates/rustok-region/contracts/region-fba-registry.json` и static evidence `crates/rustok-region/contracts/evidence/region-contract-test-static-matrix.json` проверяются быстрым gate `npm run verify:region:fba` без повышения до `boundary_ready` до runtime contract/fallback smoke.

На 2026-06-20 `channel` добавлен как provider track для channel/default/host-target read-projection boundary: `ChannelReadPort`/`channel.read_projection.v1`, registry `crates/rustok-channel/contracts/channel-fba-registry.json` и static evidence `crates/rustok-channel/contracts/evidence/channel-contract-test-static-matrix.json` проверяются быстрым gate `npm run verify:channel:fba` без повышения до `boundary_ready` до runtime contract/fallback smoke.

Проверка структуры на текущем состоянии выявила один исправленный gap: `page_builder` уже
имел FBA provider metadata и registry, но отсутствовал в readiness board и не имел local
FFA/FBA status block. Теперь `page_builder` и `pages` отражены единообразно: local plan +
central board + machine-readable registry/evidence. Оставшиеся gaps не являются нарушением
стандарта, потому что явно зафиксированы как `not_started`/`deferred` или как compile/runtime
blocker в verification output.

## 2.3 Соответствие целевой crate-структуре

Предложенная ранее схема близка к целевой модели, но в RusTok она применяется с поправками
на текущую модульную платформу:

```text
crates/rustok-<module>/
  src/dto|domain|error      # доменные типы, DTO, errors; название папок может отличаться
  src/services|ports        # service layer и/или явные порты owner-модуля
  src/entities|migrations   # SeaORM storage ownership; repository interfaces появляются, когда нужен remote/test seam
  src/graphql|controllers   # transport adapters, thin mapping поверх service/port
  admin|storefront          # optional module-owned UI packages with core/transport/ui split
  rustok-module.toml        # runtime metadata, dependencies, provider/consumer FBA sections
  contracts/                # optional machine-readable registry/evidence для provider/consumer tracks

crates/rustok-<module>-grpc/ # optional late-stage adapter crate, не default requirement
  proto/schema              # gRPC/protobuf contract только после ADR/DoR
  server adapter            # вызывает тот же service/port, не содержит domain rules
  client adapter            # remote implementation того же port
  PortContext/error mapping # mapping нейтральных port primitives в transport metadata/status

apps/server/
  composition/root wiring   # module registry, GraphQL/REST/controllers, health/metrics
  transport selection       # future per-module runtime profile; сейчас в основном in-process/native/GraphQL
  public API                # host API не владеет domain rules
```

Ключевые отличия от схемы `service trait + in-process impl + repository interfaces` как
обязательного шаблона:

1. **Trait-порт вводится не механически.** Если модуль ещё не готов к remote/profile split,
   service struct остаётся допустимым owner service layer; trait/adapter выделяется при первом
   реальном boundary или contract-test инкременте.
2. **Repository interfaces не обязательны с первого PR.** Сейчас многие модули владеют SeaORM
   entities/migrations напрямую; abstraction seam добавляется, когда нужен remote adapter,
   test double или запрет foreign-table доступа.
3. **`rustok-<module>-grpc` — поздний optional adapter.** До закрытия DoR нельзя заводить gRPC
   crate только ради формы; сначала нужны stable port, `PortContext`, typed errors, events/outbox,
   contract tests и ADR.
4. **Transport adapters уже существуют, но не все remote.** GraphQL/REST/`#[server]` живут как
   thin adapters поверх owner service/port; gRPC станет ещё одним adapter profile, а не новой
   реализацией бизнес-логики.
5. **Machine-readable provider/consumer metadata уже является частью структуры.** Для
   `page_builder -> pages` это `page-builder-fba-registry.json` + `rustok-module.toml`; новые
   provider/consumer tracks должны повторять этот паттерн или расширять его в едином плане.

Итого: концепция совпадает по слоям (`domain/service-port/implementation/storage/adapter/server
wiring`), но RusTok standard не требует создавать все папки и `*-grpc` crate заранее. Структура
эволюционирует по readiness gates, чтобы не получить формальные интерфейсы без evidence.

---

## 3) Этап A — Аудит и readiness matrix

## 3.1 Обязательные артефакты

- `Module Inventory Table` (по каждому целевому модулю):
  - slug, owner, owned storage, публичные use-cases;
  - входящие/исходящие события;
  - зависимости (Cargo + modules graph);
  - роль: orchestrator/facade, write-model owner, read-model provider, support service.
- `Coupling Debt Register`:
  - прямые вызовы соседних доменов;
  - прямой SQL к чужим таблицам;
  - отсутствие idempotency/deadline;
  - event gaps (нет outbox/versioning/replay policy).
- `Readiness Matrix`: High / Medium / Low.

## 3.2 Критерии готовности этапа A

- Все модули в целевом скоупе имеют заполненную inventory-строку.
- Для каждого Medium/Low модуля зафиксирован remediation backlog.
- Для каждого кандидата на remote есть ADR-черновик с рисками и rollback-подходом.

---

## 4) Этап B — Базовые FBA-контракты (Ports before transports)

## 4.1 Единый `PortContext`

Стартовая shared-реализация находится в `rustok-api::ports` и намеренно остаётся transport-agnostic: это контрактный примитив для портов/адаптеров, а не доменный сервис.

Обязательные поля:

- tenant;
- actor/service identity;
- claims/role;
- channel + locale;
- correlation/causation + trace context;
- idempotency key (write);
- deadline/timeout/cancellation.

Правило: передаётся явным параметром каждого порта.

## 4.2 Unified error model

Единый набор доменных ошибок (validation/not_found/conflict/forbidden/unavailable/timeout/invariant violation) + предсказуемый mapping в REST/GraphQL/gRPC.

## 4.3 Портовый слой

Минимальный целевой набор портов:

- `ProductPort`, `PricingPort`, `InventoryPort`, `CartPort`,
- `OrderPort`, `PaymentPort`, `FulfillmentPort`, `TaxPort`.

Требование: сначала in-process impl, потом remote adapters.

## 4.4 Data ownership policy

- Модуль пишет/читает только свой storage.
- Межмодульный data access — только через порт/snapshot DTO/read model.

## 4.5 Критерии готовности этапа B

- Все целевые порты определены в transport-agnostic виде.
- `PortContext` и error model используются во всех новых/обновлённых портовых вызовах.
- Новые прямые foreign-table доступы не допускаются.

---

## 5) Этап C — События, outbox и контрактное тестирование

## 5.1 Event vocabulary

Для критичных доменов задать versioned vocabulary (например: `ProductPublished`, `PriceChanged`, `InventoryReserved`, `OrderPlaced`, `PaymentAuthorized`).

Каждое событие обязано иметь: tenant, aggregate id, schema version, correlation/causation, idempotency semantics.

## 5.2 Outbox discipline

- Запись domain state + outbox в одной транзакции.
- Публикация через worker/dispatcher.
- Consumer-ы idempotent + replay-safe + tolerant к out-of-order.

## 5.3 Contract tests

Для каждого порта один и тот же набор тестов запускается:

- против in-process impl;
- против remote adapter.

Бизнес-результат должен совпадать, отличия допустимы только по latency/failure envelope.

## 5.4 Критерии готовности этапа C

- Outbox включён для всех write owners в пилотном скоупе.
- Contract tests есть для всех портов пилотного скоупа.
- Есть сценарии replay/idempotency/out-of-order в тестах.

---

## 6) Этап D — Пилот 1 (async/read-boundary)

## 6.1 Кандидаты

- search/indexing;
- AI enrichment/recommendations.

## 6.2 Шаги

1. Вынести boundary в порт и адаптер (gRPC либо async worker — по характеру use-case).
2. Подключить переключение embedded/remote через runtime config.
3. Перевести вызовы host/facade на порт.
4. Проверить SLO: latency, error rate, throughput, retry behavior.

## 6.3 Exit Criteria

- Функциональный паритет с embedded профилем подтверждён.
- Метрики и трассировка стабильны минимум на согласованном окне наблюдения.

---

## 7) Этап E — Пилот 2 (Inventory Reservation)

## 7.1 Шаги

1. Ввести `reservation` модель: idempotency key, TTL/expiration, статусный lifecycle.
2. Закрепить события: `InventoryReserved`, `InventoryReservationReleased`, `InventoryAdjusted`.
3. Реализовать `InventoryPort` remote server/client.
4. Встроить компенсации в checkout saga (`release_reservation`).
5. Провести нагрузочные тесты на пиковых checkout-сценариях.

## 7.2 Exit Criteria

- Reservation команды retry-safe.
- Компенсации корректно отрабатывают на контролируемых сбоях.
- Нагрузочный профиль не деградирует ниже согласованных порогов.

---

## 8) Этап F — Пилот 3 (Payment/Fulfillment/Product read/Pricing)

Порядок обязателен:

1. `PaymentPort` и `FulfillmentPort` как remote adapters (внешние провайдеры).
2. `ProductPort` read-side snapshots (`get_product_snapshot`, `list_publishable_catalog_page`).
3. `PricingPort` после стабилизации product read contracts.
4. `TaxPort` как explicit support boundary (embedded/stateless remote/provider adapter — решается ADR).

## 8.1 Exit Criteria

- Нет прямого чтения product internals из pricing.
- Checkout orchestration работает через порты с теми же бизнес-результатами.
- Synchronous path и async post-processing разделены архитектурно.

---

## 9) Этап G — Поздние стадии (storage и write extraction)

Разрешённые режимы хранения:

1. shared DB + in-process;
2. shared DB + remote process;
3. service-owned DB;
4. read-model replica/projection.

Правило: переход к `service-owned DB` только после стабильной remote работы модуля, зрелой saga/outbox модели и утверждённого ADR.

---

## 10) Единый Definition of Ready для перевода модуля в remote

Модуль можно переводить в remote profile только при выполнении **всех** условий:

1. Stable transport-agnostic port + contract tests (in-process/remote).
2. Полный `PortContext` на всех командах/запросах.
3. Outbox + versioned events + replay/idempotency policy.
4. Отсутствие foreign-table доступа вне owner boundary.
5. Write методы имеют idempotency key и deadline semantics.
6. Health/readiness/metrics/tracing parity между профилями.
7. Отдельный ADR с причинами, рисками, rollback и ownership impact.

---

## 11) Минимальный квартальный rollout (шаблон)

- **Q1:** Этапы A+B.
- **Q2:** Этап C + Пилот 1.
- **Q3:** Пилот 2.
- **Q4:** Пилот 3 + решения по selective storage evolution.

Если Exit Criteria этапа не выполнены, следующий квартальный шаг не стартует.

---

## 12) Управление изменениями документа

- Этот документ — каноничный implementation plan по FBA.
- Изменения в sequence/criteria вносятся только вместе с обновлением связанных ADR.
- Новые «параллельные планы FBA» не создаются; расширения добавляются сюда.
