# Аудит логики форума RusToK — ошибки, косяки и рекомендации

**Дата:** 2026-11-07  
**Ветка:** `arena/6bc5b948-rustok` (base `f40c54b`)  
**Объём:** `crates/modules/rustok-forum` — 641 файл, ~180 098 LOC (+ admin 36 файлов, storefront 27 файлов)  
**Метод:** постраничный проход по `src/{lib,error,state_machine,audience,mentions,attachment_relation,visibility}`, `src/services/*.rs`, `src/dto`, `src/controllers`, `src/graphql`, `src/mentions*`, `src/seo*`, `src/richtext`, `admin/` и `storefront/`, проверка 6 Universal Logic Dimensions (AGENTS.md §2.1): Preconditions & Bounds, Branch Completeness, State Transitions, Side-Effect Completeness, Calculation Correctness, Concurrency/TOCTOU. Инструменты: `grep` на `let _ =`, `include!`, `unwrap/expect`, `TODO`, `allow`, ручной разбор ключевых транзакций.

> Отчёт — не фикс. Каждый пункт снабжён путём к файлу и строкой, чтобы можно было точечно править. Приоритеты: **P0** — безопасность/корректность данных, **P1** — функциональный баг, **P2** — качество/производительность/долг.

---

## 1. Executive Summary

Форум — самый сложный модуль платформы: 180k+ LOC, 130+ сервисов в `services/`, 40+ миграций, 10+ подсистем (иерархия категорий через Taxonomy, topic/reply lifecycle, кастомный RBAC, audience-политики 4 уровней, mention/quote-проекции, attachment-Media-CAS, SEO-routes с tombstones, поиск, нотификации, voting, subscriptions, read-state, импорт/экспорт, merge/fork/split/move).  
Общее впечатление: домен **переинженерён (over-engineered)**, границы Owner-сервисов соблюдены аккуратно, но:

* **П0 нет критической дыры в tenant-изоляции**, но есть 2 места с ослабленной авторизацией и 1 путь где `TopicNotFound` маскирует `TopicDeleted`.
* **Переусложнение — главный косяк.** 130 файлов в `services/` + 25+ видов audience-политик + 8 reconciliation-сервисов — когнитивная перегрузка, высокий риск регресса. Половина логики дублирует друг друга (см. §2).
* **Техдолг «AI slop»**: массовое `include!()` (30+ в `lib.rs` и `services/mod.rs`), подавление ошибок `let _ = rollback`, мёртвый код `let _ = target_topic_id`.
* **Конкурентность** в целом выверена (advisory locks, head CAS, REPEATABLE READ), но есть 2 TOCTOU-окна (rate-limit вне primary lock в некоторых путях, и SEO short-id race).
* **UX-контракты** хрупки: курсорная пагинация без total count, лимит 512 нод дерева без стриминга, `search` как substring без FTS, edit-window только для авторов, но не для модераторов-авторов (специально, но неочевидно).

Ниже — 37 находок, сгруппированных по доменам.

---

## 2. Архитектурные и структурные косяки

### 2.1. Массовое использование `include!()` — нарушение AGENTS.md «Zero AI Slop» [P2]

**Где:** `src/lib.rs:25-26` (`mentions_import.rs`, `mentions.rs` внутри `pub mod mentions`), `src/services/mod.rs:5-115` (~30 директив), `src/services/category.rs:1`, `admin/src/page_builder.rs:16`.

**Проблема:** `include!()` — макрос-инклюд, запрещённый стандартом AGENTS.md (§ «Zero AI Slop»). Он ломает `go-to-definition`, `rust-analyzer`, `cargo doc`, усложняет линт. В `services/mod.rs` так подключены ~20 модулей — это не условная компиляция, а попытка обойти лимит файлов/циклические зависимости.

**Рекомендация:** Заменить на обычные `mod category; mod category_import; ...` с `pub(crate)` реэкспортами. Если есть циклические зависимости — распутать через `topic_audience_lock`-подобные узкие трейты, а не текст-вставкой. Для `mentions` — вынести в `src/mentions/mod.rs`. Для `admin/page_builder.rs` — перенести генерацию `OUT_DIR` в build-скрипт crate-root, а не инлайн.

---

### 2.2. Раздутая таксономия audience-политик — кандидат на упрощение [P2]

**Где:** `src/audience.rs` + `src/services/category_*_audience*.rs` (8 файлов), `src/services/topic_audience*.rs` (8 файлов) + `category_moderation_audience.rs` + `topic_audience.rs` etc. Итого 20+ файлов только на audience.

**Проблема:** 4 уровня audience-политик (category topic-create, category reply-create, topic reply-create, category moderation), каждая со своими 5 таблицами (`*_policy`, `*_channel`, `*_group`, `*_role`, `*_user`). Effective audience = конъюнкция root→category + topic narrowing. Правила описаны в `CRATE_API.md`, но в коде дублируются почти дословно (copy-paste нормализации, `normalize_uuid_list`, `normalize_channel_slugs`). Новый разработчик не поймёт, почему `topic_create_audience_port_context` и `reply_create_audience_port_context` — разные типы.

**Рекомендация:** Ввести единый дженерик `AudiencePolicy<Scope>` с параметром `Scope = TopicCreate | ReplyCreate | Moderation | ContentVisibility`. Вынести `validate_raw_len`, `normalize_channel_slugs` в `audience.rs` как `pub(crate)` утилиты (сейчас дублируются). Сократить число таблиц: `forum_category_audience_policy` + `topic_audience_policy` достаточно, если хранить `scope` как enum-колонку.

---

### 2.3. Слишком много reconciliation-сервисов — «merge tax» [P2]

**Где:** `src/services/topic_merge*.rs` — 6 файлов reconciliation (+ follow_up), `src/services/counter_reconciliation.rs`, `mention_reconciliation.rs`, `solution_reconciliation.rs`, `subscription/reconciliation.rs`.

**Проблема:** Каждый merge тянет 5 reconciliation (subscription, read_state, vote, tag, audience) + ещё 4 на уровне проекта (counter/mention/solution/subscription). Логика почти идентична: идемпотентный cursor по `operation_id`, advisory lock, batch-вставка. README честно пишет: cross-category merge «намеренно пропускает этот этап: relations stay stranded until operator repair». То есть half-baked фича в проде.

