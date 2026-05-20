# План реализации `rustok-channel`

Статус: experimental core capability; `v0 baseline complete`. Текущий фокус —
post-v0 rollout policy lifecycle и runtime integration parity.

## Execution checkpoint

- Current phase: plan_sync
- Last checkpoint: Initial bootstrap by registry workflow.
- Next step: Синхронизировать план с текущим кодом и выбрать первый незавершённый пункт.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-05-20T00:00:00Z

## Область работ

- удерживать `rustok-channel` как domain-owned resolution module, а не host middleware bucket;
- синхронизировать channel runtime contract, admin UI и manifest metadata;
- развивать typed resolution policies без возврата к ad-hoc host logic.

## Сводка текущего exploration

- resolver precedence уже закреплён в `crates/rustok-channel/src/resolution.rs`:
  `explicit selectors -> built-in host slice -> typed policies -> explicit default -> unresolved`;
- storage и domain слой для policy уже есть (`channel_resolution_policy_sets` +
  `channel_resolution_policy_rules`);
- server transport (`apps/server/src/controllers/channel.rs`) расширяется вместе с policy lifecycle;
- admin UI (`crates/rustok-channel/admin/src/lib.rs`) уже покрывает базовые operator flows и
  rollout rule-level lifecycle;
- middleware request facts (`apps/server/src/middleware/channel.rs`) пока передаёт
  `oauth_app_id = None` и `locale = None`, из-за чего часть typed predicates работает
  только в synthetic/tests сценариях.

## Необходимые изменения

### 1) Domain contract (`rustok-channel`)

- добавить DTO для update lifecycle policy set/rule (rename/active-toggle/rule update/reorder);
- расширить `ChannelService` методами:
  - `update_resolution_policy_set(...)`,
  - `update_resolution_rule(...)`,
  - `reorder_resolution_rules(...)` (bulk или single move);
- закрепить partial-update contract для `update_resolution_rule(...)`:
  - `priority/is_active/action_channel_id` optional: отсутствие в payload => поле не меняется;
  - `host_equals/host_suffix/oauth_app_id/surface/locale` optional patch fields:
    отсутствие => без изменений, пустая строка => удалить соответствующий predicate, непустое значение => заменить/установить predicate с обычной валидацией/нормализацией;
- зафиксировать инварианты:
  - tenant ownership для policy set, rule и action channel,
  - deterministic order после reorder (без hidden tie-break drift),
  - inactive rule не участвует в `list_active_resolution_rules`.

### 2) Host transport (`apps/server`)

- расширить REST controller `apps/server/src/controllers/channel.rs` для update/reorder/disable policy flows;
- оставить текущую cache invalidation contract (`invalidate_tenant_channel_cache`) для всех новых write-paths;
- при добавлении новых request payload удерживать shared validation semantics
  (host normalization, locale normalization, surface whitelist).

### 3) Runtime facts и middleware integration

- довести `RequestFacts` в `middleware/channel.rs` до реального runtime:
  - прокидывать `locale` из resolved request locale,
  - прокидывать `oauth_app_id` из auth context (`client_id`);
- при необходимости скорректировать middleware ordering в
  `apps/server/src/services/app_router.rs`, чтобы channel resolver видел нужные extension-данные;
- добавить targeted middleware tests на policy predicates `LocaleEquals` и `OAuthAppEquals`
  в реальном request pipeline, а не только на unit-level resolver.

### 4) Admin package (`rustok-channel/admin`)

- закрыть native-first parity для policy operations в `admin/src/api.rs`
  (`#[server]` path + REST fallback, как у channel/target/module flows);
- расширить `PolicyWorkbench` / `PolicySetCard` (`admin/src/lib.rs`) до полного operator flow:
  - rule active toggle,
  - rule reorder (up/down или explicit priority move),
  - rule edit без удаления/пересоздания;
- при появлении отдельного selection state для policy-set/rule держать URL-owned contract
  через `rustok-api` route keys (без package-local state contract).

### 5) Proof points в доменных модулях

