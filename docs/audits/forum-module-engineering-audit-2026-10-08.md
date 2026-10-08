# Глубокий инженерный аудит Forum

**Дата:** 2026-10-08  
**Ветка / исходный снимок:** `arena/2bd6abd8-rustok`, `82743794c64cc83dca7ddadc545cc81bb37654cd`  
**Объём:** `crates/modules/rustok-forum/`, его `admin/` и `storefront/`, Next.js-пакеты Forum во frontend/admin, Forum-интеграции `apps/server/`, связанные Search, Notifications, Reactions, Moderation, Taxonomy, Profiles и Media контракты, планы и проверочные скрипты.  
**Метод:** статический разбор кода и контрактов на указанном HEAD. Это аудит исходного кода, а не production-проверка работающего сайта.

---

## 1. Краткий итог

`rustok-forum` — не маленький CRUD-форум. Серверное ядро содержит зрелую предметную модель: tenant-scoped категории и контент, локализованные записи, статусы и ревизии, tombstone/soft-delete-пути, правила видимости, модерацию ответов, принятые решения, голосование, подписки, read-state, mention/quote relations, поисковые проекции, канонические маршруты, импорт/экспорт и административные операции с темами. Явно видны хорошие архитектурные границы: категории принадлежат Taxonomy, файлы — Media, уведомления — Notifications, жалобы/кейсы — Moderation, а Forum публикует адаптеры и факты вместо копирования чужих хранилищ.

**Основной риск — не отсутствие серверных сущностей, а разрыв между ними и реально доступными пользователю UI/host-путями.** Один публичный путь публикации в Rust/Leptos storefront сейчас выглядит реализованным, но фактически не отправляет данные; Next.js-пермалинк строится в формате, который его route не разрешает; вкладка «Unread» Next не получает нужные поля. Параллельно часть готовых API — подписки, votes, tags, вложения и reporting — не имеет полного пользовательского пути в Forum UI.

### Наиболее важные выводы

1. **P1 — Rust storefront composer является no-op:** кнопка `Create Topic`/`Post Reply` вызывает только локальный callback и сбрасывает состояние; mutation/transport не вызывается.
2. **P1 — Next.js-ссылки на тему не соответствуют route:** карточка ведёт на `/modules/forum/t/{slug}`, но route требует UUID либо `{shortId}/{slug}`. Backend Search/Rust-host использует другой canonical namespace `/forum/t/{shortId}/{slug}`.
3. **P2 — Next.js вкладка Unread структурно не работает:** запрос списка не выбирает unread-поля, отдельный unread query объявлен, но не используется.
4. **P2 — ограниченные страницы выдачи не имеют продолжения:** UI запрашивает максимум 50 тем/категорий и 100 ответов и не предоставляет надёжную пагинацию; переход к посту после первых 100 не находит DOM-элемент.
5. **P2 — watch/reply-notification путь неполон:** Forum provider поддерживает `forum.topic.created` и `forum.mention.user_added`, но не событие нового ответа; существующие topic subscriptions поэтому не дают полного уведомления о ходе темы.
6. **P2 — часть функций существует только как backend-контракт:** attachments, subscriptions, votes/reactions, user report, пользовательское редактирование, выбор tag и durable drafts/bookmarks не собраны в сопоставимый публичный поток.
7. **P2 — антиспам-политика не доведена до write-path:** факты и evaluator есть, но production-вызов из команд создания тем/ответов не найден; план прямо оставляет persistence/enforcement/shared rate limits открытыми.
8. **Проверки сами нуждаются в ремонте:** часть Node-verifiers проходит, часть падает на переименованных файлах, устаревших ticket markers и прежних ожиданиях к тексту документов. Cargo/Rust и frontend build проверить было нельзя.

**Заключение:** доменный backend — мощная и хорошо разделённая основа; публичную Forum-поверхность по текущему снимку нельзя считать функционально завершённой или одинаково надёжной на всех host-вариантах. Перед релизом нужны как минимум фиксы composer и маршрутов, полноценные read/pagination paths и end-to-end доказательства на реально смонтированном frontend.

---

## 2. Границы аудита и достоверность

### Что проверено

- Rust-модуль Forum: сервисы, GraphQL/REST, DTO, модели/миграции, permissions/RBAC, события, Search/SEO/Notifications/Moderation/Reactions/Media/Profiles адаптеры.
- Forum-owned Leptos admin/storefront и host composition в `apps/admin/` и `apps/storefront/`.
- Next.js Forum frontend/admin компоненты, запросы, маршруты и locale-файлы.
- README/API/implementation-plan, связанные Notifications документы и выбранные `scripts/verify/verify-forum-*`.

Кодовая база крупная: около **391 Rust source-файла** под Forum `src/`, **68 файлов миграций**, **152 Rust integration-test-файла** под `tests/` и **189 файлов контрактов**. Эти количества описывают поверхность, но сами по себе не подтверждают, что она компилируется или исполняется.

### Что не удалось проверить