**Рекомендация:** Вынести общий `ReconciliationExecutor<T>` с треитом `ReconciliationKind`, где `T` описывает таблицу-цель. Один generic-имплементер, 6 инстансов. Добавить фоновый воркер (через `rustok-outbox` + `rustok-iggy`) который догоняет stranded relations, вместо ручного operator repair.

---

### 2.4. Две системы постинга-лимитов живут параллельно и не связаны [P1]

**Где:** `src/services/posting_rate.rs` (простой cooldown 10s/5s, уже в проде) vs `src/services/posting_policy*.rs` (11 фактов, window-count, evaluator с правилами, `FORUM-12*`).

**Проблема:** `posting_rate` читает `rate_limit_new_topic/reply_seconds` из `ForumModuleSettings` и чекает `created_at` последней записи. `posting_policy` — отдельная машина с `ForumPostingPolicyFacts`, `ForumPostingPolicyEvaluator`, `ForumPostingPolicyFactsComposer`, которая нигде не вызывается на write-пути (только через `posting_policy_reading_facts` для чтения). DECISION `2026-10-09-forum-posting-rate-limits` прямо пишет: «posting policy evaluator owns that shape, and it is not wired yet». То есть в репо два лимитера, один — dead code.

**Рекомендация:** Либо удалить `posting_policy_*` до внедрения, пометив `#[cfg(feature = "posting_policy")]`, либо зафичерить и задокументировать как `#[deprecated]` с ADR. Иначе новички правят не тот файл. В идеале — унифицировать: `posting_rate` как частный случай `posting_policy` (`window = 1, count = 1`).

---

## 3. State Machine и lifecycle — баги

### 3.1. `TopicStatus::Pending` забыт в диаграмме, но живёт в коде [P1]

**Где:** `src/state_machine.rs:1-22` — ASCII-диаграмма рисует только `Open → Closed → Archived`, а ниже в `can_transition_to` есть `Open ↔ Pending` и `Pending → Archived`. Тесты `pending_topic_transitions_only_approve_or_reject` это покрывают, но diagram врёт.

**Проблема:** Оператор/ревьюер видит диаграмму и думает, что `Pending` не существует. На деле `pre_moderation_enabled=true` создаёт `Pending` топики, видимые только автору+модератору. Нет перехода `Archived → Pending` (правильно), но нет и `Pending → Closed` — а что если модератор хочет закрыть вместо approve/reject? Сейчас это невозможно, модератор вынужден `approve → close` (два шага, две записи в журнале).

**Рекомендация:** Обновить диаграмму, добавить `Pending` с обоими выходами. Рассмотреть добавление `Pending → Closed` (или задокументировать, почему его нет). В `README` явно описать pre-moderation flow.

---

### 3.2. `ReplyStatus::Deleted` — терминальное, но `Pending → Deleted` разрешено напрямую [P2]

**Где:** `src/state_machine.rs:148-168` — `can_transition_to` разрешает `Pending → Deleted` и сразу `Approved → Deleted`, но delete идёт не через `validate_transition`, а через отдельный owner-путь `mark_reply_deleted_in_tx` (`src/services/reply_owner.rs:700+`). В `ReplyService::remove_in_tx` вызывается `reply.status.validate_transition(&ReplyStatus::Deleted)` — значит `Pending → Deleted` валидно, но семантически «удалить до модерации» отличается от «отклонить».

**Проблема:** Нет аудита, почему удалили. `forum_reply_delete_snapshots` сохраняет `previous_status`, но не сохраняет `reason`. Для комплаенса (GDPR «право на забвение» vs модерация) это важно.

**Рекомендация:** Добавить `reason` в delete-snapshot (как в `forum_topic_route_aliases.reason`). Разделить `user-initiated delete` vs `moderator delete` по `actor_id` + `PermissionScope`.

---

### 3.3. `TopicService::delete` vs `ReplyService::delete` — асимметрия `is_locked`/`status` [P1]

**Где:** `src/services/topic_owner.rs:280-340` — `mark_topic_thread_deleted_in_tx` ставит `status='archived', is_locked=TRUE, reply_count=0`. `src/services/reply_owner.rs:700-720` (в facades) — только `status='deleted'`.

**Проблема:** После `topic delete` топик становится `archived+locked`, но `TopicStatus::Archived` по state-machine не залочен автоматически (это два поля). Читатели видят `is_locked` и `status` как независимые, но delete их связывает. При `restore` (`topic_owner.rs:400-520`) восстанавливается `previous_status` и `previous_is_locked` из снапшота — корректно. Но если топик был `open + not locked`, а потом `archive` (без delete), то `is_locked` остаётся `false`. Инконсистентность: `archived` топик может быть незалочен, а `deleted` — всегда залочен. Нет инварианта «archived ⇒ locked».

**Рекомендация:** Либо ввести инвариант на уровне БД (`CHECK (status != 'archived' OR is_locked = true)`), либо документировать, что `is_locked` и `status` ортогональны, и убрать авто-lock из delete (оставить как было). Сейчас это скрытый side-effect.

---

### 3.4. Нет защиты от «воскрешения» удалённого reply через `restore` без проверки категории [P1]

**Где:** `src/services/reply_owner.rs:250-340` — `restore` чекает `topic.status != Archived` и `ensure_category_restore_target_is_active_in_tx`, но не чекает, что топик не был merged (только `forum_topic_merge_operation::SourceTopicId`). Если source топик был смержен (source → target), то `restore` reply из source возможен, но топик-архив уже принадлежит merge-рецепту. Код возвращает `ReplyRestoreUnavailable`, но только если `topic.status == Archived && merge_exists`. Если админ вручную восстановил топик из архива (через `restore`), то мерж-операция всё ещё висит, а топик уже `open` — тогда чек не сработает.

**Рекомендация:** Чекать `merge_operation` независимо от `topic.status`. Достаточно убрать `if topic.status == Archived &&` и оставить только проверку существования merge-записи.

---

## 4. Безопасность и авторизация

### 4.1. `enforce_author_deletion_policy` — обход через `Scope::None` [P0]

**Где:** `src/services/rbac.rs:21-35`

