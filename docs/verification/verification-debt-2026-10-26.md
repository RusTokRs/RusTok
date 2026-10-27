# Разбор падающих проверок RusTok — инвентаризация и план

Ветка `arena/a90c8cc9-rustok`. Дата разбора: 2026-10-26. Замеры сделаны на `5d0d325`;
после прохода 5 (раздел 1.3) тот же прогон даёт **455 / 833**, после прохода 6 (раздел 1.4) —
**447 / 846**, после проходов 7–8 (раздел 1.5, 1.6) — **443 / 850**, после прохода 9
(раздел 1.7) — **436 / 857**, после проходов 10–13 (раздел 1.8) — **407 / 886**,
после проходов 14–15 (раздел 1.9) — **368 / 939** на 1307 guard'ах.

> Числа в этом документе получены двойным прогоном корпуса guard'ов: базовый коммит в
> `git worktree` и рабочее дерево, с записью stdout+stderr и кода выхода. Артефакты прогона —
> `guards.txt`, `base.status`, `now.status`, `triage.json` — лежат вне репозитория, в рабочей
> папке сессии; методика воспроизводится командами из раздела 0.

---

## 0. Коротко

| Показатель | Значение |
|---|---|
| Guard-скриптов `verify-*.mjs` (без `*.test.mjs`) | **1288** |
| Проходят на `5d0d325` | **823** |
| Падают | **465** |
| Проходят после прохода 5 | **833** |
| Падают после прохода 5 | **455** |
| Из них не guard'ы (CLI-инструменты, ждут аргументов) | **2** |
| Из них нужна среда (`node_modules`/сеть) | **5** |
| **Красных статических guard'ов** | **458** (465 минус 2 CLI и 5 env) |
| Регрессий в проходе 4 (`550afbe` → `5d0d325`) | **0** |
| Всего починено за четыре прохода | 40 guard'ов, 3 удалённых legacy-источника, 2 реальных дефекта в коде |
| Всего починено за пять проходов | 50 guard'ов, 3 удалённых legacy-источника, 2 реальных дефекта в коде |
| Проходят после прохода 6 | **846** (из 1293) |
| Падают после прохода 6 | **447** |
| Проходят после прохода 8 | **850** (из 1293) |
| Падают после прохода 8 | **443** |
| Проходят после прохода 9 | **857** (из 1293) |
| Падают после прохода 9 | **436** |
| Проходят после проходов 10–13 | **886** (из 1293) |
| Падают после проходов 10–13 | **407** |
| Регрессий в проходах 5–13 | **0** |
| Снято red'ов с прохода 10 (436 → 407) | **29** |
| Всего починено за девять проходов | 73 guard'а, 4 удалённых legacy-источника, 3 реальных дефекта в коде |

**Методика.** Один и тот же список файлов прогоняется дважды: на базовом коммите в
`git worktree` и на рабочем дереве, с записью stdout+stderr и кода выхода. Диф даёт два числа,
которым можно верить: сколько стало зелёными и сколько стало красными. Всё остальное —
классификация текста ошибок.

```
guards.txt     список 1288 файлов
base.status    коды выхода на 550afbe
base/          stdout+stderr каждого guard'а на 550afbe
now.status     коды выхода на 5d0d325
now/           stdout+stderr каждого guard'а на 5d0d325
triage.json    строка на красный guard: путь, класс, wiring
runone.sh      раннер одного файла
```

Воспроизведение:

```bash
git worktree add --detach ../base 550afbe
find scripts/verify crates -type f -name 'verify-*.mjs' -not -name '*.test.mjs' | sort -u > guards.txt
xargs -a guards.txt -P 8 -n 1 ./runone.sh base > base.status   # в worktree
xargs -a guards.txt -P 8 -n 1 ./runone.sh now  > now.status    # в рабочем дереве
```

Один прогон — около 36 секунд на 16 ядрах.

**Классы 465 красных:**

| Класс | Сколько | Что значит |
|---|---|---|
| `missing` | 360 | guard требует маркер, которого нет в коде |
| `other` | 74 | падает по своей причине (assert, исключение, парсинг) |
| `missing+forbidden` | 18 | и требует одного, и требует отсутствия другого |
| `forbidden` | 6 | находит то, что запрещено |
| `env:node-module` | 5 | нет `node_modules` (playwright и пр.) |
| `cli-not-a-guard` | 2 | CLI-инструмент, без аргументов печатает usage |

**Wiring (как guard вообще запускается):**

| | Сколько |
|---|---|
| нигде не подключён (orphan) | **408** |
| в GitHub workflow | 38 |
| в `package.json` скрипте | 16 |
| и там, и там | 3 |

То есть **87% красных guard'ов никто никогда не запускает**. Красный guard, которого нет в CI,
не защищает ничего — он просто лежит.

---

## 1. Что уже сделано (хронология)

