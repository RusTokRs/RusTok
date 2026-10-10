# Глубокий инженерный аудит модуля Blog

- **Дата**: 2026-10-09
- **Модуль**: `rustok-blog` + связанные модули
- **Объём**: 141 Rust-файл, ~34 000 строк кода, 27 миграций, 55 контрактных evidence-файлов, 21 интеграционный тест

---

## 1. Архитектурный обзор

### 1.1 Структура модуля

```
rustok-blog/
├── src/
│   ├── entities/        (8 SeaORM-сущностей)
│   ├── domain/          (State Machine, RichText, Comment Policy)
│   ├── dto/             (Post, Category, Tag, Comment DTOs)
│   ├── services/        (PostService, CategoryService, TagService, CommentService, RBAC)
│   ├── controllers/     (Axum REST: posts, categories, comments)
│   ├── graphql/         (Query, Mutation, Types, Rate Limit, Runtime Data)
│   ├── integrations/    (SEO Targets, Reactions, Public Comments Snapshot)
│   ├── ports/           (Port abstraction)
│   ├── migrations/      (27 миграций)
│   └── error/           (BlogError + RichError mapping)
├── admin/               (Leptos/WASM Admin UI)
├── storefront/          (Leptos/WASM Storefront UI)
├── tests/               (21 интеграционный тест)
├── contracts/evidence/  (55 контрактных evidence-файлов)
└── docs/                (implementation plans, slice docs)
```

### 1.2 Связанные модули (межмодульное взаимодействие)

| Модуль | Роль в Blog | Тип интеграции |
|--------|-------------|----------------|
| `rustok-taxonomy` | Категории (иерархия, локализация), Теги (terms) | Прямой DB-запись в транзакции |
| `rustok-content` | RichText профили, canonical URL registry, locale resolution | Порт + DB |
| `rustok-comments-api` | Комментарии (создание, модерация, листинг) | `CommentsThreadPort` (async trait) |
| `rustok-seo-targets` | SEO metadata, Open Graph, sitemap, structured data | `SeoTargetProvider` (async trait) |
| `rustok-reactions-api` | Лайки (like) на посты | `ReactionSubjectProvider` (async trait) |
| `rustok-channel` | Мультиканальная видимость постов | `ChannelService` |
| `rustok-profiles-api` | Профили авторов | `ProfileSummaryAudience` |
| `rustok-outbox` | Транзакционная шина событий | `TransactionalEventBus` |
| `rustok-events` | Domain events | `DomainEvent::*` |
| `rustok-auth` | RBAC permissions | `SecurityContext` |
| `rustok-core` | Error types, SecurityContext | Прямой импорт |
| `rustok-api` | Permissions, RichText types, GraphQL helpers | Прямой импорт |
| `rustok-telemetry` | Metrics recording | `metrics::*` |
| `rustok-ai` | AI-интеграция для генерации контента | Реэкспорт из `rustok-blog` |
| `rustok-translation` | Переводы категорий (через Taxonomy) | Порт `TaxonomyTranslationOwner` |
| `rustok-workflow` | Workflow-оркестрация (контент промоушн/демоушн) | Domain events |

### 1.3 Dual-UI стратегия

Модуль поддерживает **два UI-стека**:
- **Leptos/WASM** — нативный Rust-рендеринг для `apps/admin` и `apps/storefront`
- **Next.js/React** — `apps/next-admin` и `apps/next-frontend`

---

## 2. Реализованный функционал

### 2.1 Управление постами (PostService)

| Функция | Статус | Детали |
|---------|--------|--------|
| Создание поста | ✅ | RichText (Article profile), теги, категории, featured image, SEO поля, channel visibility, metadata |
| Обновление поста | ✅ | Optimistic locking (version), Patch-семантика для nullable полей, locale-aware |
| Удаление поста | ✅ | Только Draft/Archived, каскадное удаление routes, тегов, SEO targets |
| Получение по ID | ✅ | С locale fallback chain |
| Получение по slug | ✅ | С резолвом retired routes (redirects) |
| Листинг (admin) | ✅ | Keyset cursor pagination, фильтрация по status/category/tag/author, сортировка |
| Публичный листинг | ✅ | Keyset pagination (published_at, id), channel visibility filter |
| Bulk scan | ✅ | Для sitemap и SEO bulk jobs |