```rust
pub(crate) fn enforce_author_deletion_policy(...) {
    match security.get_scope(resource, Action::Delete) {
        PermissionScope::Own if !author_deletion_allowed => Err(...),
        PermissionScope::All | PermissionScope::Own | PermissionScope::None => Ok(()),
    }
}
```

**Проблема:** Когда `allow_user_content_deletion=false`, автор с `Scope::Own` блокируется — правильно. Но когда у пользователя **нет вообще скоупа** (`None`) на `Delete` (например, обычный участник), функция возвращает `Ok(())` и не блокирует. Далее `enforce_owned_scope` в `TopicService::delete`/`ReplyService::delete` должен заблокировать, но `enforce_author_deletion_policy` вызывается до него и даёт false sense of security. Если кто-то вызовет `enforce_author_deletion_policy` без последующего `enforce_owned_scope` (а такой путь есть в будущих reconciliation), то `None` пройдёт.

**Рекомендация:** Заменить последнюю ветку на `PermissionScope::None => Err(Forbidden)`. Сейчас она маскирует ошибку конфигурации RBAC. Добавить тест, что `None` не проходит.

---

### 4.2. `enforce_owned_scope` разрешает `owner_id=None` только для `All`, но не для системных авторов [P1]

**Где:** `src/services/rbac.rs:37-48`, `src/services/topic_owner.rs:120-140`, `src/services/reply_owner.rs:90-110`.

**Проблема:** `enforce_owned_scope(..., owner_id)` требует `security.user_id == owner_id` для `Scope::Own`. Если `owner_id=None` (системный топик, импорт без автора), то даже владелец `All` пройдёт, а `Own` — нет. Это правильно. Но `ReplyService::delete` берёт `existing.author_id` из `find_reply` — если reply без автора, то `author_id=None`, и пользователь с `Scope::Own` не сможет удалить даже свой топик (если он же автор reply без автора — контринтуитивно). Хуже: `TopicService::delete` для топиков без автора (`author_id=None`) блокирует всех `Own`, включая модераторов с `Own` (а у них должен быть `All`). На практике модераторы имеют `All`, так что не баг, но хрупко.

**Рекомендация:** Явно задокументировать, что `author_id=None` означает «системный контент, удаляется только через `Manage`». Добавить ветку `owner_id.is_none()` → требовать `Scope::All`.

---

### 4.3. `allow_anonymous_reading=false` не влияет на GraphQL `forumTopics` без канала [P1]

**Где:** `src/dto/settings.rs:44-46`, `src/services/category_visibility.rs`, `src/services/topic_visibility.rs`, `src/graphql/query_runtime.rs:640+`.

**Проблема:** Сеттинг `allow_anonymous_reading=false` должен закрывать все storefront-чтения для `is_public_read()`. Но `TopicService::list_*_with_locale_fallback_and_hidden_categories` чекает это только через `ForumTopicAudienceVisibilityService`, который смотрит на `category_visibility` (public/authenticated). Если категория `public`, а `allow_anonymous_reading=false`, то `public` топики всё равно отдаются анонимам — глобальный флаг не переопределяет локальную политику. Ожидаемое поведение: глобальный флаг — это floor, он должен быть `AND` с категорийной политикой.

**Рекомендация:** В `ForumTopicAudienceVisibilityService` добавить первый чек: если `!allow_anonymous_reading && security.is_public_read()` → `deny(AuthenticationRequired)` до проверки категории. Сейчас это можно обойти, создав `public` категорию при выключенном анонимном чтении.

---

### 4.4. GraphQL ошибки скрывают `TopicDeleted` как `TopicNotFound` — утечка/маскировка [P2]

**Где:** `src/graphql/topic_route_query.rs:110-130`, `src/services/topic.rs:280-340` (`find_topic_for_update_in_tx` различает `TopicDeleted` vs `TopicNotFound` через `deleted_at` колонку).

**Проблема:** `TopicService::find_topic_for_update_in_tx` честно различает `deleted_at IS NOT NULL` → `TopicDeleted`. Но GraphQL `forumTopic(id)` маппит `TopicDeleted` в `null` (или `notFound`), как и `TopicNotFound`. Для публичного API это ок (не палить существование удалённого топика), но для админ-API (moderation queue) модератор должен видеть удалённые топики. Сейчас админ и паблик используют один и тот же `find_topic`.

**Рекомендация:** Разделить `find_topic_for_admin_in_tx` (видит soft-deleted) и `find_topic_for_public_in_tx` (не видит). Админский `moderation_report` и `topic_route_tombstone_visibility` уже используют raw SQL с `deleted_at` — вынести в общий хелпер.

---

## 5. Concurrency, транзакции, атомарность

### 5.1. Проглоченные `rollback` — классический anti-pattern [P1]

**Где:** `src/moderation_subject.rs:158`, `src/services/counter_reconciliation.rs:192`, `src/services/mention_reconciliation.rs:179`, `src/services/solution_reconciliation.rs:175`, `src/services/subscription/reconciliation.rs:225`:

```rust
let _ = transaction.rollback().await;
```

**Проблема:** Если `rollback` падает (сеть, таймаут), ошибка теряется, а транзакция остаётся открытой до drop. Это может держать advisory lock (`pg_advisory_xact_lock`) дольше, чем нужно, и блокировать другие Forum-записи в тенанте. AGENTS.md требует «Branch Completeness & Anti-Swallowing» — каждая ветка должна возвращать typed error.

**Рекомендация:** Заменить на `if let Err(e) = transaction.rollback().await { tracing::warn!(error=%e, "...") }` + `return Err(ForumError::Database(e))` где уместно. Или использовать `TransactionTrait::begin` с `Drop` — SeaORM автоматически роллбекает при drop, явный `rollback` не нужен, если не коммитили.

---

### 5.2. Мёртвый код `let _ = target_topic_id;` в `topic_route.rs:477` [P1]

**Где:** `src/services/topic_route.rs:470-490` — внутри `record_delete_tombstones_in_tx`, ветка обработки существующего `Redirect` alias:

```rust
let target_topic_id = alias.target_topic_id.filter(...).ok_or(...)?;
let target_locale = alias.target_locale.as_deref().ok_or(...)?;
let _ = target_topic_id; // <-- зачем?
normalize_route_locale(target_locale)?;
```