| Проход | Коммит | Итог |
|---|---|---|
| 1 | `4a7f4df` | 5 нерабочих guard'ов + реальный дефект: `FulfillmentCreateLabelRecoveryService::retry` декодировал `request_payload` до `claim_execution` |
| 2 | `550afbe` | 27 guard'ов: FBA-контракты вместо wildcard-экспортов, пины GitHub Actions по имени+40-hex, снятие git-blob пинов, библиотека `source-text.mjs` |
| 3 | `b46b6d2` | ограниченная диагностика order-владельца + ретайрмент мёртвого дубликата пайплайна |
| 4 | `5d0d325` | ретайрмент `checkout_payment_stages_legacy.rs` и `checkout_fulfillment_stages_legacy.rs` |

### 1.1. Проход 3: утечка текста order-владельца (`b46b6d2`)

`checkout_order_stages.rs` читал заказ напрямую через `OrderService` и заворачивал ошибку в
`CheckoutOrderStageError::Conflict(format!("… {err}"))`; `staged_checkout.rs` сохранял
`pipeline.to_string()` в журнал операции через `mark_retryable_error` /
`mark_compensation_required`. Сырой `Display` ошибки rustok-order (включая текст `sea_orm::DbErr`
с SQL-деталями) попадал в **персистентное поле операции** и в API статуса checkout-операции.

Сделано:

1. `checkout_order_owner_failure.rs` — тотальная проекция всех вариантов `OrderError` на
   стабильный код `checkout.order_*` + статичное сообщение + retryability; текст владельца живёт
   только как `owner_message_shape`/`owner_message_len` за `Debug = redacted`.
2. Чтение проекции переведено на `CheckoutOrderRecoveryAdapter::read_checkout_order`
   (владелец сохраняет `require_policy(read)`), стадия больше не держит `OrderService`.
3. `OrderBoundary { code, message, retryable }` в creation/confirmation, `#[from] OrderError`
   удалены — новый вызов не обойдёт проекцию.
4. `bounded_failure()` отдаёт код и retryability в пайплайн: retryable-сбой заказа больше не
   эскалируется в `CompensationRequired`.
5. `pipeline_failure_message` персистит только commerce-authored текст, иначе — статичную фразу.
6. `checkout_stage_pipeline.rs` (575 строк, мёртвый дубликат смонтированного
   `checkout_stage_pipeline_owner_ports.rs`) удалён.

### 1.2. Проход 4: ретайрмент payment/fulfillment (`5d0d325`)

`checkout_payment_stages_legacy.rs` (600 строк) и `checkout_fulfillment_stages_legacy.rs`
(578 строк) были смонтированы только через `#[path]`/`include!` и несли «сырые» контекстные
диагностики (`tenant_id = %context.tenant_id`, `actor = ?context.actor`,
`correlation_id = %context.correlation_id`, `idempotency_key = ?context.idempotency_key`) под
`tracing_shim`, единственная задача которого — реэкспортировать настоящие макросы.

Не «перепин», а слияние: ограниченные адаптеры (`sanitize_owner_error`) и мапперы стадий, уже
жившие в фасадах, теперь лежат в смонтированных модулях (`payment_execution_boundary`,
`payment_execution_port`, `owner_execution_boundary`, `fulfillment_execution_port`,
`order_payment_settlement_port`). Удалены вместе с файлами: `rustok_api_shim`,
`rustok_payment_shim`, `rustok_fulfillment_shim`, `rustok_order_shim`, `tracing_shim`.

**Усиление, которого не было:** маппер теперь *пересчитывает* статичное сообщение из
типизированного `PortErrorKind`, а не берёт `error.message` из `BoundaryPortError`. Кастомный
порт (`with_payment_port` / `with_fulfillment_port` / `with_order_payment_port`) больше не может
протащить текст владельца через `Boundary`-вариант.

---

### 1.3. Проход 5: guard'ы, требующие сырых значений (`262d25b` + текущий коммит)

Первый шаг плана из раздела 4 — системная причина 3.1. Вместо 21 отдельной правки появилась одна
общая политика, на которую теперь ссылаются все проверки семейства:

- `scripts/verify/lib/bounded-diagnostics.mjs` — список запрещённых директив
  (`tenant_id = %…`, `actor = ?context.actor`, `channel`, `locale`, `causation_id`, `traceparent`,
  `idempotency_key`, `actor_id`, `user_id`, `internal_message = %…error.message`,
  `message: error.message`, `error = ?boundary_error`) и три функции:
  `rawContextLogging`, `forbidRawContextLogging`, `requireFactFields`,
  `assertBoundedDiagnostics`. Покрыт `node --test` (9 тестов рядом с модулем).
  Сознательно **не** запрещены `correlation_id` (ключ склейки событий, его сохраняют
  payment-срезы) и `deadline_ms` (число, посчитанное платформой).
- `scripts/verify/lib/cart-checkout-source.mjs` — модель `checkout_snapshot.rs`: 17 блоков
  (`parser`, `admission`, `portImpl`, `prepare`, …, `publicErrorMapper`) и контракт bounded-фактов
  (`CART_CONTEXT_FACT_FIELDS` и др.). Пять guard'ов больше не держат собственных копий
  `between()`/`from()` и строк-якорей.