### 2.2 State Machine (жизненный цикл поста)

```
   ┌───────┐
   │ Draft │ ◄─────────────────────────────┐
   └───┬───┘                                │
       │ publish()                          │ unpublish() / restore()
       ↓                                    │
   ┌───────────┐                            │
   │ Published │──── archive() ──→ ┌──────────┐
   └───────────┘                   │ Archived │
                                   └──────────┘
```

- **Type-safe** реализация: `BlogPost<Draft>`, `BlogPost<Published>`, `BlogPost<Archived>` — невалидные переходы невозможны на уровне компиляции
- **Runtime-таблица** `BlogPostStatus::can_transition_to()` синхронизирована с type-state графом
- **Optimistic locking** через `version` (i32) с CAS-проверкой при каждом переходе
- **`published_at`** — first-publication timestamp, сохраняется навсегда (не сбрасывается при unpublish)

### 2.3 Мультиязычность (i18n)

- **Таблица `blog_post_translations`**: title, excerpt, body, seo_title, seo_description per locale
- **Fallback chain**: `requested → explicit fallback → en → first available`
- **Слаги глобальные** (locale-neutral) — нормализуются через `rustok_taxonomy::normalize_term_route_key` с транслитерацией
- **Категории и теги** локализуются через Taxonomy
- **Платформенный fallback locale** (`en`)

### 2.4 Категории (CategoryService)

- **Иерархия**: parent_id, position, tree capacity check
- **Локализация**: через Taxonomy (canonical key, translations)
- **Settings**: extensible JSON (до 64 KiB)
- **CRUD**: create, update, delete, move (смена родителя)
- **Таксономическая интеграция**: `TaxonomyOwnerCategoryReader`, `TaxonomyScopeType::Blog`
- **Каскадное удаление**: при удалении категории — посты переводятся в uncategorized

### 2.5 Теги (TagService)

- **Хранение**: через Taxonomy module terms (TaxonomyTermKind::Tag)
- **Usage tracking**: `blog_tag_usage` projection с `use_count`
- **CRUD**: create, update, delete, list с пагинацией
- **Auto-create**: при создании поста можно создавать теги на лету (если есть permission)
- **Tenant integrity**: FK-constraints на tenant_id

### 2.6 Комментарии (CommentService)

- **Порт-архитектура**: `CommentsThreadPort` (async trait) — Blog не владеет данными комментариев
- **Политика**: `BlogCommentsMode` (Open / ReadOnly / Disabled) через module settings
- **Создание**: с idempotency (command_id), compensating delete при race conditions
- **Модерация**: approve / spam / trash
- **Публичный листинг**: snapshot-cache с graceful degradation (Available / Disabled / ReadOnly / Unavailable / Timeout)
- **Threaded comments**: parent_comment_id поддержка
- **Comment projection**: idempotent delivery tracking через `blog_comment_projection_deliveries`

### 2.7 SEO-интеграция

- **`BlogSeoTargetProvider`** — реализует `SeoTargetProvider`:
  - `load_target` — загрузка SEO-данных поста
  - `resolve_route` — резолв URL → post
  - `sitemap_candidates` — генерация sitemap entries
  - `list_bulk_summaries_page` — bulk scan для SEO jobs
- **Structured Data**: JSON-LD (BlogPosting schema)
- **Open Graph**: title, description, images, locale, type
- **Canonical routes**: `/modules/blog?slug={slug}`
- **Alternate routes**: hreflang для каждого available locale
- **Redirects**: при смене slug — старая route становится alias, SEO генерирует 301/308