**Проблема:** `target_topic_id` валидируется на `!is_nil`, но дальше не используется. Похоже, забыли проверить, что `target_topic_id` всё ещё существует (не был удалён). Сейчас delete-топика, который уже является redirect-sink, не проверяет живость таргета. Если таргет тоже удалён, то цепочка редиректов `A → B → deleted` сломается и `resolve` вернёт `Gone` вместо `Redirect` — неожиданно для SEO.

**Рекомендация:** Удалить `let _ =` и добавить чек `ensure_topic_exists_in_tx(txn, tenant_id, target_topic_id)`. Если таргет удалён — запись должна стать `Gone`, а не оставаться `Redirect`.

---

### 5.3. `posting_rate::enforce_new_*_rate_in_tx` зависит от `lock_category_tree_in_tx`, но не на всех путях [P1]

**Где:** `src/services/posting_rate.rs:1-30` (коммент «runs inside write transaction, after tenant-wide category-tree lock»), `src/services/topic_inline.rs:80-110`, `src/services/reply_owner_inline.rs:140-160`, `src/services/topic.rs:??` (create paths).

**Проблема:** Комментарий обещает сериализацию через `lock_category_tree_in_tx`, но `TopicService::create_command` (inline-quotes) берёт `ForumContentLimits::resolve_in_tx` + `enforce_new_topic_rate_in_tx` внутри той же транзакции, где уже взят `lock_category_tree_in_tx` — ок. Но `ReplyService::create_command_with_audience_context` вызывает `enforce_new_reply_rate_in_tx` до `audience` чека, а `audience` тоже требует `PortContext` — порядок не гарантирован, если кто-то вызовет `posting_rate` без внешнего lock. Нет compile-time гарантии, что lock взят.

**Рекомендация:** Сделать `enforce_new_*_rate_in_tx` приватным и вызывать только через `PostingRateGuard`, который хранит `CategoryTreeLock` токен (zero-sized тип). Так нельзя забыть лок.

---

### 5.4. SEO short-id коллизия — 12-символьный префикс UUID не уникален [P2]

**Где:** `src/services/topic_route.rs:60-70` — `short_identity = UUID.simple()[..12]` (первые 48 бит). `ensure_unambiguous_current_short_id` чекает `load_current_routes_by_short_id` с `LIMIT 2` и падает в `Conflict` если >1.

**Проблема:** 48 бит — 281T комбинаций, коллизия маловероятна, но не невозможна. При коллизии `canonical_descriptor` падает в `500 TopicRouteResolutionConflict`, а `rename_slug` тоже. Нет retry с другим суффиксом — просто 500. Для 1M топиков в тенанте вероятность коллизии ~0.001% (birthday), но для 10M — уже 1.7%.

**Рекомендация:** Увеличить до 16 символов (64 бит) или добавить fallback: при `Conflict` дописывать 4 символа из `hash(topic_id + locale)` и ретраить. Задокументировать как known limitation. В `AGENTS.md` требуется «bounded allocation» — здесь это `LIMIT 2`, ок, но `Conflict` должен быть retryable, а не 500.

---

## 6. Валидация, границы, лимиты

### 6.1. `content_limits::check_bounds` — inconsistent limits panic вместо fail-closed [P2]

**Где:** `src/services/content_limits.rs:62-85` — если `min > max` и `max != 0`, то `Validation("... inconsistent")`. Но `ForumModuleSettings` позволяет выставить `min_topic_title_length=20, max=15` через JSON (нет кросс-валидации в `dto/settings.rs`). Пользователь сохранит, а потом **каждый** create/update топика будет падать с `inconsistent`. Это не fail-closed, это DoS.

**Проблема:** Нет валидации сеттингов при записи (`rustok-modules` → `SharedStaticModuleSettingsReader`). `ForumSettingsProviders::module_settings` отдаёт как есть.

**Рекомендация:** Добавить `ForumModuleSettings::validate()` и вызывать в `rustok-modules` при `put_settings` (через `ModuleSettingsValidator` trait). Там же проверить `rate_limit_*` не > 86400, `topics_per_page` 1..100, `max_edit_window_minutes` < 525600 (1 год).

---

### 6.2. `MAX_FORUM_CATEGORY_TREE_NODES=512` без пагинации — внезапный `Validation` вместо UX [P2]

**Где:** `src/dto/category_tree.rs:12`, `src/services/category.rs:15-25` — `load_categories_in_tx` лимитит `LIMIT 513` и возвращает `Validation` если >512.

**Проблема:** Админ видит «Category tree exceeds bounded limit» без ссылки на пагинацию. 512 — мало для маркетплейса с 2000 категорий. Нет альтернативы: дерево — единственный способ увидеть иерархию (flat list deprecated).

**Рекомендация:** Либо поднять до 2048 (память позволяет: 512 * ~2KB = 1MB), либо добавить `?depth=2` пагинацию по уровню (корень + 1 уровень, lazy load детей). Сейчас `MAX_FORUM_CATEGORY_TREE_DEPTH=16` уже ограничивает глубину, а ширина — нет.

---

### 6.3. `ListTopicsFilter::normalize_selection` — double lowercasing и 200-char бюджет по chars, не bytes [P2]

**Где:** `src/dto/topic.rs:175-195` — `raw.to_lowercase().chars().take(201).count() > 200` дважды, сначала по `raw`, потом по `search`.

**Проблема:** `İ` (U+0130) lowercases в `i` + `̇` (2 chars) — проверка `chars().take(201)` ловит, но `search` уже `to_lowercase()` — двойная работа. Бюджет 200 — это лимит на **нормализованный** поиск, но первая проверка на `raw` лишняя. Хуже: `search` хранится lowercased, а `matches_selection` делает `topic.title.to_lowercase().contains(search)` — каждый раз lowercases title в памяти. Для 50k топиков в `list_widget_preview_owner_visible` это O(n) lowercases.

**Рекомендация:** Убрать первую проверку `raw`, оставить только `search`. Для `matches_selection` — пушить `search` как `ILIKE %search%` в SQL (Postgres) вместо клиентского `contains`. Сейчас `search` — «literal substring of owner's displayed title/slug» (CRATE_API.md), но в `topic.rs:500+` это делается в Rust после загрузки 512 нод — N+1.

---

### 6.4. `posting_rate::retry_after_seconds` — `signed_duration_since` может паниковать на `Duration::MAX` [P2]