Переписаны на bounded-контракт (10 guard'ов, все зелёные): `verify-cart-checkout-{tenant,
local-error,admission,service-error}-context`, `verify-commerce-storefront-order-customer-context`,
`verify-commerce-storefront-cart-http-error-safety`,
`verify-commerce-storefront-auxiliary-http-error-safety`,
`verify-commerce-admin-post-order-read-diagnostic-safety`,
`verify-commerce-graphql-cart-pricing-context`,
`verify-fulfillment-storefront-native-client-error-safety`.

Где пришлось поправить не проверку, а код (иначе guard справедливо красный):

- `rustok-cart/src/checkout_snapshot.rs` — четыре `tracing::debug!` с `tenant_id = %tenant_id`
  и `cart_id = %…` заменены одним `log_cart_checkout_operation_start`, который печатает
  bounded-факты и `cart_id_non_nil` / `order_id_present`;
- `rustok-commerce/src/controllers/store/orders.rs` — пять route-entry логов с
  `tenant_id = %tenant.id` / `actor_id = %auth.user_id` заменены `log_storefront_order_route_entry`;
  в customer-маппер добавлены недостающие факты (`claim_count`, `role_count`,
  `causation_id_length`, `traceparent_length`, `idempotency_key_length`, `deadline_ms`);
- `rustok-commerce/src/controllers/store/carts.rs` — шесть route-entry логов заменены
  `log_storefront_cart_route_entry`, а маппер теперь выбирает `error!`/`warn!` по публичному
  статусу, как в остальных storefront-мапперах;
- `rustok-commerce/src/controllers/store/products.rs`, `.../admin/post_order_reads.rs` —
  то же для пяти и четырёх route-entry логов.

Два guard'а вдобавок правили *документ и evidence*, потому что политика изменилась не только в
коде: `admin-post-order-read-diagnostic-safety.md` и его evidence-файл больше не заявляют
маркер `redacted` — источник печатает `error_kind` и `owner_code_length`;
`storefront-native-client-error-safety-source.json` — `raw_error_shape_logged` вместо
`original_error_logged_privately`.

**Оставшиеся 11 guard'ов семейства — это уже другая болезнь.** Их падение не про сырые поля:
`verify-commerce-admin-checkout-operation-error-context` (89 замечаний),
`verify-commerce-storefront-line-item-http-error-safety` (88),
`verify-commerce-storefront-cart-shipping-http-error-safety` (67),
`verify-commerce-admin-order-return-owner-error-context` (50),
`verify-commerce-admin-order-return-orchestration-error-context` (51),
`verify-commerce-storefront-line-item-owner-context` (41),
`verify-commerce-admin-checkout-operation-diagnostic-safety` (40),
`verify-commerce-admin-order-detail-payment-error-context` (37),
`verify-commerce-admin-order-change-owner-error-context` (22),
`verify-commerce-graphql-provider-operation-error-safety` (20),
`verify-payment-checkout-execution-local-context` (17). Они описаны в 3.2: маркеры блоков и
имена функций ушли, поэтому guard не находит даже свой участок файла. Это отдельная работа —
заново вывести контракт по текущему коду, — и её нельзя делать «на глаз» по диффу.

---

### 1.4. Проход 6: изоляция блоков в admin-commerce

Второй шаг плана — системная причина 3.2. Здесь не появлялось новой политики: наоборот,
каждый guard пришлось писать заново, потому что прежний описывал архитектуру, которой уже нет.
Правило, которое я вывел и больше не нарушаю: **если guard называет исчезнувшие helper'ы
(`payment_error_policy` внутри контроллера, `admin_checkout_operation_http_error<E>(`,
`AdminOrderErrorContext::new`, `PaymentService::new(self.db.clone())`), это не «съехавший якорь» —
это guard о другом коде, и его надо выводить заново по текущему контракту.**

Одиннадцать guard'ов переписаны (458 → 447 красных, регрессий нет):

| Guard | Было | Стало |
|---|---|---|
| `verify-commerce-storefront-line-item-http-error-safety` | 88 | 0 |
| `verify-commerce-storefront-line-item-owner-context` | 41 | 0 |
| `verify-commerce-storefront-cart-shipping-http-error-safety` | 67 | 0 |
| `verify-commerce-admin-checkout-operation-error-context` | 89 | 0 |
| `verify-commerce-admin-checkout-operation-diagnostic-safety` | 40 | 0 |
| `verify-commerce-admin-order-return-owner-error-context` | 50 | 0 |
| `verify-commerce-admin-order-return-orchestration-error-context` | 51 | 0 |
| `verify-commerce-admin-order-route-error-context` | 15 | 0 |
| `verify-commerce-admin-order-detail-payment-error-context` | 37 | 0 |
| `verify-commerce-admin-order-change-owner-error-context` | 22 | 0 |
| `verify-commerce-admin-order-change-orchestration-error-context` | 20 | 0 |

Что изменилось в коде, а не только в проверках:

- **Найден настоящий дефект компиляции.** `map_compensation_error`
  (`controllers/admin/checkout_operations.rs`) разбирал 7 из 9 вариантов
  `CheckoutCompensationError`: ветки `Payment`, `PaymentOrchestration` и `Order` пропали, когда
  политики переезжали в общий модуль. Match был неисчерпывающим, а компенсация, упавшая в
  payment- или order-владельце, не получала публичного конверта вообще. Ветки восстановлены,
  личность владельца подставляется в контекст (`adopt_payment_error_identity`,
  `adopt_order_error_identity`).
- **Политики владельцев стали общими.** `controllers/admin/mod.rs` теперь хранит их как данные:
  `AdminErrorPolicy` (status, code, message, error_kind) и `payment_error_policy`,
  `reserved_refund_error_policy`, `order_error_policy`, `payment_orchestration_error_policy`.
  `changes.rs` и `returns.rs` свои копии удалили (по три в каждом) — конверты не изменились.
- **Route-entry логи во всех затронутых контроллерах перешли на bounded-хелперы**:
  `log_admin_checkout_operation_route_entry`, `log_admin_checkout_reconciliation_recorded`,
  `log_admin_post_order_command_entry`, `log_admin_order_return_entry`, `log_admin_order_entry`,
  `log_admin_order_change_entry`. `post_order_commands.rs`, `post_order_reads.rs`, `returns.rs`,
  `changes.rs`, `orders_owner_ports.rs`, `checkout_operations.rs` теперь дают `0` сырых директив.
  В `returns.rs` заодно ушёл `action = %input.decision.action` — это пользовательский текст,
  вместо него пишутся `decision_action_length` и `decision_moves_money`.
- **Guard'ы выводят обязательные ветки из enum'ов, а не из списка в тесте.** Каждый
  «policy/mapper»-guard парсит `pub enum` соответствующего владельца и требует ветку на вариант;
  добавление варианта теперь валит guard, а не сборку. Мутационная проверка подтвердила: удаление
  ветки `Order`, сырой `tenant_id = %tenant_id`, подмена `reserved_refund_error_policy` и
  возвращённая приватная копия политики — все четыре мутации ловятся.

Практический вывод для продолжения: у оставшихся admin-семейств тот же рецепт — сначала
boundary-хелпер и общая политика в коде, потом guard по живому контракту.

### 1.5. Проход 7: fulfillment-маршруты admin-commerce

Восемь маршрутов `/admin/fulfillments/*` писали строку входа и строку ошибки вручную, поэтому
каждый guard приходилось выводить по тексту конкретного `tracing::debug!`. Вместо этого в
`controllers/admin/fulfillments_owner_commands.rs` появился один вход и одна политика:

- `ADMIN_FULFILLMENT_COMMAND_OWNER`, `ADMIN_FULFILLMENT_CREATE_OWNER`, `ADMIN_FULFILLMENT_COMMAND_BOUNDARY`;
- `AdminFulfillmentRouteFacts` + `log_admin_fulfillment_route_entry` — `tenant`/`actor` как наличие,
  ресурс как наличие и nil-форма, фильтры как `status_filter_present`/`customer_filter_present`,
  перевозчик как `carrier_length` (текст не пишется);
- `fulfillment_command_error_policy` (`AdminErrorPolicy`) + `map_fulfillment_{command,create,read}_error`,
  где code-first идут три кода владельца (`fulfillment.reconciliation_required` → 409,
  `fulfillment.database_unavailable` → 503, `fulfillment.invalid_transition` → 409), затем 7 ветвей
  `PortErrorKind`.

Сырых директив в файле было 16, стало 0. Два guard'а переписаны по живому контракту
(`verify-commerce-admin-order-fulfillment-http-error-safety` 10 → 0,
`verify-commerce-admin-fulfillment-route-error-context` 1 → 0), всего 447 → 445 красных,
регрессий нет. Мутации проверены: удалённый `Idempotency-Key`, сырой tenant в логе, снятый
`carrier_length` — все три ловятся.

### 1.6. Проход 8: удаление мёртвого `controllers/admin/orders.rs`

Файл на 764 строки не входил в дерево модулей (`admin/mod.rs` монтирует
`#[path = "orders_owner_ports.rs"] pub mod orders;`), но именно его читали три guard'а:
`verify-order-read-port` (зелёный — то есть зелёный на мёртвом коде) и два
`*-diagnostic-safety` (по 4 падения). Это худший вид проверки: она подтверждает контракт,
которого нет в сборке. Файл удалён, guard'ы переписаны на живой контроллер:

| Guard | Было | Стало |
|---|---|---|
| `verify-order-read-port` | 4 | 0 |
| `verify-commerce-admin-order-mutation-diagnostic-safety` | 4 | 0 |
| `verify-commerce-admin-order-read-diagnostic-safety` | 4 | 0 |

Что теперь проверяется вместо описания удалённого файла:

- **мутации** — 4 обработчика (`mark_order_paid`, `ship_order`, `deliver_order`, `cancel_order`)
  идут через `.order_admin_command_port()` с типизированными `Owner*Request`, у каждого
  `ORDERS_UPDATE`, guard'ы tenant/id, bounded-строка входа, обязательный
  `require_idempotency_key(&headers)?` и общий `map_order_port_error`; проверка требует ветвь
  на каждый из 7 `PortErrorKind`, поэтому новый вариант enum'а роняет guard, а не сборку;
- **чтения** — `list_orders`/`show_order` не принимают `HeaderMap` и не вызывают
  `require_idempotency_key` (чтение не может писать), фильтры логируются как наличие,
  detail-маршрут читает три порта (`OrderReadPort`, `PaymentOrderReadPort`,
  `FulfillmentReadPort`), а его два mapper'а (`map_payment_detail_port_error`,
  `map_fulfillment_detail_port_error`) имеют ровно одну definition и один callsite — read-ошибка
  не может уйти через write-конверт и наоборот;
- **удаление** — guard падает, если файл `controllers/admin/orders.rs` появится снова.

Мутационная проверка: сырой `tenant_id`, снятый `Idempotency-Key` в `ship_order`, wildcard в
политике, возвращённый legacy-файл, read-ошибка через общий mapper, снятый `Timeout` из ветви
`Unavailable | Timeout` — все ловятся. Итог: 445 → 443 красных, регрессий нет.

### 1.7. Проход 9: канонический ledger форума и «плавающий» pin

Восемь guard'ов forum-корпуса падали на фразе «canonical plan is missing ### Delivered in
`FORUM-20xx`». Причина оказалась не в проверках: канонический план потерял
append-only-раздел о поставленном. Глава `FORUM-20` была заглушкой на пять строк, главы
`FORUM-26` не было вообще, а строки program-ledger'а всё ещё описывали рубеж `FORUM-20A-G`.
При этом owner-заметки (например `forum-20ba-audience-plan-sync.md`) прямо ссылались на
«canonical ledger», которого не существовало, и двадцать один guard честно падал.

Что сделано:

- главы `FORUM-16`, `FORUM-20`, `FORUM-26` восстановлены из
  `crates/modules/rustok-forum/docs/archive/implementation-plan-2026-08-06.snapshot` —
  именно разделы о поставленном (`FORUM-16A-F`, `FORUM-20A-BA`, `FORUM-26A-J`), а не статусы;
  каждое упомянутое в них тестовое имя и контракт проверены на существование в дереве
  (все 22 `cargo test -p rustok-forum --test ...` цели и все упомянутые json-контракты
  на месте), поэтому записи подтверждены, а не скопированы на веру;
- строка `FORUM-20` в program ledger теперь говорит текущий рубеж (`FORUM-20A-BA`), а не `A-G`.

Дальше вскрылся настоящий дефект: guards проверяли **строку-сводку** (`FORUM-20A-G provide`,
`FORUM-20A-AO provide`, `FORUM-20A-AU provide`…). Строка, которая двигается с каждой сдачей,
может удовлетворить только один guard за раз — девятнадцать из них не могли быть зелёными
одновременно, а честный план их же и валил. Правило: **pin ставится на append-only-заголовки
`### Delivered in ...`, а не на сводку.**

`scripts/verify/lib/forum-source-verification.mjs` теперь содержит этот разбор:
`deliveredLedgerSlices()` понимает оба стиля (`### Delivered in `X` through `Y`` и
`### `X / Y``) и разворачивает диапазоны, `assertCanonicalLedger()` требует, чтобы слайсы
контракта были в множестве сданного, а статус контракта — `synchronized`, если ledger уже
ушёл вперёд. Двадцать один guard переведён на хелпер, семь контрактов с ложным `pending`
стали `synchronized`.

Мутации: удалённый заголовок `FORUM-20BA` и возвращённый `pending` ловятся; сужение
диапазона `H`–`Q` до `H`–`M` остаётся зелёным по делу — `FORUM-20T` закрыт отдельным
заголовком, то есть слайс действительно не потерян. Итог: 443 → 436 красных, регрессий нет,
семь guard'ов позеленели.

Осталось в forum-кластере (128 → 121): тридцать guard'ов требуют маркеров из README/live-contract
модуля Notifications, ~20 — из разбитых файлов сервисов (`services/moderation.rs` →
`moderation_owner.rs`, `read_model.rs` → `read_model_owner.rs`), остальные — дрейф
source-маркеров и пять guard'ов, которым нужен `cargo` (в песочнице его нет).


### 1.8. Проходы 10–13: предикат вместо предложения (436 → 407)

Три с половиной десятка guard'ов форумного корпуса сравнивали **точную редакцию фразы**, а не
правило. Владелец за это время переписал абзацы, ничего не сломав: сократил формулировки,
свернул отдельные разделы live-contract'а в общий, разложил `Cargo.toml` на строки,
переименовал локальную функцию валидации. Каждый такой рефакторинг давал «красный» без дефекта.

Что сделано:

- **проход 10** — `verify-order-read-port.mjs` и `verify-commerce-admin-order-*-diagnostic-safety.mjs`:
  порт чтения заказа и диагностические границы проверяются по типам и вызовам, а не по тексту
  сообщений; 443 → 436;
- **проход 11** (раздел 1.7 + `requireValidationDisclosure`) — девять owner-заметок раскрывают
  отсутствие прогонов четырьмя разными оборотами («were not run by», «not run by the
  implementation agent», «None were run by the implementation agent», «No tests…»). Хелпер
  принимает любой из них на «сплющенном» тексте, guard'ы перестали пиниться за один перенос
  строки; семь guard'ов переехали с мутабельной строки-сводки `FORUM-20A-AS/AU provide` на
  append-only заголовки `### Delivered in ...`; 436 → 433;
- **проход 12** — два guard'а видимости (`-owner-read-visibility`,
  `-category-owner-read-visibility`) после рефакторинга читающего пути указывали на снятые
  функции. Они переписаны на живой путь (`resolve_canonical_topic_for_security` →
  `get_with_canonical_resolution_and_locale_fallback`, keyset-список,
  `visible_categories_query`), а не на прежние имена; 433 → 430. Мутационный стенд
  (`/tmp/mut7`, полная копия модуля + `RUSTOK_VERIFY_REPO_ROOT`) показал четыре мутации, три
  пойманы, четвёртая (потеря проброса `hidden_category_ids`) — нет, поэтому добавлен pin
  проброса, а не заменена мутация;
- **проход 13** — двадцать один guard корпуса Notifications. Владелец свела помесячные разделы
  live-contract'а в «Exact and bounded inbox state commands», переписала абзацы про
  mark-all/группы и заменила «External transport adapters and grouped UI remain closed» на
  описание поставленного storefront'а. Введён `requireProse()`: сравнение идёт по тексту с
  нормализованными пробелами, поэтому перенос строки перестал быть изменением контракта, а
  guard продолжает пинить числа, имена API и смысл фразы; 428 → 407.

Заодно сняты два класса ложных pin'ов:

- **счётчики миграций** пинились как `assert_eq!(module.migrations().len(), 6/7)`, причём
  искались в `lib.rs`, хотя самотест живёт в `module.rs`. Теперь guard считает
  `Box::new(...::Migration)` и `MigrationDependencyDescriptor::new(` в реестре и требует, чтобы
  самотест утверждал ровно это число; добавление миграции больше не ломает проверку, а
  расхождение реестра и теста — ломает. Путь к самотесту берётся из нового ключа контракта
  `notifications_module_file`;
- **SQL и манифест** пинились одной строкой. SQLite-ветка backfill'а оборачивает UUID в
  `CASE WHEN typeof(...) = 'blob'`, поэтому pin'ы теперь проверяют выражение
  `'g1:' || NEW.target_owner || ':'` и наличие blob-ветки отдельно; фича `rustok-api`
  проверяется как зависимость + `features = ["runtime"]`, а не как `rustok-api.workspace = true`.

Найден и исправлен реальный дрейф документации: owner README утверждал «GraphQL group-state
write parity remains open», хотя мутация `notificationInboxApplyGroupState` поставлена
(`storefront/src/transport/graphql_adapter.rs`). README приведён к факту, мутация названа в
разделе GraphQL.

Мутации прохода 13: возврат «remain closed» в README и удаление имени мутации из контракта
ловятся; снятие `default = []` и шести-миграционного счётчика больше не считаются дефектом —
это и было целью.

Осталось в forum-кластере (113 guard'ов списка, 42 красных): шестнадцать `*-source` guard'ов
падают исключением (`throw`) вместо сбора `failures`, ~20 ссылаются на снятые
`moderation.rs` / `read_model.rs` / `mention_relation.rs`, остальные — дрейф source-маркеров
(`topic.status == TopicStatus::Open`, `recipient.into_topic_viewer()`,
`forum_topic_channel_access`), plan/ledger ~9 и четыре guard'а, которым нужен `cargo`.

---

## 2. Оставшиеся 368: по владельцам

Замер на текущем коммите (368 красных из 1307). Это то же число, что «Падают» в §0, то есть
в нём есть 2 CLI-инструмента и guard'ы, которым нужна среда (в том числе пять forum-guard'ов,
вызывающих `cargo`); со «статическими красными» §0 оно напрямую не сравнивается. Правило
разнесения по областям одно и то же для всех строк: сначала модуль из пути
(`crates/modules/rustok-<module>/…`), иначе — первое совпадение токена модуля в имени файла.

| Область | Красных |
|---|---|
| forum | **43** |
| commerce | **64** |
| rustok-pages | 43 |
| index | 39 |
| product | 21 |
| groups | 19 |
| payment | 14 |
| fulfillment | 14 |
| blog | 10 |
| social / cart / notifications | 9 / 2 / 7 |
| прочее (≤5 каждая: page-builder, reaction, ai, inventory, order, rbac, profile, …) | 83 |

Разнесение сделано тем же правилом, что и в прошлый раз (модуль из пути, иначе первое
совпадение токена модуля в имени файла), поэтому forum 121 → 92, pages 41 → 43 и product 19 → 21
отражают ещё и уточнение правила на границах (guard'ы `apps/` и `crates/` без модульного пути).
Общий счётчик красных считается по списку файлов и от разнесения не зависит.

Сравнивать с прошлой таблицей можно только commerce: там и правило, и замер совпадают —
69 красных на `0f1314e` против 65 сейчас (проходы 7–8 сняли ровно четыре). Остальные строки
прошлой таблицы разносились по более грубым группам, поэтому pages 43 → 41, index 43 → 38 и
social 5 → 10 — это разница **правила**, а не регрессия или прогресс. Общий счётчик красных
(447 → 436) считается по списку файлов и от разнесения не зависит.

### 1.9. Проходы 14–15: контракт вместо редакции (407 → 368)

Двадцать четыре forum-guard'а падали на маркерах, которые описывали **не правило, а редакцию**
файла, в котором правило когда-то жило. Разбор разложился на четыре класса.

1. **Crate-root glob.** `pub use module::*;` больше не существует: `lib.rs` перечисляет имена
   явно. Добавлен `requirePublicReexport()` — он требует `pub mod X` **и** фактический
   `pub use X::{…}` или `pub use X::*;`, поэтому список имён может меняться свободно (шесть
   guard'ов).
2. **Маркер искал текст там, где текст был, а не там, где правило.** Общая проверка авторизации
   GraphQL-мутаций переехала в `require_forum_permission()` / `resolve_tenant_scope()`, нативный
   серверный контур — в `native_server_support.rs` (`runtime()`, `require_tenant_scope`,
   `require_forum_module_enabled`), а member-card статистика — в `services/member_card.rs`.
   Guard'ы теперь пинят вызов общего стража и его инварианты, а не строку, которую он когда-то
   печатал на месте вызова (семь guard'ов).
3. **Переносы строк.** `assert.ok(text.includes(...))` на Rust-коде ломался от `cargo fmt`
   (`&[a, b]`, `.await?,\n        0`, многострочные цепочки). Введён `containsMarker()`: сравнение
   идёт по тексту с нормализованными переносами и пробелами вокруг `.`, токены не меняются.
   Переведены `includesAll`/`contains` в 23 guard'ах.
4. **Ledger вместо прозы.** `implementation-plan.md` теперь содержит по одной строке на задачу;
   помесячные «Delivered through `FORUM-21X`» и статусные предложения из него ушли. Введён
   `ledgerRowProblems()`: он проверяет, что строка задачи есть, её статус — одно из допустимых
   значений и что описание всё ещё называет поставленную поверхность. Восемнадцать guard'ов
   перестали зависеть от редакции плана.

Найден и исправлен реальный дрейф: `forum-21a`, `forum-21b` и `forum-21i` утверждали, что канонная
запись `FORUM-21` «остаётся `planned`» и что split/fork/reply-range «ещё не сделаны», хотя эти
слайсы поставлены; документы приведены к факту (`in_progress`, остались evidence и нативный
cutover). Домены событий реконсиляции приведены к канонному виду `forum.topic.merge.<slice>_reconciled`,
который принимает CHECK журнала: контракты и handoff-документы называли
`forum.topic.merge_<slice>_reconciled` — форма, которую база отвергла бы.

Заодно сняты два класса ложных pin'ов: `.insert(`/`.update(`/`.delete(` в чистых мапперах
(`BTreeMap`/`BTreeSet` — это не персистентность; вместо них пинятся `exec`, `execute`,
`insert_one`, `update_many`, `delete_many`), и запрет `or_else` по всему файлу, включая
`#[cfg(test)]`, где он используется в тестовом обходе исходника.

Замер: 379 → 368 красных на 1307 guard'ах, forum 63 → 43, 0 регрессий в остальных областях
(commerce 64, pages 43, index 39, product 21, groups 19, payment 14, fulfillment 14, blog 10).

---

## 3. Три системные причины (а не 455 разных проблем)

### 3.1. Guard'ы требуют ровно того, что политика запрещает — 21 guard

Это самый опасный класс: **не код нарушает контракт, а проверка его требует нарушить**.

```
✗ tenant log: missing tenant_id = %context.tenant_id
✗ tenant logging: missing tenant_id = %tenant_id
✗ internal message: missing internal_message = %error.message
✗ message: error.message
```

21 guard (≈76 отдельных требований) буквально требует писать сырые tenant/correlation/
idempotency-значения и текст ошибки владельца в лог. В commerce и cart такие проверки писались
до перехода на bounded-диагностику и с тех пор не обновлялись. Примеры:
`verify-cart-checkout-tenant-context` (9), `verify-cart-checkout-local-error-context` (9),
`verify-cart-checkout-admission-context` (9), `verify-cart-checkout-service-error-context` (8),
`verify-commerce-storefront-order-customer-context` (10).

**Вывод:** эти 21 guard надо переписывать *наоборот* — с `forbidText` вместо `requireText`, как
уже сделано в `verify-commerce-checkout-owner-stage-boundary`. Это не «покрасить», это смена
политики в проверке.

**Статус:** 10 из 21 переписаны в проходе 5 (раздел 1.3), у всех общий источник политики —
`scripts/verify/lib/bounded-diagnostics.mjs`. Оставшиеся 11 упираются уже не в поля, а в
разъехавшиеся маркеры блоков, то есть относятся к 3.2.

### 3.2. Guard'ы не умеют находить свой блок в изменившемся файле — 19 guard

Сообщение `unable to isolate source block` / `failed to isolate` означает: helper
`between(source, start, end)` не нашёл одну из границ. Причина почти всегда одна — **маркер
конца блока изменился** (добавили `#[cfg]`, `map_err` стал однострочным, поменялся порядок
функций), а начало нашлось. Guard падает не потому, что код плохой, а потому что перестал
узнавать хороший код.

Пример из этого прохода: `verify-commerce-staged-checkout-payment-retry-disposition` искал конец
теста по `#[test]\n    fn retryable_marketplace…`, а между ними стоял
`#[cfg(feature = "marketplace-financial")]`. Guard был красным годами.

Лечение: `between` должен сообщать, **какая** граница не найдена, и по возможности искать по
уникальному якорю функции, а не по паре «начало+конец».

### 3.3. Guard'ы-сироты — 408 из 465

408 красных guard'ов не вызываются ни из `package.json`, ни из workflow. Их нельзя «держать
зелёными», потому что никто не заметит, когда они станут красными. Это главная причина, по
которой долг накопился: 128 forum- и 84 commerce-проверки писались под архитектуру, которую
потом меняли, и никто об этом не узнал.

**Первое, что нужно сделать (до любой починки):** решить судьбу каждого сироты — подключить
в `verify:all`, либо удалить. Мёртвый guard хуже отсутствующего: он создаёт иллюзию покрытия и
съедает время при каждом массовом прогоне.

---

## 4. Что чинить и в каком порядке

| # | Блок | Объём | Почему сейчас |
|---|---|---|---|
| 1 | 21 guard «требует сырых значений» → `forbidText` | 21 → **10 сделано** | Прямое противоречие безопасности; пока они красные, их нельзя включить в CI, а включить надо. Общая политика — `lib/bounded-diagnostics.mjs`; остаток упирается в маркеры блоков и переходит в пункт 2 |
| 2 | 19 guard'ов с `unable to isolate` | 19 | Механическая причина, лечится библиотекой `between` с диагностикой |
| 3 | forum: 128 | 128 | Самый большой кусок; после (1) и (2) станет видно, сколько из них — настоящий дрейф |
| 4 | content-gap в rustok-pages (41) | 41 | Требует дописывания кода, а не guard'ов |
| 5 | index (39), product (21), groups (19) | 79 | Маркерный дрейф, правится пакетами |
| 6 | inventory 422 сироты: подключить/удалить | 408 | Без этого всё остальное не имеет смысла |

---

## 5. Ошибки, которые я уже совершил и не повторяю

Записано, чтобы не наступить снова:

- **Нельзя «перепинить» guard на код механически.** В commerce код уже на bounded-диагностике,
  а guard'ы пиннят `error = ?error`. Замена пути даёт `unable to isolate`; нужна
  пере-выводка маркеров из реального контракта.
- **Нельзя хардкодить один SHA workflow-экшена.** Правильно: имя + 40 hex.
- **Нельзя реанимировать git-blob пины.** Вместо них — контентные маркеры и `historical_record`.
- **Удалённый legacy нельзя ни удалять из guard'а, ни перепинивать**: правильная реакция guard'а —
  утверждение, что файла нет.
- **`include!` — не единственный способ монтирования**: `#[path]` монтирует файл без `include!`,
  поэтому grep по `include!` не находит такие модули.
- **`node guard.mjs | head` маскирует код выхода** — только `node … > file 2>&1; echo $?`.
- **Нельзя судить по коду выхода**: 2 guard'а — CLI-инструменты, 5 падают из-за отсутствия
  `node_modules`, 8 — из-за отсутствия cargo. Это среда, а не дрейф.

---

## 6. Ограничение, которое надо помнить

Всё выше — статический анализ исходников Node-скриптами. **Rust не собирался, тесты не
запускались**: в песочнице нет toolchain'а. Результаты сборки может дать только ваш прогон:

```bash
cargo check -p rustok-commerce --lib
cargo clippy --workspace --all-targets --no-deps -- -D warnings   # CI гоняет именно это
node --test scripts/verify/verify-commerce-checkout-completion-cutover.test.mjs
```

Отдельно стоит посмотреть на слитые в проходе 4 модули (`checkout_payment_stages.rs` 896 строк,
`checkout_fulfillment_stages.rs` 877 строк): после удаления шимов там мог остаться
неиспользуемый импорт, а clippy в CI идёт с `-D warnings`.