- В окружении отсутствуют `cargo` и `rustc`; не запускались `cargo check`, `cargo test`, `cargo clippy`, миграции SQLite/PostgreSQL и конкурентные runtime-тесты.
- Не установлены `node_modules` в корне/Next-пакетах; не запускались Next typecheck/build/lint, Playwright, браузер и реальный API.
- Не подтверждались работа Search/Notifications workers, настройки capability flags, outbox/Iggy и production migrations.
- Не проверялись фактические разрешения/ответы API на живом tenant, межтенантная изоляция и сценарии нескольких аккаунтов. Все выводы об этих свойствах, если они касаются только отсутствующей UI/вызова, ограничены тем, что найдено в исходном коде.

Поэтому **нельзя трактовать отсутствие ошибок в выбранном источниковом скрипте как успешную сборку**, и нельзя называть Rust/DB/browser-поведение проверенным. Отдельно ниже маркируется «реализовано в backend», «подключено к UI» и «runtime evidence отсутствует».

---

## 3. Архитектура и границы владения

Высокоуровневый путь выглядит так:

```text
Next.js / Leptos UI
  -> Forum GraphQL, REST либо native server-function adapter
  -> Forum services (RBAC, tenant scope, audience/visibility, revisions, events)
  -> Forum persistence + TransactionalEventBus
  -> публичные порты общих модулей
     Taxonomy / Profiles / Media / Search / SEO / Notifications /
     Reactions / Moderation / Outbox / Events
```

### Сильные архитектурные стороны

- **Tenant-scoped данные и permissions.** API проверяет модуль/permission, сервисы повторно проверяют scope. Много команд получают `SecurityContext`, tenant ID разрешается из host context; это правильнее, чем доверять UI или произвольному ID из клиента.
- **Видимость — отдельная политика.** Для категорий/тем/ответов реализованы audience-aware read paths, фильтрация по channel и richer audience facts. Некоторые query comments подчёркивают, что `total` и page рассчитываются по одному разрешённому множеству.
- **Операции над темами рассчитаны на replay и согласованность.** Реализованы move/merge/split/fork, перемещение диапазона ответов, slug rename, архивирование source-темы, reconciliation подписок/read-state/tags/votes/audience и явные receipts/revisions.
- **Shared ownership не дублируется.** Canonical Category — Taxonomy; public identity — Profiles; бинарные активы и удержания — Media; inbox/fanout — Notifications; отчёты/кейсы/решения — Moderation; reaction state — Reactions; Search/SEO — shared owners.
- **Rich-text read path использует общий renderer boundary.** Forum storefront проходит через `RichTextView`/`RichTextHtml`, а не собственный произвольный HTML sink; это хорошее основание для единой санитаризации.
- **Есть reconciliation/observability-контуры.** Реализованы read-only отчёты/проверки для counters, solutions, mentions, subscriptions, attachment holds и Search projection; автоматический write-repair сознательно ограничен отдельными safety gates.

Слабое место не в идее этих границ, а в том, что **часть owner services ещё source-ready или backend-only, а UI и runtime composition не доказывают, что они доступны клиенту**.

---

## 4. Функциональность, обнаруженная в коде