### 2.8 Реакции (Likes)

- **`BlogReactionSubjectProvider`**: авторизация лайков на опубликованные посты
- **Каталог**: одна реакция — `like` (Single selection policy)
- **Settings**: `use_reactions` toggle в module settings
- **Deletion binding**: при удалении поста — удаление связанных реакций

### 2.9 API

#### REST (Axum)
```
GET    /api/blog/posts                    — list
POST   /api/blog/posts                    — create
GET    /api/blog/posts/{id}               — get
PUT    /api/blog/posts/{id}               — update
DELETE /api/blog/posts/{id}               — delete
POST   /api/blog/posts/{id}/publish       — publish
POST   /api/blog/posts/{id}/unpublish     — unpublish
POST   /api/blog/posts/{id}/archive       — archive
POST   /api/blog/posts/{id}/restore       — restore
GET    /api/blog/categories               — list categories
POST   /api/blog/categories               — create category
GET    /api/blog/categories/{id}          — get category
PUT    /api/blog/categories/{id}          — update category
DELETE /api/blog/categories/{id}          — delete category
POST   /api/blog/categories/{id}/move     — move category
POST   /api/blog/comments/{id}/moderate   — moderate comment
```

#### GraphQL
```graphql
# Queries
post(id, locale)                    → GqlPost
postBySlug(slug, locale)            → GqlPost
posts(filter)                       → GqlPostList (admin)
publicBlogPosts(filter)             → GqlPublicPostList (storefront)
blogCategories(filter)              → GqlBlogCategoryList
blogTags(filter)                    → GqlBlogTagList

# Mutations
createPost(input)                   → Uuid
updatePost(id, input)               → Boolean
deletePost(id)                      → Boolean
publishPost(id)                     → Boolean
unpublishPost(id)                   → Boolean
archivePost(id, reason)             → Boolean
restorePost(id)                     → Boolean
createBlogComment(postId, input)    → GqlBlogComment
moderateComment(id, commandId, status) → Boolean
createBlogCategory(input)           → Uuid
updateBlogCategory(id, input)       → GqlBlogCategory
deleteBlogCategory(id)              → Boolean
```

#### GraphQL Rate Limiting
- Per-surface rate limiting (Post, PostBySlug, Posts, CreatePost, UpdatePost, etc.)
- Tenant-scoped keys с actor differentiation (user / service / IP / anonymous)
- Write-операции rate-limitятся только для авторизованных пользователей
- `Retry-After` header в ответе
- Backend: pluggable `BlogGraphqlRateLimiter` trait

### 2.10 RBAC

| Permission | Scope |
|-----------|-------|
| `blog_posts:create` | Создание постов |
| `blog_posts:read` | Чтение (публичное + непубличное для не-Customer) |
| `blog_posts:update` | Редактирование (+ own scope) |
| `blog_posts:delete` | Удаление (+ own scope) |
| `blog_posts:list` | Листинг |
| `blog_posts:publish` | Publish / unpublish / archive / restore (+ own scope) |
| `blog_posts:manage` | Модерация комментариев |
| `blog_categories:*` | CRUD категорий |
| `tags:*` | CRUD тегов |

### 2.11 Frontend (Storefront)

| Компонент | Статус |
|-----------|--------|
| Blog Section (лента постов) | ✅ |
| Post Card | ✅ |
| Post Detail Page (SSR) | ✅ |
| RichText rendering | ✅ |
| Table of Contents (sticky sidebar) | ✅ |
| Author Card (mini + bio) | ✅ |
| Reading Progress Bar | ✅ |
| Reading Time estimation | ✅ |
| Share Buttons (social) | ✅ |
| Reaction Bar (likes) | ✅ |
| Comments Section (threaded) | ✅ |
| Comment Composer | ✅ |
| Comments Pagination | ✅ |
| Blog Pagination (cursor) | ✅ |
| Related Articles | ✅ |
| RSS Feed (XML) | ✅ |
| JSON-LD Structured Data | ✅ |
| SEO Metadata (OpenGraph, Twitter) | ✅ |
| Permanent Redirect (retired slugs) | ✅ |