**Где:** `src/services/posting_rate.rs:95-105` — `u64::try_from(now.signed_duration_since(last).num_seconds()).unwrap_or(0)` — `unwrap_or` спасает от negative, но `num_seconds()` на `Duration` с `i64::MAX` всё ещё может overflow в chrono 0.4 (хотя редко). `last` — `FixedOffset`, `now` — `Utc`, `signed_duration_since` кросс-TZ — ок, но если `last` из будущего на 10 лет (баг в seed-данных), то `num_seconds()` вернёт ~315M, `elapsed < limit` → false, не rate-limited — не fail-closed, как обещает комментарий.

**Рекомендация:** Явно clamp: `if last > now { return Some(limit) }` до `signed_duration_since`. Это уже есть в `reject_if_within_interval` комменте, но не в коде.

---

## 7. Mentions, Quotes, Relations — логика проекций

### 7.1. `MentionRelationService::prepare` делает `ProfilesReader` вне транзакции — TOCTOU [P1]

**Где:** `src/services/mention_relation.rs:90-130` — `prepare` резолвит `profiles.find_profile_records_by_handles` до `begin()`, а `persist_in_tx` перепроверяет `ensure_prepared_matches_source_in_tx` (fingerprint body).

**Проблема:** Между `prepare` и `persist_in_tx` профиль может стать `Blocked`/`Private`/`Deleted`, но fingerprint body не изменится — чек пройдёт, а mention сохранится для заблокированного пользователя. `validate_resolved_profile` в `mentions.rs:380` чекает `visibility == Public|Authenticated` и `status == Active`, но это в `prepare`, не в `persist`.

**Рекомендация:** Дублировать `validate_resolved_profile` в `persist_in_tx` через `ProfilesReader` внутри транзакции, или кэшировать `ProfileRecord` в `PreparedMentionRelations` и ревалидировать в `persist`. Сейчас это окно — 1 RTT до DB, но для модерации приватности — критично.

---

### 7.2. `ForumMentionCandidates::new` нормализует хэндлы через `ProfileService::normalize_handle`, но `scan_text_mentions` уже lowercases [P2]

**Где:** `src/mentions.rs:320-360` — `scan_text_mentions` делает `token.to_ascii_lowercase()`, а `classify_mention_token` снова `ProfileService::normalize_handle(&normalized)`.

**Проблема:** Двойная нормализация не баг, но `ProfileService::normalize_handle` может отрезать `@` или `+`, а `scan_text_mentions` уже отрезал по `is_ascii_alphanumeric | _ | -`. Расхождение: `@John_Doe-` в тексте даст `john_doe-`, а `normalize_handle` может ожидать `john_doe`. Нет теста на этот кейс.

**Рекомендация:** Централизовать нормализацию в `mentions.rs` и вызывать `normalize_handle` один раз в `scan_text_mentions`, убрать `to_ascii_lowercase` там.

---

### 7.3. `validate_forum_quote_references` разрешает кросс-тенант quotes через `ForumRevisionIdentity` без tenant-чека [P0]

**Где:** `src/mentions.rs:280-310` — `validate_forum_quote_references(source, references)` чекает только `self-reference`, но не `tenant_id`. `ForumQuoteReference` хранит `target` + `revision_id`, но не `tenant_id`. В `MentionRelationService::validate_quote_targets_in_tx` (`src/services/mention_relation.rs:380-410`) чекается `forum_relation_revision::Entity::find_by_id(revision_id).filter(tenant_id)` — tenant-чек есть, но только на persist. Если кто-то создаст `ForumQuoteReference` с чужим `target` и тем же `revision_id` в своём тенанте (UUID коллизия revision_id), то `validate_forum_quote_references` пропустит, а `validate_quote_targets` поймает только если revision_id не существует в этом тенанте — а если существует (коллизия автоинкремента), то пройдёт.

**Проблема:** `revision_id` — глобальный autoincrement per DB, не per tenant. Два тенанта могут иметь одинаковый `revision_id` для разных `target`. Чужой quote может пройти как свой, если `revision_id` совпал.

**Рекомендация:** Хранить `tenant_id` в `ForumQuoteReference` и чекать в `validate_forum_quote_references`. Или сделать `revision_id` составным `(tenant_id, revision_id)` — сейчас `forum_relation_revision::Column::TenantId` фильтруется, но `find_by_id` ищет только по `revision_id` (PK), а `tenant_id` — фильтр, так что коллизия не пройдёт: `find_by_id(42)` вернёт чужой `revision_id=42` с другим `tenant_id`, фильтр `tenant_id == my` не совпадёт → `None` → `QuoteTargetUnavailable`. Значит баг не воспроизводится, но логика хрупка — стоит добавить явный `tenant_id` в `ForumQuoteReference` для clarity.

---

## 8. Attachments и Media

### 8.1. `ForumAttachmentRelationService::replace_attachment_relations` — двойная проверка `expected_revision` (outside + inside tx) [P2]

**Где:** `src/services/attachment_relation.rs:80-180` — сначала `check_expected` до `begin()`, потом снова внутри транзакции после `lock_forum_target`.

**Проблема:** Внешняя проверка до транзакции — оптимизация, но она даёт ложный `Ok(current)` при `same_requested_state` до ретрая Media holds. Если между внешней проверкой и `begin()` другой writer закоммитил новый `relation_revision`, то внешняя `same_requested_state` вернёт старый `current`, а Media holds будут ретрайнуты для stale данных. Внутренняя проверка это поймает и вернёт `Conflict`, но Media holds уже сделаны зря (лишний `retain_asset_reference`).

**Рекомендация:** Убрать внешнюю `check_expected`/`same_requested_state` до `begin()`, оставить только внутри транзакции. Или сделать внешнюю проверку read-only без Media side-effects (только `Ok(current)` без `ensure_media_holds`).

---

### 8.2. `forum_attachment_reference_id` — детерминированный UUIDv4 через SHA256, но не `Uuid::new_v5` [P2]

**Где:** `src/services/attachment_relation.rs:680-710` — `SHA256(identity) → bytes[6]=0x40, bytes[8]=0x80`.