| Область | Что обнаружено | Фактическая зрелость / замечание |
|---|---|---|
| Категории | Canonical Taxonomy ID, локализованные title/slug/description, дерево, parent/child, icon/color, policy/counters, category route resolution и host route. | Backend и Rust storefront path развиты; Next.js категория работает в основном как локальный фильтр. Readme и verification scripts расходятся с текущим route mount. |
| Темы | Create/read/update/delete-пути, locale-specific translations, slug, tags, metadata, channel-slugs, pin/lock/close/archive, статус и revisions. | GraphQL/REST и admin-команды широкие. Next.js create работает через GraphQL; Rust/Leptos public composer не вызывает write mutation (F-01). |
| Ответы | Locale-specific rich content, последовательные позиции, parent reply, statuses включая pending/approved/rejected, revisions, delete/restore/moderation. | Backend полноценнее UI. Next показывает плоский поток, загружает только первую страницу и не даёт public edit/delete path. |
| Rich text | Общий rich-text document/editor/renderer, plain text projection, effective locale и направление контента. | В Next composer подключён `RichTextEditor`; Rust storefront composer использует обычный textarea/простой preview и не пишет в API. |
| Audience/доступ | Tenant, RBAC, category/topic/reply policy, channel/group/member/trust facts и visibility-scoped reads. | Сильная серверная модель. Ниже не заявляется как runtime-tested: toolchain и test DB недоступны. |
| Модерация Forum | Approve/reject reply, hide/delete/restore, pin/lock/close/reopen topic, solution marking, moderator audience composition; отдельные административные UI/API. | Это не означает наличие полноценного user report flow: кнопки «Пожаловаться» в пользовательском Forum UI не найдено (F-10). Cross-domain case owner остаётся Moderation. |
| Админские операции | Категорийный tree/DnD, localized editors; move/merge/split/fork topics; move reply range; rename canonical slug. | Широкий набор, в Rust и Next-admin есть специальные формы/transport adapters. В основном maintainer/moderator scope. |
| Q&A и engagement | Accepted solution; topic/reply votes; solution/user stats; tags; topic/category subscriptions с уровнями. | Backend/GraphQL развит. В публичных UI значительная часть read-only или отсутствует (F-07). Reactions находятся в отдельном модуле и host-composition, не в Forum-owned catalog. |
| Чтение | per-user read cursor/revision, unread projections, mark topic/category/all read и bulk-owner paths. | Backend есть; Next unread tab не запрашивает unread projection (F-03), bulk/pagination/runtime evidence частично открыты. |
| Mentions / quotes | revision-bound quote relations, mention extraction/relations, bounded target snapshots, semantic event `forum.mention.user_added`. | В Next есть quote selection/insert и отправка quote refs; mention suggestions/autocomplete не обнаружены. Mention privacy/runtime/notification proof остаётся открытой частью плана. |
| Notifications | Forum source provider, topic-created и user-mention descriptors, recipient context, authorization текущей цели; общий Notifications inbox/grouped UI/state commands существует отдельно. | Reply-created событие не входит в поддерживаемые типы Forum provider (F-06). Preferences, quiet hours, digests, delivery providers и часть runtime evidence остаются за рамками готового end-to-end. |
| Search / SEO / routes | Отдельные проекции topic/reply по locale, facets/author/category/tag/channel, visibility eligibility, SEO, canonical route и reply target `?reply=...`. | Код backend богатый, но Forum feed search — локальный title/slug фильтр. Search projection/worker/reindex proof остаются в плане; Next/Rust URL contracts не совпадают (F-02/F-05). |
| Media / attachments | Forum attachment relation service, ordered relation rows, CAS revision, Media owner-reference holds и read-only reconciliation. | Не найден Forum GraphQL/REST/storefront/admin consumer для `get_attachment_relations`/`replace_attachment_relations`; наличие сервиса не даёт пользователю upload/attach UI (F-08). |
| Member cards | Profiles reader, batch member cards, profile summary и Forum topic/reply/solution counts. | Есть карточки авторов; полноценный Forum directory/activity profile, follow/block composition и reputation/badges не завершены (F-13). |
| Структурные работы и импорт | NodeBB source mapping, bounded import/export plans, merge receipts, Page Builder widget contracts/preview/property validation. | Часть source-ready; FORUM-34 всё ещё ждёт shared migration-runner integration; Page Builder runtime/Wave evidence остаются открытыми. |
| Локализация | locale fallback, `requested_locale`/`effective_locale`/`available_locales`, Taxonomy localized category data, Rust storefront catalogs en/ru/ar. | Пользовательские строки Forum не локализованы end-to-end, особенно Next frontend; есть English hard-coded labels и incomplete owner-copy i18n (F-12). |

---

## 5. Подробные находки

### F-01 — P1: Rust/Leptos storefront composer не отправляет тему или ответ

**Где:** `crates/modules/rustok-forum/storefront/src/ui/composer.rs:364–375`; компонент монтируется из `storefront/src/ui/leptos.rs:253`.

Кнопка «Create Topic»/«Post Reply» получает текущее состояние `composer`, при наличии callback вызывает его с `topic_id`, затем безусловно делает `reset()`. В этом обработчике нет вызова `transport::...`, GraphQL mutation, native server function, валидации title/body, установки `is_submitting`, error handling или сохранения результата. В `ForumView` composer монтируется без `on_submitted`. Для создания темы `topic_id` вообще отсутствует, поэтому callback не сработал бы даже при наличии. Следствие: пользователь набирает содержимое, нажимает submit и теряет draft, не создавая контент.

Это отличается от Next.js composer: там `handleSubmit` вызывает `createForumReply`/`createForumTopic`, проверяет пустой body/title и очищает только после успешного результата (`apps/next-frontend/packages/rustok-forum/src/components/composer.tsx:130–208`). Значит, функциональность зависит от того, какой storefront фактически смонтирован.

**Рекомендация:** подключить один typed write adapter к Leptos-пути (native SSR/hydrate + GraphQL CSR по принятой транспортной схеме), оставить draft при ошибке, показывать error/busy state, обновлять выбранную тему после успеха. Добавить компонентный/host E2E: создать тему и ответ через смонтированный `apps/storefront`, убедиться, что строка появилась в DB/read model; проверить auth failure, locked topic, пустой body и retry.

### F-02 — P1: Next.js permalink и canonical URL не совпадают с route contract

**Где:**
- `apps/next-frontend/packages/rustok-forum/src/components/topic-feed.tsx:315–324`;
- `apps/next-frontend/src/app/[locale]/modules/forum/t/[...slug]/page.tsx:21–65, 110–147`;
- `crates/modules/rustok-forum/storefront/src/core.rs:159–172`;
- `apps/storefront/src/lib.rs:664–688`.

TopicFeed строит ссылку `/${locale}/modules/forum/t/${topic.slug}` — один сегмент со slug. Next route обрабатывает один сегмент как полный UUID или передаёт его в resolver в качестве `shortId`; slug-only resolver не ищет topic по произвольному slug. В каноническом двухсегментном случае route ждёт `{shortId}/{slug}`. Поэтому обычная ссылка `t/{slug}` из списка не разрешается и уходит в `notFound()`.