### 2.12 Frontend (Admin)

| Компонент | Статус |
|-----------|--------|
| Post List (table) | ✅ |
| Post Form (create/edit) | ✅ |
| RichText Editor | ✅ |
| Category Management | ✅ |
| Category Modal (CRUD) | ✅ |
| Post Actions (publish/unpublish/archive/delete) | ✅ |

### 2.13 Domain Events

| Event | Trigger |
|-------|---------|
| `BlogPostCreated` | Создание поста |
| `BlogPostPublished` | Publish |
| `BlogPostUnpublished` | Unpublish |
| `BlogPostArchived` | Archive |
| `BlogPostDeleted` | Delete |
| `ReindexRequested` | Restore (SEO reindex) |
| `TargetDeleted` | Delete (SEO cleanup) |
| `CanonicalUrlChanged` | Slug rename (via content registry) |
| `UrlAliasPurged` | Delete / slug claim |

---

## 3. Обнаруженные пробелы в реализации

### 3.1 Критические (HIGH) — блокируют production-readiness

#### H-1. Нет отложенной публикации (Scheduled Publishing)
- **Описание**: Невозможно запланировать публикацию поста на будущее время
- **Влияние**: Контент-менеджеры не могут готовить контент заранее и планировать публикации
- **Конкуренты**: WordPress, Ghost, Medium, Strapi, Contentful — все поддерживают
- **Сложность**: Средняя (нужен scheduler worker + cron-подобный механизм + UI)

#### H-2. Нет истории версий контента (Content Revision History)
- **Описание**: При обновлении поста предыдущие версии контента теряются безвозвратно
- **Влияние**: Невозможно откатить изменения, посмотреть diff, восстановить удалённый текст
- **Конкуренты**: WordPress (revisions), Ghost (version history), Notion (page history), Strapi (draft/published versions)
- **Сложность**: Высокая (нужна отдельная таблица revision + diff-engine + UI)

#### H-3. Нет полнотекстового поиска по контенту (Full-Text Search)
- **Описание**: Нет серверного поиска по заголовкам, excerpt и body постов
- **Влияние**: Админ-панель не может искать посты по содержимому; публичный поиск невозможен
- **Конкуренты**: WordPress (built-in), Ghost (search), all headless CMS
- **Сложность**: Средняя (PostgreSQL FTS / Meilisearch / Typesense integration)

#### H-4. Нет предварительного просмотра черновиков (Draft Preview)
- **Описание**: Невозможно дать ссылку на предпросмотр черновика без публикации
- **Влияние**: Редакторы и ревьюеры не могут видеть пост до публикации
- **Конкуренты**: WordPress (preview link), Ghost (private preview), Contentful (preview API)
- **Сложность**: Средняя (preview tokens с TTL + отдельный endpoint)

#### H-5. Нет медиа-ассет менеджмента для изображений
- **Описание**: `featured_image_url` — это просто URL-строка. Нет связи с медиа-библиотекой, нет загрузки, нет оптимизации, нет CDN
- **Влияние**: Пользователь должен сам загружать изображения на внешний хостинг
- **Конкуренты**: Все CMS имеют media library
- **Примечание**: Решение от 2026-10-08 оставляет это как follow-up work

#### H-6. Нет bulk-операций
- **Описание**: Невозможно массово опубликовать, архивировать или удалить посты
- **Влияние**: При большом количестве постов управление крайне неэффективно
- **Конкуренты**: WordPress (bulk actions), Ghost (bulk), Strapi (bulk publish)
- **Сложность**: Низкая-средняя (batch endpoint + UI checkboxes)

### 3.2 Важные (MEDIUM) — ограничивают конкурентоспособность