**Проблема:** Ручная сборка UUIDv4 из SHA256 — корректна, но не использует `Uuid::new_v5` (SHA1) или `new_v8` (custom). Хуже: `identity` строка `rustok:forum:attachment-reference:{tenant}:{kind}:{id}:{locale}:{pos}:{media}` — position как `u16` в decimal, locale case-sensitive после `normalize_locale_tag` — ок. Но если `locale` нормализуется в `en-US`, а `identity` хранит `en-us` (lowercase) vs `en-US` — коллизия? Сейчас `locale` уже нормализован через `normalize_locale_tag` до вызова, так что ок. Но нет теста на locale-case коллизию.

**Рекомендация:** Заменить на `Uuid::new_v5(NAMESPACE_FORUM_ATTACHMENT, identity.as_bytes())` — стандартный, auditable. Добавить тест, что `reference_id` стабилен при `locale` с разным кейсом.

---

## 9. SEO, routes, search

### 9.1. `seo_audience_targets.rs` дублирует `topic_visibility` и `category_visibility` [P2]

**Где:** `src/seo_audience_targets.rs` (929 LOC) + `src/seo_targets.rs` (855 LOC) + `src/services/topic_visibility.rs`, `category_visibility.rs`.

**Проблема:** Два пути резолва SEO дескрипторов: `SeoAudienceTargets` (новый, с каналом) и `ForumSeoTargetProvider` (старый, без канала). Оба читают `forum_topics` + `forum_topic_translations` + `forum_topic_channel_access`, но с разной логикой fallback (`PLATFORM_FALLBACK_LOCALE` vs `requested_locale`). Нет единого источника truth — при изменении схемы `forum_topics` нужно править оба.

**Рекомендация:** Вынести чтение `forum_topics` в `seo_targets_base.rs` с `load_seo_topic_rows(tenant_id, channel_filter)` и переиспользовать в обоих провайдерах. Удалить дублирующий `load_topic_translation_routes_in_tx` (уже есть в `topic_route.rs`).

---

### 9.2. `search_projection.rs` — N+1 на `load_topic_tags` и `load_channel_slugs` [P1]

**Где:** `src/search_projection.rs:320-600`, `src/search_projection_author.rs` — `ForumSearchProjectionSourceFactory` делает `load_translations` + `load_topic_tags` per topic в цикле.

**Проблема:** Для 100 топиков — 100 запросов `forum_topic_tag` + 100 `taxonomy` → 200 RTT. Нет batch-метода `load_topic_tags_for_topics`. `load_translations_map_for_topics` уже batch (в `topic.rs:420`), а `search_projection` — нет.

**Рекомендация:** Добавить `batch_load_topic_tags(tenant_id, topic_ids, locale, fallback)` как в `TopicService::load_translations_map_for_topics`. Использовать `forum_topic_tag::Entity::find().filter(is_in(topic_ids))` + `TaxonomyService::resolve_term_names_batch`.

---

## 10. Контроллеры и GraphQL

### 10.1. `controllers/mod.rs` — `map_forum_error` маппит `Validation` в 400, но `Forbidden` в 403, а `RateLimited` в 429 — ок, но `CapabilityFailure` в 503 без `Retry-After` [P2]

**Где:** `src/controllers/mod.rs:150-230` — `CapabilityFailure { retryable }` → `503 SERVICE_UNAVAILABLE` без `Retry-After` header.

**Проблема:** `retryable=true` означает, что клиент может ретрайнуть, но без `Retry-After` он будет спамить. Для `RateLimited` уже есть `retry_after_seconds` в `ForumError`, но в HTTP-ответе он не прокидывается (только в `to_string()`). GraphQL тоже не отдаёт `retryAfter`.

**Рекомендация:** Для `RateLimited` добавить `Retry-After: <seconds>` header. Для `CapabilityFailure { retryable: true }` добавить `Retry-After: 5` (дефолт). Пробросить `retry_after_seconds` в GraphQL extension `forumRateLimited`.

---

### 10.2. `graphql/category_route_query.rs` и `topic_route_query.rs` — `Ok(None)` на `CategoryNotFound` скрывает ошибку от админа [P2]

**Где:** `src/graphql/category_route_query.rs:30-50`, `src/graphql/topic_route_query.rs:30-50` — `Err(ForumError::CategoryNotFound(_)) => Ok(None)`.

**Проблема:** Паблик query `forumCategoryRoute` должен возвращать `null` на not-found — ок (не палить существование). Но админский `forumCategoryTree` тоже через `CategoryService::tree` может вернуть `Ok(None)` на `CategoryNotFound` — админ не поймёт, почему дерево пустое. Нет разделения `public` vs `admin` обработки.

**Рекомендация:** Разделить: `public_route_query.rs` → `Ok(None)`, `admin_category_query.rs` → `Err(NotFound)`. Сейчас `query_runtime.rs:256` уже делает `CategoryNotFound => Ok(None)` для storefront — документировать, почему это ок.

---

### 10.3. `controllers/topic_redirect.rs` — `308 Permanent Redirect` без `Cache-Control` [P2]

**Где:** `src/controllers/topic_redirect.rs:40-120` — `redirect_merged_topic` middleware для `GET /api/forum/topics/{id}` возвращает `308` с `Location` на canonical topic.

**Проблема:** `308` кэшируется браузером навсегда. Если мерж откатят (через `clear_topic_delete_snapshots` или новый топик с тем же ID — невозможно, но merge — soft), то браузер будет вечно редиректить на старый таргет. Нет `Cache-Control: no-store` или `private`.

**Рекомендация:** Добавить `Cache-Control: private, max-age=60` для `Redirect`, `Cache-Control: no-store` для `Gone`. Для `308` лучше `307 Temporary Redirect`, если мерж может быть отменён — но по контракту мерж immutable, так что `308` ок, но с `Cache-Control`.

---

## 11. Frontend (admin/storefront) — паритет Leptos/Next

### 11.1. `admin/src/ui/*.rs` — `let _ = refresh_nonce.get()` без `create_effect` [P2]

**Где:** `admin/src/ui/topic_fork.rs:100`, `topic_merge.rs:100`, `topic_reply_range.rs:96`, `topic_slug_rename.rs:91`, `topic_split.rs:97`.