Одновременно Rust-host canonical builder возвращает `/{locale}/forum/t/{shortId}/{slug}`, а Search projection материализует этот route; Next app имеет Forum route под `/modules/forum/...`, но соответствующий `/forum/t/...` mount в Next не найден. Это создаёт второй разрыв: URL, выданный backend Search/Rust route owner, не равен Next route namespace. Route также не разбирает `GONE` как особый host response, а сводит отсутствие `topicId` к обычному 404. Reply Search URL содержит `?reply={id}`, но Next page не принимает/передаёт этот anchor к view.

**Рекомендация:** вынести одну route-builder/route-resolution abstraction для обоих host’ов; карточки должны получать canonical `path` от owner, а не конкатенировать `slug`. Зафиксировать один URL namespace либо явную rewrite/redirect compatibility mapping. Добавить тесты feed-link → page → topic, старый slug redirect, deleted/merged/tombstone status, reply deep link и Search result.

### F-03 — P2: вкладка Unread в Next не получает unread-данные

**Где:** `apps/next-frontend/packages/rustok-forum/src/api/forum.ts:134–155, 250–279`; `components/topic-feed.tsx:62–66, 84–88`.

`STOREFRONT_TOPICS_QUERY` запрашивает topic fields, но не `isUnread`, `unreadCount`, `lastReadPosition` или `hasUnreadTopicRevision`. Отдельная `STOREFRONT_UNREAD_TOPICS_QUERY` такие поля запрашивает, однако в пакете не найден fetch-helper/call site, который использовал бы этот query. Feed фильтрует список по `topic.isUnread`/`unreadCount`, поэтому приходящие элементы в обычном query не содержат нужных значений — tab получается пустым, а unread badge/count отсутствует. Backend явно разделяет public topic page и user-specific unread composition (`graphql/storefront_audience_topics.rs:39–44`).

**Рекомендация:** вызывать unread query отдельно для выбранной вкладки (с category/locale/tenant/current actor и pagination), либо добавить viewer-specific fields только в авторизованный read surface. Нужен тест с конкретным read cursor: unread → показывается; mark-read → исчезает; anonymous → безопасный fallback.

### F-04 — P2: пагинация API есть, но UI ограничивается первой страницей

**Где:** `api/forum.ts:220–248, 250–280, 303–330`; `components/forum-section.tsx:61–111, 113–158`; `components/topic-detail-view.tsx:64–94, 196–203`; `components/thread-panel.tsx:40–46, 207–212`.

Next host загружает 50 категорий, 50 тем и 100 ответов по умолчанию. Компоненты Forum делают один вызов и показывают `total`, но «load more», cursor/offset state или следующую страницу в этих UI-путях не обнаружены. В результате категории/темы выше cap не попадают в rail/feed, а ответы после первых 100 не видны. При этом `TimelineScroller` получает `totalPosts = repliesTotal + 1`; прыжок в конец вычисляет DOM ID из общего `postNumber`, но element для ещё не загруженного ответа отсутствует. Ошибка загрузки в `ForumSection` подавляется пустым `catch`, без error/retry surface.

**Рекомендация:** перейти на bounded cursor pagination, отображать `hasNext`, добавить load-more/infinite-scroll с отменой stale запросов, не показывать false-empty при ошибке. Для больших тем реализовать серверный jump-to-position/anchor и загружать окно вокруг reply, а не предполагать, что все ответы уже в DOM.

### F-05 — P2: Forum feed «Search» — локальный фильтр загруженного списка, не поиск по форуму

**Где:** `components/topic-feed.tsx:43–55`; `crates/modules/rustok-forum/src/search_projection.rs:320–387, 390–480`; `apps/next-frontend/src/app/[locale]/search/page.tsx:40–49, 87–104`.

Inline search фильтрует только `title` и `slug` уже загруженных тем; не вызывает `storefrontSearch`, не ищет текст тела, ответы, автора, теги по индексному query и не может найти тему вне первых 50 результатов. В TopicFeed вкладка `Top` также сортирует локальную страницу только по `replyCount`, не используя vote/ranking/временное окно. При этом в backend есть отдельные locale-aware projections для темы и ответа, но program ledger оставляет FORUM-23 в `in_progress` до PostgreSQL/Iggy/cross-module evidence. Общая Next Search страница представлена как «Catalog Search» с product facets, а не как явная Forum search UX; хотя GraphQL response несёт `item.url`, карточка результата рендерит только entity type/title/snippet и не превращает URL в ссылку (`apps/next-frontend/packages/search/src/index.tsx:626–639`). Только document suggestions используют `suggestion.url` для навигации (`:558–573`).

**Рекомендация:** отделить быстрый client-side filter текущей страницы от настоящего Forum Search; добавить `forum_topic`/`forum_reply` preset, category/tag/author/solved filters, result pagination и корректные canonical topic/reply routes. Проверить канал/audience ACL и устаревание индексной проекции до показа результата.

### F-06 — P2: Notifications не подписаны на появление новых ответов

**Где:** `crates/modules/rustok-forum/src/notification_source.rs:35–38, 562–573, 575–633, 635–755, 845–846`; `crates/modules/rustok-notifications/README.md:436–458`.