#### M-1. Нет полнотекстового поиска для публичных пользователей
- **Описание**: Storefront не имеет виджета поиска по статьям
- **Влияние**: Посетители не могут найти нужный контент

#### M-2. Нет защиты от спама в комментариях
- **Описание**: Комментарии создаются со статусом `Pending`, но нет автоматического spam-фильтра
- **Влияние**: Модераторы вынуждены вручную проверять каждый комментарий
- **Конкуренты**: WordPress (Akismet), Ghost (Akismet), Disqus (auto-filter)
- **Сложность**: Средняя (Akismet API / AI-based classifier)

#### M-3. Нет уведомлений о комментариях
- **Описание**: Ни автор поста, ни комментаторы не получают уведомления о новых ответах
- **Влияние**: Снижение вовлечённости, пользователи не знают об ответах
- **Конкуренты**: WordPress (email notifications), Disqus, Ghost (email)

#### M-4. Нет редактирования комментариев пользователями
- **Описание**: Пользователь не может отредактировать свой комментарий после отправки
- **Влияние**: Опечатки и ошибки остаются навсегда
- **Конкуренты**: Reddit, Disqus, Ghost (edit within time window)

#### M-5. Нет закрепления/избранных постов (Pinned/Featured Posts)
- **Описание**: Невозможно закрепить пост наверху ленты или отметить как «избранный»
- **Влияние**: Важные статьи теряются в ленте
- **Конкуренты**: WordPress (sticky posts), Ghost (featured), Medium (pin)
- **Сложность**: Низкая (добавить `is_pinned: bool` + `pinned_at` + порядок сортировки)

#### M-6. Нет серий статей / мульти-частных публикаций
- **Описание**: Нет группировки постов в серии (Part 1, Part 2, ...)
- **Влияние**: Длинные руководства нельзя структурировать
- **Конкуренты**: Dev.to (series), Medium (series), Ghost (collections)

#### M-7. Нет импорта/экспорта контента
- **Описание**: Невозможно импортировать посты из Markdown/WordPress/CSV или экспортировать
- **Влияние**: Миграция с/на другие платформы затруднена
- **Конкуренты**: WordPress (WXR export/import), Ghost (JSON export/import), Medium (export)

#### M-8. Нет интеграции с социальными сетями
- **Описание**: При публикации поста нет автоматического постинга в Twitter/LinkedIn/Telegram
- **Влияние**: Ручное копирование для каждой соцсети
- **Конкуренты**: Ghost (native integrations), WordPress (Jetpack Social), Buffer/Hootsuite

#### M-9. Нет email-подписки на блог (Newsletter)
- **Описание**: Читатели не могут подписаться на новые статьи по email
- **Влияние**: Нет канала удержания аудитории
- **Конкуренты**: Ghost (built-in newsletter), Substack, WordPress (Jetpack subscriptions)

#### M-10. Нет A/B тестирования заголовков
- **Описание**: Невозможно протестировать несколько вариантов заголовка
- **Влияние**: Нельзя оптимизировать CTR
- **Конкуренты**: Medium (A/B title testing), Optimizely

#### M-11. Нет закладок/reading list для пользователей
- **Описание**: Пользователь не может сохранить пост для чтения позже
- **Влияние**: Снижение retention
- **Конкуренты**: Medium (bookmarks), Pocket, Dev.to (reading list)

#### M-12. Нет collaborative editing / соавторства
- **Описание**: Пост привязан к одному `author_id`. Нет возможности совместного редактирования или указания нескольких авторов
- **Влияние**: Невозможна командная работа над контентом
- **Конкуренты**: WordPress (multi-author), Notion (real-time collab), Google Docs

#### M-13. Нет аналитики просмотров
- **Описание**: `view_count` удалён из схемы (решение 2026-10-08), замены нет
- **Влияние**: Нет данных о популярности постов, невозможно строить «Popular» / «Trending»
- **Примечание**: Решение корректное (аналитика — отдельный owner), но owner не назначен