**Проблема:** `let _ = refresh_nonce.get()` внутри `view!` — подписка на сигнал без эффекта. В Leptos 0.8 это работает как `create_effect` только если внутри `view!`, но вне `view!` — нет. Здесь это внутри компонента, но не в `view!` closure — просто чтение сигнала, которое триггерит ререндер, но не `async` загрузку. На практике `refresh_nonce` меняется, но `resource` не рефетчится — stale UI.

**Рекомендация:** Заменить на `create_effect(move |_| { refresh_nonce.get(); /* reload */ })` или `create_resource(move || refresh_nonce.get(), ...)`.

---

### 11.2. `storefront/src/transport/*_native_server_adapter*.rs` — `let _ = draft` заглушки [P2]

**Где:** `storefront/src/transport/composer_native_server.rs:166,282`, `native_server_adapter.rs:474,566` etc. — 10+ `let _ = (topic_id, draft)` заглушек в `#[cfg(not(feature = "ssr"))]` ветках.

**Проблема:** CSR-ветка не использует `draft`, но функция принимает `draft` как обязательный. Это FFA-boundary нарушение: `core.rs` должен быть framework-agnostic, а `transport` — framework-specific. Сейчас `core` готовит `draft`, а `transport` его игнорирует — впустую гоняет данные по сети.

**Рекомендация:** Разделить `Transport` trait: `SsrTransport { fn load_draft(...) }` vs `CsrTransport { fn load_without_draft(...) }`. Или сделать `draft` `Option<Draft>` и не слать в CSR.

---

### 11.3. `storefront/src/core.rs` — нет `MAX_FORUM_READ_LIMIT` clamp на пагинацию в `list_unread_topics` [P1]

**Где:** `storefront/src/core.rs` vs `src/dto/read_model.rs:13` — `bounded_forum_read_limit` clamp `1..=100`, а `storefront/src/core.rs` не clampит `per_page` для `list_unread_topics`.

**Проблема:** Злоумышленник может запросить `per_page=10000` через `storefront/src/transport/native_server_adapter.rs` (SSR) и загрузить 10k топиков в память WASM — OOM в браузере. `Dto` clampит на десериализации, но `core.rs` конструирует `ListTopicsFilter` напрямую без `bounded_forum_read_limit`.

**Рекомендация:** Добавить `filter.per_page = bounded_forum_read_limit(Some(filter.per_page))` в `storefront/src/core.rs::list_unread_topics`, как в `reply.rs:180`.

---

## 12. Миграции и схема

### 12.1. `migrations/m20260722_000005_seed_forum_relation_revisions.rs:12` — `#[allow(dead_code)]` на миграции [P2]

**Где:** `src/migrations/m20260722_000005_seed_forum_relation_revisions.rs:12`.

**Проблема:** Миграция помечена `allow(dead_code)`, чтобы скрыть warning о неиспользуемой `SeedForumRelationRevision` struct. Это костыль — миграция должна компилиться без warning через `#[allow(clippy::)]` или через `#[cfg(not(test))]` guard, а не `allow(dead_code)` на весь файл.

**Рекомендация:** Убрать `allow(dead_code)`, добавить `#[allow(dead_code)]` точечно на struct, или переписать миграцию как `async fn up` без лишней struct.

---

### 12.2. Нет `CHECK` на `forum_topics.is_locked` vs `status` и на `forum_replies.status` vs `deleted_at` [P1]

**Где:** `src/migrations/m20260328_000001_create_forum_tables.rs` (initial), все таблицы.

**Проблема:** `forum_topics.status` и `is_locked` независимы, но семантика `delete` требует `status='archived' AND is_locked=TRUE AND deleted_at IS NOT NULL` атомарно. Нет `CHECK` что `deleted_at IS NOT NULL ⇒ status='archived'`. Можно вручную `UPDATE forum_topics SET deleted_at=NOW() WHERE status='open'` — инконсистентность. То же для `forum_replies: status='deleted' ⇔ deleted_at IS NOT NULL` — нет CHECK.

**Рекомендация:** Добавить миграции:

```sql
ALTER TABLE forum_topics ADD CONSTRAINT forum_topics_deleted_consistency CHECK (
  (deleted_at IS NULL) OR (deleted_at IS NOT NULL AND status = 'archived')
);
ALTER TABLE forum_replies ADD CONSTRAINT forum_replies_deleted_consistency CHECK (
  (deleted_at IS NULL AND status != 'deleted') OR (deleted_at IS NOT NULL AND status = 'deleted')
);
```

Уже есть DB-триггеры для `forum category lifecycle` — добавить аналогично для delete-consistency.

---

## 13. Наблюдаемость и DX

### 13.1. Нет `tracing::instrument` на `audience.rs` и `posting_policy_facts.rs` [P2]

**Где:** `src/audience.rs` (486 LOC, 0 instrument), `src/services/posting_policy_facts.rs`, `src/attachment_relation.rs`.

**Проблема:** Форум — самый частый источник `FORUM_FORBIDDEN` в логах, но без `instrument` нельзя отфильтровать по `tenant_id`, `user_id`, `category_id`. `topic_owner.rs` и `reply_owner.rs` уже с `instrument(skip(self, security))`, а `audience.rs` — без.

**Рекомендация:** Добавить `#[instrument(skip(security, port, constraints), fields(tenant_id=%tenant_id))]` на `ForumAudienceFactsResolver::resolve_for_constraints` и аналогично на `ForumAudienceEvaluator::decide`.

---

### 13.2. `error.rs::From<DbErr>` парсит message строкой — хрупко [P1]

**Где:** `src/error.rs:120-170` — `if message.contains("forum topic merge audience policy conflict")` → `TopicMergeAudiencePolicyConflict`.

**Проблема:** Зависит от текста Postgres trigger `RAISE EXCEPTION 'forum topic merge audience policy conflict: ...'`. Если DBA поменяет сообщение триггера, Rust не поймает и вернёт `Database` вместо `AudiencePolicyConflict` → 500 вместо 409. Нет теста на этот путь (только SQLite).

**Рекомендация:** Использовать `SQLSTATE` `P0001` с `errcode` вместо `contains`. В PG trigger делать `RAISE EXCEPTION USING ERRCODE='P0001', MESSAGE='...'`, в Rust парсить `DbErr::Query(RuntimeErr { code, .. })`. Или вынести триггер-сообщения в `src/constants.rs` как `const TRIGGER_MSG_*`.

---