Forum source provider объявляет только два поддерживаемых типа: `forum.topic.created` и `forum.mention.user_added`. Первый может разрешать аудиторию topic/category subscriptions; второй адресует упомянутого пользователя. Provider не принимает событие `forum.reply.created`/`forum.reply.published` или эквивалентный activity event. Поэтому подписчик темы не получает обычное уведомление о новом ответе только на основании факта ответа; это возможно лишь при явном mention-пути, если он прошёл все остальные gates. В общем Notifications owner есть inbox/state/grouped UI, но в README перечислены незакрытые prefs/quiet hours/digests, provider delivery, channel delivery authorization, PostgreSQL lease/lag evidence и административное восстановление.

**Рекомендация:** определить семантику уровня подписки для topic reply activity, публиковать idempotent transactional source event на разрешённом lifecycle transition, исключать автора/мьютеров, выполнять recipient privacy и current-target reauthorization при fanout/open. Добавить SQLite/PostgreSQL и host runtime tests «reply → inbox candidate → visible notification → authorized deep link».

### F-07 — P2: подписки, votes/reactions и часть authoring доступны в backend, но не в Forum UI

**Где:** `crates/modules/rustok-forum/src/graphql/mutation.rs:517–674, 809…`; `apps/next-frontend/packages/rustok-forum/src/api/forum.ts`; `components/topic-feed.tsx:303–308`; `components/composer.tsx:179–194`; `crates/modules/rustok-forum/docs/implementation-plan.md:307–310, 320`.

Backend имеет category/topic subscription setters, vote mutations и read paths, а Next feed показывает лишь положительный `voteScore`; Next Forum API/UI не содержит соответствующих write-вызовов подписки/vote. Composer на Next может создавать topic, но передаёт `tags: []` и не предоставляет tag editor. В публичном Next topic/reply view не обнаружены собственные edit/delete controls, хотя admin/topic/reply mutation-пути существуют. Forum subscriptions поэтому нельзя считать полноценной пользовательской функцией только потому, что есть service/GraphQL.

Reactions owner — отдельный модуль. По plan, bounded controls и host composition предусмотрены в Rust `apps/storefront` для выбранной темы/ответа, но Next Forum UI их не монтирует; это не равно полноценной реакционной панели на всех доступных поверхностях.

**Рекомендация:** составить end-user capability map отдельно от GraphQL map; для каждой поддерживаемой роли указать watch/mute, vote/reaction, edit/delete, tags и разрешённые границы времени редактирования. Либо показать controls, либо явно пометить функцию как admin/API-only, чтобы score и UI не обещали действие, которого нет.

### F-08 — P2: attachment relations — сервис без обнаруженного API или пользовательского пути

**Где:** `crates/modules/rustok-forum/src/services/attachment_relation.rs:29–70`, migration `m20260924_000036_add_forum_attachment_relations.rs`, exports в `src/lib.rs`.

`ForumAttachmentRelationService` предоставляет `get_attachment_relations` и `replace_attachment_relations`, CAS revision и Media hold path. По глобальному поиску call sites этих методов за пределами definition/export не найдено в Forum GraphQL/REST, admin/storefront UI и host API. Это не означает, что owner service отсутствует; означает, что end-user путь attach/remove/reorder/caption не прослеживается. FORUM-14 в плане прямо оставляет runtime integration/reconciliation evidence открытым.

**Рекомендация:** определить write surface и permission contract, подключить типизированный Media picker/upload owner (не хранить файл в Forum), затем добавить read/write transport, editor controls, remove/reorder tests, durable hold release/reconciliation. До этого не рекламировать attachments как доступную Forum-функцию.

### F-09 — P2: Forum posting-policy facts/evaluator не подключены к командам публикации

**Где:** `crates/modules/rustok-forum/src/services/posting_policy.rs`, `posting_policy_facts.rs`, `posting_policy_evaluator.rs`; `apps/server/src/services/forum_posting_policy_facts.rs`; план `docs/implementation-plan.md:318`.

В коде есть типизированные факты trust/account age/topics read/approved posts/flags/reputation/window counts и evaluator rules; host-level facts adapters тоже имеются. Однако поиск `ForumPostingPolicyEvaluator`/`ForumPostingPolicyEvaluationInput` не показал production call path из `TopicService`/`ReplyService` create/update или GraphQL mutation — использование сосредоточено на типах, composer и tests. План FORUM-26 сам говорит: facts есть, persistence/enforcement/shared rate limits/transports/UI/evidence ещё нужно добавить. Значит, это foundation, а не завершённая защита от спама.

**Рекомендация:** вызывать policy в owner write boundary до записи (не только из транспортного слоя), установить fail-closed/degraded semantics для required facts, подключить распределённый rate limit и метрики, не допустить race/replay обходов. Добавить реальные тесты create-window, burst across processes, trust threshold и недоступного facts provider.

### F-10 — P2: пользовательский report/flag workflow не обнаружен

**Где:** Forum Next/Leptos storefront packages и `apps/next-frontend/packages/rustok-forum/src/components/*`; cross-domain owner — `crates/modules/rustok-moderation/src/commands/report.rs`.

