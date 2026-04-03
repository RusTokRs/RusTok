# rustok-media

Доменный модуль управления медиаактивами платформы RusTok.
Реализует загрузку, хранение, поиск, удаление и мультиязычные метаданные (alt-текст, подписи) для медиафайлов в рамках мультитенантной архитектуры.

## Назначение

- Единая точка управления файлами: изображения, видео, аудио, PDF.
- Полная мультитенантная изоляция: каждый запрос привязан к `tenant_id`.
- Хранение метаданных в БД (`media` + `media_translations`), физических файлов — через `rustok-storage`.
- Предоставляет публичные URL файлам через слой хранилища.

## Структура

```
crates/rustok-media/
├── src/
│   ├── lib.rs               # pub-реэкспорт
│   ├── controllers/         # REST-адаптеры модуля media
│   ├── dto.rs               # UploadInput, MediaItem, MediaTranslationItem, константы
│   ├── error.rs             # MediaError, Result<T>
│   ├── graphql/             # GraphQL Query/Mutation/types модуля media
│   ├── service.rs           # MediaService (основная бизнес-логика)
│   └── entities/
│       ├── mod.rs
│       ├── media.rs          # SeaORM entity: таблица `media`
│       └── media_translation.rs  # SeaORM entity: таблица `media_translations`
└── Cargo.toml
```

## Доменная модель

### Таблица `media`

| Поле | Тип | Описание |
|------|-----|----------|
| `id` | UUID | Primary key |
| `tenant_id` | UUID | FK → `tenants.id`, CASCADE DELETE |
| `uploaded_by` | UUID? | FK → `users.id`, SET NULL |
| `filename` | VARCHAR(255) | Имя файла в хранилище (UUID + расширение) |
| `original_name` | VARCHAR(255) | Оригинальное имя от клиента |
| `mime_type` | VARCHAR(100) | MIME-тип (`image/jpeg`, `video/mp4`, …) |
| `size` | BIGINT | Размер в байтах |
| `storage_path` | VARCHAR(500) | Относительный путь в хранилище |
| `storage_driver` | VARCHAR(32) | Имя бэкенда (`local`, `s3`, …) |
| `width` | INT? | Ширина (для изображений/видео) |
| `height` | INT? | Высота (для изображений/видео) |
| `metadata` | JSONB | Произвольные метаданные |
| `created_at` | TIMESTAMPTZ | Дата создания |

### Таблица `media_translations`

| Поле | Тип | Описание |
|------|-----|----------|
| `id` | UUID | Primary key |
| `media_id` | UUID | FK → `media.id`, CASCADE DELETE |
| `locale` | VARCHAR(5) | Код локали (`en`, `ru`, `fr-CA`) |
| `title` | VARCHAR(255)? | Заголовок файла |
| `alt_text` | VARCHAR(255)? | Alt-текст (accessibility, SEO) |
| `caption` | TEXT? | Подпись / описание |

Уникальный индекс: `(media_id, locale)`.

## Публичный API

### `MediaService`

```rust
let service = MediaService::new(db: DatabaseConnection, storage: StorageService);

// Загрузка файла
let item: MediaItem = service.upload(UploadInput { ... }).await?;

// Получение по ID
let item: MediaItem = service.get(tenant_id, id).await?;

// Список с пагинацией
let (items, total): (Vec<MediaItem>, u64) =
    service.list(tenant_id, limit, offset).await?;

// Удаление (удаляет файл из хранилища + запись из БД)
service.delete(tenant_id, id).await?;

// Upsert перевода
let translation: MediaTranslationItem =
    service.upsert_translation(tenant_id, media_id, input).await?;

// Получение переводов
let translations: Vec<MediaTranslationItem> =
    service.get_translations(tenant_id, media_id).await?;
```

### `UploadInput`

```rust
pub struct UploadInput {
    pub tenant_id: Uuid,
    pub uploaded_by: Option<Uuid>,
    pub original_name: String,
    pub content_type: String,
    pub data: bytes::Bytes,
}
```

### Ограничения при загрузке

| Параметр | Значение |
|----------|----------|
| Максимальный размер | **50 МиБ** (`DEFAULT_MAX_SIZE`) |
| Допустимые MIME-префиксы | `image/`, `video/`, `audio/`, `application/pdf` |

При нарушении — `MediaError::FileTooLarge` или `MediaError::UnsupportedMimeType`.

## Ошибки

```rust
pub enum MediaError {
    NotFound(Uuid),
    Forbidden,
    UnsupportedMimeType(String),
    FileTooLarge { size: u64, max: u64 },
    Storage(StorageError),
    Db(DbErr),
}
```

## REST API

Все эндпоинты требуют аутентификации (Bearer JWT) и определения тенанта через middleware.