#### M-14. Нет контрибуции от внешних авторов (Guest Posts)
- **Описание**: Нет workflow для guest-постов с ревью и approval
- **Влияние**: Внешние авторы не могут предлагать контент

#### M-15. Ограниченная сортировка в admin
- **Описание**: GraphQL `posts` query всегда сортирует по `CreatedAt DESC`. Нет сортировки по title, comment_count, relevance
- **Влияние**: Неудобно при большом количестве постов

### 3.3 Желательные (LOW) — улучшают UX, но не блокируют

#### L-1. Нет OEmbed/Embed-поддержки
- **Описание**: RichText не поддерживает встраивание внешних видео/виджетов (YouTube, Twitter, etc.)
- **Конкуренты**: WordPress (oEmbed), Ghost (embeds), Notion (embeds)

#### L-2. Нет дат истечения срока / напоминаний о ревью
- **Описание**: Невозможно установить дату, когда контент нужно пересмотреть/обновить
- **Конкуренты**: Contentful (review dates), WordPress (editorial calendar plugins)

#### L-3. Нет inline-комментариев к контенту
- **Описание**: Комментарии только внизу поста, нет возможности комментировать конкретный абзац
- **Конкуренты**: Medium (highlights + notes), Notion (inline comments), Google Docs

#### L-4. Нет подсветки синтаксиса кода (конфигурируемой)
- **Описание**: RichText Article profile фиксирован, нет явной поддержки code blocks с подсветкой
- **Конкуренты**: Ghost (code blocks), Dev.to, Medium

#### L-5. Нет галереи/лайтбокса для изображений в контенте
- **Описание**: Статья поддерживает только текстовые параграфы, нет image gallery node
- **Конкуренты**: WordPress (Gutenberg gallery), Ghost (gallery card)

#### L-6. Нет print-friendly вида
- **Описание**: Нет CSS для печати статей

#### L-7. Нет контент-шаблонов (Post Templates)
- **Описание**: Нет предустановленных шаблонов для разных типов контента (tutorial, news, review)
- **Конкуренты**: WordPress (post templates), Notion (templates)

#### L-8. Нет социального proof (social proof)
- **Описание**: Нет счётчика просмотров, нет «N людей читают сейчас»
- **Связано с**: M-13 (аналитика)

#### L-9. Нет календарного вида для контент-планирования
- **Описание**: Нет визуального календаря публикаций
- **Связано с**: H-1 (scheduled publishing)
- **Конкуренты**: WordPress (editorial calendar), CoSchedule

#### L-10. Нет тегов Open Graph preview в админке
- **Описание**: Нет предпросмотра того, как пост будет выглядеть при шаринге в соцсетях
- **Конкуренты**: Yoast SEO (WordPress), Ghost (social preview)

#### L-11. Нет Webhooks для событий блога
- **Описание**: Domain events генерируются, но нет механизма webhook-доставки для внешних систем
- **Конкуренты**: GitHub, Strapi, Contentful (webhooks)

---

## 4. Сравнение с популярными платформами

### 4.1 Матрица функционала