Forum admin умеет модерировать pending/approved/rejected content и имеет локальные pin/lock/etc actions. Отдельно Moderation хранит reports/cases/decisions. Однако в пользовательских topic/reply cards, thread panel, Next Forum API и Leptos storefront не найден report/flag control или `submitModerationReport` integration. Это не отсутствие Moderation owner; это разрыв между пользователем Forum и owner жалоб.

**Рекомендация:** добавить только Forum-specific subject adapter и report transport, передавая typed `forum_topic`/`forum_post` target в Moderation; не создавать вторую локальную очередь. Нужны abuse-rate limits, duplicate report protection, private reporter identity и правила обратной связи/appeal.

### F-11 — P2: Next localStorage drafts не разделены по tenant и actor

**Где:** `apps/next-frontend/packages/rustok-forum/src/context/composer-context.tsx:43–50, 66–77, 83–96`.

Ключи draft формируются как `rustok_forum_draft_reply_{topicId}` либо `rustok_forum_draft_topic_{categoryId|general}`. В ключе нет tenant/account/user identity; локальное содержимое хранится без срока жизни, а при открытии читается тем же browser origin. На общем устройстве/браузере следующий вошедший аккаунт может открыть draft предыдущего пользователя к тому же topic/category. Для post drafts это может раскрыть приватный текст до публикации. Это отличается от серверных durable drafts: FORUM-17 по-прежнему `planned`.

**Рекомендация:** включить стабильный tenant + authenticated principal в ключ и жизненный цикл; при logout/account switch очищать или изолировать старое содержимое; задать TTL/versioning и явное предупреждение для shared devices. Не подменять этим серверные drafts/bookmarks.

### F-12 — P2: Forum chrome не локализован end-to-end

**Где:** `apps/next-frontend/messages/en.ftl`, `ru.ftl`; `packages/rustok-forum/src/components/*`; `crates/modules/rustok-forum/storefront/src/ui/composer.rs`; план FORUM-28, `implementation-plan.md:320`.

В Next translation catalog не обнаружено Forum-specific keys; много текстов («Latest», «Unread», «Create Topic», errors, labels) задано английскими литералами в Forum components. Rust catalogs объявляют en/ru/ar и часть feed/category labels берётся через `t()`, но composer и часть tab/placeholder текста остаются hard-coded English. Route locale и localized content есть, но это не полная локализация UI; Arabic RTL/browser proof отдельно не запускался.

**Рекомендация:** завести owner-owned keys для всех пользовательских строк, покрыть en/ru/ar, pluralization, dates, validation/errors; проверять `dir/lang`, RTL, category/topic fallback и сохранение locale через authoring/browser E2E.

### F-13 — P2: готовые route/category controls отображают не всё дерево и теряют URL identity

**Где:** `apps/next-frontend/packages/rustok-forum/src/api/forum.ts:220–248`; `components/category-overview.tsx:28–64`; Leptos counterpart `storefront/src/ui/category_overview.rs` и callback в `ui/leptos.rs:215–221`.

Категорийный loader имеет default limit 50. Next category overview собирает roots и только прямых subcategories; более глубокие потомки не присоединяются рекурсивно. Leptos overview также группирует только один уровень; его `on_select_category` callback игнорирует переданный ID и лишь переключает `view_mode` на Topics, поэтому кнопка «Browse» не фильтрует список выбранной категорией. Next локальный UI умеет фильтровать при выборе в rail, но не обновляет URL, а canonical category route существует в Rust host. Это создаёт различное поведение между host’ами и неполную навигацию по большим/глубоким деревьям.

**Рекомендация:** рендерить дерево рекурсивно, поддерживать пагинацию/виртуализацию, а category select направлять на canonical path или синхронно менять query state. Тесты: глубина >=3, >50 категорий, приватная category, back/forward и RTL.

### F-14 — P2: ключевые продуктовые capabilities остаются будущим объёмом, хотя база уже намечена

Это не следует считать уже реализованным пользовательским функционалом:

- durable drafts, bookmarks и optional reminders — `FORUM-17 planned`; Next localStorage draft покрывает только незавершённый composer;
- first-class Q&A/wiki/announcement topic kinds и scheduled publishing — `FORUM-22 planned`; accepted solution покрывает один Q&A workflow, но не остальные kind/lifecycle;
- shared real-time transport — `FORUM-29 planned`;
- полноценная Forum profile/directory/activity composition — `FORUM-27 planned`; сегодня есть member cards и counters, но не полный directory/follow/block/reputation UI;
- Forum storefront composition с Search/Notifications/Media/Reactions/Profile — `FORUM-31 planned`;
- NodeBB import/export mapping source-ready, но `FORUM-34` ждёт shared migration runner integration.

**Рекомендация:** договориться о product scope/edition и разделить «готово», «backend/API», «source-ready», «browser-tested», «планируется». Не сравнивать только имена owner services с платформами, где эти capabilities доступны пользователю.

### F-15 — P2: source verifiers и документация разошлись с текущей структурой

**Наблюдения в выбранном наборе проверок:**