| Метод | Путь | Описание |
|-------|------|----------|
| `POST` | `/api/media` | Загрузка файла (`multipart/form-data`, поле `file`) |
| `GET` | `/api/media?limit=&offset=` | Список медиа тенанта |
| `GET` | `/api/media/{id}` | Получение одного файла |
| `DELETE` | `/api/media/{id}` | Удаление файла (хранилище + БД) |
| `PUT` | `/api/media/{id}/translations/{locale}` | Upsert alt-текст/заголовок |

### Пример: загрузка через cURL

```bash
curl -X POST https://api.example.com/api/media \
  -H "Authorization: Bearer $TOKEN" \
  -H "X-Tenant-ID: $TENANT_ID" \
  -F "file=@photo.jpg"
```

### Пример: ответ `MediaItem`

```json
{
  "id": "01935d4f-...",
  "tenant_id": "...",
  "uploaded_by": "...",
  "filename": "01935d4f-....jpg",
  "original_name": "photo.jpg",
  "mime_type": "image/jpeg",
  "size": 102400,
  "storage_driver": "local",
  "public_url": "/media/tenant-id/2026/03/01935d4f-....jpg",
  "width": null,
  "height": null,
  "created_at": "2026-03-16T12:00:00Z"
}
```

## GraphQL API

Модуль добавляет `MediaQuery` и `MediaMutation` к корневой схеме (активируются через `mod-media` feature).

### Queries

```graphql
# Список медиа тенанта
media(tenantId: UUID!, pagination: PaginationInput): GqlMediaList!

# Получение одного файла
mediaItem(tenantId: UUID!, id: UUID!): GqlMediaItem

# Переводы файла
mediaTranslations(tenantId: UUID!, mediaId: UUID!): [GqlMediaTranslation!]!
```

### Mutations

```graphql
# Удаление файла
deleteMedia(tenantId: UUID!, id: UUID!): Boolean!

# Upsert alt-текст/заголовок для локали
upsertMediaTranslation(
  tenantId: UUID!
  mediaId: UUID!
  input: UpsertMediaTranslationInput!
): GqlMediaTranslation!
```

> **Загрузка** (upload) выполняется через REST `POST /api/media` — GraphQL не поддерживает multipart нативно.

Все резолверы проверяют `require_module_enabled(MEDIA)` (runtime guard по `tenant_modules`).

## Module-owned Leptos admin UI

- Модуль теперь публикует `rustok-media-admin` как module-owned Leptos admin surface для `/modules/media`.
- Внутренний data-layer для этого UI строится по модели native `#[server]` first.
- Для `library`, `detail`, `translations`, `delete` и `usage` сохраняется GraphQL fallback; существующие media queries/mutations не удаляются.
- Для upload сохраняется REST-first путь `POST /api/media`; отдельный GraphQL upload adapter не добавляется.
- Этот UI остаётся additive-поверхностью и не заменяет существующие GraphQL/REST contracts.

## SystemQuery (Observability)

`SystemQuery` (всегда включён) предоставляет статистику по медиа:

```graphql
mediaUsage(tenantId: UUID!): MediaUsageStats!
# → { tenantId, fileCount, totalBytes }
```

## Prometheus-метрики

| Метрика | Тип | Метки | Описание |
|---------|-----|-------|----------|
| `rustok_media_uploads_total` | Counter | `tenant_id`, `mime_category` | Кол-во загрузок |
| `rustok_media_upload_bytes_total` | Counter | `tenant_id` | Суммарный объём загруженных байтов |
| `rustok_media_deletes_total` | Counter | `tenant_id` | Кол-во удалений |
| `rustok_media_storage_health` | Gauge | `driver` | 1=healthy, 0=unhealthy |

Метрики инкрементируются в `crates/rustok-media/src/controllers/mod.rs` после каждой успешной операции.

## Задача обслуживания

### `media_cleanup` (Loco Task)

```bash
cargo loco task --name media_cleanup
```

Сканирует все записи `media`, удаляет из БД те, чьи файлы не существуют в хранилище (`StorageError::InvalidPath`). Прерывается, если хранилище недоступно.

Запускается автоматически через планировщик (`scheduler.yaml`): **ежедневно в 03:00 UTC**.

## Интеграция в сервер

Фича-флаг: `mod-media` (включён по умолчанию).

При старте сервера:
1. `init_storage()` → `StorageService` помещается в `ctx.shared_store`
2. `MediaService::new(db, storage)` создаётся на каждый запрос внутри `rustok-media::controllers` и `rustok-media::graphql`
3. `apps/server` подключает `rustok_media::controllers::routes()` и `rustok_media::graphql::*` как composition-root shim

## Зависимости

| Crate | Зачем |
|-------|-------|
| `rustok-core` | `generate_id()` |
| `rustok-storage` | `StorageService` |
| `async-trait` | Async методы |
| `bytes` | Буфер загружаемых данных |
| `chrono` | Timestamp при создании записи |
| `sea-orm` | ORM: entities, queries, pagination |
| `serde` / `serde_json` | Сериализация DTOs и metadata JSONB |
| `thiserror` | `MediaError` |
| `uuid` | IDs |