| Функция | RusTok Blog | WordPress | Ghost | Medium | Strapi | Contentful |
|---------|:-----------:|:---------:|:-----:|:------:|:------:|:----------:|
| CRUD постов | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| RichText редактор | ✅ | ✅ (Gutenberg) | ✅ (Koenig) | ✅ | ✅ | ✅ |
| Markdown поддержка | ❌ | ✅ (plugin) | ✅ | ✅ | ✅ | ✅ |
| Мультиязычность | ✅ | ✅ (WPML) | ✅ | ❌ | ✅ | ✅ |
| Категории (иерархия) | ✅ | ✅ | ✅ (tags only) | ❌ | ✅ | ✅ |
| Теги | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Комментарии | ✅ | ✅ | ✅ (native) | ✅ (claps) | ❌ (plugin) | ❌ |
| Threaded comments | ✅ | ✅ | ❌ | ✅ | ❌ | ❌ |
| Лайки/реакции | ✅ | ❌ (plugin) | ❌ | ✅ (claps) | ❌ | ❌ |
| SEO (meta, OG, JSON-LD) | ✅ | ✅ (Yoast) | ✅ | ✅ (auto) | ✅ | ✅ |
| Sitemap | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| RSS/Atom | ✅ | ✅ | ✅ | ✅ | ✅ (plugin) | ✅ |
| Scheduled publishing | ❌ | ✅ | ✅ | ✅ (partial) | ✅ | ✅ |
| Revision history | ❌ | ✅ | ✅ | ❌ | ✅ | ✅ |
| Draft preview | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ (preview API) |
| Media library | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Full-text search | ❌ | ✅ | ✅ | ✅ | ✅ (plugin) | ✅ |
| Bulk operations | ❌ | ✅ | ✅ | ❌ | ✅ | ✅ |
| Featured/pinned posts | ❌ | ✅ (sticky) | ✅ | ✅ (pin) | ❌ | ❌ |
| Newsletter/email | ❌ | ✅ (Jetpack) | ✅ (native!) | ✅ (Digest) | ❌ | ❌ |
| Post series | ❌ | ✅ (plugin) | ❌ | ✅ | ❌ | ❌ |
| Collaborative editing | ❌ | ✅ (multi-author) | ❌ | ❌ | ❌ | ✅ |
| Social auto-publish | ❌ | ✅ (Jetpack) | ✅ (native) | ✅ (auto) | ❌ | ❌ |
| A/B title testing | ❌ | ❌ (plugin) | ❌ | ✅ | ❌ | ❌ |
| Spam protection | ❌ | ✅ (Akismet) | ✅ (Akismet) | ✅ (auto) | ❌ | ❌ |
| Content import/export | ❌ | ✅ (WXR) | ✅ (JSON) | ✅ (export) | ✅ | ✅ |
| Guest posts | ❌ | ✅ | ❌ | ✅ | ❌ | ❌ |
| Analytics | ❌ | ✅ (Stats) | ✅ (native) | ✅ (native) | ❌ | ❌ |
| Webhooks | ❌ | ✅ | ✅ | ❌ | ✅ | ✅ |
| Rate limiting | ✅ | ❌ (plugin) | ✅ | ✅ | ✅ | ✅ |
| Multi-tenancy | ✅ | ✅ (multisite) | ❌ | ❌ | ✅ | ✅ |
| RBAC (granular) | ✅ | ✅ (roles) | ✅ (roles) | ❌ | ✅ | ✅ |
| Channel visibility | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ (environments) |
| Content orchestration | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Type-safe state machine | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Domain events (outbox) | ✅ | ❌ | ❌ | ❌ | ❌ (webhooks) | ✅ (webhooks) |
| Keyset pagination | ✅ | ❌ (offset) | ❌ (offset) | ❌ | ✅ | ✅ |
| Transactional integrity | ✅ | ❌ | ❌ | ❌ | ✅ | ✅ |

### 4.2 Сильные стороны RusTok Blog (vs конкуренты)

1. **Type-safe State Machine** — уникальная особенность. Ни одна из популярных CMS не имеет compile-time гарантий корректности жизненного цикла.
2. **Транзакционный Outbox для событий** — гарантированная доставка событий, в отличие от webhook-fire-and-forget.
3. **Мультиканальная видимость** — уникальная возможность ограничивать видимость постов по каналам (web, mobile, API). Нет аналогов.
4. **Content Orchestration** — промоушн/демоушн контента между типами (blog ↔ forum) — уникально.
5. **Keyset Cursor Pagination** — корректная пагинация без OFFSET. Лучше чем offset-based у WordPress/Ghost.
6. **Idempotent Comment Projection** — гарантия exactly-once доставки комментариев.
7. **Per-surface GraphQL Rate Limiting** — гранулярный rate limiting на уровне GraphQL-полей.
8. **Multi-tenancy из коробки** — полноценная изоляция данных. У WordPress — только через multisite.
9. **Компенсирующие транзакции** — при race conditions в комментариях автоматически откатываются.
10. **Canonical URL Registry** — единый реестр URL для всех модулей, включая redirects при смене slug.