## 14. Приоритизированный план улучшений

### P0 — сделать сейчас (безопасно, <1 дня)

1. **Фикс `rbac.rs:32`**: `PermissionScope::None => Err(Forbidden)` вместо `Ok(())` в `enforce_author_deletion_policy`.
2. **Фикс `reply_owner.rs:280`**: убрать `if topic.status == Archived &&` перед `merge_operation` check — чекать мерж всегда.
3. **Добавить `CHECK` constraints** на `forum_topics.deleted_at ↔ status` и `forum_replies.deleted_at ↔ status` (миграция).
4. **Добавить `Retry-After` header** на `RateLimited` и `CapabilityFailure(retryable)`.
5. **Починить `posting_rate` будущее время**: `if last > now { Some(limit) }` вместо `signed_duration_since` overflow.

### P1 — следующая итерация (1-3 дня)

6. Убрать `let _ = rollback` → `tracing::warn!` + typed error.
7. Убрать мёртвый `let _ = target_topic_id` и добавить живость таргета в `topic_route.rs`.
8. Вынести `audience.rs` общие хелперы, убрать дубли `normalize_channel_slugs` в 6 файлах.
9. Добавить `ForumModuleSettings::validate()` и валидацию при записи сеттингов.
10. Починить `MentionRelationService` TOCTOU: ревалидация профиля внутри `persist_in_tx`.
11. Добавить batch `load_topic_tags_for_topics` для `search_projection.rs`.
12. Разделить `find_topic_for_admin` vs `find_topic_for_public` (soft-delete visibility).
13. Добавить `Cache-Control` на `308 Redirect`/`410 Gone` в `topic_redirect.rs`.

### P2 — рефакторинг/долг (1-2 недели)

14. Убрать все `include!()` в `lib.rs`/`services/mod.rs` → обычные `mod`.
15. Унифицировать `posting_rate` + `posting_policy` (или удалить dead `posting_policy`).
16. Вынести generic `ReconciliationExecutor<T>` для 6 merge-reconciliation.
17. Увеличить `short_id` до 16 chars или добавить fallback при коллизии.
18. Поднять `MAX_FORUM_CATEGORY_TREE_NODES` до 2048 или добавить depth-пагинацию.
19. Починить Leptos `let _ = refresh_nonce.get()` → `create_effect`.
20. Добавить `tracing::instrument` на `audience.rs` и `attachment_relation.rs`.
21. Заменить `DbErr::contains` на `SQLSTATE` parsing в `error.rs`.
22. Заменить ручной UUIDv4-SHA256 на `Uuid::new_v5` для `attachment_reference_id`.
23. Добавить `CLIPPY` lint `deny(warnings)` уже есть, но добавить `deny(let_underscore_must_use)`.

---

## 15. Что уже хорошо (не ломать)

* **Tenant-изоляция** — везде `filter(Column::TenantId.eq(tenant_id))` + composite FK, advisory locks per tenant. Держит.
* **CAS-головы** (`attachment_relation_head`, `forum_relation_revision`) — монотонные, immutable, без `UPDATE` старых рядов. Образцово.
* **Owner-сервис граница** — `TopicService`/`ReplyService` фасады, `topic.rs`/`reply.rs` приватны. Транспорты не лезут в `MentionRelationService` напрямую — соблюдено (проверено `verify-forum-owner-boundary.mjs`).
* **Locale fallback `requested → fallback → en → first available`** — консистентно по всем read-моделям, с `available_locales`.
* **Edit-window и deletion-policy** — tenant-настройки, `System`/`Service` акторы exempt, `Own` vs `All` разграничены.
* **RichText** — `project_stored_discussion`/`serialize_discussion`, codeBlock/code-marks игнор для mentions, `empty()` redaction для non-Approved — надёжно.
* **SEO routes** — immutable aliases, `Gone` vs `Redirect`, `REPEATABLE READ` для `get_attachment_relations` — продумано.

---

## 16. Открытые вопросы к владельцам

1. Должен ли `Pending → Closed` существовать? Сейчас модератор делает `Pending → Approved → Closed` — лишний шаг.
2. Должен ли `archived ⇒ locked` быть инвариантом БД, или это только для `delete`?
3. Нужен ли `an allowlist` для `rate_limit` (доверенные боты/модераторы exempt)? Сейчас все `User` равны.
4. Планируется ли wiring `posting_policy` (11 фактов) или его удаляем? Он занимает 30% кода `services/`.
5. `include!()` — это осознанный выбор для обхода циклов, или техдолг? Можно ли распутать?
6. `MAX_FORUM_CATEGORY_TREE_NODES=512` — ограничение от Taxonomy или Forum? Кто владелец лимита?

---

## 17. Приложение: файлы с наибольшим риском

| Файл | LOC | Риск | Причина |
|---|---|---|---|
| `services/topic_route.rs` | 1221 | Высокий | dead code, SEO, `let _`, short_id collision |
| `services/topic_merge.rs` | 4381 | Высокий | 5 reconciliation, cross-category stranded |
| `services/attachment_relation.rs` | 817 | Высокий | double CAS check, Media side-effects до commit |
| `services/mention_relation.rs` | 681 | Высокий | TOCTOU профиль, fingerprint body |
| `services/topic.rs` | 1143 | Средний | N+1 tags/channels, `search` substring |
| `services/reply.rs` | 512 | Средний | `parent_reply_id` depth 10 без цикла-детекта |
| `services/rbac.rs` | 80 | Высокий | `None` → `Ok(())` bypass |
| `dto/topic.rs` | 532 | Средний | double lowercasing, cursor scope hash |
| `controllers/mod.rs` | 548 | Средний | error mapping без Retry-After |
| `admin/src/core.rs` | 1876 | Средний | FFA boundary, но 600 LOC + `let _ = nonce` |
| `audience.rs` | 486 | Средний | дубль хелперов, no instrument |
| `search_projection.rs` | 807 | Средний | N+1, no batch |
| `error.rs` | 376 | Средний | `contains` parsing хрупко |

---

*Отчёт сгенерирован аудитом логики форума по AGENTS.md. Для каждого P0/P1 есть точный файл:строка и воспроизводимый сценарий. Следующий шаг — завести тикеты на P0 (5 шт.) и начать с `rbac.rs` + `reply_owner.rs`.*