- Прошли `verify-forum-admin-boundary.mjs`, admin boundary fixtures (11/11), mention-notification integration source/test (7/7), `verify-forum-public-discovery-seo.mjs`, `verify-forum-category-taxonomy-browser-evidence.mjs`, `verify-forum-reactions-storefront-browser-evidence.mjs` и wave freshness fixtures (12/12). Это source/fixture checks, не runtime.
- `verify-forum-read-model.mjs` падает, потому что требует отсутствующий `src/services/read_model.rs` (в текущем дереве read-model разделён, например, на `read_model_owner.rs`).
- `verify-forum-topic-canonical-resolution.mjs` требует ticket `FORUM-21J`, а актуальное значение в источнике — `FORUM-21L`.
- `verify-forum-category-route-storefront-mount.mjs` проверяет прежние literal markers/функцию `safe_owner_path`; текущий route mount есть, но verifier ожидает другой shape.
- `verify-forum-attachment-relation-admission-source.mjs` открывает отсутствующий `src/category_presentation.rs`.
- `verify-forum-notification-inbox-listing.mjs` требует удалённые/перемещённые описания и API markers; актуальный `rustok-notifications/README.md` уже перечисляет bounded list/state/grouped UI.
- `verify-forum-search-projection.mjs` требует старый dependency marker без нынешнего `media`.
- `verify-forum-storefront-boundary.mjs` и несколько posting-policy/read verifiers тоже отказываются на устаревшем source shape или roadmap wording; это нужно перепроверить владельцем, а не считать автоматическим подтверждением функционального дефекта.

Есть и прямой doc drift: `crates/modules/rustok-forum/README.md:24–25, 45` говорит, что публичный category URL не смонтирован, тогда как `crates/modules/rustok-forum/docs/README.md:70–71` описывает FORUM-24O как mounted Rust-host route и `apps/storefront/src/lib.rs:664–688` содержит route registration. Аналогично Forum implementation ledger помечает `NOTIFY-04 planned`, в то время как Notifications README документирует bounded inbox/state APIs и grouped storefront UI. Это может быть разделением межмодульной ответственности, но в текущем виде status semantics не объяснены.

**Рекомендация:** актуализировать roadmap/README и заменить хрупкие textual assertions на tests against exported behavior/fixture composition. Для каждого verifier определить owner, действующий contract, актуальные source paths и доказательство, которое должен защищать скрипт; stale gate не должен оставаться красным без triage.

---

## 6. Сравнение с популярными форумными платформами

Сравнение ниже — **сравнение уровня возможностей продукта**, а не сертификация конкретного релиза/плагинного набора. У Discourse, NodeBB и Flarum многие детали зависят от версии, настройки и extensions/plugins. `✓` означает обычную user-facing capability в зрелом product setup; `◐` — ограниченно, через настройки/расширения или не полностью подтверждено; `△` — контракт/backend есть, но поверхность неполна; `—` — в проверенной Forum-поверхности не обнаружено.

| Возможность | Discourse | NodeBB | Flarum | RusToK Forum на этом снимке |
|---|---|---|---|---|
| Категории/теги и обсуждения | ✓ | ✓ | ✓ (теги как базовая модель; иерархия зависит от расширений) | Категории и taxonomy сильные; теги хранятся/читаются, но Next composer отправляет пустой список; category UI имеет depth/page gaps. |
| Публикация темы/ответа и rich authoring | ✓ | ✓ | ✓ | Next composer действительно вызывает GraphQL create; Rust/Leptos composer no-op — функциональность зависит от host. |
| Читать длинную тему, jump-to-post, cursor/pagination | ✓ | ✓ | ◐ | Backend positions/cursors существуют, но Next ограничен первой страницей до 100 ответов и не может прыгнуть в незагруженный пост. |
| Unread/latest/top и персональная история чтения | ✓ | ✓ | ◐ | Read-state backend реализован; Next Unread tab не заполняется из-за query mismatch, сортировка Top — локально по числу ответов, не рейтингу/периоду. |
| Like/vote/reactions/bookmarks | ✓ | ✓ | ◐ | Votes есть в owner/API; отдельный Reactions module частично компонуется в Rust host; Next controls и bookmarks отсутствуют. |
| Watch/mute темы и уведомления по ответам/mentions | ✓ | ✓ | ◐ | Subscription owner и mention/topic-created source есть; reply activity event не поддержан, подписка не выставляется из Forum UI, digest/preferences/delivery неполны. |
| User reports, queue, trust/anti-spam | ✓ | ✓ | ◐ (часть через extensions) | Admin local moderation и общий Moderation owner есть; публичный report button/transport не найден, posting-policy evaluator не встроен в write path, rate-limit scope открыт. |
| Search по телам темы/ответов, facets | ✓ | ✓ | ◐ (часто расширения) | Backend projections/filtering существуют, но Forum feed ищет только title/slug загруженной страницы; Search/reindex proof открыт. |
| SEO, permalink/redirect и deep links | ✓ | ✓ | ◐ | Есть сильный route/SEO owner в Rust host; Next feed path неверен, route namespace с Search canonical не совпадает, reply target не проброшен. |
| Profiles/member discovery, attachments/media | ✓ | ✓ | ◐ | Member cards/stats есть; Forum directory/social-graph UI и Media attach surface не завершены. |
| Переводы UI и RTL | ◐ (локализуется, зависит от поставки) | ◐ | ◐ | Content locale и Rust en/ru/ar foundation есть; Next chrome преимущественно английский, RTL browser proof не выполнен. |
| Операции модератора: merge/split/move/fork | ✓ | ✓/◐ | ◐ | В backend/admin необычно богатый набор; это сильная часть реализации, хотя runtime proof в этой среде не запускался. |
| Модульность и multi-tenant audience | По своей модели/установке | По своей модели/плагинам | По своей модели/расширениям | Сильная архитектурная сторона RusToK: explicit tenant, RBAC, channel/group/member/trust facts и owner boundaries. Это не заменяет parity публичного UX. |