### 4.3 Слабые стороны (vs конкуренты)

1. **Контент-менеджмент**: Отсутствуют scheduled publishing, revision history, draft preview — базовые функции для любой CMS.
2. **Медиа**: Полное отсутствие media library — критический пробел для контентной платформы.
3. **Поиск**: Нет полнотекстового поиска — одна из самых востребованных функций.
4. **Email-маркетинг**: Ghost убивает этой фичей (built-in newsletter), у RusTok — ничего.
5. **Аналитика**: Нет встроенной аналитики. Ghost и Medium имеют её из коробки.
6. **Социальная интеграция**: Нет автопостинга, нет OEmbed.
7. **Collaborative**: Нет совместного редактирования, нет guest posts workflow.

---

## 5. Технические наблюдения и рекомендации

### 5.1 Архитектурные достоинства

- **Чёткое разделение owner boundaries**: Blog владеет постами, Taxonomy — категориями/тегами, Comments — комментариями, Content — URL registry
- **Порт-адаптер паттерн** для Comments: `CommentsThreadPort` позволяет подменять провайдера без изменений в Blog
- **Domain Events через outbox**: События публикуются в той же транзакции, что и данные
- **Contract evidence**: 55 evidence-файлов с верифицированными контрактами — впечатляющий уровень формализации
- **Dual-UI стратегия**: Leptos + Next.js — покрывает оба сценария использования

### 5.2 Технические риски

1. **N+1 запросы в GraphQL**: `GqlPost.public_comments` и `GqlPost.moderation_comments` делают отдельные запросы на каждый пост. При запросе списка постов с комментариями — потенциальная проблема производительности.
2. **Комментарии через внешний порт**: Разделение Blog и Comments через `CommentsThreadPort` добавляет latency и сложность компенсаций.
3. **JSON metadata**: `blog_posts.metadata` и `blog_categories.settings` — JSON без схемы. Валидация только на уровне размера и зарезервированных ключей.
4. **Отсутствие кэширования**: Публичные листинги и SEO-данные не кэшируются на уровне приложения (только CDN через Cache-Control headers).

### 5.3 Приоритизированный roadmap

#### Фаза 1 — Production-readiness (критично)
1. Scheduled Publishing (H-1)
2. Draft Preview tokens (H-4)
3. Full-text search (H-3)
4. Bulk operations (H-6)
5. Spam protection для комментариев (M-2)

#### Фаза 2 — Конкурентоспособность
6. Content Revision History (H-2)
7. Media asset integration (H-5)
8. Featured/Pinned posts (M-5)
9. Newsletter integration (M-9)
10. Content import/export (M-7)

#### Фаза 3 — Расширение
11. Analytics module (M-13)
12. Social auto-publish (M-8)
13. Comment notifications (M-3)
14. Post series (M-6)
15. Guest post workflow (M-14)

---

## 6. Резюме

Модуль Blog в RusTok реализован на **высоком инженерном уровне**: type-safe state machine, transactional outbox, idempotent projections, чёткие owner boundaries, мультиязычность, мультиканальность и granular RBAC ставят его **выше** большинства конкурентов по архитектурному качеству.

Однако по **пользовательскому функционалу** модуль отстаёт от зрелых CMS (WordPress, Ghost) на **~30-40%**. Главные пробелы — scheduled publishing, revision history, media library, full-text search и draft preview — являются базовыми ожиданиями для любой контентной платформы.

Рекомендуемый приоритет — закрыть фазу 1 (production-readiness), чтобы архитектурное превосходство стало доступно конечным пользователям.
