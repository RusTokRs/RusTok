# Аудит логики функционала channel — ошибки, косяки и рекомендации

**Дата:** 2026-11-26
**Ветка:** `arena/a3d16606-rustok` (base `29f184e`)
**Объём:** `crates/modules/rustok-channel` целиком (`src` ~7,3k строк Rust, `admin` ~5,3k, `ports` ~0,5k, миграции, контракты/evidence), транспорт и рантайм `apps/server` (`controllers/channel.rs`, `middleware/channel.rs`, `middleware/channel_native_wrapper.rs`, `services/channel_cache_invalidation*.rs`, `services/runtime_guardrails.rs`, `services/app_router.rs`), потребители (`rustok-blog`, `rustok-pages`, `rustok-navigation`, `rustok-commerce`, `rustok-pricing`), ADR `2026-03-25-rustok-channel-experimental-core`, `2026-03-27-channel-resolution-pipeline-and-typed-policies`, `2026-07-22-channel-binding-policy-boundary`.
**Метод:** статический сквозной проход по исполняемым путям (жизненный цикл канала, резолюция, политики, кэш/инвалидация, персистентность, тенант-скоуп, UI-логика, транспорт native↔REST), проверка 6 Universal Logic Dimensions из `AGENTS.md` §2.1.5.
**Ограничение:** в песочнице нет Rust-тулчейна (`cargo` отсутствует), поэтому `cargo check/test/clippy` и компиляция `apps/server` **не запускались**. Все утверждения — о коде и о данных репозитория на указанном HEAD; рантайм-эвиденс не заявляется (§15 AGENTS).

> Приоритеты: **P1** — функциональный дефект, доступный клиенту, дублирование мутаций, конкурентная потеря инварианта; **P2** — целостность контрактов, перф/готовность, архитектурные нарушения; **P3** — гигиена и UX-мелочи.

---

## 1. Executive Summary

Каркас модуля зрелый: одна каноническая модель канала, детерминированный порядок резолюции, типизированные предикаты, DB-триггеры в одной транзакции с мутацией, per-tenant версия локального кэша, guardrail на durable-инвалидацию. Инъекций/обхода тенанта на текущих транспортах не найдено: REST-контроллер, native-адаптер и `ChannelResolver` проверяют принадлежность тенанту.

Тем не менее есть **4 дефекта уровня P1** и систематические P2:

1. **Неизвестный `X-Channel-ID` превращается в HTTP 500 для любого маршрута.** Неизвестный id — не «нет совпадения», а `ChannelError::NotFound`, который `resolve` пробрасывает `?`, а middleware мапит в `INTERNAL_SERVER_ERROR`. Клиентский заголовок ломает страницу/API и генерирует ложные 500-е в мониторинге.
2. **Слепой fallback «native → REST» на любую ошибку** в админ-транспорте: маскирует 400/403/404, дублирует неидемпотентные мутации, а в SSR ходит по `RUSTOK_API_URL`/`localhost:5150` с Bearer, которого при HttpOnly-сессии нет.
3. **Жизненный цикл канала не реализован:** нет команд update/activate/deactivate/delete. Как следствие `channels.is_active` всегда `true`, а ветки `InactiveChannel`, выбор неактивного канала, tombstone/`index_revision`, identity-generation на DELETE, «tenant movement» — **недостижимы через API** и проверяются только прямым SQL в тестах. Поле `status` навсегда `"experimental"`.
4. **Конкурентные инварианты не защищены БД:** default-канал, «один активный policy-set», уникальность host-таргета, `bind_module`/`bind_oauth_app` — check-then-act без транзакции и без частичных уникальных индексов; при гонке получаем два дефолта/два активных набора либо периодический 500 из-за unique-violation.

Плюс расхождения «документация/контракт vs код»: `ChannelReadPort` объявлен `deadline_required: true` с эвиденсом «context_deadline_preserved», но таймаут не реализован; админ-UI модуля — единственный, кто не использует канонические `rustok-ui`/`rustok-grid` (сырые `<button>/<input>/<select>`), что прямо нарушает FFA §5.3/§5.4; Next.js-админка не имеет канала вовсе (нет паритета §5.6), и это не отмечено в Known Limitations.

---

## 2. Что уже объявлено в документации (повторно не считаю новым)

Чтобы не выдавать известное за находку:

- `docs/implementation-plan.md` → «Open results» п.1 (**`channels.name` — локализуемая копия в базовой строке**, нужен `channel_translations`), п.2 (**PR #2469**: DB-owned promotion default/active/primary, host-claims, cross-tenant guards), п.3–4 (permanent cache gate, runtime evidence), п.6 (не расширять таксономию таргетов спекулятивно).
- `README.md` → «Known Limitations»: standalone-профиль только для identity-снимков, `read_selected_channel` — source-only.
- `docs/modules/registry.md`: `channel` — `in_progress` / `boundary_ready` / `core_transport_ui`, поверхность `admin`.
- FFA-статус `in_progress`.

**Новые находки ниже — это не пункты плана**: ошибка 500 на неизвестный селектор, двойной транспортный fallback, недостижимый жизненный цикл, P1-гонки в `create_channel`/`bind_*`/policy-set, глобальная (не per-tenant) генерация инвалидации, отсутствие deadline в порту, FFA-нарушение в UI, N+1 в админ-bootstrap.

---

## 3. Находки P1

### P1.1. Неизвестный `X-Channel-ID` (и любая ошибка резолвера) → HTTP 500 на всех маршрутах

**Корень.** `crates/modules/rustok-channel/src/resolution.rs:124`:

```rust
match self.service.get_channel_detail(channel_id).await? {   // NotFound -> Err, а не Miss
```

`get_channel_detail` (`services/channel_service.rs:155-167`) отдаёт `ChannelError::NotFound` для отсутствующего id. Далее `ChannelResolver::resolve` возвращает `Err`, а `apps/server/src/middleware/channel.rs:337-368` мапит **любую** ошибку резолвера в `StatusCode::INTERNAL_SERVER_ERROR`.

Режим отказа: запрос `GET /` или `GET /api/graphql` с заголовком `X-Channel-ID: 11111111-1111-1111-1111-111111111111` → 500 вместо обычного разрешения. Middleware навешан на весь роутер (`services/app_router.rs:625-628`), включая health-эндпоинты, сторфронт и server functions.

Показательна асимметрия: для slug путь уже мягкий (`get_channel_detail_by_slug` → `Option` → `Miss` в трейсе), для id — жёсткий.

**Фикс.** Трактовать `NotFound` как `ResolutionOutcome::Miss` и продолжать конвейер (как для slug), а из middleware отдельно различать `ChannelError::Database` → 503/500 и прочее → 400:

```rust
let detail = match self.service.get_channel_detail(channel_id).await {
    Ok(detail) => detail,
    Err(ChannelError::NotFound(_)) => {
        trace.push(ResolutionTraceStep { stage: ResolutionStage::HeaderId,
            outcome: ResolutionOutcome::Miss,
            detail: format!("No channel with id '{channel_id}' for this tenant") });
        // продолжаем к HeaderSlug/Query/Host/Policy/Default
        …
    }
    Err(err) => return Err(err),   // реальная инфраструктурная ошибка
};
```

Тест-лок: `apps/server/src/middleware/channel_tests.rs` не содержит ни одного кейса «неизвестный id/slug» — добавить `unknown_header_id_falls_through_to_host/default` и `unknown_header_id_returns_200`.

### P1.2. Админ-транспорт: `native` → `REST` fallback на **любую** ошибку

**Корень.** `crates/modules/rustok-channel/admin/src/transport/mod.rs:38-56` и все последующие функции (`:66-70, :103-107, :128-132, :155-158, :180-183, :205-208, :229-232, :256-259, :278-281, :303-306, :328-331, :352-355`):

```rust
match native_server_adapter::channel_create_native(payload.clone()).await {
    Ok(payload) => Ok(payload),
    Err(_) => rest_adapter::post_json("/api/channels/", payload, token, tenant_slug).await,
}
```

Три проблемы:

1. **Маскирование авторизации и валидации.** `ServerFnError` «Channel admin access is denied» / «tenant scope mismatch» / «invalid web_domain» уходит в повтор по REST — оператор видит чужое сообщение, а диагностика теряется. При этом REST-ветка бьёт по `RUSTOK_API_URL` или **жёстко зашитому** `http://localhost:5150` (`rest_adapter.rs:21-30`).
2. **Возможное дублирование мутации.** Если native-хендлер выполнился и закоммитил, а ответ не смапился/оборвался, REST-повтор создаёт второй канал/таргет либо возвращает 409 против только что созданного. Fallback применяется и к `PATCH`/`DELETE`.
3. **SSR ломается по определению.** Для `Authorization: Bearer` нужен `use_token()`; при действующем контракте HttpOnly-сессии (ADR `2026-09-29-leptos-httponly-session-migration`) токена в JS/SSR может не быть, а cookie в межпроцессный HTTP-вызов не переносится. То есть fallback — не рабочая страховка, а вторая реализация того же контракта (нарушение §13 «no fallback-to-legacy / parallel implementation»).

**Фикс.** Никогда не повторять мутацию по неизвестной ошибке. Либо канонический `rustok-ui-transport::execute_selected_transport` с явным профилем, либо классификация: fallback только на «транспорт недоступен» (сетевой отказ/`ServerFnError` уровня транспорта), и всегда — с явным признаком `retryable` из ошибки. Для SSR — только native-путь.

### P1.3. Жизненный цикл канала отсутствует: `is_active`, `status`, delete и tombstone недостижимы через API

**Корень.** В `ChannelService` нет `update_channel`, `set_channel_active`, `delete_channel`. `is_active` ставится `true` один раз при создании (`channel_service.rs:112`) и больше не меняется нигде в `src`/`apps` (проверено поиском по `ChannelActiveModel`/`is_active`); `status` — константа `"experimental"` (`:114`), попадающая при этом в DTO, в `ChannelContext`, в индексную проекцию (`index_projection.rs:14`).

Следствия — мёртвые ветки и мёртвые механизмы:

| Механизм | Где | Достижим через API? |
|---|---|---|
| `ChannelError::InactiveChannel` | `channel_service.rs:366,384,457`, `controllers/channel.rs:641` | нет |
| `if !detail.channel.is_active` в резолюции | `resolution.rs:135,172,197,393` | нет |
| выбор только активного default/host | `channel_service.rs:250-268` | частично (фильтр всегда истинен) |
| `channel_index_tombstones` + `index_revision` bump на DELETE | `migrations/m20260731_000011*` | нет (нет команды удаления) |
| identity-generation на DELETE / перемещение тенанта | `migrations/m20260807_000012*` | нет |
| UI-бейдж `is_active` / `status` | `admin/src/model.rs`, `channel_card.rs` | всегда одно значение |

Тесты это подтверждают: чтобы проверить неактивный канал, `apps/server/src/middleware/channel_tests.rs:648-659` делает **прямой `UPDATE channels SET is_active = ? WHERE id = ?`** через `execute_raw`.

Это §14 AGENTS («unwired feature / fake completion»): контракт описывает состояния и ревизии, которые владелец не умеет производить. `README.md` этого не фиксирует в Known Limitations.

**Фикс (выбрать одно, не оба):**
- **Реализовать owner-команды** `update_channel(tenant, id, name/settings)`, `set_channel_active(tenant, id, bool)`, `delete_channel(tenant, id)` с атомарными эффектами (tombstone, identity generation, инвалидация) + REST/native/UI + тесты; либо
- **Удалить недостижимое**: `channels.is_active`/`status` из модели, DTO, индекса и UI; ветки неактивности; tombstone-механику — с миграциями и синхронизацией docs/evidence.

Второй вариант дешевле, но ломает заявленную контрактную поверхность Distribution (SalesChannel индекс). Поэтому предпочтителен первый, поэтапно: сначала `set_channel_active` + `update_channel` (name), затем `delete_channel` с tombstone.

### P1.4. Конкурентные инварианты: check-then-act без транзакции и без DB-ограничений

Все места ниже — TOCTOU; часть покрыта PR #2469 (default/active/primary promotion, host claims), но **не всё**:

| # | Место | Что ломается | Защита сейчас | Нужно |
|---|---|---|---|---|
| 1 | `create_channel` `:104-121` | два параллельных create → **два `is_default`** | SELECT перед INSERT вне транзакции | `CREATE UNIQUE INDEX … ON channels(tenant_id) WHERE is_default` + retry/409 |
| 2 | `add_target`/`update_target` `:471-548,549-631` | два разных канала получают один и тот же `web_domain` в тенанте; `get_channel_by_host_target_value:213` делает `.one()` → **произвольный канал** | сервисный SELECT; индекс `idx_channel_targets_type_value` **не уникальный** (`migrations/m20260326_000005`) | частичный уникальный индекс по `(lower(value))` в рамках тенанта (нужен `tenant_id` в таргете или exclusion через join-таблицу) + advisory lock |
| 3 | `create_channel`/`create_resolution_policy_set` при гонке по slug | unique-violation → `ChannelError::Database` → **HTTP 500** вместо 409 | предварительный SELECT | перехватывать unique-violation и мапить в `SlugAlreadyExists`/409 |
| 4 | `bind_module` `:659-713`, `bind_oauth_app` `:741-785` | гонка find-then-insert → unique-violation → 500 | предварительный SELECT, вне транзакции | `INSERT … ON CONFLICT DO UPDATE` (upsert) вместо find-then-insert |
| 5 | `create_resolution_policy_set` `:813-877` | INSERT и `replace_active_policy_set` — **две разные транзакции**; падение второй оставляет **два активных набора** → `list_active_resolution_rules:1301` молча смешивает правила из обоих | нет | одна транзакция + частичный уникальный индекс `(tenant_id) WHERE is_active` |
| 6 | `set_default_channel`/`set_active_resolution_policy_set` | нет сериализации на уровне БД (row-lock случайно спасает, но порядок не гарантирован) | нет | advisory lock per tenant + частичные уникальные индексы |

Дополнительно: `bind_oauth_app` **не проверяет тенант OAuth-приложения** (проверка живёт только в двух транспортах — REST `controllers/channel.rs:259-272` и native `native_server_adapter.rs:723`). Любой будущий потребитель owner-сервиса сможет связать канал тенанта A с приложением тенанта B (FK пройдёт). Аналогично `bind_module` не валидирует `module_slug` по реестру модулей.

**Фикс.** (а) «Инвариант живёт в БД» — частичные уникальные индексы + `ON CONFLICT`; (б) owner-методы должны принимать `tenant_id` и проверять его сами, а не полагаться на транспорт (см. P2.6); (в) unique-violation → типизированный 409, а не 500.

---

## 4. Находки P2

### P2.1. `update_resolution_rule` теряет предикаты и их порядок

`services/channel_service.rs:1040-1075`: при любом патче definition предикаты восстанавливаются «по одному значению на тип» в фиксированном порядке `host_equals, host_suffix, oauth, surface, locale`:

```rust
let mut host_equals = next_definition.predicates.iter().find_map(…);
…
next_definition.predicates = predicates;
```

- дубликаты одного типа **молча схлопываются** (остаётся первый);
- исходный порядок теряется;
- `ChannelResolutionRuleDefinition::validated()` (`policy.rs:55-94`) не запрещает ни дубликаты, ни противоречия, ни `priority` вне диапазона — значит создать правило с `HostEquals("a") + HostEquals("b")` (никогда не сработает) можно через `create`, а после любой правки через UI оно превратится в `HostEquals("a")`.

UI это гарантированно триггерит: `admin/src/core.rs:97-111` (`update_payload`) всегда отправляет все пять полей `Some(..)`, включая пустые строки.

**Фикс.** Сделать definition явной структурой (`host_equals: Option<String>, host_suffix: Option<String>, …` или enum с уникальными вариантами), валидировать дубликаты/противоречия при create, а patch применять как patch, а не как «пересборку из первого значения». Плюс валидация `priority` (например `0..=10000`, целые шаги).

### P2.2. `?channel=` с пустым значением или >100 символов роняет запрос в 400

`apps/server/src/middleware/channel.rs:548-573`:

```rust
let parsed = bounded_channel_selector(value.as_ref())
    .ok_or(axum::http::StatusCode::BAD_REQUEST)?;
```

`bounded_channel_selector` (`:443`) возвращает `None` для пустой строки → **`?channel=` (пусто) даёт 400 на всю страницу**. Такие ссылки появляются из шаблонов, редиректов, трекеров. Длинное значение (>100) — тоже 400 вместо «не нашлось».

**Фикс.** Пустой/невалидный по длине query-селектор → `None` (Miss в трейсе, `warn` в лог). 400 оставить только для конфликтующих дубликатов (и, возможно, тоже заменить на «первый + warn», как это уже сделано для заголовков).

### P2.3. Генерация инвалидации — глобальная, а не per-tenant: амплификация сброса кэша и сериализация мутаций

`migrations/m20260716_000009*`: одна строка `scope='resolution'`, триггер `FOR EACH STATEMENT … UPDATE … SET generation = generation + 1` для шести таблиц. `apps/server/src/services/channel_cache_invalidation.rs:273-283`: при любом расхождении → `invalidate_all_channel_cache_local(&self.ctx)`.

Следствия:
- изменение каналов **одного** тенанта сбрасывает локальный кэш **всех** тенантов;
- мутации каналов всех тенантов сериализуются на одной строке (row lock);
- `replace_default_channel` (`channel_service.rs:1362-1388`) обновляет каналы по одному → N statement-триггеров → N инкрементов генерации.

**Фикс.** Таблица `(tenant_id, generation)` + триггеры с `tenant_id` из `NEW/OLD`, инвалидация только затронутого тенанта; `replace_default_channel` — один `UPDATE … WHERE id <> $1` вместо цикла.

### P2.4. Готовность процесса привязана к кэшу канала, хотя есть безопасная деградация

`apps/server/src/services/runtime_guardrails.rs:58-64` объявляет `channel resolution durable invalidation runtime` как `Critical` → `ReadinessStatus::Unhealthy` (`controllers/health.rs:1149-1151`). `is_ready()` становится ложным при любом неуспешном `apply` или транзиентной ошибке чтения генерации (`channel_cache_invalidation.rs:243-260, 300-305`). То есть кратковременный сбой чтения одной таблицы выводит реплику из балансировки.

При этом кэш при нездоровье **продолжает отдавать данные** (fail-stale), хотя безопасный режим — «не кэшировать» — уже есть в виде токенов версий.

**Фикс.** При `not ready` переключать `resolve` в режим «без кэша» (bypass), а статус guardrail понизить до `Degraded`; `Critical` оставить только для случая, когда durable-состояние отсутствует (миграция не применена).

### P2.5. `ChannelReadPort` не реализует deadline, хотя контракт и evidence это утверждают

- `contracts/channel-fba-registry.json:14` → `"deadline_required": true`, assertions `"context_deadline_preserved"`.
- `contracts/evidence/channel-contract-test-static-matrix.json:13,18` — те же assertions.
- `README.md` («Interactions»/entry points) — «deadline-aware read semantics».
- Факт: `src/ports.rs:77-175` `PortContext` читает (`parse_tenant_id`, `require_policy`), но `context.deadline_ms` не используется; в `ports_tests.rs` deadline выставляется и никогда не проверяется. Для сравнения, у соседнего порта он реализован: `ports/src/lib.rs:78-85` + `src/index_identity.rs:125` (`tokio::time::timeout`).

Это §14 (свидетельство о невыполненной функциональности) плюс отсутствие таймаута на реальном пути чтения.

**Фикс.** `tokio::time::timeout(Duration::from_millis(context.deadline_ms.unwrap_or(DEFAULT)), …)` с мапом в `PortErrorKind::Timeout`, добавить поведенческий тест «deadline exceeded → typed timeout», синхронизировать evidence-файлы.

### P2.6. Owner-сервис не тенант-безопасен: мутации принимают произвольный `channel_id`

`add_target`, `update_target`, `delete_target`, `bind_module`, `remove_module_binding`, `bind_oauth_app`, `revoke_oauth_app_binding`, `set_default_channel` — все принимают только `channel_id`; принадлежность тенанту проверяют транспорты (REST `ensure_channel_belongs_to_tenant`, native `ensure_channel_belongs_to_tenant`). Сам владелец контракта не может отклонить чужой канал.

Сейчас эксплуатационного обхода нет, но по §7 AGENTS («Tenant … MUST be resolved through canonical typed context and propagated explicitly») это архитектурный дефект: следующий потребитель (GraphQL/воркер/CLI) получит небезопасный примитив.

**Фикс.** Добавить `tenant_id` в сигнатуры мутаций (или типизированный `ChannelCommandContext`) и проверку внутри; в транспортах оставить только пере-проверку. Побочно это убирает дублирование `ensure_channel_belongs_to_tenant` в трёх местах.

### P2.7. FFA-нарушение: админ-UI модуля не использует канонический UI-стек

`crates/modules/rustok-channel/admin/Cargo.toml` не содержит `rustok-ui`, `rustok-ui-leptos`, `rustok-grid`, `rustok-grid-leptos` — в отличие от `rustok-blog/admin`, `rustok-forum/admin`, `rustok-auth/admin`. Вся разметка — сырые элементы с ручными tailwind-классами: `channel_card.rs:587,620,648,685,712,785,792,794,797,817,…`, `policy_set_card.rs`, `policy_workbench.rs` (см. `grep -n "<button\\|<input\\|<select"`).

AGENTS §5.3/§5.4: «MUST NOT write raw `<button>` or `<input>` with custom styling», «MUST NOT build manual `<table>` loops», «MUST NOT hand-roll form validation». Модуль — единственный outlier в репозитории.

Плюс часть доменных решений осталась в компонентах: `target_primary` по умолчанию `true` (`channel_card.rs:163`) — добавление любого таргета молча перехватывает primary; очистка/установка query-параметров; `policy_rule_edit_form_state` (`core.rs:141-190`) при отсутствии action-канала в списке **молча подменяет** его на первый канал тенанта и такой патч уйдёт на сервер.

**Фикс.** Перевести на `rustok-ui-leptos` (`Button`, `Input`, `Select`, `Badge`, `Card`, `Alert`, `Dialog`) и `rustok-grid-leptos` для списков; перенести оставшиеся ветвления/нормализацию в `core/` (тестируемо без UI); при недоступном action-канале — явная ошибка формы, а не подмена.

### P2.8. N+1 на чтении + непагинируемый bootstrap

- `build_channel_detail` (`channel_service.rs:1452-1490`) — 3 отдельных запроса на канал; `list_channel_details`/`list_channel_details_page` (`:297-356`) → **3N+1**.
- `list_resolution_policy_sets` (`:1238-1274`) — +1 запрос правил на каждый набор.
- `bootstrap` (`controllers/channel.rs:45-56` и native `native_server_adapter.rs:308-322`) грузит **все** каналы со всеми таргетами/биндингами/OAuth и **все** policy-сеты с правилами, без пагинации, плюс полный реестр модулей.

На тенанте с десятками каналов и правилами это десятки запросов на каждый рендер админки.

**Фикс.** Батч-загрузка по `IN (channel_ids)` (3 запроса на страницу вместо 3N), пагинация/ленивая загрузка правил по выбранному набору, кэш статического списка модулей.

### P2.9. Потребители обрезают список каналов на 250 без сигнала

`list_channels(tenant_id, 1, 250)` — жёсткий кап, без пагинации и без ошибки:
- `crates/modules/rustok-commerce/src/graphql/query.rs:300-304`
- `crates/modules/rustok-pricing/admin/src/transport/native_server_adapter/server.rs:54-58`
- `crates/modules/rustok-pricing/storefront/src/transport/native_server_adapter.rs:387-391`

Плюс `list_channels` (`channel_service.rs:270-295`), в отличие от `list_channel_details_page`, **не фильтрует `is_active`** и не имеет `include_inactive` — два разных контракта на «список каналов».

**Фикс.** Для потребителей — `ChannelReadPort::list_channels_for_tenant` (уже существует и пагинируемый); в сервисе — единая семантика `include_inactive`; при превышении лимита — типизированная ошибка, а не тихая обрезка.

### P2.10. Нарушение границы: Channel читает приватные таблицы Auth

`crates/modules/rustok-channel/admin/src/transport/native_server_adapter.rs:350-420, 720-760` — прямые SQL-запросы к `oauth_apps` и `oauth_app_translations`; `apps/server/src/controllers/channel.rs:259-272` — `models::oauth_apps::Entity::find_active_by_tenant`.

§5 AGENTS: «A consumer MUST NOT read another module's private tables … Use an owner-defined contract». Здесь владелец Auth-данных — `rustok-auth`, а Channel читает их таблицы в двух транспортах и с двумя разными правилами (в native — с join по локали, в REST — через модель).

**Фикс.** Порт от Auth (`list_active_apps_for_tenant(tenant, locale)`), один на оба транспорта.

### P2.11. Bootstrap падает при локали `und`

`native_server_adapter.rs:341-348`:

```rust
let effective_locale = normalize_locale_tag(&request_context.locale)
    .filter(|locale| locale != "und")
    .ok_or_else(|| ServerFnError::new("…requires a normalized effective locale other than `und`"))?;
```

При `effective_locale = "und"` (ICU принимает `und`; tenant default или Accept-Language могут дать это значение) **вся страница каналов** отдаёт ошибку. При этом SQL использует `LEFT JOIN oauth_app_translations`, т.е. fallback не требует строгой локали.

**Фикс.** Fallback: `und`/нераспознанное → tenant `default_locale` → `PLATFORM_FALLBACK_LOCALE` (`en`), как это делает `resolve_request_locale` в `rustok-api`.

### P2.12. `is_module_enabled` без тенанта — публичный небезопасный примитив, и он используется

`channel_service.rs:375-395` (`is_module_enabled`) не проверяет тенант, а doc-comment про «Consumers must use this form whenever both tenant and channel identifiers cross a transport boundary» приклеен **к другой функции** (`:397-402`, над `ensure_channel_slugs_exist_for_tenant_in_tx`). Фактические потребители:

- безопасный вариант: `rustok-blog`, `rustok-forum`, `rustok-commerce` (`is_module_enabled_for_tenant`);
- небезопасный: `rustok-pages` (`controllers/mod.rs:695`, `graphql/query.rs:598`, `storefront/.../host_route_adapter.rs:114`, `native_server_adapter.rs:88`), `rustok-navigation` (`controllers.rs:270`, `graphql/query.rs:125`, `storefront/.../native_server_adapter.rs:44`).

Сейчас `channel_id` приходит из проверенного `RequestContext`, но контракт остаётся «прочитаю биндинг любого канала по id».

**Фикс.** Удалить (или сделать `pub(crate)`) небезопасный вариант, перевести pages/navigation на `is_module_enabled_for_tenant`, поправить doc-comment. Бонус: убрать лишний `ensure_channel_exists` (2 запроса → 1).

### P2.13. Нет паритета с Next.js и нет записи об этом в Known Limitations

`apps/next-admin` не содержит ни `packages/channel`, ни `app/dashboard/channel` (проверено: `ls apps/next-admin/src/app/dashboard` — канала нет; `apps/next-admin/src/modules` — нет регистрации). Channel — core-модуль с админкой в Leptos.

AGENTS §5.6 требует route/surface parity для модулей с UI. В `docs/modules/registry.md` это не отмечено как gap (заявлено `admin` / `core_transport_ui`), в `README.md` — тоже.

**Фикс.** Минимум — зафиксировать в `README.md` → «Known Limitations» и в `docs/implementation-plan.md` как явный пункт (с владельцем и критерием готовности). Полноценно — порт админки в Next.js по образцу `packages/pages`.

### P2.14. Мультиязычность: `channels.name` и `channel_resolution_policy_sets.name`

Заявлено в плане (Open results п.1), но не в README Known Limitations; при этом базовые строки содержат пользовательскую копию (`channels.name`, `parse`-валидируемое как единственное имя; `policy_sets.name` — «Policy-set display names must be classified» до сих пор не классифицировано). Это прямое нарушение §5/§13 и ADR о мультиязычности; UI при этом рендерит `name` как есть в обеих локалях (`en`/`ru`).

**Фикс.** `channel_translations(tenant_id, channel_id, locale, name)` с tenant-composite FK, бэкфилл с честной provenance, требование effective locale на запись, единая проекция exact-locale→fallback в REST/native/портах/кэше/UI; классифицировать `policy_sets.name` в той же миграции.

---

## 5. Находки P3

1. **Валидация локали в предикате** (`policy.rs:160-170`): `normalize_locale` принимает любую непустую строку (`"ru_by!"`, `"x"`), хотя в платформе есть `rustok_api::normalize_locale_tag`. Правило с такой локалью никогда не сработает (а `.ok()` в `matches` ещё и молча проглатывает ошибку).
2. **`HostSuffix`/`HostEquals`** могут сосуществовать и конфликтовать; `HostSuffix` не требует `.*`-границы — ок, но комбинации не валидируются (см. P2.1).
3. **Primary-таргет не переиспользуется**: `delete_target` (`:633-657`) и `update_target(is_primary=false)` (`:610-630`) оставляют канал без primary; резолвер молча берёт `targets.first()` (`middleware/channel.rs:150-153`), т.е. диагностика `ChannelContext.target_*` становится произвольной.
4. **Дубли таргетов для не-host типов**: `add_target` проверяет уникальность только для `web_domain` (`:492-501`); два одинаковых `mobile_app`/`external` на одном канале — норма.
5. **`external`/`embedded`/`api_client`** — практически невалидируемый free-form текст (только trim + 500 символов); нет ни формата, ни семантики резолюции (что честно записано в плане п.6).
6. **`ensure_channel_slugs_exist_for_tenant_in_tx`** (`:403-441`) возвращает `InvalidTargetValue` для несуществующего слага (семантически чужая ошибка) и не проверяет `is_active` — после появления disable можно будет сослаться на выключенный канал.
7. **`reorder_resolution_rules`** (`:1095-1189`) требует полный список правил: параллельное добавление правила делает reorder 400 с невнятным текстом; `rules.is_empty()` → `NotFound(policy_set_id)` (неверный код). Приоритеты, созданные через `create_resolution_rule`, не проверяются на коллизии — порядок держится на `created_at`.
8. **`get_channel_for_tenant`** с nil tenant отдаёт `NotFound`, хотя это `Validation` (`:126-141`).
9. **`ChannelModule::health`** всегда `Healthy` (`module.rs:56-58`), не отражает ни схему, ни состояние инвалидации (последнее отдельно покрыто guardrail — не дублировать, но и «здоровье» модуля тогда бессмысленно).
10. **`NotFound`/`InactiveChannel` в `map_channel_error`** (`controllers/channel.rs:632-668`) для `InactiveChannel` отдаёт 409 CONFLICT — при появлении реального disable это спорный код (скорее 404/410 для публичного чтения).
11. **`create_resolution_policy_set(is_active=true)`** возвращает `get_resolution_policy_set` после `replace_active_policy_set` — ещё один лишний round-trip в той же операции.
12. **`channel_module_bindings.module_slug`** не валидируется по реестру: можно привязать несуществующий модуль (мусор в бутстрапе), и `is_module_enabled` вернёт `false` только для реального слага.
13. **Negative-cache 10 с** (`middleware/channel.rs:41`) при reconcile 5 с: только что созданный канал не виден по слагу до 10 с, если попал в negative-кэш по прямому изменению в БД (не через мутацию API).
14. **`CachedChannelResolution::from_decision`** (`middleware/channel.rs:120-160`) для не-host источников пишет в `ChannelContext.target_type/value` первый primary-таргет: для `X-Channel-ID` запроса это не тот таргет, через который пришёл запрос — диагностика вводит в заблуждение.
15. **Локализация UI**: `admin/locales/{en,ru}.ftl` — проверить полноту ключей (`common.*`, `channel.*`) после перевода на `rustok-ui`; сейчас часть подписей дублируется дефолтами прямо в коде (`t(…, "flag", "Default")`).

---

## 6. Контрактные и документационные расхождения (§1/§14 AGENTS)

| Утверждение | Где | Реальность |
|---|---|---|
| `ChannelReadPort` deadline-aware, `deadline_required: true`, `context_deadline_preserved` | `README.md`, `contracts/channel-fba-registry.json:14,44`, `contracts/evidence/channel-contract-test-static-matrix.json` | deadline не реализован и не тестируется (P2.5) |
| «REST secondary path parity» | `README.md`, план | fallback на любую ошибку, два разных контракта ошибок, SSR-ветка неаутентифицируема (P1.2) |
| «channel … hard delete retains an exact source version» | `README.md` | команды удаления нет, предикат недостижим (P1.3) |
| `channels.is_active` как рабочий признак | DTO/UI/резолвер/порты | всегда `true` (P1.3) |
| «Sixteen authenticated native endpoints» | план | 15 mutation-path + bootstrap — совпадает по количеству, но список `NATIVE_CHANNEL_MUTATION_PATHS` (`middleware/channel_native_wrapper.rs:16-32`) дублируется вручную и не выводится из адаптера |
| «Known Limitations» | `README.md` | отсутствуют: неизвестный селектор → 500, отсутствие lifecycle-команд, отсутствие Next.js-поверхности, отсутствие deadline-реализации |
| `pricing`-потребитель списка каналов | `registry.md` | кап 250 без пагинации (P2.9) |

Проверочные скрипты (`npm run verify:channel:resolution-contract`) фиксируют **порядок строк** в `resolution.rs` (`scripts/verify/verify-channel-resolution-contract.mjs:28-41`), а не поведение при ошибке/неизвестном селекторе. Поэтому P1.1 проходит все гейты.

---

## 7. Рекомендации

### 7.1. Быстрые правки (1–2 дня, без изменения схемы)

1. **P1.1**: `NotFound` в `HeaderId`-стадии → `Miss` + продолжение конвейера; в middleware разделить `Database` (5xx) и остальное (400). Тесты: неизвестный id, неизвестный slug, битый UUID (уже есть), дублирующийся заголовок (уже есть).
2. **P2.2**: пустой/длинный `?channel=` → Miss, а не 400.
3. **P1.2**: убрать blind fallback; fallback только по «транспорт недоступен», мутации не повторять никогда; убрать `localhost:5150` как production default.
4. **P2.11**: fallback локали вместо `Err` при `und`.
5. **P2.5**: реализовать deadline в `ChannelReadPort` **или** убрать `deadline_required`/`context_deadline_preserved` из реестра и evidence (нельзя оставлять ложное свидетельство).
6. **P3.8/доккоменты**: поправить doc-comment над `ensure_channel_slugs_exist_for_tenant_in_tx`; `Validation` для nil tenant.
7. Внести в `README.md` → «Known Limitations»: отсутствие Next.js-поверхности, отсутствие lifecycle-команд, отсутствие deadline.
8. Добавить в `docs/standards/logic-hygiene-patterns.md` (AGENTS §4) новые паттерны:
   - `P-SELECTOR-ERROR-ESCALATION` — клиентский селектор не должен превращать ошибку в 5xx;
   - `P-BLIND-TRANSPORT-FALLBACK` — fallback только на транспортные ошибки, мутации не повторяются;
   - `P-UNREACHABLE-LIFECYCLE-STATE` — объявленные состояния должны производиться owner-командой (или удаляться);
   - `P-GLOBAL-GENERATION-INVALIDATION` — генерация инвалидации по ключу, который меняет смысл кэша (tenant);
   - `P-DECLARED-PORT-DEADLINE` — контракт порта и evidence обязаны совпадать с реализацией.

### 7.2. Структурные правки (1–2 недели)

1. **Жизненный цикл канала** (P1.3): `set_channel_active`, `update_channel` (name/settings), затем `delete_channel` с атомарным tombstone/identity bump; REST + native + UI + тесты. Решение оставить/удалить `status`.
2. **DB-инварианты** (P1.4): частичные уникальные индексы `channels(tenant_id) WHERE is_default`, `channel_resolution_policy_sets(tenant_id) WHERE is_active`, уникальность host-претензий в тенанте; `upsert` для `bind_module`/`bind_oauth_app`; unique-violation → 409; одна транзакция для `create_resolution_policy_set(is_active)`.
3. **Tenant-safe owner API** (P2.6): все мутации принимают tenant и проверяют его внутри; `bind_oauth_app` проверяет тенант приложения; `bind_module` — существование модуля.
4. **Per-tenant генерация инвалидации** (P2.3) + bypass кэша вместо `Unhealthy` (P2.4).
5. **Definition-модель правил** (P2.1): явная структура предикатов, запрет дублей/противоречий, валидация `priority`.
6. **UI на каноническом стеке** (P2.7): `rustok-ui-leptos` + `rustok-grid-leptos`, тесты `core/` на все ветвления.
7. **Границы** (P2.10): порт Auth для OAuth-приложений вместо прямых SQL/моделей.
8. **Перф** (P2.8/2.9): батч-загрузка detail, пагинация bootstrap, единая семантика `list_channels`.

### 7.3. Продуктовые/архитектурные

1. **`channel_translations`** и классификация `policy_sets.name` (P2.14) — с бэкфиллом и тестами стабильности локали.
2. **Next.js админка канала** или явно документированный gap с владельцем и сроком (P2.13).
3. **Уточнить семантику канала в гейтах**: `channel_module_bindings` — это *дистрибуционный скоуп*, а не авторизация, потому что клиент сам выбирает канал (`X-Channel-ID`/`X-Channel-Slug`/`?channel=`). Сейчас `pages`/`navigation` используют биндинг как 404-гейт, и вызывающий может обойти его, выбрав другой (легальный) канал. Если это не задумано — нужно либо запретить клиентские селекторы для публичных запросов (только host+policy), либо явно записать в документации, что это не граница безопасности.
4. **Единый источник списка мутационных путей** для `NATIVE_CHANNEL_MUTATION_PATHS` (генерировать из адаптера или проверять скриптом, а не вручную).

---

## 8. Что добавить в верификацию

- Поведенческие тесты резолвера: неизвестный `X-Channel-ID`, неизвестный `X-Channel-Slug`, пустой `?channel=`, конфликтующие дубликаты, неактивный канал (после появления disable).
- Тесты гонок (SQLite/PG): два параллельных `create_channel` → ровно один default; два параллельных `set_default_channel` → ровно один default; параллельные `bind_module` → upsert без 500; `create_resolution_policy_set(is_active)` → ровно один активный.
- Тесты инвариантов при ошибке: `ChannelReadPort` deadline → `PortErrorKind::Timeout`; unique-violation → 409.
- Мутационный гейт для UI: запретить сырые `<button>/<input>/<select>` в `crates/modules/*/admin/src/ui` (расширить существующий `verify:ffa:ui:migration`).
- Проверка соответствия `NATIVE_CHANNEL_MUTATION_PATHS` ↔ серверным функциям адаптера.
- Локально: `cargo check -p rustok-channel`, `cargo test -p rustok-channel --lib`, `cargo test -p rustok-channel-admin --test tenant_scope_contract`, `cargo check -p rustok-channel-admin --features ssr`, `cargo test -p rustok-server middleware::channel::tests --lib` (по правилу §4 — без workspace-wide прогонов).

---

## 9. Проверено / не проверено

**Проверено статически:** `src/**` целиком (резолюция, политики, сервис, DTO, порты, entity, все 12 миграций, index identity/projection), `admin/**` (core, model, transport native/REST, ui leptos), `ports/**` (gRPC client/server, scope), серверный транспорт и рантайм кэша/инвалидации/guardrails, потребители pages/navigation/blog/forum/commerce/pricing, `README.md`, `CRATE_API.md`, `docs/**`, контракты/evidence, ADR по каналу.

**Не проверено:** компиляция, `clippy`, тесты, PostgreSQL/SQLite-прогоны миграций, Redis-сценарии инвалидации, рантайм `apps/server`, E2E-поведение админки и сторфронта — в песочнице отсутствует Rust-тулчейн (и внешняя сеть ограничена). Утверждения о поведении БД основаны на тексте миграций, а не на исполнении.

---

## 10. Статус исправлений (2026-11-26, ветка `arena/a3d16606-rustok`)

Реализовано в коде (не только в отчёте). Каждый пункт — конкретные файлы.

### 10.1. P1.1 — неизвестный селектор больше не даёт 500

- `crates/modules/rustok-channel/src/resolution.rs`: `X-Channel-ID` резолвится через `match` по `Result`; `ChannelError::NotFound` → `ResolutionOutcome::Miss` и переход к следующей стадии (как у slug), остальные ошибки по-прежнему пробрасываются.
- Следствие: неизвестный/чужой/неактивный id не ломает запрос, а падает в `Rejected`/`Miss` с корректной трассой; поведение симметрично slug-стадиям.

### 10.2. P1.2 — один транспорт, без «слепого» fallback, без localhost

- `admin/src/transport/mod.rs` переписан: `selected_transport_path()` (SSR/hydrate → server function, browser → REST) + единственная точка `dispatch(surface, native, rest)`. Fallback удалён полностью, поэтому мутация никогда не выполняется дважды, а ошибка 4xx больше не маскируется повторным запросом; ошибка транспорта логируется с `transport = path.as_str()`.
- `admin/src/transport/rest_adapter.rs`: удалены `RUSTOK_API_URL` и `http://localhost:5150`; URL строится от `window.location.origin`, вне браузера REST-путь возвращает явную ошибку (SSR всегда идёт в server function).
- `scripts/verify/verify-channel-admin-boundary.mjs` обновлён под новый контракт (запрещает `Err(_) =>`-fallback, `RUSTOK_API_URL`, `localhost`; требует `selected_transport_path`/`dispatch`/origin).

### 10.3. P1.3 — жизненный цикл канала доступен и корректен

- `ChannelService`: новые `update_channel` (имя/settings, slug неизменяем), `set_channel_active`, `delete_channel`, `set_default_channel_for_tenant`, `ensure_channel_for_tenant`. Деактивация/удаление default-канала в той же транзакции повышают старейший активный канал, иначе `is_default` повис бы на неактивном.
- Удаление опирается на каскадные FK (`channel_targets`, `channel_module_bindings`, `channel_oauth_apps`, `channel_resolution_policy_rules`) и PG-триггер tombstone (`index_revision`), поэтому `InactiveChannel`, tombstones и identity-поколение больше не «недостижимые» ветки.
- `channels.status` удалён (`m20261126_000013`): публикационное состояние выводится из `is_active` (`ChannelStatus`), в индексе — `CASE WHEN c.is_active THEN 'active' ELSE 'inactive' END`. Литерал `"experimental"` и второй источник правды исчезли.
- REST: `PATCH /api/channels/{id}`, `POST /api/channels/{id}/active`, `DELETE /api/channels/{id}` (`apps/server/src/controllers/channel.rs`), ровно те же операции в SSR-адаптере (`channel/update-channel`, `channel/set-active`, `channel/delete-channel`) и в `NATIVE_CHANNEL_MUTATION_PATHS`.
- Админка: в `channel_card.rs` появились Deactivate/Activate, inline Rename и Delete с подтверждением; тест `tenant_scope_contract` обновлён (19 scoped endpoint'ов).

### 10.4. P1.4 — инварианты на уровне БД + атомарные записи

- Миграция `m20261126_000013_channel_lifecycle_invariants` (PG + SQLite): частичные уникальные индексы `uq_channels_tenant_default`, `uq_channel_resolution_policy_sets_tenant_active`, `uq_channel_targets_channel_primary`, `uq_channel_targets_tenant_host`, композитный FK `(tenant_id, channel_id) → channels(tenant_id, id)`, `channel_targets.tenant_id` (денормализованный tenant для этих инвариантов); перед созданием индексов данные «лечатся» (дедуп default/active/primary/host).
- `create_channel`: вставка как проверка конфликта (slug → `SlugAlreadyExists`, гонка за default → один ретрай без default); `is_unique_violation`/`is_channel_slug_conflict`/`map_unique_violation`.
- `add_target`/`update_target`: одна транзакция, `clear_primary_target` вместо цикла, unique-violation → `TargetAlreadyExists` (409), а не 500.
- `bind_module`/`bind_oauth_app`: `INSERT … ON CONFLICT … DO UPDATE` (upsert по существующим уникальным индексам) вместо find-then-insert.
- `create_resolution_policy_set`: создание + активация в одной транзакции (`replace_active_policy_set_in_tx`); `replace_default_channel`/`replace_active_policy_set` — два UPDATE (clear→set) вместо перебора строк.
- `ChannelError::Conflict`; REST-контроллер маппит его в `409`, витрина — в `503` с retry-советом (§10.15). Хелперы `is_conflict()`/`is_client_error()` в итоге удалены как фантомный API (см. §10.8).

### 10.5. Прочие закрытые пункты

- P2.1: `ChannelResolutionRuleDefinition::validated()` запрещает дубли предикатов и комбинацию `host_equals`+`host_suffix`; `locale_equals` валидируется как BCP-47-подобный; `create_resolution_rule_input` возвращает уже нормализованное определение (одно представление вместо двух).
- P2.6 (deadline): `ChannelReadPort` реально соблюдает `deadline_ms` (`within_deadline` → `port.deadline_exceeded`, `PortErrorKind::Timeout`), включая `list_channels_for_tenant`.
- P2.5 (tenant): `ensure_channel_for_tenant` в owner'е + существующие проверки на обоих транспортных границах (REST и SSR) — теперь единый owner-примитив, а не «доверие вызывающему».
- P3: восстановлена испорченная doc-строка `is_module_enabled_for_tenant`; CRATE_API обновлён (`status`, lifecycle, deadline).

### 10.6. Что осталось (осознанно не в этом проходе)

1. **P2.7 — миграция админ-UI на `rustok-ui-leptos`/`rustok-grid`**: карточки канала всё ещё на сырых `<button>/<input>` с inline-классами; это самый большой остаток и он требует отдельного прохода с `verify:ffa:ui:migration`.
2. **P2.2/P2.3 (глобальная invalidation generation)**: сознательно оставлено как есть — это платформенный паттерн (`tenant_locale_generation` использует тот же namespace-wide fence), per-tenant refinement требует изменений в raft нескольких тестов инвалидации.
3. **P2.10** (порт Auth для `oauth_apps`), **P2.11** (`und` локаль), **P2.13** (Next.js parity), **P2.14** (`channel_translations`) — не тронуты.
4. ~~**`Conflict` в витринном контроллере commerce** отдаёт `500 commerce_store_channel_failed`~~ — закрыто в §10.15: конфликт на read-path стал `503` с retry-советом.
5. Долг по тестам: `crates/modules/rustok-forum/tests/category_route_storefront_transport_contract.rs` ссылается на несуществующий `native_server_adapter_category_route.rs`, поэтому тест падал и до этого прохода (форумный код уже переведён на `is_module_enabled_for_tenant`).

### 10.7. P2.4, P2.8, P2.9, P2.12 — закрыто во втором проходе

- **P2.4 (guardrail инвалидации).** `runtime_guardrails.rs` больше не поднимает весь репликант в `Critical` при неготовом слушателе инвалидации: статус — `Degraded`, а корректность обеспечивается схемой «нет инвалидации → нет кеша». В `middleware/channel.rs` появился `channel_resolution_cache_usable()`: если handle зарегистрирован и не готов, резолв идёт в owner на каждом запросе (`channel_resolution_cache_bypassed` в debug-логе), а не из локального кеша; отсутствующий handle (embedded/тесты) по-прежнему считается нормой — там единственный путь инвалидации локальный fence.
- **P2.8 (3N+1 в деталях).** `ChannelService::build_channel_details(Vec<Model>)` собирает targets/module bindings/oauth apps тремя set-based запросами (`is_in`) и раскладывает по `HashMap`; `list_channel_details_page` и `list_channel_details` больше не делают по три запроса на канал. `build_channel_detail` остался тонкой обёрткой (одиночные вызовы `get_default_channel`/`get_channel_by_host_target_value`).
- **P2.9 (лимит 250).** Новый owner-метод `list_channel_options(tenant_id, include_inactive)` без пагинации и без молчаливого обрезания. Потребители переведены: commerce `storefront_pricing_channels` → `false` (сторфронт не должен предлагать неактивные каналы), pricing storefront → `false`, pricing admin bootstrap → `true` (деактивированный канал может владеть прайс-листами). Гейты `verify-commerce-graphql-query-channel-error-safety` и `verify-pricing-storefront-native-error-safety` обновлены под новый маркер, док `graphql-query-channel-error-safety.md` поправлен.
- **P2.12 (module gate без tenant).** Все 7 запросных вызовов `is_module_enabled(channel_id, slug)` переведены на `is_module_enabled_for_tenant(tenant_id, channel_id, slug)`: pages HTTP-контроллер, pages GraphQL gate, pages storefront (host-route + native), navigation HTTP-контроллер, navigation GraphQL gate, navigation storefront. В navigation storefront гейт перенесён после резолва tenant'а (чтобы не сломать путь «tenant_slug без TenantContext»). Нескоупленный `is_module_enabled` остаётся только внутри owner'а.
- **Фикстуры.** Из ручных SQL-фикстур убран столбец `channels.status` (commerce `storefront_channel.rs`, `checkout_service_test`, `graphql_runtime_parity_test`, `store/tests`, pricing admin `support.rs`, forum `storefront_channel_scope_sqlite.rs`, `rustok-starter` `starter_import_integration.rs`): после миграции 000013 этих фикстур с `status` быть не может.

### 10.8. Код-ревью прохода (найдено и исправлено)

Ревью по `docs/CONTINUOUS_CODE_REVIEW.md` (столпы 0–8 + FFA/FBA) по всему диффу:

**Ошибки компиляции (тип `status` стал `ChannelStatus`):**
- `rustok-commerce/src/graphql/types.rs` (`GqlPricingChannelOption`), `rustok-pricing/admin/…/mappers.rs`, `rustok-pricing/storefront/…/native_server_adapter.rs` — `String`-полю присваивался `ChannelStatus` → `value.status.as_str().to_string()`.
- 5 тестовых `channel_context()` (`rustok-pages/storefront/tests/*`, `apps/server/tests/pages_production_relay_native_route_sqlite.rs`) — `channel.status.clone()` в `rustok_api::ChannelContext.status: String` → `as_str().to_string()`.
- `rustok-commerce/tests/pricing_service_test/main.rs` — `channel::ActiveModel { status: Set(...) }` больше не существует; строка удалена.

**Ложь в публичных контрактах:**
- `ChannelError::Conflict` в commerce GraphQL попадал в бакет «invalid request» / `CHANNEL_REQUEST_INVALID` с `retryable: false`. Гонку нельзя называть невалидным запросом: добавлен отдельный конверт `CHANNEL_CONCURRENT_CONFLICT` (`retryable: true`), обновлены гейт и `graphql-query-channel-error-safety.md`.
- `owner_detail` для `Conflict` отдавал бессмысленную метрику `…count().min(1)`; теперь обычная длина как у прочих неотредактированных вариантов.
- `bind_module`/`bind_oauth_app` отвечали `404 channel not found`, если строка upsert'а не находилась — канал при этом существует. Теперь `ChannelError::Database(DbErr::RecordNotInserted)`.

**FFA / i18n:**
- Новые ключи UI не существовали в каталогах (`admin/locales/en.ftl`, `ru.ftl`) и один префикс был не по конвенции (`channel.errors.*` вместо `channel.error.*`). Добавлены 13 ключей в оба каталога, ключи переименованы, проверено скриптом: 141 используемый ключ модуля — все присутствуют в обоих бандлах.
- Переход состояния канала (`!is_active` + выбор подписи/фидбэка) жил в Leptos-адаптере. Вынесен в core: `core::channel_lifecycle_toggle(is_active) -> ChannelLifecycleToggle { next_is_active, label_key, label_fallback, feedback_key, feedback_fallback }` с unit-тестом; адаптер только биндит значения.
- Документация: `admin/README.md` всё ещё обещал REST как «parallel fallback path» — переписан под единственный транспорт.

**FBA / data integrity:**
- `delete_module_binding` / `delete_oauth_app_binding` оставались read-then-delete без tenant-скоупа в запросе на удаление; переведены на `delete_many` + `rows_affected`, как `delete_target`.
- `ports.rs`: проверка нулевого дедлайна дублировала платформенный `PortContext::require_deadline_semantics()` (который уже вызывается через `require_policy(PortCallPolicy::read())`); хелпер сведён к проекции бюджета, добавлены два теста на `within_deadline` (таймаут с `port.deadline_exceeded` и сохранение результата в бюджете).
- Удалены неиспользуемые `ChannelError::is_conflict` / `is_client_error` (фантомный API: ни одного потребителя, а оба транспортных маппинга уже исчерпывающие).
- `policy.rs`: `map_or(true, …)` → `is_none_or(…)` (clippy `unnecessary_map_or` при `-D warnings`).
- Фикстуры `rustok-api/src/request.rs` и `admin/src/core.rs` заполняли `status: "experimental"` — заменено на реальное значение.

**Проверено и оставлено как есть:** `PortErrorKind::Conflict` с `retryable: false` — совпадает с платформенным `PortError::conflict`; `Conflict` в storefront-маппере commerce остаётся fail-closed `500` (read-path, гонки записи там нет); verb'ы REST-транспорта сверены с роутером (`GET bootstrap`, `POST /api/channels/`, `PATCH|DELETE {id}`, `POST {id}/active`, `POST {id}/default`, targets/modules/oauth/policies).

### 10.9. Третий проход: добитые риски и остатки FFA

**Проверено по прецеденту в кодовой базе (было в списке «не проверено из-за отсутствия `cargo`»):**

- `.update_columns([...]).to_owned()` в двух upsert'ах `channel_service.rs` — тот же байт-в-байт паттерн уже есть в компилирующемся `rustok-flex/src/attached_field_policy.rs` (и в 73 других `OnConflict::` местах), поэтому `&mut OnConflict` → `to_owned()` типизируется корректно.
- `Expr::value(DateTimeWithTimeZone)` в `update_many().col_expr(..)` — платформенный паттерн (`rustok-cart`, `rustok-ai`, `rustok-blog`, `alloy`), включая модули с SQLite-тестами; биндинг поддерживается.
- Импорты `channel_service.rs` (`PaginatorTrait`, `ConnectionTrait`, `TransactionTrait`, `QueryOrder`, `ActiveModelTrait`, алиасы `*ActiveModel`) — все используются через методы (`paginate`, `begin`, `order_by_*`, `insert/update/delete`), «мёртвых» импортов после переписывания unbind'ов нет.

**FFA: семантика записи ушла из адаптера целиком.**

- В `core.rs` добавлены `channel_rename_command(raw_name)` (trim, отказ на пустом имени с `channel.error.nameRequired`, готовый `UpdateChannelPayload`, ключ и фолбэк фидбэка) и `CHANNEL_DELETE_FEEDBACK`. `channel_card.rs` больше не собирает payload, не валидирует имя и не хранит копию фидбэка; в адаптере осталось только view-состояние (`renaming`, `confirming_delete`, `busy`).
- Гейт `verify-channel-admin-boundary` расширен в том же изменении: core обязан владеть `ChannelLifecycleToggle`/`channel_lifecycle_toggle`/`channel_rename_command`/`CHANNEL_DELETE_FEEDBACK`, UI обязан их потреблять и не содержать `UpdateChannelPayload {`, `channel.feedback.renamed`, `channel.feedback.deleted`, `channel.error.nameRequired`. В `.test.mjs` добавлены две негативные фикстуры (инлайн-payload, инлайн-копия) — 16/16 тестов.
- Единообразие с `update_payload()/create_payload()` у `PolicyRuleFormState`: адаптер вызывает хелпер, а не конструирует DTO.

**Миграция 000013: симметрия up/down на SQLite.** `down` падал на старых SQLite (до 3.35) при `DROP COLUMN tenant_id`, хотя `up` терпит ту же ситуацию с `channels.status`. Теперь оба направления трактуют «no such column»/«syntax error» как «движок не умеет — оставляем колонку», порядок `DROP INDEX` перед `DROP COLUMN` сохранён (SQLite отказывается удалять индексированную колонку).

**i18n перепроверено после переноса:** 143 UI-ключа (без 20 транспортных идентификаторов `channel.admin.*`) резолвятся в обоих каталогах, ключи rename/delete/nameRequired существуют в `en.ftl`/`ru.ftl`.

### 10.10. Построчный аудит модуля: найденные дефекты

Проход по `crates/modules/rustok-channel` файл за файлом (миграции ↔ сущности, `channel_service`, `policy`, `resolution`, `ports`, `dto`, `index_projection`, admin) и по затронутым интеграциям.

**Исправлено:**

1. **Критично: шим GraphQL-safe_query не повторял вызываемый owner-метод.** `graphql/query.rs` компилируется дважды — напрямую и внутри `safe_query`, где имя `rustok_channel` резолвится в шим. Резолвер вызывает `list_channel_options`, а шим объявлял `list_channels`, поэтому boundary-сборка не компилировалась (Node-гейты этого не видели). Шим приведён к фактическому вызову, а гейт `verify-commerce-graphql-query-channel-error-safety` теперь структурно сверяет: каждый `channel_service.<method>(` из резолвера обязан существовать в шиме. Проверено негативным прогоном — со старым шимом гейт падает.
2. **Не компилировалось: `rustok-navigation` storefront.** При переносе tenant-scoped гейта после резолва tenant'а удалили `let tenant = extract::<TenantContext>()`, а `tenant.as_ref()` ниже остался. Биндинг восстановлен; дополнительно прогнан скрипт «удалённый `let` всё ещё используется» по всему диффу.
3. **`verify-tenant-context-isolation` пинал старую форму ветки cross-tenant rejection** в `resolution.rs`; маркер обновлён на `Ok(detail) if detail.channel.tenant_id != facts.tenant_id` плюс отдельная проверка текста трейса. Этот гейт не входил в прежний свип из 471 скрипта — поэтому теперь свип идёт по всем 1223.
4. **`verify-channel-proof-points` пинал снятый unscoped `is_module_enabled`** для pages (storefront и GraphQL) — переведён на форму `is_module_enabled_for_tenant(...)`.
5. **Admin-DTO дублировали `status`** (`ResolvedChannelContext`, `ChannelRecord`), хотя после 000013 это derived-поле, а карточка печатала машинное `active`/`inactive` без перевода. Поля удалены (serde игнорирует лишнее в ответе сервера), бейдж выводится из `is_active` через новый `core::channel_status_badge` и ключи `channel.card.statusActive` / `channel.card.statusInactive` (en + ru).
6. **Гонки после 000013 отдавали `500`.** `replace_default_channel` и `replace_active_policy_set_in_tx` упирались в новые partial unique индексы и возвращали `ChannelError::Database`; теперь оба маппятся через `map_unique_violation` → `Conflict` (409 REST / retryable).
7. **`list_resolution_policy_sets` — 1+N запросов** (используется в bootstrap админки): правила читаются одним `is_in`-запросом и раскладываются по `HashMap`, как в `build_channel_details`.
8. **`CRATE_API.md` описывал несуществующий контракт**: `CreateChannelInput(name, slug, default_locale, currency, is_default, is_active)` и `RequestFacts(host, path, headers, client_ip, target_surface)` вместо реальных полей; в списке ошибок не было шести вариантов, у `PortError` — `Conflict`/`Timeout`/`InvariantViolation`. Раздел переписан по коду, добавлены инварианты БД (default активен, один `web_domain` на tenant, один primary target на канал).
9. **Миграция 000013, `down` на SQLite**: `DROP COLUMN tenant_id` падал на движках < 3.35, хотя `up` терпит ту же ситуацию с `columns.status`. Направления выровнены.

**Найдено, но не исправлено (предсуществующий дрейф, вне этого изменения):**

- ~~`ensure_oauth_app_belongs_to_tenant` отвечает `400`~~ — закрыто в §10.13.
- ~~`remove_resolution_rule` использует `ActiveModel::delete()`~~ — закрыто в §10.12.
- ~~`add_target`/`update_target` любой unique-violation отдают как `TargetAlreadyExists`~~ — закрыто в §10.13.
- ~~`verify-channel-proof-points` падает на отсутствующем `topic_visibility_list.rs`~~ — закрыто в §10.15: пин переехал на фактический `services/topic.rs`, скрипт зелёный.
- Storefront-контроллер commerce по-прежнему отдаёт `500` на `ChannelError::Conflict` — read-path без гонки записи, fail-closed; менять вместе с его гейтом.

### 10.11. Верификация этого прохода

Выполнено (node-гейты, тулчейна Rust в песочнице нет):

- сверка пунктов 1–9 §10.10 с кодом: каждый пункт подтверждён чтением файла (шим `list_channel_options` + зеркальная проверка резолвер↔шим, биндинг `TenantContext` в navigation, `detail.channel.tenant_id`-маркер в `verify-tenant-context-isolation`, `is_module_enabled_for_tenant` в `verify-channel-proof-points`, отсутствие `status` в admin-моделях + ключи `channel.card.statusActive|statusInactive` в en/ru, `map_unique_violation` на трёх точках продвижения default и на активации policy-set, `is_in`+`HashMap` в `list_resolution_policy_sets`, `Conflict(String)` в `CRATE_API.md` + все 13 вариантов ошибок, ветка толерантности в `down()` 000013). Ни один пункт не отозван;
- `verify-channel-admin-boundary` — pass (+ его `.test.mjs`, 17/17 негативных фикстур);
- `verify-channel-fba`, `verify-channel-resolution-contract`, `verify-channel-runtime-fallback-smoke`, `verify-ffa-ui-migration-contract` — pass;
- `verify-channel-lifecycle-invariants` — новый гейт (§10.12, дополнен в §10.13), pass (+ его `.test.mjs`, 16/16 негативных фикстур);
- `verify-channel-proof-points` — **pass** после правки пина в §10.15 (на базе падал), т.е. хвост скрипта (forum `seo_targets`, README, список модулей) проверяется впервые.
- `scripts/verify/verify-remediation-gate.py --skip-cargo` — diff-инварианты pass (нет `#[allow]`, нет `todo!()`, нет удалённых тестов); гейт в целом отклоняется на архитектурном guard'е, который шеллит `cargo metadata` (в песочнице `cargo` нет) — это состояние базы, а не следствие изменения.
- Все 42 гейта, упоминающие `rustok-channel`/`rustok_channel`, прогнаны после правок §10.12: 33 pass, 9 падений — все входят в набор предсуществующих 333 падений базы (в т.ч. `verify-channel-proof-points` и `verify-pages-ui-boundary`).
- Полный свип по **всем 1224** `scripts/verify/*.mjs` (кроме `*.test.mjs`), каждая база — `timeout 120 node <script>`; база прогнана в отдельном `git worktree` на `29f184e`. Итог: база — **333** падения, ветка — **331**; множество падений ветки является подмножеством базы, **новых падений нет**, ровно два расхождения — `verify-channel-proof-points` (§10.15) и `verify-commerce-storefront-shared-http-error-safety` (§10.16), которые на ветке зелёные. Оставшееся 331 — предсуществующий дрейф репозитория (в т.ч. `verify-pages-ui-boundary` на `rustok-pages/admin/src/builder.rs`, forum audience/visibility-гейты, `verify-ecommerce-public-port-error-safety-v2`).

**Не проверено:** `cargo check/test/clippy`, прогон миграции 000013 на PG и SQLite, поведение партиальных индексов и upsert'ов под конкурентностью, работа новых серверных функций и UI-кнопок. Всё перечисленное требует реального тулчейна. Для этого написан интеграционный набор `crates/modules/rustok-channel/tests/lifecycle_invariants_sqlite.rs` (§10.14) — он и должен дать первое runtime-доказательство, как только появится `cargo`.

### 10.12. Закрытые остатки и проверенные «не-дефекты»

После сверки §10.10 с кодом остаток списка «найдено, но не исправлено» был отработан. Итог по каждому пункту:

**Исправлено в этом проходе:**

1. **`remove_resolution_rule`: гонка удаления отдавала `500`.** `ActiveModel::delete()` при конкурентном удалении возвращает `DbErr::RecordNotDeleted`, который контроллер маппил в `500`. Теперь удаление — один `delete_many()` с фильтром по `id` **и** `policy_set_id`, а `rows_affected == 0` возвращает `NotFound` (`404`). Побочно закрыт TOCTOU между проверкой «правило принадлежит набору» и самим удалением: фильтр по `policy_set_id` в самом `DELETE` не даёт удалить правило чужого набора даже при гонке. Владелец больше не использует `ActiveModel::delete()` нигде (гейт это пиннит).
2. **`settings` канала не валидировался вовсе.** `create_channel` подставлял `{}`, `update_channel` писал любое значение как есть: массив или скаляр попадали в JSONB-column, а падало это позже — на чтении в первом модуле, который загрузил канал. Добавлен `normalize_channel_settings`: значение обязано быть JSON-объектом и не превышать 64 KiB; нарушение — `ChannelError::Validation` (`400`). Валидация применяется и на создании, и на обновлении, т.е. поле нельзя «обойти» через `PATCH`.
3. **Структурные инварианты 000013 не были закреплены ни одним гейтом.** Добавлен `scripts/verify/verify-channel-lifecycle-invariants.mjs` (+ `.test.mjs`, 13 негативных фикстур) и npm-алиасы `verify:channel:lifecycle-invariants` / `test:verify:channel:lifecycle-invariants`; гейт включён в цепочку `verify:channel:fba`. Он пиннит: четыре partial unique индекса с их предикатами на обоих бэкендах; `NOT NULL` + композитный FK `(tenant_id, channel_id) → channels (tenant_id, id) ON DELETE CASCADE`; порядок «починка легаси-строк → создание индекса»; снятие `channels.status` в `up()` и восстановление в `down()`; порядок «снять прежний default/active → поставить новый» в `replace_default_channel` и `replace_active_policy_set_in_tx`; маппинг гонок через `map_unique_violation` во всех четырёх точках; отсутствие `ActiveModel::delete()` и чтения `channels.status` вне миграций; `409` в REST и `PortErrorKind::Conflict` в порту.

**Проверено и признано не-дефектом (правка не требуется):**

- **«Host-стадия игнорирует `is_active`»** — неверно: фильтр `channel::Column::IsActive.eq(true)` стоит в owner-методе `get_channel_by_host_target_value`, а не в резолвере; policy-стадия отдельно отбрасывает неактивный канал с записью в трейс. Оба поведения закреплены существующими тестами резолвера.
- **«Мутаторы policy rule/set работают только по id»** — мутаторы правил (`update`, `remove`, `reorder`) проверяют принадлежность правила набору внутри owner'а (после правки §10.12.1 — ещё и в самом `DELETE`/`UPDATE`), а tenant-скоуп набора проверяют оба транспорта (`ensure_policy_set_belongs_to_tenant` в REST-контроллере и в `native_server_adapter`). `set_active_resolution_policy_set` остаётся id-only **в owner'е**, но оба вызывающих транспорта проверяют tenant до вызова; отдельный `_for_tenant`-вариант не добавлен, чтобы не раздувать поверхность ради второго барьера.
- **`is_channel_slug_conflict` матчит имена индексов** — безопасно: в сообщениях PG по новым индексам (`uq_channels_tenant_default` и др.) нет подстроки `slug`, а SQLite для slug-конфликта даёт `UNIQUE constraint failed: channels.tenant_id, channels.slug`. Единственный неоднозначный случай (гонка за tenant default) обрабатывается отдельно — повтором вставки без `is_default`.
- **Создание правил в неактивном наборе и перезапись приоритетов в `reorder`** — легитимный сценарий «подготовить набор и активировать позже»; `reorder` валидирует полноту и уникальность списка правил и работает в одной транзакции.
- **`normalize_web_domain` принимает `localhost` и IPv4-литералы** — намеренно: это валидные host-цели для локальных и self-hosted развёртываний, а конфликт хостов в рамках tenant'а теперь ловится уникальным индексом `uq_channel_targets_tenant_host`.
- **000010-триггер поднимает `index_revision` на любом `UPDATE`** — предсуществующее поведение миграции, вне этого изменения: менять его значит менять семантику инвалидации, закреплённую 000011 (tombstones) и `invalidation_generation`; оставлено как есть и вынесено в остаток.

**Остаётся открытым (предсуществующее, вне этого изменения):** ничего из этого блока — обрыв `verify-channel-proof-points` и `500` на `Conflict` в storefront-read commerce закрыты в §10.15.


### 10.13. Классификация конфликтов целей, транзакционный bind и ответ на чужой OAuth-app

1. **Гонка за primary-slot отдавалась как «цель уже существует».** `add_target`/`update_target` ловили любое unique-violation и возвращали `TargetAlreadyExists(target_type, value)`, хотя `uq_channel_targets_channel_primary` — это гонка двух писателей за один primary-слот, а не дубликат значения (текст ошибки при этом указывал на несуществующий дубликат). Код `409` был верный, сообщение — ложное. Различать индексы по тексту ошибки нельзя: PostgreSQL называет индекс, SQLite — только колонки. Поэтому введён `classify_target_conflict`, который после отката транзакции смотрит на фактическое состояние: если в tenant'е уже есть цель с такой же `(target_type, value)` **и** это `web_domain` — это настоящий дубликат (`TargetAlreadyExists`), иначе — проигранная гонка (`Conflict`, retryable; отдельный текст для primary-слота). Ограничение по `web_domain` важно: уникальность значения в схеме обеспечивает только `uq_channel_targets_tenant_host`, тогда как прочие типы целей могут легально повторять значение на разных каналах tenant'а.
2. **Upsert ↔ read-back в `bind_module`/`bind_oauth_app` не были транзакционными.** Вставка с `ON CONFLICT` и последующий `SELECT` шли отдельными соединениями: конкурентный unbind между ними превращал успешный bind в `Database(RecordNotInserted)` → `500`. Теперь upsert и чтение строки-результата идут в одной транзакции (строка залочена на время транзакции, поэтому чтение всегда видит собственный upsert), а `commit` происходит только после успешного чтения.
3. **Чужой OAuth-app отвечал `400`, а не `404`.** Проверки канала и набора правил в том же контроллере отвечают `404`, а OAuth-app — `400` с текстом «does not belong to the current tenant»: это не только расхождение статусов, но и способ отличить «app существует у другого tenant'а» от «app не существует». Ответ приведён к `404`; on-prem native-транспорт (`ServerFnError`) не различает HTTP-статусы и оставлен как есть.

Гейт `verify-channel-lifecycle-invariants` расширен под эти три пункта: он требует классификатор в обеих точках записи цели, запрещает возвращать `TargetAlreadyExists` из `add_target`/`update_target` напрямую, требует `begin`/`commit` и чтение из `&txn` в обоих bind-методах, и требует `404` (и запрещает `BadRequest("OAuth app…`) в проверке чужого OAuth-app. Негативных фикстур стало 17.

### 10.14. Runtime-доказательство: SQLite-набор по инвариантам жизненного цикла

Всё, что сделано в §10.9–§10.13, до сих пор подтверждено только статикой: в песочнице нет `cargo`. Чтобы следующий прогон CI не проверял «на глаз», добавлен интеграционный набор `crates/modules/rustok-channel/tests/lifecycle_invariants_sqlite.rs` по образцу уже существующего харнесса `resolution.rs` (in-memory SQLite + все миграции модуля). Он использует публичный API `ChannelService`, а для проверки самих индексов — прямые `UPDATE`, которые схема обязана отклонить.

Покрытие (10 тестов):

- `only_one_default_channel_per_tenant` — второй `is_default = 1` отклоняется `uq_channels_tenant_default`;
- `only_one_active_policy_set_per_tenant` — второй `is_active = 1` отклоняется `uq_channel_resolution_policy_sets_tenant_active`;
- `deactivating_the_default_promotes_the_oldest_active_channel` — деактивация default переводит указатель на самый старый активный канал (`created_at` в тесте пришпилен, чтобы порядок не зависел от скорости вставок) и не оставляет неактивный default;
- `deactivating_the_last_active_channel_clears_the_default` — tenant без активных каналов не имеет default вовсе;
- `deleting_the_default_promotes_the_oldest_active_channel` — удаление default продвигает выживший канал в одной транзакции;
- `a_web_host_can_only_be_claimed_once_per_tenant` — повторный захват хоста другим каналом того же tenant'а → `TargetAlreadyExists` (и именно с этим значением, т.е. текстом владеет классификатор §10.13);
- `the_same_web_host_is_allowed_in_another_tenant` — тот же хост в другом tenant'е разрешён: исключительность tenant-скоупная;
- `adding_a_second_primary_target_demotes_the_previous_one` — «снять прежний primary → поставить новый» действительно оставляет ровно один primary target;
- `channel_settings_must_be_a_json_object` — `create_channel` и `update_channel` отвергают массив/скаляр, объект принимают и сохраняют;
- `removing_a_resolution_rule_twice_answers_not_found` — повторное удаление правила даёт `NotFound`, а не `RecordNotDeleted` → `500`.

Два первых кейса заодно доказывают, что партиальные индексы реально применяются на SQLite (миграция 000013 их создаёт с `WHERE … = 1`).

Гейт `verify-channel-lifecycle-invariants` теперь пиннит и сам набор: файл должен существовать и содержать все перечисленные кейсы, поэтому тесты нельзя тихо удалить, «починив» гейт (18 негативных фикстур, все проходят).

Ограничение: набор идёт на SQLite. Композитный FK на `(tenant_id, channel_id)` и `SET NOT NULL` существуют только в PG-ветке 000013, поэтому кросс-tenant целостность целей и `NOT NULL` остаются непокрытыми до прогона против PostgreSQL (`tests/index_identity_postgres.rs` показывает, как такой тест оформляется).

### 10.15. PG-половина 000013, оживление `verify-channel-proof-points` и конфликт на витрине

1. **PG-only инварианты 000013 получили тест.** SQLite-набор (§10.14) не может проверить `SET NOT NULL` по `channel_targets.tenant_id`, композитный FK `(tenant_id, channel_id) → channels (tenant_id, id)` и симметричность `down()` — этих конструкций в SQLite-ветке нет. Добавлен `crates/modules/rustok-channel/tests/lifecycle_invariants_postgres.rs` в стиле `index_identity_postgres.rs`: `#[ignore]` + `RUSTOK_MIGRATION_SMOKE_ADMIN_URL`, одноразовая БД, реальные миграции модуля, затем 6 проверок `up()` (partиальный `uq_channels_tenant_default`, `NOT NULL` по tenant'у цели, отказ композитного FK на канал чужого tenant'а, `uq_channel_targets_tenant_host` внутри tenant'а и разрешённый тот же хост в другом, `uq_channel_targets_channel_primary`, каскад целей при удалении канала) и проверка `down()` (колонка `channels.status` вернулась, индексы и композитный FK сняты, цель снова вставляется без `tenant_id`).
2. **`verify-channel-proof-points` был мёртв на своём же пине.** Скрипт требовал `crates/modules/rustok-forum/src/services/topic_visibility_list.rs`, которого в репозитории нет (файл слит в `services/topic.rs`), и падал исключением `ENOENT` — то есть весь хвост (forum `seo_targets`, README-маркеры, список модулей) не проверялся ни разу. Пин переведён на фактический файл; все три маркера (`apply_tenant_scoped_storefront_channel_filter`, `matching_tenant_topic_channel_access_subquery`, `normalize_public_channel_slug`) там есть, поэтому гейт теперь **green** и действительно проверяет хвост. Поведение канального кода не менялось — это правка устаревшего пина, а не «подгонка» модуля.
3. **`Conflict` на витринном read-path больше не `500`.** `map_storefront_channel_error` отправлял `ChannelError::Conflict` в catch-all `500 commerce_store_channel_failed`. Для витрины это одновременно неверно (гонка записи — временное состояние, а не «сервер сломан») и бесполезно (клиент не знает, что можно повторить). Теперь конфликт — это `503 commerce_store_channel_conflict` с `retryable: true`; `409` для `GET` не выставляется намеренно. Заодно `Database`-ветка тоже получает `retryable: true` — совет берётся из типа ошибки владельца, а не из статуса. Идентичный набор падений у `verify-commerce-storefront-shared-http-error-safety` до и после сверен с базой (8 против 8, тот же список) — новых расхождений нет.

Гейт `verify-channel-lifecycle-invariants` расширен: он требует PG-набор (opt-in `#[ignore]`, реальные миграции, доказательство `NOT NULL`, композитного FK и симметричного `down()`) и явный маппинг канального конфликта в витрине (`SERVICE_UNAVAILABLE` + `commerce_store_channel_conflict`, запрет попадать в 500-arm). Негативных фикстур 20/20.

**Что это меняет для свипа:** `verify-channel-proof-points` уходит из набора предсуществующих 333 падений — гейтов, падающих на ветке, теперь на один меньше, чем на базе; новых падений не появилось.

### 10.16. Оживление `verify-commerce-storefront-shared-http-error-safety`

Этот гейт покрывает ровно ту функцию, которую правил §10.15 (`map_storefront_channel_error`), но сам он падал на базе на семи устаревших маркерах — то есть мою правку витрины он не проверял. Все семь пинов переведены на фактический код, при этом каждый заменённый пин сохраняет исходную гарантию:

1. `StoreContextService::new(` → `StoreContextService::with_ports(` — владелец store-context'а теперь собирается из per-owner read-портов, а не получает `DatabaseConnection`.
2. `rustok_region::RegionService::new(db.clone())` → `rustok_region::in_process_region_read_port(db.clone())` — регион подключён как read-порт.
3. `request_context.channel_slug.as_deref().unwrap_or("current")` (лог-поле) → статичный текст отказа `"The commerce module is not available for the current channel"`: ответ на запрет модуля больше не интерполирует slug канала, поэтому пин закрепляет именно это (никакого эха входных данных в публичном сообщении).
4. `storefront_channel.rs`: `.is_module_enabled(channel_id, module_slug)` → `.is_module_enabled_for_tenant(request_context.tenant_id, channel_id, module_slug)` — пин ужесточён: проверка модуля обязана быть tenant-скоупной (это как раз канальная часть гейта).
5. `pub(crate) async fn ensure_customer_owns_order_for_db(` (лежала в `mod.rs`) → хелпер переехал в `controllers/store/orders.rs` и стал приватным `async fn ensure_customer_owns_order(`; гейт теперь читает `orders.rs` и дополнительно пиннит саму проверку владения (`order.customer_id != Some(customer_id)`) и её код отказа (`commerce_store_order_access_denied`) — раньше проверялось только существование функции.
6. `apply_cart_context_patch_for_db(` → `requested_cart_context(` — слияние патча корзины с контекстом запроса.
7. `enrich_storefront_cart_for_db(` → `reprice_storefront_cart_line_items_for_db(` — «обогащение» корзины теперь идёт через pricing-владельца с типизированным маппингом.

Проверка, что пины не вакуумные: гейт поддерживает `RUSTOK_VERIFY_REPO_ROOT`, поэтому он прогнан на фикстуре из семи реальных файлов — сначала чистой (green), затем с тремя мутациями: возврат `RegionService::new` → падает `region owner read port construction`; возврат unscoped `.is_module_enabled` → падает `tenant-scoped channel service operation`; удаление проверки владения заказом → падает `order ownership assertion`. Все три сработали, после восстановления гейт снова green.

**Итог свипа:** на ветке падает 331 скрипт против 333 на базе; новых падений нет, исправлено два гейта (`verify-channel-proof-points`, `verify-commerce-storefront-shared-http-error-safety`), остальные 331 — предсуществующий дрейф чужих модулей.

### 10.17. Перепроверка сделанного (2026-11-26, второй проход по запросу «перепроверь всё на ошибки»)

Прогон «проверка проверки»: перечитан весь дифф ветки (71 файл), а не только новые места, и каждая статическая гарантия пересчитана заново.

**Что проверено инструментами**

1. **Синтаксис всех изменённых Rust-файлов.** Все 60 изменённых `.rs` разобраны tree-sitter (`tree_sitter_rust`): 0 узлов `ERROR`/`MISSING`. Это строже прежней проверки баланса скобок, потому что ловит и семантику разбора (вложенные `match`, макросы, cfg-блоки).
2. **Импорты нового кода.** Все 18 добавленных `use`-строк имеют потребителя (`std::future::Future` в `transport::dispatch`, `HashMap` в `build_channel_details`, `ChannelStatus` в DTO, `ChannelCacheInvalidationListenerHandle` в middleware, три набора `rustok_api`/`leptos` в новых server-функциях).
3. **Исчерпывающие `match` по `ChannelError`.** Файлов, где ошибка канала разбирается по всем вариантам, пять; в каждом есть arm `Conflict` (`rustok-blog/src/error/mod.rs`, `rustok-channel/src/ports.rs`, commerce `store/mod.rs` и GraphQL-шим, `apps/server/src/controllers/channel.rs`). Остальные потребители (`storefront_channel.rs`, SQLite-набор) используют wildcard.
4. **Литералы `ChannelResponse`.** Все три построения литерала (`ports_tests.rs:48`, `apps/server/src/services/forum_audience_facts.rs:289`, `to_channel_response` в сервисе) содержат поле `status`, остальные вхождения — определение структуры и возвращаемые типы. Структурные литералы `ChannelTargetActiveModel` в репозитории один — в `add_target`, с `tenant_id`.
5. **Совместимость API.** `list_channels`, `set_default_channel`, `is_module_enabled` не удалены — старые вызовы не ломаются; `list_channel_options(tenant_id, include_inactive)` совпадает по сигнатуре во всех 4 точках вызова; `create_resolution_rule_input` возвращает `ChannelResult`, все вызовы используют `?`.
6. **Транспорт ↔ эндпоинты.** `NATIVE_CHANNEL_MUTATION_PATHS` (18 путей) совпадает с набором из 19 `#[server(endpoint = "channel/...")]` в адаптере за вычетом read-only `channel/bootstrap`, что и пиннит тест шима (`!is_native_channel_mutation("/api/fn/channel/bootstrap")`); REST-маршруты в `router()` не дублируются; пути `PATCH /api/channels/{id}`, `POST /api/channels/{id}/active`, `DELETE /api/channels/{id}` совпадают в адаптере, нативном шиме и контроллере.
7. **Миграция 000013 и зависимости.** Ни один триггер, индекс или SQL-файл не ссылается на `channels.status` (000009–000012 чисты), поэтому `DROP COLUMN` не блокируется зависимостями; `fk_channel_targets_channel_id` из 000002 каскадный, значит `SET NOT NULL` после бэкфилла не может упасть на «осиротевших» целях; `uq_channels_tenant_id_id` (уникальный индекс) достаточен как цель композитного FK.
8. **Deadline-контракт порта.** `within_deadline` вызывается только после `require_policy(PortCallPolicy::read())`, который отвергает нулевой/отсутствующий `deadline_ms`, поэтому «нулевой бюджет = мгновенный timeout» не может сработать на легальном вызове; тестовые фикстуры порта выставляют `with_deadline`.
9. **Гейты.** `verify-channel-lifecycle-invariants` (20/20 фикстур), `verify-channel-proof-points`, `verify-commerce-storefront-shared-http-error-safety`, `verify-channel-admin-boundary` (+16 фикстур), `verify-tenant-context-isolation`, `verify-pricing-storefront-native-error-safety`, `verify-commerce-graphql-query-channel-error-safety`, `verify-channel-resolution-contract`, `verify-channel-runtime-fallback-smoke` — все green на текущем дереве; `git diff --check` чист.

**Что исправлено (дефекты документации, не кода)**

1. §10.14: «Покрытие (11 тестов)» → **10**. Одиннадцатый пункт был не тестом, а пересказом двух первых кейсов; он вынесен отдельной фразой. Файл `lifecycle_invariants_sqlite.rs` содержит ровно 10 `#[tokio::test]`, и столько же имён пиннит гейт — то есть цифра «11» расходилась и с кодом, и с гейтом.
2. §10.4 утверждал, что у `ChannelError` есть `is_conflict()`/`is_client_error()`, хотя §10.8 в этом же документе описывает их удаление, а в коде их нет. Формулировка приведена к факту: вариант `Conflict` есть, REST отвечает `409`, витрина — `503`.
3. `docs/implementation-plan.md`: в одном пункте осталось «PG-only часть 000013 ещё требует теста», хотя PG-набор в этом же списке уже описан. Пункт переписан на ссылку на `tests/lifecycle_invariants_postgres.rs` с явной оговоркой, что набор не исполнялся в песочнице.
4. §10.7 (P2.9): имя резолвера `storefront_channels` заменено на фактическое `storefront_pricing_channels`.

**Найденные дефекты кода:** нет. Ни одна из проверок выше не выявила расхождения между кодом и его контрактом; правки этого прохода касаются только документов.

### 10.18. Предсуществующие дефекты, исправленные по разрешению

Оба пункта падали/шумели уже на базе `29f184e`; исправлены только после явного «чини».

1. **`channel_cache_is_registered_atomically` падал на собственной вложенности.** Пин требовал `ctx.shared_get::<Arc<ChannelResolutionCache>>()\n        .unwrap_or(candidate)` — ровно восемь пробелов перед `.unwrap_or`, тогда как rustfmt ставит шестнадцать (`git show HEAD:apps/server/src/middleware/channel.rs` показывает те же шестнадцать и на базе, то есть ассерт был красным всегда, а не из-за моих правок). Вместо подгонки отступа под код ассерт переписан на whitespace-нормализованное сравнение (`split_whitespace().collect::<Vec<_>>().join(" ")`): гарантия — это последовательность «прочитать общий кеш, иначе взять кандидата», а не количество пробелов, поэтому теперь её нельзя «потерять» переформатированием. Сверено статически: needle в нормализованном тексте присутствует, оба остальных ассерта функции (`shared_insert_if_absent(candidate.clone())` есть, `shared_insert(cache.clone());` отсутствует) тоже выполняются.
2. **Мёртвый импорт в pricing admin.** `crates/modules/rustok-pricing/admin/src/transport/native_server_adapter/mappers.rs:2` импортировал `uuid::Uuid` без единого потребителя (файл начинается с `#![allow(unused_imports)]`, поэтому `-D warnings` этого не видел). Импорт удалён; `allow` оставлен как есть, потому что вторая его половина (`clippy::empty_line_after_outer_attr`) всё ещё нужна для пустой строки после атрибута.

Обе правки проверены тем же статическим набором, что и §10.17: файлы разбираются tree-sitter без ошибок, ни один verify-скрипт не пиннит удалённый импорт или старую форму ассерта, гейты `verify-pricing-storefront-native-error-safety`, `verify-channel-admin-boundary`, `verify-channel-lifecycle-invariants`, `verify-commerce-graphql-query-channel-error-safety` и `verify-commerce-storefront-shared-http-error-safety` остаются green.

Отдельно отмечу: `verify-pricing-admin-boundary` падает на `crates/modules/rustok-pricing/admin/src/transport.rs`, которого нет ни на ветке, ни на базе (`git ls-tree HEAD` подтверждает), — это предсуществующее падение из набора 331 и оно **не** тронуто.

**Не проверено (и остаётся непроверенным):** компиляция, `cargo test`, реальные PostgreSQL-миграции и прогон SQLite-набора. В песочнице нет Rust-тулчейна, поэтому все выводы этого прохода — статические (разбор, сверка имён/сигнатур/счётчиков, повторный прогон Node-гейтов) и не заменяют `cargo`-эвиденс.

### 10.19. Проверки, которые были написаны, но нигде не запускались

Глубокое ревью ветки нашло три вещи, которые существовали, были зелёными локально — и при этом не выполнялись ни в одном job'е CI. Гейт, который никто не запускает, не защищает от регресса, поэтому это исправлено, а не задокументировано.

| Что | Состояние до | Состояние после |
| :--- | :--- | :--- |
| `scripts/verify/verify-channel-lifecycle-invariants.mjs` (+ 20 фикстур в `.test.mjs`) | был только в `package.json`; ни один workflow его не вызывал | шаг «Channel lifecycle invariants (gate)» в `.github/workflows/cache-hardening.yml`, job `compiled-contract`, **первым** в списке шагов — до `cargo`: workspace-`check` в этом job'е красный на `main`, и шаг, идущий после него, не выполнился бы никогда. `hardening-gates.yml` не подошёл: он входит в `PROTECTED_PATHS` гейта `verify-migration-infrastructure-approval`, и правка workflow'а требует метки `migration-infra-approved` |
| `crates/modules/rustok-channel/tests/lifecycle_invariants_sqlite.rs` (10 тестов) | исполнялся только косвенно — внутри `cargo llvm-cov nextest --workspace` в job'е `coverage`; отдельного шага не было, и при выключенном покрытии набор не запускался | явный шаг «SQLite channel lifecycle invariants» в `.github/workflows/cache-hardening.yml`, job `postgres-channel` (он не зависит от `needs: check`, который красный на `main`) |
| `crates/modules/rustok-channel/tests/lifecycle_invariants_postgres.rs` | `#[ignore]`, требует `RUSTOK_MIGRATION_SMOKE_ADMIN_URL` — и не запускался **нигде**: job `migration-smoke` в `ci.yml` даёт эту переменную, но висит на `needs: [check, coverage]`, а `Cargo Check` красный на `main` | явный шаг «PostgreSQL channel lifecycle invariants» в том же job'е `cache-hardening`, с `RUSTOK_MIGRATION_SMOKE_ADMIN_URL`, указывающим на сервисный контейнер: тест сам создаёт одноразовую базу и удаляет её |

Почему именно `cache-hardening.yml`: у его job'ов нет `needs: check` (а он красный на `main`, и всё, что от него зависит, не запускается вовсе), он триггерится на `crates/modules/rustok-channel/**`, а его pg-job поднимает живой PostgreSQL. Плюс он не входит в `PROTECTED_PATHS` гейта `verify-migration-infrastructure-approval` — правка `hardening-gates.yml` из этого списка требует метки `migration-infra-approved`, и первый заход в этот файл PR честно завалил. Семантически набор про миграции, но «job, который выполнится» полезнее «job'а, который правильнее называется», — а `migration-smoke` остаётся тем местом, где миграции проверяются на пустой базе.

Ограничение, которое остаётся: в песочнице по-прежнему нет `cargo`, поэтому сами эти три шага здесь не исполнялись — исполнены только Node-гейт (`verify-channel-lifecycle-invariants` — OK, 20/20 фикстур) и проверка пинов экшенов (`verify-workflow-action-pins` — OK).