### Сводка сравнения

В сравнении с типичной зрелой установкой Discourse/NodeBB RusToK **уже покрывает необычно много глубокой серверной инфраструктуры и moderator/admin операций**: ревизии, события, exact visibility, merge receipts, reconciliation, shared ownership и route identity. Но для обычного участника форума решают не только сущности/API, а работающие flows: открыть каноническую ссылку, написать ответ, получить unread, подписаться, найти старый ответ, сообщить о нарушении, загрузить изображение. Именно здесь текущая реализация проигрывает платформам, где эти действия связаны в один проверенный продуктовый путь.

Правильная формулировка статуса — не «форум уже равен Discourse/NodeBB», а: **сильный modular forum backend с рядом зрелых workflows; UI/integration/runtime completeness заметно ниже и не одинаково для Rust-host и Next-host.**

---

## 7. Рекомендуемый порядок исправлений

### До публичного beta (блокирующие flows)

1. **Сделать native storefront composer настоящей формой записи** и добавить host E2E для create topic/reply.
2. **Унифицировать route contract**: card, Search result, SEO metadata, redirects и reply deep links должны сходиться на один canonical path во всех storefront hosts.
3. **Подключить unread query** в Next и проверить end-to-end с реальным read cursor/current user.
4. **Добавить pagination и jump-to-position** для feed, taxonomy и long thread; определить поведение API errors вместо пустого состояния.

### До заявлений о полном форуме

5. Подключить Forum Search UI к owner Search (topic + reply body, facets, cursors, ACL).
6. Определить и реализовать user controls: subscribe/mute, vote/reaction, tag authoring, own edit/delete и report.
7. Добавить reply activity в Notification event/source/fanout и завершить privacy/delivery evidence.
8. Подключить Forum posting-policy evaluator и shared rate limits к service write boundary.
9. Спроектировать Media attach flow; сервис без transport не считать storefront-функцией.
10. Разделить/изолировать local drafts по tenant/actor и определить durable draft/bookmark product scope.
11. Довести Forum UI translations/RTL до end-to-end и тестировать локали в реальном host.

### Перед production

12. Выполнить Cargo check/test/clippy, все Forum SQLite/PostgreSQL migration/runtime/concurrency scenarios, host composition tests и браузерный regression suite.
13. Заменить/обновить устаревшие source verifiers, удалить неверные path/ticket markers и привести README/roadmap/status к одному фактическому состоянию.
14. Выполнить Search reindex, Notifications fanout/lease, attachment hold/reconciliation и Page Builder composition evidence на зарегистрированном tenant/runtime. Source-ready доказательство не подменять observed production run.

### Минимальный acceptance suite

- Next и Rust host создают тему и ответ; отказ не теряет draft.
- Список → canonical topic route работает с UUID/short ID/slug aliases; старые slug redirect, merged/tombstone и GONE имеют верный статус.
- Unread topic виден только нужному user; после mark-read уходит из выборки.
- Списки >50 тем/категорий и темы >100 ответов доступны полностью; jump к reply N загружает корректное окно.
- Reply event доставляется topic subscribers; mention не раскрывает hidden/private target; notification open повторно проверяет право.
- Report проходит в Moderation owner без локальной второй очереди; posting policy/rate limit нельзя обойти повтором или параллельными запросами.
- Attachment add/remove/reorder согласованы с Media holds и reconciliation.
- Draft не читается между tenant/account; logout/account-switch соблюдает ожидаемое поведение.
- en/ru/ar strings, RTL, content locale fallback, Search links и admin actions проверены в mounted browser.

---

## 8. Итоговое заключение

Forum core показывает сильную инженерную работу в tenant isolation, доменных границах, visibility, транзакционных событиях, revisioned relations, moderation subject integration, route ownership и bounded reconciliation. Это существенная база, и её не следует упрощать до «форум ещё не написан».

Однако итоговый продукт нельзя оценивать только по количеству сервисов и GraphQL mutation. В публичном коде есть прямые несвязанные/неверные flows (composer и permalink), read-state mismatch, отсутствующие пользовательские controls, частично подключённые attachments/notifications/search и заметный verification/doc drift. **Перед релизом сначала надо доказать сквозные пользовательские сценарии на двух реальных host-поверхностях, а уже затем закрывать паритет-функции вроде realtime, bookmarks и topic kinds.**