- расширять channel-aware proof points (`pages` / `blog` / `commerce`) только вместе
  с explicit tests и локальной документацией;
- для новых channel-aware чтений использовать уже резолвленный host channel context,
  не создавая второй канал выбора в module-local logic.

## Точки интеграции

| Слой | Компонент | Текущая роль | Планируемое изменение |
|---|---|---|---|
| Domain | `crates/rustok-channel/src/services/channel_service.rs` | create/activate/delete policy lifecycle | update/reorder/disable lifecycle + invariants |
| Domain | `crates/rustok-channel/src/resolution.rs` | execution pipeline и trace | подтвердить deterministic policy order после reorder |
| Host REST | `apps/server/src/controllers/channel.rs` | thin channel bootstrap/write API | новые policy update/reorder endpoints |
| Host middleware | `apps/server/src/middleware/channel.rs` | request -> `RequestFacts` -> `ChannelContext` | locale/oauth facts parity с runtime extensions |
| Host composition | `apps/server/src/services/app_router.rs` | middleware chaining | при необходимости корректировка порядка middleware |
| Admin transport | `crates/rustok-channel/admin/src/api.rs` | mixed native/REST, policy mostly REST | native-first `#[server]` parity для policy writes |
| Admin UI | `crates/rustok-channel/admin/src/lib.rs` | базовый policy workbench | full operator flow (reorder/disable/edit) |
| Shared UI routing | `crates/rustok-api/src/route_selection.rs` | channel query keys (`channel_id/target_id/module_slug/oauth_app_id`) + policy edit keys (`policy_set_id/policy_rule_id`) | поддерживать URL-owned selection contract и dependency cleanup (`policy_set_id -> policy_rule_id`) |

## Этапы

### 1. Contract stability

- [x] зафиксировать финальную resolution-модель `explicit selectors -> built-in target slice -> typed policies -> explicit default -> unresolved`;
- [x] удерживать domain-owned resolver внутри `rustok-channel`;
- [x] удерживать sync между runtime contract, admin UI и server middleware tests.

### 2. Policy lifecycle parity

- [x] довести policy trace в admin bootstrap/runtime diagnostics;
- [x] добавить базовые operator flows для policy-set activation и policy-rule authoring/removal;
- [x] добавить policy rule update/reorder/disable lifecycle на уровне `ChannelService`, REST transport и admin UI controls;
- [x] добавить targeted tests на deterministic rule order и inactive-rule exclusion;
- [ ] решить, остаётся ли built-in host slice отдельным fast-path после полного policy rollout.

### 3. Admin operator UX parity

- [x] довести `rustok-channel-admin` до operator flow для policy rules (reorder/disable);
- [x] добавить полноценный rule edit flow (изменение predicates/action без delete+recreate);
- [x] выровнять native-first `#[server]` transport для policy operations с существующими channel CRUD flows;
- [x] при добавлении policy edit-selection state закрепить URL query contract через shared `AdminQueryKey`.

### 4. Runtime integration rollout

- [ ] подключить real request locale и OAuth app id в `RequestFacts`;
- [ ] закрепить middleware ordering и trace parity тестами в `apps/server`;
- [ ] принять решение по built-in host slice (`fast-path` vs policy-only mode) только после закрытия lifecycle parity и с явной документацией решения.

### 5. Semantic expansion

- [ ] возвращаться к richer target/connector taxonomy только при реальном runtime pressure;
- [ ] расширять channel-aware proof points в доменных модулях только вместе с локальной документацией и tests.

## Проверка

- `cargo xtask module validate channel`
- `cargo xtask module test channel`
- targeted server middleware tests для resolution order, explicit selectors, policy predicates и default semantics
- targeted channel service tests для policy lifecycle (`create/update/reorder/disable/delete`)

## Правила обновления

1. При изменении resolution/policy contract сначала обновлять этот файл.
2. При изменении public/runtime contract синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata и UI wiring синхронизировать `rustok-module.toml`.
4. При изменении route-selection contract синхронизировать `rustok-api` (`AdminQueryKey`) и UI docs.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
